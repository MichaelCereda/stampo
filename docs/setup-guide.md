# Ring-CLI Setup Guide

How to install ring-cli and create configuration files.

---

## Part 1: Installing Ring-CLI

### Prerequisites

- **Rust toolchain** (rustc + cargo). If not installed, run:
  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
  source "$HOME/.cargo/env"
  ```
- **Git** (to clone the repository)
- A shell config file must exist for alias installation to work. At least one of:
  - `~/.bashrc` (Bash)
  - `~/.zshrc` (Zsh)
  - `~/.config/fish/config.fish` (Fish)
  - `~/.config/powershell/Microsoft.PowerShell_profile.ps1` (PowerShell)

### Installation Steps

1. **Clone the repository:**
   ```bash
   git clone https://github.com/MichaelCereda/ring-cli.git
   cd ring-cli
   ```

2. **Build and install the release binary:**
   ```bash
   cargo install --path .
   ```
   This installs `ring-cli` to `~/.cargo/bin/ring-cli`.

3. **Verify the installation:**
   ```bash
   ring-cli --version
   # Expected output: ring-cli 2.0.0
   ```

4. **If `ring-cli` is not found**, ensure `~/.cargo/bin` is in your `PATH`:
   ```bash
   export PATH="$HOME/.cargo/bin:$PATH"
   ```

### Alternative: Install via Homebrew (macOS/Linux)

After a release is published:
```bash
brew install michaelcereda/ring-cli/ring-cli
```

---

## Part 2: Creating Configuration Files

Ring-CLI reads YAML configuration files that define commands, flags, and subcommands. Each file defines one named group of commands.

### YAML Schema Reference

Every configuration file must conform to this exact schema:

```yaml
# REQUIRED: Must be exactly "2.0"
version: "2.0"

# REQUIRED: Unique name for this config group.
# This becomes a top-level subcommand under the alias.
# Must be a valid CLI identifier (lowercase, hyphens, underscores).
name: "<group-name>"

# REQUIRED: Human-readable description shown in --help output.
description: "<what this group does>"

# OPTIONAL: Working directory for all commands in this config.
# If set, every shell command runs with this as its cwd.
# Supports absolute paths only.
base-dir: "/absolute/path/to/directory"

# REQUIRED: Map of command names to command definitions.
# Can be empty: `commands: {}`
commands:
  <command-name>:
    # REQUIRED: Description shown in --help
    description: "<what this command does>"

    # OPTIONAL: List of flags this command accepts. Defaults to [].
    flags:
      - name: "<flag-name>"         # REQUIRED: used as --flag-name
        short: "<char>"             # OPTIONAL: single character, used as -c
        description: "<help text>"  # REQUIRED: shown in --help

    # EXACTLY ONE of `cmd` or `subcommands` must be present. Not both. Not neither.

    # Option A: cmd - a command to execute
    cmd:
      # Either `run` (shell commands) or `http` (HTTP request). Not both.

      # Shell commands: list of strings executed sequentially via `sh -c`.
      # If any step fails (non-zero exit), execution stops.
      run:
        - "<shell command 1>"
        - "<shell command 2>"

      # OR HTTP request:
      http:
        method: "GET"  # GET, POST, PUT, DELETE, PATCH, or HEAD
        url: "<url>"
        headers:                    # OPTIONAL: map of header name to value
          Content-Type: "application/json"
        body: "<request body>"      # OPTIONAL: only for POST, PUT, PATCH

    # Option B: subcommands - nested command tree (arbitrarily deep)
    subcommands:
      <sub-name>:
        description: "..."
        flags: []
        cmd:
          run:
            - "..."
