# Terminal Browser Onboarding Guide

## Overview

The LARQL Terminal Browser provides a terminal-based interface to the LARQL workbench using Carbonyl (Chromium rendered in your terminal).

## First-Run Setup

When you first run `larql-terminal-browser`, a setup wizard will guide you through:

1. **Carbonyl Installation Check**: Verifies that Carbonyl is installed and accessible
2. **Installation Guidance**: Provides instructions if Carbonyl is not found
3. **Usage Instructions**: Displays basic usage commands

The setup state is saved in `~/.config/larql/.terminal-browser-setup-complete` and the wizard will not run again unless this file is removed.

## Installing Carbonyl

### Option 1: Install Repo Runtime (Recommended)

```bash
./apps/terminal-runtime/scripts/install-carbonyl-runtime.sh
```

This downloads the pinned Carbonyl release (Linux x86_64) with SHA256 verification and installs the minimal runtime subset. No manual configuration required.

### Option 2: Install via Cargo

```bash
cargo install carbonyl
```

### Option 3: Manual Installation

Download Carbonyl from its repository and place it in your PATH, or set the `CARBONYL_BIN` environment variable to point to the binary.

## Starting the Workbench

Before launching the terminal browser, ensure the LARQL workbench is running:

```bash
larql-workbench
```

The workbench defaults to `http://127.0.0.1:8000`.

## Launching the Terminal Browser

### Basic Usage

```bash
# Connect to default workbench URL (http://127.0.0.1:8000)
larql-terminal-browser

# Connect to custom workbench URL
larql-terminal-browser --url http://127.0.0.1:8001

# Show Carbonyl UI chrome
larql-terminal-browser --show-ui

# Disable fullscreen mode
larql-terminal-browser --no-fullscreen

# Pass extra arguments to Carbonyl
larql-terminal-browser --carbonyl-arg=--zoom --carbonyl-arg=1.25
```

### Specifying Carbonyl Binary

The terminal browser resolves Carbonyl in this order:

1. `--carbonyl-bin` command-line flag
2. `CARBONYL_BIN` environment variable
3. In-repo component runtime: `apps/terminal-runtime/bin/carbonyl`
4. `carbonyl` on PATH

## Troubleshooting

### Workbench Health Check Fails

If you see "Workbench health check failed":

1. Ensure the workbench is running: `larql-workbench`
2. Check the URL is correct (default: `http://127.0.0.1:8000`)
3. Use `--url` to specify a different workbench URL
4. Check if the workbench port is blocked by a firewall

### Carbonyl Not Found

If you see "Carbonyl binary exists but is not executable":

1. Check file permissions: `chmod +x <carbonyl-binary>`
2. Ensure the binary matches your system architecture
3. Verify the binary is not corrupted (reinstall if needed)

### Failed to Launch Carbonyl

If Carbonyl fails to launch:

1. Check if Carbonyl is installed correctly
2. Try running Carbonyl directly: `carbonyl --help`
3. Check terminal compatibility (Carbonyl requires terminal with sixel or kitty graphics protocol)
4. Try a different terminal emulator

## Advanced Configuration

### Environment Variables

- `CARBONYL_BIN`: Path to Carbonyl binary
- `LARQL_PATHS__HOME_DIR`: Home directory for config storage

### Carbonyl Arguments

You can pass additional arguments to Carbonyl using `--carbonyl-arg`:

```bash
# Set zoom level
larql-terminal-browser --carbonyl-arg=--zoom --carbonyl-arg=1.5

# Enable verbose logging
larql-terminal-browser --carbonyl-arg=--verbose
```

## Resetting First-Run Setup

To re-run the setup wizard, remove the setup marker file:

```bash
rm ~/.config/larql/.terminal-browser-setup-complete
```

## Next Steps

- Read the [LARQL documentation](../../docs/) for more information
- Explore the workbench features
- Check the [CLI documentation](../../docs/cli.md) for available commands
