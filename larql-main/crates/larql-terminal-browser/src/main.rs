//! First-party terminal browser launcher for the LARQL workbench.
//!
//! This delegates rendering to Carbonyl (Chromium in terminal).

use clap::Parser;
use larql_tty_io::{is_tty, query_graphics_capabilities};
use std::{
    env,
    fs,
    path::{Path, PathBuf},
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

fn check_carbonyl_health(bin: &Path) -> Result<(), String> {
    use std::process::Command;
    let output = Command::new(bin).arg("--help").output();
    match output {
        Ok(output) if output.status.success() => Ok(()),
        Ok(_) => Err(format!("Carbonyl binary exists but is not executable: {:?}", bin)),
        Err(e) => Err(format!("Failed to execute Carbonyl binary {:?}: {}", bin, e)),
    }
}

async fn check_workbench_health(url: &str) -> Result<(), String> {
    let health_url = format!("{}/health", url.trim_end_matches('/'));
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;
    
    let response = client.get(&health_url).send().await;
    match response {
        Ok(resp) if resp.status().is_success() => Ok(()),
        Ok(resp) => Err(format!("Workbench returned status {}: {}", resp.status(), health_url)),
        Err(e) => Err(format!("Failed to connect to workbench at {}: {}", health_url, e)),
    }
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
    // Fall back to PATH, but the caller will provide clear guidance if not found.
    PathBuf::from("carbonyl")
}

/// Check if first-run setup is needed.
fn check_first_run() -> bool {
    let config_dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from(env::var("HOME").unwrap_or_else(|_| ".".to_string())))
        .join("larql");
    
    let marker_file = config_dir.join(".terminal-browser-setup-complete");
    !marker_file.exists()
}

/// Mark first-run setup as complete.
fn mark_first_run_complete() -> Result<(), std::io::Error> {
    let config_dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from(env::var("HOME").unwrap_or_else(|_| ".".to_string())))
        .join("larql");
    
    fs::create_dir_all(&config_dir)?;
    fs::write(config_dir.join(".terminal-browser-setup-complete"), "setup-complete")
}

/// Run first-run setup wizard.
fn run_first_run_setup() -> Result<(), String> {
    println!("=== LARQL Terminal Browser - First-Run Setup ===");
    println!();
    println!("This wizard will help you set up the terminal browser.");
    println!();

    // Check TTY
    println!("Checking terminal capabilities...");
    if is_tty() {
        println!("✓ Running in a TTY");

        // Query graphics capabilities
        if let Some(caps) = query_graphics_capabilities() {
            println!("  Graphics support:");
            if caps.sixel {
                println!("    ✓ Sixel detected");
            } else {
                println!("    - Sixel not detected");
            }
            if caps.kitty {
                println!("    ✓ Kitty graphics detected");
            } else {
                println!("    - Kitty graphics not detected");
            }
        } else {
            println!("  Graphics capability probe unavailable");
        }
    } else {
        println!("⚠ Not running in a TTY - terminal browser may not work correctly");
    }
    println!();

    // Check Carbonyl
    println!("Checking Carbonyl installation...");
    let carbonyl = resolve_carbonyl_bin(&Args {
        url: String::new(),
        carbonyl_bin: None,
        fullscreen: true,
        no_fullscreen: false,
        hide_ui: true,
        show_ui: false,
        carbonyl_arg: vec![],
    });

    if carbonyl.exists() {
        println!("✓ Carbonyl found at: {:?}", carbonyl);
    } else {
        println!("✗ Carbonyl not found");
        println!();
        println!("To install Carbonyl (recommended - auto-download):");
        println!("  ./apps/terminal-runtime/scripts/install-carbonyl-runtime.sh");
        println!();
        println!("Or install via cargo:");
        println!("  cargo install carbonyl");
        println!();
        print!("Press Enter to continue after installing Carbonyl, or Ctrl+C to exit: ");
        use std::io::{self, Write};
        io::stdout().flush().unwrap();
        io::stdin().read_line(&mut String::new()).unwrap();
    }

    println!();
    println!("Setup complete! The terminal browser is ready to use.");
    println!();
    println!("Usage:");
    println!("  larql-terminal-browser                    # Connect to http://127.0.0.1:8000");
    println!("  larql-terminal-browser --url <URL>         # Connect to custom URL");
    println!("  larql-terminal-browser --help              # See all options");
    println!();

    mark_first_run_complete().map_err(|e| format!("Failed to save setup state: {}", e))?;

    Ok(())
}

#[tokio::main]
async fn main() -> ExitCode {
    let args = Args::parse();
    
    // First-run setup
    if check_first_run() {
        if let Err(err) = run_first_run_setup() {
            eprintln!("First-run setup failed: {}", err);
            return ExitCode::from(1);
        }
        println!();
        println!("Press Enter to launch the terminal browser, or Ctrl+C to exit: ");
        use std::io::{self, Write};
        io::stdout().flush().unwrap();
        io::stdin().read_line(&mut String::new()).unwrap();
    }
    
    // Workbench health check before launching Carbonyl
    if let Err(err) = check_workbench_health(&args.url).await {
        eprintln!("Error: Workbench health check failed");
        eprintln!("Details: {}", err);
        eprintln!();
        eprintln!("Troubleshooting:");
        eprintln!("  1. Ensure workbench is running at: {}", args.url);
        eprintln!("  2. Start workbench: larql-workbench");
        eprintln!("  3. Check if URL is correct (default: http://127.0.0.1:8000)");
        eprintln!("  4. Use --url flag to specify different workbench URL");
        return ExitCode::from(1);
    }
    
    let carbonyl = resolve_carbonyl_bin(&args);

    // Health check before launching Carbonyl
    if let Err(err) = check_carbonyl_health(&carbonyl) {
        eprintln!("larql-terminal-browser: {}", err);
        eprintln!();
        eprintln!("Install Carbonyl for terminal browser functionality:");
        eprintln!("  ./apps/terminal-runtime/scripts/install-carbonyl-runtime.sh  # auto-download");
        eprintln!("  cargo install carbonyl");
        eprintln!();
        eprintln!("Or set CARBONYL_BIN environment variable to custom path");
        return ExitCode::from(1);
    }

    // Wrap Carbonyl launch with TtyGuard for cleanup on failure or exit.
    // The guard ensures the terminal is restored to its original state if
    // Carbonyl fails to launch or exits abnormally.
    let _guard = if is_tty() {
        Some(larql_tty_io::TtyGuard::new())
    } else {
        None
    };

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
            eprintln!("Error: Failed to launch Carbonyl ({:?})", carbonyl);
            eprintln!("Details: {}", err);
            eprintln!();
            eprintln!("Installation options:");
            eprintln!("  1. Install repo runtime (auto-download): ./apps/terminal-runtime/scripts/install-carbonyl-runtime.sh");
            eprintln!("  2. Install via cargo: cargo install carbonyl");
            eprintln!("  3. Set CARBONYL_BIN env var to custom path");
            eprintln!("  4. Use --carbonyl-bin flag to specify path");
            eprintln!();
            eprintln!("Common issues:");
            eprintln!("  - Binary not found on PATH (try option 1 or 2)");
            eprintln!("  - Permission denied (chmod +x the binary)");
            eprintln!("  - Wrong architecture (check carbonyl supports your system)");
            return ExitCode::from(1);
        }
    };

    ExitCode::from(status.code().unwrap_or(1) as u8)
}
