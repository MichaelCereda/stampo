# ring-cli

One binary. No runtime. No dependencies. Turn YAML configs and OpenAPI specs into fully-featured CLIs with tab completion, security you control, and zero attack surface.

## Why ring-cli

ring-cli is a single static binary that generates complete CLI tools from YAML or OpenAPI specs -- no interpreters, no package managers, no frameworks to install. Drop it on any machine (Linux, macOS, Windows) and start building CLIs immediately.

Define your commands in YAML, point it at an OpenAPI spec, or mix both. You get a shell alias with automatic tab completion, nested subcommands, environment variable substitution, and a trust-based security model. The binary has no network capabilities -- it never phones home, never downloads anything on its own, and carries zero attack surface. Safe to run on production servers, CI runners, and air-gapped environments.

## Install

**Linux / macOS:**
```bash
curl -fsSL https://raw.githubusercontent.com/MichaelCereda/ring-cli/master/install.sh | sh
```

**Windows (PowerShell):**
```powershell
irm https://github.com/MichaelCereda/ring-cli/releases/latest/download/ring-cli-Windows-x86_64.zip -OutFile ring-cli.zip; Expand-Archive ring-cli.zip -DestinationPath $env:LOCALAPPDATA\ring-cli -Force; $env:PATH += ";$env:LOCALAPPDATA\ring-cli"
```

**Homebrew (macOS / Linux):**
```bash
brew install michaelcereda/ring-cli/ring-cli
```

**From source:**
```bash
cargo install ring-cli
```

## Quick Start: From YAML

Define commands in a YAML file:

```yaml
version: "2.0"
name: "deploy"
description: "Deployment operations"
commands:
  staging:
    description: "Deploy to staging"
    flags:
      - name: "branch"
        description: "Branch to deploy"
    cmd:
      run:
        - "echo Deploying ${{branch}} to staging"
```

Install as a shell alias with one command:

```bash
ring-cli init --alias ops --config-path deploy.yml
```

Use it immediately:

```bash
ops deploy staging --branch main
ops deploy --help           # see available commands
ops --help                  # see all commands
ops <TAB>                   # tab completion works at every level
```

## Quick Start: From OpenAPI

Transform an OpenAPI spec into commands in seconds:

```bash
ring-cli init --alias petstore \
  --config-path openapi:https://petstore3.swagger.io/api/v3/openapi.json
```

Now you have a CLI based on the spec:

```bash
petstore pets list
petstore pets get --pet-id 5
petstore pets create --name "Buddy" --tag "dog"
petstore pets delete --pet-id 3
petstore <TAB>              # see all commands and flags
```

## Features

- **Single Binary, No Dependencies** -- One static binary, no runtime, no interpreters, no package managers. Drop it on a machine and it works. Builds for 7 platforms out of the box.

- **YAML-Driven CLI Generation** -- Define commands, flags, and subcommands in plain YAML. Supports shell commands, scripts, multi-step execution, and environment variable substitution.

- **OpenAPI 3.0 Support** -- Point ring-cli at an OpenAPI spec (local file or remote URL) and get a working CLI automatically. Paths become commands, parameters become flags, request bodies become dot-notation flags, and curl/wget handles execution.

- **Tab Completion** -- Bash, Zsh, Fish, and PowerShell completions installed automatically. Works at every level: top-level commands, nested subcommands, and flags.

- **Multi-Config Composition** -- Combine multiple YAML configs or OpenAPI specs into one alias. Each config becomes a top-level subcommand. Use a references file to manage them together.

- **Trust-Based Security** -- Configs are cached with SHA-256 hashes and only run from your trusted cache. Use `refresh-configuration` to review changes before accepting them.

- **Zero Network Footprint** -- No HTTP client in the binary. OpenAPI specs are fetched via your own curl/wget with explicit consent. No callbacks, no analytics, no phone-home. Safe to deploy on production servers.

- **Built for Automation** -- Stdout/stderr separation for reliable piping. `-q` quiet mode, `--yes` for CI/CD, ASCII-only output, nonzero exit codes on error. Pre-built for Linux (x86_64, aarch64, ARM), macOS (Intel, Apple Silicon), and Windows (x86_64, ARM64).

- **Variable Substitution** -- `${{flag_name}}` for command flags, `${{env.VAR_NAME}}` for environment variables.

- **Nested Subcommands** -- Unlimited nesting depth. Organize complex CLIs into natural hierarchies.

- **Configurable Banners** -- Display messages on alias invocation. Per-config or global via references file. Prints to stderr, suppressed with `-q`.

- **Standards Compliant** -- Respects `NO_COLOR` env var. `--color=always|never|auto` override. `-v` verbose mode for debugging.

- **AI-Powered CLI Builder** -- Use `/ring-cli-builder` in Claude Code to create CLIs from natural language or convert MCP server tools into shell commands.

## Documentation

- [Getting Started Guide](docs/getting-started.md) — Detailed walkthrough, configuration format, shell commands, nested subcommands
- [Configuration Reference](docs/configuration-reference.md) — Complete YAML schema and field descriptions
- [OpenAPI Guide](docs/openapi-guide.md) — Using OpenAPI specs, flag mapping, authentication, limitations
- [Setup Guide](docs/setup-guide.md) — Installation from source, platform-specific notes, troubleshooting
- [CLI Builder Guide](docs/ring-cli-builder-guide.md) — Using the Claude Code `/ring-cli-builder` skill

## License

MIT
