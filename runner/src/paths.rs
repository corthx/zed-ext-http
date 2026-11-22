//! The copy the extension installed writes only inside the extension's working directory, so uninstalling the
//! extension removes everything.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

pub fn cache_dir() -> Result<PathBuf> {
    match extension_dir() {
        Some(dir) => Ok(dir),
        None => Ok(dirs::cache_dir()
            .context("no cache directory available")?
            .join("http-runner")),
    }
}

pub fn config_dir() -> Result<PathBuf> {
    if let Some(dir) = std::env::var_os("HTTP_RUNNER_CONFIG_DIR").filter(|v| !v.is_empty()) {
        return Ok(dir.into());
    }
    match extension_dir() {
        Some(dir) => Ok(dir),
        None => Ok(dirs::config_dir()
            .context("no config directory available")?
            .join("http-runner")),
    }
}

fn extension_dir() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?.canonicalize().ok()?;
    extension_dir_of(&exe)
}

// The extension installs `<work dir>/bin/http-runner` next to a `bin/version` marker (src/lib.rs).
fn extension_dir_of(exe: &Path) -> Option<PathBuf> {
    let bin = exe.parent()?;
    let installed = bin.file_name()? == "bin" && bin.join("version").is_file();
    installed
        .then(|| bin.parent().map(Path::to_path_buf))
        .flatten()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn detects_the_extension_installed_copy() {
        let tmp = tempfile::tempdir().unwrap();
        let bin = tmp.path().join("bin");
        fs::create_dir(&bin).unwrap();
        let exe = bin.join("http-runner");
        assert_eq!(
            extension_dir_of(&exe),
            None,
            "no marker: cargo-installed copy"
        );
        fs::write(bin.join("version"), "0.1.0").unwrap();
        assert_eq!(extension_dir_of(&exe).as_deref(), Some(tmp.path()));
        assert_eq!(extension_dir_of(&tmp.path().join("http-runner")), None);
    }
}
