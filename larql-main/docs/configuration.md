# Configuration Guide

LARQL uses a centralized configuration system to eliminate hardcoded values and path dependencies from the codebase.

## Configuration Sources

Configuration is loaded from multiple sources in priority order (later sources override earlier ones):

1. **Default values** - Built-in defaults from config structs
2. **config/default.toml** - Default configuration file (optional)
3. **config/local.toml** - Local overrides (git-ignored, for development)
4. **.env file** - Environment variables file (optional)
5. **Environment variables** - `LARQL__` prefixed environment variables

## Environment Variables

All configuration can be set via environment variables with the `LARQL__` prefix:

| Config Path | Environment Variable | Description |
|------------|---------------------|-------------|
| `vindex.path` | `LARQL_VINDEX__PATH` | Vindex path for loading vector index |
| `models.path` | `LARQL_MODEL__PATH` | Model path (HuggingFace model ID or local path) |
| `huggingface.token` | `LARQL_HUGGINGFACE__TOKEN` | HuggingFace API token |
| `huggingface.cache_dir` | `LARQL_HUGGINGFACE__CACHE_DIR` | HuggingFace cache directory (relative to home) |
| `paths.home_dir` | `LARQL_PATHS__HOME_DIR` | Home directory |

## Configuration Files

### config/default.toml

Default configuration values. This file is checked into git and provides sensible defaults for all users.

```toml
[vindex]
path = "data/bitnet_b1_58-large/vindex"

[models]
path = "google/gemma-3-4b-it"

[huggingface]
# token = ""

[paths]
# home_dir = ""
```

### config/local.toml

Local overrides for development. This file is git-ignored and should not be committed.

```toml
[vindex]
path = "/path/to/your/vindex"

[huggingface]
token = "your-hf-token-here"
```

### .env File

Environment variables file (dotenv format). Also git-ignored.

```bash
LARQL_VINDEX__PATH=/path/to/vindex
LARQL_HUGGINGFACE__TOKEN=your-token-here
LARQL_HUGGINGFACE__CACHE_DIR=.cache/huggingface/hub
```

## Path Resolution

The configuration system supports path expansion:

- **`~` expansion** - `~/data/vindex` expands to `$HOME/data/vindex`
- **`$HOME` expansion** - `$HOME/data/vindex` expands to home directory
- **Relative paths** - Relative to current working directory
- **Absolute paths** - Used as-is


## Usage in Code

### Loading Configuration

```rust
use larql_core::{load_config, AppConfig};

// Load configuration from all sources
let config = load_config()?;

// Or use defaults if loading fails
let config = load_config().unwrap_or_else(|_| AppConfig::default());
```

### Accessing Configuration Values

```rust
let vindex_path = &config.vindex.path;
let model_path = &config.models.path;
```

### Path Resolution

```rust
use larql_core::PathConfig;

let path_config = config.paths;
let resolved = path_config.resolve_path("~/data/vindex");
```

## Model Path Configuration

The configuration system uses a single model path that can be either:
- A HuggingFace model ID (e.g., `google/gemma-3-4b-it`)
- A local path to model weights

This allows seamless switching between different models without code changes.

## Best Practices

1. **Never hardcode paths** - Always use configuration or environment variables
2. **Use .env for local development** - Keep sensitive tokens in .env, not in code
3. **Commit default.toml** - Provide sensible defaults for all users
4. **Git-ignore local.toml** - Local overrides should not be committed
5. **Use path expansion** - Support both `~` and `$HOME` for user directories
6. **Document required env vars** - Update .env.example when adding new configuration options

## Migration from Hardcoded Values

When migrating hardcoded values to configuration:

1. Add the configuration option to the appropriate config struct
2. Update config/default.toml with the hardcoded value as default
3. Replace the hardcoded value with config access in code
4. Add environment variable mapping documentation
5. Update .env.example if needed

## Troubleshooting

### Config Not Loading

If configuration fails to load:
1. Check that config/default.toml is valid TOML
2. Verify environment variable names use double underscores (`__`)
3. Check for typos in config file structure
4. Ensure .env file is in the working directory

### Path Resolution Issues

If paths aren't resolving correctly:
1. Use absolute paths for debugging
2. Check that `LARQL_PATHS__HOME_DIR` is set
3. Verify current working directory is correct
4. Use `resolve_path()` for consistent path handling

### Breaking Changes

The following legacy environment variables are **no longer supported**:
- `HF_TOKEN` - Use `LARQL_HUGGINGFACE__TOKEN` instead
- `HOME` - Use `LARQL_PATHS__HOME_DIR` instead
- `VINDEX_PATH` - Use `LARQL_VINDEX__PATH` instead
- `MODEL_PATH` - Use `LARQL_MODEL__PATH` instead
