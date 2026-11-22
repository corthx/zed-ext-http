use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::PathBuf;

pub const VERSION: &str = "6.16.7";
const RELEASE_URL: &str =
    "https://gitlab.com/corthx/zed-ext-http/-/releases/{tag}/downloads/{asset}";
/// `sha256  asset` lines (SHA256SUMS of the `httpyac-v<VERSION>` release).
const CHECKSUMS: &str = include_str!("../httpyac.sha256");

fn asset_name() -> Result<String> {
    let os = match std::env::consts::OS {
        "macos" => "darwin",
        "linux" => "linux",
        "windows" => "windows",
        other => bail!("unsupported OS: {other}"),
    };
    let arch = match std::env::consts::ARCH {
        "aarch64" => "arm64",
        "x86_64" => "x64",
        other => bail!("unsupported architecture: {other}"),
    };
    let ext = if os == "windows" { ".exe" } else { "" };
    Ok(format!("httpyac-{os}-{arch}{ext}"))
}

fn expected_checksum(asset: &str) -> Option<&'static str> {
    CHECKSUMS.lines().find_map(|line| {
        let (sum, name) = line.split_once(char::is_whitespace)?;
        (name.trim().trim_start_matches('*') == asset).then_some(sum)
    })
}

fn install_path(asset: &str) -> Result<PathBuf> {
    Ok(crate::paths::cache_dir()?
        .join(format!("httpyac-{VERSION}"))
        .join(asset))
}

pub fn executable() -> Result<PathBuf> {
    if let Some(custom) = std::env::var_os("HTTP_RUNNER_HTTPYAC").filter(|v| !v.is_empty()) {
        return Ok(custom.into());
    }
    let asset = asset_name()?;
    let path = install_path(&asset)?;
    if !path.is_file() {
        download(&asset, &path)?;
    }
    Ok(path)
}

fn download(asset: &str, dest: &PathBuf) -> Result<()> {
    let Some(expected) = expected_checksum(asset) else {
        bail!(
            "no checksum for {asset} in this build of http-runner; \
             set HTTP_RUNNER_HTTPYAC to an httpyac executable instead"
        );
    };
    let url = std::env::var("HTTP_RUNNER_RELEASES_URL")
        .unwrap_or_else(|_| RELEASE_URL.into())
        .replace("{tag}", &format!("httpyac-v{VERSION}"))
        .replace("{asset}", asset);
    eprintln!("Downloading httpyac {VERSION} ({asset}), first run only…");

    let mut response = reqwest::blocking::Client::builder()
        .timeout(None)
        .build()?
        .get(&url)
        .send()
        .and_then(|r| r.error_for_status())
        .with_context(|| format!("downloading {url}"))?;
    let mut bytes = Vec::new();
    response.read_to_end(&mut bytes)?;

    let actual: String = Sha256::digest(&bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    if actual != expected {
        bail!("checksum mismatch for {url}: expected {expected}, got {actual}");
    }

    let dir = dest.parent().unwrap();
    fs::create_dir_all(dir)?;
    let tmp = dir.join(format!(".{asset}.{}.partial", std::process::id()));
    fs::write(&tmp, &bytes)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&tmp, fs::Permissions::from_mode(0o755))?;
    }
    fs::rename(&tmp, dest)?;
    eprintln!("Installed {}", dest.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_name_for_current_platform() {
        let name = asset_name().unwrap();
        assert!(name.starts_with("httpyac-"));
    }

    #[test]
    fn checksums_file_is_well_formed() {
        for line in CHECKSUMS.lines().filter(|l| !l.trim().is_empty()) {
            let (sum, name) = line.split_once(char::is_whitespace).unwrap();
            assert_eq!(sum.len(), 64, "{line}");
            assert!(name.trim().starts_with("httpyac-"), "{line}");
        }
    }
}
