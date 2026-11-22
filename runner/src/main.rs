mod env;
mod httpyac;
mod lsp;
mod paths;

use anyhow::{Context, Result, bail};
use clap::{Args, Parser, Subcommand};
use env::Environments;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::process::{Command as Process, ExitCode};

/// Run `.http` requests with httpyac, remembering the `http-client.env.json` environment per project.
#[derive(Parser)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Send the request at ROW (1-based, any line of its block) in FILE.
    Run {
        file: PathBuf,
        row: usize,
        #[command(flatten)]
        opts: SendOpts,
    },
    /// Send every request in FILE.
    All {
        file: PathBuf,
        #[command(flatten)]
        opts: SendOpts,
    },
    /// Manage the environment used for FILE's project.
    Env {
        #[command(subcommand)]
        action: EnvAction,
    },
    /// Download the httpyac executable now instead of on first run.
    Setup,
    /// Language server started by the Zed extension (no features; pre-downloads httpyac).
    #[command(hide = true)]
    Lsp,
}

#[derive(Args)]
struct SendOpts {
    /// Environment to use (overrides the saved selection).
    #[arg(short, long, env = "HTTP_ENV")]
    env: Option<String>,
    /// Pick the environment interactively and remember it.
    #[arg(long)]
    select_env: bool,
    /// Extra arguments passed to `httpyac send`, after `--`.
    #[arg(last = true)]
    httpyac_args: Vec<String>,
}

#[derive(Subcommand)]
enum EnvAction {
    /// Interactively choose the environment and remember it.
    Select { file: PathBuf },
    /// Set the environment non-interactively ("none" clears it).
    Set { file: PathBuf, name: String },
    /// List environments, marking the selected one.
    List { file: PathBuf },
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(code) => code,
        Err(err) => {
            eprintln!("error: {err:#}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<ExitCode> {
    match cli.command {
        Command::Run { file, row, opts } => send(&file, &["--line".into(), row.to_string()], opts),
        Command::All { file, opts } => send(&file, &["--all".into()], opts),
        Command::Env { action } => env_command(action).map(|()| ExitCode::SUCCESS),
        Command::Lsp => lsp::serve().map(|()| ExitCode::SUCCESS),
        Command::Setup => {
            let path = httpyac::executable()?;
            println!("httpyac {} ready at {}", httpyac::VERSION, path.display());
            Ok(ExitCode::SUCCESS)
        }
    }
}

fn send(file: &Path, selector: &[String], opts: SendOpts) -> Result<ExitCode> {
    let file = resolve(file)?;
    let envs = Environments::discover(&file)?;
    let env_name = resolve_env(envs.as_ref(), &opts)?;
    let httpyac = httpyac::executable()?;

    let mut cmd = Process::new(&httpyac);
    cmd.arg("send").arg(&file).args(selector);
    if let Some(name) = &env_name {
        cmd.args(["--env", name]);
        let label = format!("[env: {name}]");
        if std::io::stderr().is_terminal() && std::env::var_os("NO_COLOR").is_none() {
            eprintln!("\x1b[2m{label}\x1b[0m");
        } else {
            eprintln!("{label}");
        }
    }
    if let Some(envs) = &envs {
        let shared = envs.shared_vars(env_name.as_deref());
        if !shared.is_empty() {
            cmd.arg("--var").args(shared);
        }
    }
    cmd.args(&opts.httpyac_args)
        .current_dir(working_dir(&file, envs.as_ref())?);

    let status = cmd
        .status()
        .with_context(|| format!("running {}", httpyac.display()))?;
    Ok(ExitCode::from(
        status.code().unwrap_or(1).clamp(0, 255) as u8
    ))
}

/// httpyac compares the file against its canonical cwd, so symlinks (macOS `/tmp`) must be resolved.
fn resolve(file: &Path) -> Result<PathBuf> {
    file.canonicalize()
        .with_context(|| format!("{} not found", file.display()))
}

fn resolve_env(envs: Option<&Environments>, opts: &SendOpts) -> Result<Option<String>> {
    let Some(envs) = envs.filter(|e| !e.names().is_empty()) else {
        if let Some(name) = &opts.env {
            bail!("--env {name}: no http-client.env.json found");
        }
        return Ok(None);
    };
    if opts.select_env {
        return select_and_save(envs).map(Some);
    }
    if let Some(name) = &opts.env {
        envs.check_known(name)?;
        return Ok(Some(name.clone()));
    }
    if let Some(saved) = envs.saved_selection() {
        return Ok(Some(saved));
    }
    match envs.names().as_slice() {
        [only] => Ok(Some(only.clone())),
        _ => select_and_save(envs).map(Some),
    }
}

fn select_and_save(envs: &Environments) -> Result<String> {
    let names = envs.names();
    if !std::io::stdin().is_terminal() {
        bail!(
            "several environments available ({}); pass --env or run `http-runner env select`",
            names.join(", ")
        );
    }
    let default = envs
        .saved_selection()
        .and_then(|c| names.iter().position(|n| *n == c))
        .unwrap_or(0);
    let index = dialoguer::Select::new()
        .with_prompt(format!("Environment ({})", envs.env_dir.display()))
        .items(&names)
        .default(default)
        .interact()?;
    envs.save_selection(Some(&names[index]))?;
    Ok(names[index].clone())
}

/// httpyac refuses files outside its cwd and only finds env files between the cwd and the
/// file's folder, plus `<cwd>/env/`. Keep the caller's cwd (Zed: the worktree root, where a
/// `.httpyac.js` would live) when it still sees our env files.
fn working_dir(file: &Path, envs: Option<&Environments>) -> Result<PathBuf> {
    let cwd = std::env::current_dir()?;
    let cwd = cwd.canonicalize().unwrap_or(cwd);
    let fallback = || envs.map_or_else(|| file.parent().unwrap().to_path_buf(), |e| e.root.clone());
    if !file.starts_with(&cwd) {
        return Ok(fallback());
    }
    let Some(envs) = envs else { return Ok(cwd) };
    let visible = if envs.env_dir != envs.root {
        envs.root == cwd
    } else {
        envs.root.starts_with(&cwd)
    };
    Ok(if visible { cwd } else { fallback() })
}

fn env_command(action: EnvAction) -> Result<()> {
    let file = match &action {
        EnvAction::Select { file } | EnvAction::Set { file, .. } | EnvAction::List { file } => {
            resolve(file)?
        }
    };
    let Some(envs) = Environments::discover(&file)? else {
        bail!("no http-client.env.json found above {}", file.display());
    };
    match action {
        EnvAction::Select { .. } => {
            println!("Environment set to \"{}\"", select_and_save(&envs)?);
        }
        EnvAction::Set { name, .. } if name == "none" => {
            envs.save_selection(None)?;
            println!("Environment cleared");
        }
        EnvAction::Set { name, .. } => {
            envs.check_known(&name)?;
            envs.save_selection(Some(&name))?;
            println!("Environment set to \"{name}\"");
        }
        EnvAction::List { .. } => {
            let current = envs.saved_selection();
            println!("Environments from {}:", envs.env_dir.display());
            for name in envs.names() {
                let mark = if current.as_deref() == Some(&name) {
                    "*"
                } else {
                    " "
                };
                println!(" {mark} {name}");
            }
        }
    }
    Ok(())
}