```

### Variable Substitution

Inside `run` strings, `http.url`, `http.headers` values, and `http.body`:

| Syntax | Meaning | Example |
|--------|---------|---------|
| `${{flag_name}}` | Value of the flag passed by the user | `${{branch}}` |
| `${{env.VAR_NAME}}` | Value of environment variable `VAR_NAME` | `${{env.API_TOKEN}}` |

- Flag placeholders that weren't provided are left as-is (not replaced).
- Environment variable placeholders fail with an error if the variable is not set.

### Validation Rules

These rules are enforced at parse time. Violating them will cause `ring-cli init` or config loading to fail:

1. `version` must be present (string `"2.0"`).
2. `name` must be present and non-empty.
3. `description` must be present at both config and command level.
4. Every command must have exactly one of `cmd` or `subcommands` — not both, not neither.
5. This rule applies recursively to all nested subcommands.
6. If two config files use the same `name` under one alias, init fails (unless `--warn-only-on-conflict` is passed).

---

## Part 3: Registering an Alias

### Basic Setup (single config)

```bash
ring-cli init --alias <alias-name> --config-path <path-to-config.yml>
```

This does four things:
1. Reads and validates the YAML config.
2. Copies it to `~/.ring-cli/aliases/<alias-name>/` with a SHA-256 hash.
3. Appends a shell alias to all detected shell config files (e.g., `~/.zshrc`).
4. Installs tab completion hooks for all detected shells.

### Multiple Configs Per Alias

```bash
ring-cli init --alias infra \
  --config-path deploy.yml \
  --config-path db.yml \
  --config-path monitoring.yml
```

Each config's `name` field becomes a top-level subcommand:
```
infra deploy staging --branch main
infra db migrate
infra monitoring status
```

### With Automatic Update Checking

```bash
ring-cli init --alias infra \
  --config-path deploy.yml \
  --check-for-updates
```

This installs a shell startup hook that checks if source config files have changed every time a new terminal opens. If changes are detected, the user is prompted to update.

### Flags Reference

| Flag | Description |
|------|-------------|
| `--alias <NAME>` | **Required.** Shell alias name. Alphanumeric, hyphens, underscores only. |
| `--config-path <PATH>` | Path to a YAML config file. Repeatable. If omitted, creates a default config at `~/.ring-cli/configurations/<alias>.yml`. |
| `--warn-only-on-conflict` | Downgrade name-conflict errors to warnings. |
| `--check-for-updates` | Install a shell hook that checks for config changes on terminal startup. |

---

## Part 4: Using the Alias

Once initialized, the alias is available after restarting the shell (or sourcing the config):

```bash
# Run a command
<alias> <config-name> <command> [--flag value]

# Example
infra deploy staging --branch main

# See all available commands
<alias> --help

# See commands within a config group
<alias> <config-name> --help

# Update configuration from source files
<alias> refresh-configuration

# Global options (before config-name)
<alias> --quiet <config-name> <command>      # suppress errors
<alias> --verbose <config-name> <command>    # verbose output
<alias> --color=never <config-name> <command> # disable color
```

---

## Part 5: Workflow — Step by Step

Follow these steps in order when setting up ring-cli:

### Step 1: Determine what commands the user needs

Ask or infer:
- What tasks should the CLI automate? (deploys, DB ops, API calls, etc.)
- Should commands be grouped? Each group = one YAML file with a `name`.
- What flags does each command need?
- Are there environment variables the commands depend on?
- Should any commands run from a specific directory? (`base-dir`)

### Step 2: Write the YAML config file(s)

For each group of related commands, create one `.yml` file. Use the schema above. Key decisions:

- **Choose `name` carefully** — it becomes the subcommand (`infra <name> <command>`).
- **Use `run` for shell commands**, `http` for API calls.
- **Use `subcommands` for hierarchy** — e.g., `cloud > aws > deploy`.
- **Use `flags`** for any user-provided values. Reference them with `${{flag_name}}`.
- **Use `${{env.VAR}}`** for secrets/tokens — never hardcode them.
- **Set `base-dir`** if commands assume a working directory.

### Step 3: Validate the config

```bash
# Dry-run: init will validate the YAML and report errors
ring-cli init --alias test-alias --config-path your-config.yml
```

If validation fails, the error message includes the path to the problematic command (e.g., `mycli > deploy > broken`).

### Step 4: Install the alias

```bash
ring-cli init --alias <chosen-name> \
  --config-path <file1.yml> \
  --config-path <file2.yml> \
  --check-for-updates
```

### Step 5: Verify

```bash
# Restart shell or source config
source ~/.zshrc  # or ~/.bashrc

