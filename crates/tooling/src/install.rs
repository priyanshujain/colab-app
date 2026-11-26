use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct Tools {
    pub uv: PathBuf,
    pub ty: PathBuf,
    pub ruff: PathBuf,
}

struct Release {
    name: &'static str,
    repo: &'static str,
    version: &'static str,
    checksums: [(&'static str, &'static str); 4],
}

const RELEASES: [Release; 3] = [
    Release {
        name: "uv",
        repo: "astral-sh/uv",
        version: "0.13.1",
        checksums: [
            ("aarch64-apple-darwin", "cfe454573412ff7380d15fc7dae5997ab53ffce72fce0a78e91ddbc84001acd2"),
            ("x86_64-apple-darwin", "bb7b805bdcf3d0bc8c74ceb94569e6b3e8aa35bd492acdbb9d46bf46d9d4ff6b"),
            ("aarch64-unknown-linux-musl", "345a8308e4d1835cdfaadf692836fd3137a6d45864773f05ad7bf59442b1ba16"),
            ("x86_64-unknown-linux-musl", "6c86040a8f045ab9adc9e3ec4cdafa03fd00e9a988ed2bf28971c7fc2d0debd0"),
        ],
    },
    Release {
        name: "ty",
        repo: "astral-sh/ty",
        version: "0.0.89",
        checksums: [
            ("aarch64-apple-darwin", "c0535481dd2ca2653dbf804bb008ca2286f9ae7f284c12f52158a1853fcdc06f"),
            ("x86_64-apple-darwin", "f522783404e9b96be12a7543fe6eaab26e3c9a87f639a53b13eed883e8fdb62a"),
            ("aarch64-unknown-linux-musl", "7583ebea89e7ae9a5019942f0b5394a6a18a19562050b2d6ed6df576e040e328"),
            ("x86_64-unknown-linux-musl", "2618c34c21096a44a797512f2533dea52d28e95a0287833719805db72874b8b1"),
        ],
    },
    Release {
        name: "ruff",
        repo: "astral-sh/ruff",
        version: "0.17.1",
        checksums: [
            ("aarch64-apple-darwin", "1c8e6e36538eaa0b996378a20c226403c4ca6369ac24884751b024d3a9530b86"),
            ("x86_64-apple-darwin", "88ee16f88dba9d440e06375f46898ec85baf578042ad086764d5a33558bb6e17"),
            ("aarch64-unknown-linux-musl", "255404ccf2f7950ff6aad8858321a6f416fe2a840ae4e37dfb4c21cb8774f325"),
            ("x86_64-unknown-linux-musl", "2d9878b5e6a3d0dedadbfc01a64bfe54e72f560adbfd235937cd29e1fc9238e3"),
        ],
    },
];

pub async fn install(dir: &Path) -> Result<Tools> {
    let mut paths = Vec::new();
    for release in &RELEASES {
        paths.push(install_one(dir, release).await?);
    }
    let [uv, ty, ruff] = paths.try_into().unwrap();
    Ok(Tools { uv, ty, ruff })
}

async fn install_one(dir: &Path, r: &Release) -> Result<PathBuf> {
    let bin = dir.join(format!("{}-{}", r.name, r.version)).join(r.name);
    if bin.exists() {
        return Ok(bin);
    }
    let os = if cfg!(target_os = "macos") { "apple-darwin" } else { "unknown-linux-musl" };
    let target = format!("{}-{os}", std::env::consts::ARCH);
    let sha = r.checksums.iter().find(|(t, _)| *t == target).with_context(|| format!("unsupported platform {target}"))?.1;
    let url = format!("https://github.com/{}/releases/download/{}/{}-{target}.tar.gz", r.repo, r.version, r.name);
    let bytes = reqwest::get(&url).await?.error_for_status()?.bytes().await?;
    let actual = hex::encode(Sha256::digest(&bytes));
    if actual != sha {
        bail!("checksum mismatch for {url}: expected {sha}, got {actual}");
    }
    let data = extract(&bytes, r.name).with_context(|| format!("{} not found in {url}", r.name))?;
    let parent = bin.parent().unwrap();
    std::fs::create_dir_all(parent)?;
    let tmp = parent.join(format!(".{}.tmp", r.name));
    std::fs::write(&tmp, data)?;
    std::fs::set_permissions(&tmp, std::os::unix::fs::PermissionsExt::from_mode(0o755))?;
    std::fs::rename(&tmp, &bin)?;
    Ok(bin)
}

fn extract(archive: &[u8], name: &str) -> Option<Vec<u8>> {
    let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(archive));
    for entry in tar.entries().ok()? {
        let mut entry = entry.ok()?;
        if entry.path().ok()?.file_name()? == name {
            let mut data = Vec::new();
            entry.read_to_end(&mut data).ok()?;
            return Some(data);
        }
    }
    None
}
