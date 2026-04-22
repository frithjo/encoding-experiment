//! First-party terminal browser launcher for the LARQL workbench.
//!
//! This delegates rendering to Carbonyl (Chromium in terminal),
//! as described in `/home/arty/Documents/projects/carbonyl/readme.md`.

use clap::Parser;
use std::{
    env,
    path::PathBuf,
    process::{Command, ExitCode},
};

#[derive(Parser, Debug)]
#[command(
    name = "larql-terminal-browser",
    about = "Terminal browser client for the LARQL workbench (Carbonyl-backed)"
)]
struct Args {
    /// Workbench base URL (served by `larql-ui`).
    #[arg(default_value = "http://127.0.0.1:8000")]
    url: String,

    /// Optional absolute path to Carbonyl binary.
    /// Resolution order when omitted:
    /// 1) CARBONYL_BIN env var
    /// 2) repo component runtime: apps/terminal-runtime/bin/carbonyl
    /// 3) `carbonyl` on PATH
    #[arg(long)]
    carbonyl_bin: Option<PathBuf>,

    /// Enable fullscreen mode (default).
    #[arg(long, default_value_t = true)]
    fullscreen: bool,

    /// Disable fullscreen mode.
    #[arg(long)]
    no_fullscreen: bool,

    /// Hide Carbonyl navigation UI/chrome (default).
    #[arg(long, default_value_t = true)]
    hide_ui: bool,

    /// Show Carbonyl navigation UI/chrome.
    #[arg(long)]
    show_ui: bool,

    /// Extra arguments forwarded directly to Carbonyl.
    /// Repeat for multiple flags, e.g.:
    /// --carbonyl-arg=--zoom --carbonyl-arg=1.25
    #[arg(long)]
    carbonyl_arg: Vec<String>,
}

fn resolve_carbonyl_bin(args: &Args) -> PathBuf {
    if let Some(bin) = args.carbonyl_bin.clone() {
        return bin;
    }
    if let Some(bin) = env::var_os("CARBONYL_BIN").map(PathBuf::from) {
        return bin;
    }
    // Prefer the in-repo component runtime binary when available.
    let repo_runtime = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../apps/terminal-runtime/bin/carbonyl");
    if repo_runtime.exists() {
        return repo_runtime;
    }
    PathBuf::from("carbonyl")
}

fn main() -> ExitCode {
    let args = Args::parse();
    let carbonyl = resolve_carbonyl_bin(&args);
    let mut cmd = Command::new(&carbonyl);
    // Terminal runtime remains mandatory; display behavior stays user-configurable.
    let fullscreen = if args.no_fullscreen {
        false
    } else {
        args.fullscreen
    };
    let hide_ui = if args.show_ui { false } else { args.hide_ui };

    if fullscreen {
        cmd.arg("--fullscreen");
        cmd.env("CARBONYL_ENV_FULLSCREEN", "1");
    }
    if hide_ui {
        cmd.arg("--hide-ui");
    }
    if !args.carbonyl_arg.is_empty() {
        cmd.args(&args.carbonyl_arg);
    }
    cmd.arg(&args.url);

    let status = match cmd.status() {
        Ok(status) => status,
        Err(err) => {
            eprintln!(
                "larql-terminal-browser: failed to launch Carbonyl ({:?}): {}",
                carbonyl, err
            );
            eprintln!(
                "Install Carbonyl (see /home/arty/Documents/projects/carbonyl/readme.md) \
or install/link repo runtime via apps/terminal-runtime/scripts/install-carbonyl-runtime.sh."
            );
            return ExitCode::from(1);
        }
    };

    ExitCode::from(status.code().unwrap_or(1) as u8)
}
