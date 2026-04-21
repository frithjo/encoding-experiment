//! First-party terminal browser for the LARQL workbench.
//!
//! **Status:** Stub. The binary will eventually connect to `larql-ui` and render in the TTY
//! using a Rust-first stack (see crate README). No third-party terminal browser install is
//! required for LARQL once this is implemented.

use clap::Parser;
use std::process::ExitCode;

#[derive(Parser, Debug)]
#[command(
    name = "larql-terminal-browser",
    about = "Terminal browser client for the LARQL workbench (in-repo, Rust-first)"
)]
struct Args {
    /// Workbench base URL (served by `larql-ui`).
    #[arg(default_value = "http://127.0.0.1:8000")]
    url: String,
}

fn main() -> ExitCode {
    let args = Args::parse();

    eprintln!(
        "larql-terminal-browser: not implemented yet.\n\
         URL argument: {}\n\
         \n\
         Run the workbench server with: uv run larql-ui --host 127.0.0.1 --port 8000\n\
         See crates/larql-terminal-browser/README.md for architecture and plan.",
        args.url
    );

    ExitCode::from(2)
}