# Test the alias
<alias> --help
<alias> <config-name> <command> --flag value
```

### Step 6: Updating configs later

After editing a YAML source file:
```bash
<alias> refresh-configuration
# Prompts: "Configuration '<name>' has changed. Trust this configuration? [y/N]"
# Type 'y' to accept the new version.
```

Or if `--check-for-updates` was used, the next new terminal will prompt automatically.

---

## Part 6: Complete Examples

### Example A: DevOps CLI

**deploy.yml:**
```yaml
version: "2.0"
name: "deploy"
description: "Deployment commands"
commands:
  staging:
    description: "Deploy to staging environment"
    flags:
      - name: "branch"
        short: "b"
        description: "Git branch to deploy"
      - name: "dry-run"
        short: "d"
        description: "Simulate without deploying"
    cmd:
      run:
        - "echo Deploying branch ${{branch}} to staging (dry-run: ${{dry-run}})"
        - "kubectl apply -f k8s/staging/"
  production:
    description: "Deploy to production environment"
    flags:
      - name: "tag"
        short: "t"
        description: "Release tag"
    cmd:
      run:
        - "echo Deploying ${{tag}} to production"
        - "kubectl apply -f k8s/production/"
```

**db.yml:**
```yaml
version: "2.0"
name: "db"
description: "Database operations"
base-dir: "/opt/app"
commands:
  migrate:
    description: "Run pending database migrations"
    flags: []
    cmd:
      run:
        - "python manage.py migrate"
  backup:
    description: "Create database backup"
    flags:
      - name: "output"
        short: "o"
        description: "Output file path"
    cmd:
      run:
        - "pg_dump ${{env.DATABASE_URL}} > ${{output}}"
```

**Install:**
```bash
ring-cli init --alias ops --config-path deploy.yml --config-path db.yml --check-for-updates
```

**Usage:**
```bash
ops deploy staging --branch main
ops deploy production --tag v2.0.0
ops db migrate
ops db backup --output /backups/db-2026-03-05.sql
```

### Example B: API Client CLI

**api.yml:**
```yaml
version: "2.0"
name: "api"
description: "Internal API operations"
commands:
  health:
    description: "Check API health"
    flags: []
    cmd:
      http:
        method: "GET"
        url: "https://api.internal.com/health"
  users:
    description: "User management"
    flags: []
    subcommands:
      list:
        description: "List all users"
        flags: []
        cmd:
          http:
            method: "GET"
            url: "https://api.internal.com/users"
            headers:
              Authorization: "Bearer ${{env.API_TOKEN}}"
      create:
        description: "Create a user"
        flags:
          - name: "payload"
            short: "p"
            description: "JSON payload"
        cmd:
          http:
            method: "POST"
            url: "https://api.internal.com/users"
            headers:
              Content-Type: "application/json"
              Authorization: "Bearer ${{env.API_TOKEN}}"
            body: "${{payload}}"
```

**Install:**
```bash
ring-cli init --alias myapi --config-path api.yml
```

**Usage:**
```bash
myapi api health
myapi api users list
myapi api users create --payload '{"name":"Alice","email":"alice@example.com"}'
```

---

## Troubleshooting

| Problem | Solution |
|---------|----------|
| `ring-cli: command not found` | Add `~/.cargo/bin` to PATH, or use full path `~/.cargo/bin/ring-cli` |
| `Alias '<name>' already exists in ~/.zshrc, skipping` | Expected on re-init. The alias line is already installed. |
| `Config name '<x>' is used by both...` | Two config files have the same `name`. Rename one, or pass `--warn-only-on-conflict`. |
| `Either 'cmd' or 'subcommands' must be present` | A command has neither `cmd` nor `subcommands`. Add one. |
| `Only 'cmd' or 'subcommands' should be present, not both` | A command has both. Remove one. |
| `Environment variable 'X' is not set` | Set the env var before running, or remove the `${{env.X}}` reference. |
| Tab completion not working | Restart your shell. Completions are installed during `init`. |
| Config changes not detected | Run `<alias> refresh-configuration`, or re-init with `--check-for-updates`. |
