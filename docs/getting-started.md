# Getting Started Guide

Ring-CLI generates custom command-line tools from YAML configuration files. Define your commands, flags, and subcommands in YAML, then install them as a shell command with automatic tab completion, a trust-based security model, and color output.

## Getting Started

### 1. Install

```bash
# From source (requires Rust toolchain)
cargo install --path .

# Or via Homebrew (macOS/Linux)
brew install michaelcereda/ring-cli/ring-cli
```

If `ring-cli` is not found after install, add `~/.cargo/bin` to your PATH:
```bash
export PATH="$HOME/.cargo/bin:$PATH"
```

### 2. Write a config file

Create `deploy.yml`:
```yaml
version: "2.0"
name: "deploy"
description: "Deployment operations"
commands:
  staging:
    description: "Deploy to staging"
    flags:
      - name: "branch"
        short: "b"
        description: "Branch to deploy"
    cmd:
      run:
        - "echo Deploying ${{branch}} to staging"
```

### 3. Init an alias

```bash
ring-cli init --alias ops --config-path deploy.yml
```

This validates the YAML, caches it securely, installs a shell function, and sets up tab completion.

### 4. Use it

```bash
# Restart your shell (or source your profile), then:
ops deploy staging --branch main
ops --help
ops deploy --help

# Tab completion works at every level
ops <TAB>              # shows: deploy, refresh-configuration
ops deploy <TAB>       # shows: staging
ops deploy staging --<TAB>  # shows: --branch
```

## Configuration Format

Each YAML file defines a named group of commands:

```yaml
version: "2.0"
name: "deploy"
description: "Deployment operations"
base-dir: ".."  # optional working directory (relative to this file, or absolute)
banner: "Deploy CLI v1.0"     # optional banner shown on use
commands:
  staging:
    description: "Deploy to staging"
    flags:
      - name: "branch"
        short: "b"
        description: "Branch to deploy"
    cmd:
      run:
        - "echo Deploying ${{branch}} to staging"
```

### Config Fields

| Field         | Required | Description                                              |
|---------------|----------|----------------------------------------------------------|
| `version`     | Yes      | Config format version. Must be `"2.0"`.                  |
| `name`        | Yes      | Name for this config group. Becomes a top-level command.  |
| `description` | Yes      | Description shown in `--help` output.                    |
| `base-dir`    | No       | Working directory for all commands. Relative paths resolve from the config file's location. |
| `banner`      | No       | Text displayed on stderr when the alias is invoked.       |
| `commands`    | Yes      | Map of command names to command definitions.              |

### Command Fields

| Field         | Required | Description                                              |
|---------------|----------|----------------------------------------------------------|
| `description` | Yes      | Description shown in `--help` output.                    |
| `flags`       | No       | List of flags the command accepts.                       |
| `cmd`         | *        | Command to execute (`run` or `http`). Required if no `subcommands`. |
| `subcommands` | *       | Nested subcommands. Required if no `cmd`.                |

A command must have either `cmd` or `subcommands`, not both.

### Flag Fields

| Field         | Required | Description                                              |
|---------------|----------|----------------------------------------------------------|
| `name`        | Yes      | Flag name (used as `--name`).                            |
| `short`       | No       | Single-character short form (used as `-n`).              |
| `description` | Yes      | Description shown in `--help` output.                    |

## Multiple Configs Per Alias

An alias can combine multiple configuration files. Each config's `name` becomes a top-level subcommand:

```bash
ring-cli init --alias infra --config-path deploy.yml --config-path db.yml
```

```
infra deploy staging    # from deploy.yml (name: "deploy")
infra db migrate        # from db.yml (name: "db")
```

If two configs use the same `name`, init will error. Use `--warn-only-on-conflict` to downgrade to a warning.

### References File

Instead of listing configs individually, point to a references file:

```bash
ring-cli init --alias ops --references .ring-cli/references.yml
```

`references.yml`:
```yaml
banner: "Welcome to Ops CLI"  # optional top-level banner
configs:
  - services.yml
  - db.yml
  - monitoring.yml
```

Paths in the references file are resolved relative to the file's own location. A top-level `banner` here takes priority over per-config banners.

## Shell Commands

Use `${{flag_name}}` to reference flag values and `${{env.VAR_NAME}}` for environment variables:

```yaml
commands:
  deploy:
    description: "Deploy with auth"
    flags:
      - name: "target"
        short: "t"
        description: "Deploy target"
    cmd:
      run:
        - "curl -H 'Authorization: Bearer ${{env.API_TOKEN}}' https://${{target}}/deploy"
```

Multi-step commands run sequentially. If any step fails, execution stops:

```yaml
commands:
  setup:
    description: "Full setup"
    flags: []
    cmd:
      run:
        - "echo Step 1: Installing..."
        - "echo Step 2: Configuring..."
        - "echo Step 3: Done!"
```

## Nested Subcommands

Commands can be nested arbitrarily deep using `subcommands`:

```yaml
commands:
  cloud:
    description: "Cloud operations"
    subcommands:
      aws:
        description: "AWS operations"
        subcommands:
          deploy:
            description: "Deploy to AWS"
            flags: []
            cmd:
              run:
                - "echo Deploying to AWS..."
```

Usage: `myalias config-name cloud aws deploy`

## Banner

