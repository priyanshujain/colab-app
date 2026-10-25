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
    aarch64: &'static str,
    x86_64: &'static str,
}

const RELEASES: [Release; 3] = [
    Release {
        name: "uv",
        repo: "astral-sh/uv",
        version: "0.13.1",
        aarch64: "cfe454573412ff7380d15fc7dae5997ab53ffce72fce0a78e91ddbc84001acd2",
        x86_64: "bb7b805bdcf3d0bc8c74ceb94569e6b3e8aa35bd492acdbb9d46bf46d9d4ff6b",
    },
    Release {
        name: "ty",
        repo: "astral-sh/ty",
        version: "0.0.89",
        aarch64: "c0535481dd2ca2653dbf804bb008ca2286f9ae7f284c12f52158a1853fcdc06f",
        x86_64: "f522783404e9b96be12a7543fe6eaab26e3c9a87f639a53b13eed883e8fdb62a",
    },
    Release {
        name: "ruff",
        repo: "astral-sh/ruff",
        version: "0.17.1",
        aarch64: "1c8e6e36538eaa0b996378a20c226403c4ca6369ac24884751b024d3a9530b86",
        x86_64: "88ee16f88dba9d440e06375f46898ec85baf578042ad086764d5a33558bb6e17",
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
    let (arch, sha) = match std::env::consts::ARCH {
        "aarch64" => ("aarch64", r.aarch64),
        "x86_64" => ("x86_64", r.x86_64),
        other => bail!("unsupported architecture {other}"),
    };
    let url = format!(
        "https://github.com/{}/releases/download/{}/{}-{arch}-apple-darwin.tar.gz",
        r.repo, r.version, r.name
    );
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
