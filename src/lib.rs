//! Installs `http-runner` (which the gutter tasks call) by posing as the HTTP language server:
//! that hook is the only extension code Zed runs when an `.http` file opens. See docs/architecture.md.

use std::fs;
use zed_extension_api::{
    self as zed, Architecture, Command, DownloadedFileType, LanguageServerId,
    LanguageServerInstallationStatus as Status, Os, Result, Worktree,
};

const RELEASE_URL: &str =
    "https://gitlab.com/corthx/zed-ext-http/-/releases/{tag}/downloads/{asset}";
/// Must match the path probed by the tasks in `languages/http/tasks.json`.
const BINARY: &str = "bin/http-runner";
/// http-runner keeps its files in this directory when it finds this marker next to itself.
const VERSION_MARKER: &str = "bin/version";
const VERSION: &str = env!("CARGO_PKG_VERSION");

struct HttpRunnerExtension;

fn asset_name() -> Result<String> {
    let (os, arch) = zed::current_platform();
    let os = match os {
        Os::Mac => "darwin",
        Os::Linux => "linux",
        Os::Windows => return Err("HTTP Runner does not support Windows yet".into()),
    };
    let arch = match arch {
        Architecture::Aarch64 => "arm64",
        Architecture::X8664 => "x64",
        Architecture::X86 => return Err("HTTP Runner does not support 32-bit x86".into()),
    };
    Ok(format!("http-runner-{os}-{arch}"))
}

fn is_installed() -> bool {
    fs::metadata(BINARY).is_ok_and(|m| m.is_file())
        && fs::read_to_string(VERSION_MARKER).is_ok_and(|v| v.trim() == VERSION)
}

fn install(id: &LanguageServerId) -> Result<()> {
    let asset = asset_name()?;
    let url = RELEASE_URL
        .replace("{tag}", &format!("http-runner-v{VERSION}"))
        .replace("{asset}", &asset);
    zed::set_language_server_installation_status(id, &Status::Downloading);
    fs::create_dir_all("bin").map_err(|e| format!("creating bin/: {e}"))?;
    let _ = fs::remove_file(BINARY);
    zed::download_file(&url, BINARY, DownloadedFileType::Uncompressed)
        .map_err(|e| format!("downloading {url}: {e}"))?;
    zed::make_file_executable(BINARY)?;
    fs::write(VERSION_MARKER, VERSION).map_err(|e| format!("writing {VERSION_MARKER}: {e}"))?;
    Ok(())
}

impl zed::Extension for HttpRunnerExtension {
    fn new() -> Self {
        Self
    }

    fn language_server_command(
        &mut self,
        id: &LanguageServerId,
        worktree: &Worktree,
    ) -> Result<Command> {
        // Same precedence as the tasks.
        if let Some(path) = worktree.which("http-runner") {
            return Ok(Command {
                command: path,
                args: vec!["lsp".into()],
                env: Vec::new(),
            });
        }
        if !is_installed()
            && let Err(err) = install(id)
        {
            zed::set_language_server_installation_status(id, &Status::Failed(err.clone()));
            return Err(err);
        }
        zed::set_language_server_installation_status(id, &Status::None);
        let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
        Ok(Command {
            command: cwd.join(BINARY).to_string_lossy().into_owned(),
            args: vec!["lsp".into()],
            env: Vec::new(),
        })
    }
}

zed::register_extension!(HttpRunnerExtension);