Display a message when the alias is invoked. Banners print to stderr so they don't interfere with piped output. Suppress with `-q` (quiet mode).

**Per-config banner** — set in each YAML config:
```yaml
version: "2.0"
name: "deploy"
description: "Deploy tools"
banner: "Deploy CLI v2.0 -- use with caution in production"
commands: ...
```

**Top-level banner** — set in a references file (takes priority over per-config banners):
```yaml
banner: "Welcome to Infrastructure CLI"
configs:
  - deploy.yml
  - db.yml
```

## Security: Trust System

ring-cli never runs commands from a config file without explicit trust.

1. **`ring-cli init`** reads the YAML, validates it, and stores a trusted copy with a SHA-256 hash in `~/.ring-cli/aliases/<name>/`. The config is auto-trusted since you just pointed to it.

2. **Your alias** runs from the cached/trusted config, not the original YAML file.

3. **`<alias> refresh-configuration`** re-reads the original YAML files, compares hashes, and shows what changed. You must type `y` to trust the new version. If you decline, the old trusted version is kept.

4. **If the original YAML is deleted**, the alias still works from cache. `refresh-configuration` reports the missing source.

### Cache Structure

```
~/.ring-cli/
  aliases/
    <alias-name>/
      <config-name>.yml   # trusted copy of each config
      metadata.json        # source paths, SHA-256 hashes, banner, trust timestamps
```

## Color Output

- **Auto-detected**: Color is enabled when output goes to a terminal, disabled when piped.
- **`NO_COLOR` env var**: Set `NO_COLOR=1` to disable all color ([no-color.org](https://no-color.org) standard).
- **`--color` flag**: Override with `--color=always`, `--color=never`, or `--color=auto` (default).

Only ring-cli's own messages (errors, warnings, success) are colored. Command output is always passed through unmodified.

## Tab Completion

Tab completion is installed automatically during `ring-cli init` for all detected shells:

- **Bash** and **Zsh** — shell functions with `compdef`/`complete -F` bindings
- **Fish** — `complete -c` directives
- **PowerShell** — `Register-ArgumentCompleter`

Completions are generated from the cached config, so they stay fast and consistent. Completions work at every level: top-level subcommands, nested commands, and flags.

After running `refresh-configuration`, restart your shell to pick up completion changes.

## CLI Reference

### `ring-cli init`

```
ring-cli init --alias <NAME> [OPTIONS]
```

| Flag                      | Short | Description                                          |
|---------------------------|-------|------------------------------------------------------|
| `--alias <NAME>`          |       | Shell alias name to install (required).              |
| `--config-path <PATH>`   |       | Path to a config file. Repeatable for multiple configs. |
| `--references <PATH>`    |       | Path to a references file listing config paths.      |
| `--force`                 | `-f`  | Overwrite existing alias (removes old one first).    |
| `--warn-only-on-conflict` |       | Warn instead of error on config name conflicts.      |
| `--check-for-updates`    |       | Check for config changes on every new terminal.      |

`--config-path` and `--references` are mutually exclusive. If neither is given, a default config is created at `~/.ring-cli/configurations/<alias>.yml`.

### Alias Commands

Once installed, your alias supports:

```
<alias> [OPTIONS] <config-name> <command> [FLAGS]
<alias> refresh-configuration
```

| Option           | Short | Description                    |
|------------------|-------|--------------------------------|
| `--quiet`        | `-q`  | Suppress banners and error messages. |
| `--verbose`      | `-v`  | Print verbose output.          |
| `--color <WHEN>` |       | Color output (`auto`, `always`, `never`). |
| `--help`         | `-h`  | Print help.                    |
| `--version`      | `-V`  | Print version.                 |

## Installation

### Quick Install (Linux / macOS)

```bash
curl -fsSL https://raw.githubusercontent.com/MichaelCereda/ring-cli/master/install.sh | sh
```

### Quick Install (Windows PowerShell)

```powershell
irm https://github.com/MichaelCereda/ring-cli/releases/latest/download/ring-cli-Windows-x86_64.zip -OutFile ring-cli.zip; Expand-Archive ring-cli.zip -DestinationPath $env:LOCALAPPDATA\ring-cli -Force; $env:PATH += ";$env:LOCALAPPDATA\ring-cli"
```

To make it permanent, add `$env:LOCALAPPDATA\ring-cli` to your system PATH.

### Homebrew

```bash
brew install michaelcereda/ring-cli/ring-cli
```

### From Source

```bash
git clone https://github.com/MichaelCereda/ring-cli.git
cd ring-cli
cargo install --path .
```

### From Releases

Pre-built binaries for Linux, macOS, and Windows are available on the [Releases](https://github.com/MichaelCereda/ring-cli/releases) page.

## Using with Claude Code

If you use [Claude Code](https://claude.com/claude-code), the `/ring-cli-builder` command helps you create ring-cli configs from natural language descriptions or convert MCP server tools into CLI commands.

```
> /ring-cli-builder
> I need a CLI for managing my Docker containers -- start, stop, logs, and deploy with an --env flag
```

The skill generates a valid ring-cli config, shows it for review, and optionally installs it as a shell alias. It can also read your MCP server configurations and convert those tools into standalone shell commands.

See the [CLI Builder Guide](ring-cli-builder-guide.md) for details.
