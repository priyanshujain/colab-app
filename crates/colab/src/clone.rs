use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};

pub struct Fetched {
    pub text: String,
    pub name: String,
}

pub fn download_url(link: &str) -> Result<(String, String)> {
    let not_colab = || anyhow::anyhow!("not a Colab notebook link: {link}");
    let url = url::Url::parse(link.trim()).map_err(|_| not_colab())?;
    if url.host_str() != Some("colab.research.google.com") {
        return Err(not_colab());
    }
    let parts: Vec<&str> = url.path_segments().ok_or_else(not_colab)?.filter(|s| !s.is_empty()).collect();
    match parts.as_slice() {
        ["drive", id, ..] => Ok((format!("https://drive.google.com/uc?export=download&id={id}"), format!("{id}.ipynb"))),
        ["github", owner, repo, "blob", branch, path @ ..] if !path.is_empty() => {
            Ok((format!("https://raw.githubusercontent.com/{owner}/{repo}/{branch}/{}", path.join("/")), path.last().unwrap().to_string()))
        }
        _ => Err(not_colab()),
    }
}

pub fn check(github: bool, status: u16, host: &str, disposition: Option<&str>, text: String, fallback: &str) -> Result<Fetched> {
    if status == 404 {
        bail!(if github { "the GitHub file was not found; check the owner, repo, branch and path" } else { "the notebook does not exist; check the link" });
    }
    if host == "accounts.google.com" || status == 401 || status == 403 || text.trim_start().starts_with('<') {
        bail!("the notebook is not shared publicly; in Colab use Share and set General access to Anyone with the link");
    }
    if !(200..300).contains(&status) {
        bail!("download failed with HTTP {status}");
    }
    let nb = nb::Notebook::parse(&text).context("the link did not return a valid notebook")?;
    let title = nb.metadata.get("colab").and_then(|c| c["name"].as_str()).map(String::from);
    let file = disposition.and_then(|d| d.split("filename=\"").nth(1)).map(|f| f.trim_end_matches('"').to_string());
    let name = title.or(file).map(|n| n.replace('/', "-")).unwrap_or_else(|| fallback.into());
    let name = if name.ends_with(".ipynb") { name } else { format!("{name}.ipynb") };
    Ok(Fetched { text, name })
}

pub async fn fetch(link: &str) -> Result<Fetched> {
    let (url, fallback) = download_url(link)?;
    let response = reqwest::get(&url).await.context("could not reach the notebook host")?;
    let (status, host) = (response.status().as_u16(), response.url().host_str().unwrap_or_default().to_string());
    let disposition = response.headers().get("content-disposition").and_then(|v| v.to_str().ok()).map(String::from);
    check(url.contains("githubusercontent"), status, &host, disposition.as_deref(), response.text().await?, &fallback)
}

pub async fn clone(link: &str, dest: &Path) -> Result<PathBuf> {
    let fetched = fetch(link).await?;
    let path = if dest.is_dir() { dest.join(fetched.name) } else { dest.to_path_buf() };
    std::fs::write(&path, fetched.text)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PUBLIC: &str = "https://colab.research.google.com/drive/12a_Wrfi2_gwwAuN3VvMTwVMz9TfqctNj";

    #[tokio::test]
    async fn real_links_report_public_missing_and_non_colab_notebooks() {
        let err = |r: Result<Fetched>| r.err().map(|e| e.to_string()).unwrap_or_default();
        let ok = fetch(&format!("{PUBLIC}?usp=sharing")).await.unwrap();
        assert_eq!(ok.name, "CLIP Guided Diffusion HQ 256x256.ipynb");
        assert!(nb::Notebook::parse(&ok.text).unwrap().cells.len() > 3);
        assert!(err(fetch("https://colab.research.google.com/drive/1AbCdEfGhIjKlMnOpQrStUvWxYz0123456").await).contains("does not exist"));
        let gh = fetch("https://colab.research.google.com/github/googlecolab/colabtools/blob/main/notebooks/colab-github-demo.ipynb").await.unwrap();
        assert_eq!(gh.name, "colab-github-demo.ipynb");
        assert!(err(fetch("https://colab.research.google.com/github/googlecolab/colabtools/blob/main/notebooks/missing.ipynb").await).contains("GitHub file was not found"));
        assert!(err(fetch("https://drive.google.com/file/d/12a_Wrfi2_gwwAuN3VvMTwVMz9TfqctNj/view").await).contains("not a Colab notebook link"));
        assert!(err(fetch("https://colab.research.google.com/notebooks/intro.ipynb").await).contains("not a Colab notebook link"));
    }

    #[test]
    fn sign_in_page_html_and_broken_json_get_clear_errors() {
        let err = |host: &str, status: u16, text: &str| check(false, status, host, None, text.into(), "x.ipynb").err().unwrap().to_string();
        assert!(err("accounts.google.com", 200, "<!doctype html><html>").contains("not shared publicly"));
        assert!(err("drive.usercontent.google.com", 403, "denied").contains("not shared publicly"));
        assert!(err("drive.usercontent.google.com", 200, "{\"cells\": 3}").contains("not return a valid notebook"));
        let nb = r#"{"nbformat":4,"nbformat_minor":5,"metadata":{},"cells":[]}"#;
        let named = check(false, 200, "drive.usercontent.google.com", Some("attachment; filename=\"AtsPy Feb/2020 Full\""), nb.into(), "id.ipynb").unwrap();
        assert_eq!(named.name, "AtsPy Feb-2020 Full.ipynb");
    }
}
