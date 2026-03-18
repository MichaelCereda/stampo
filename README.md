# ring-cli -- YAML to CLI Generator

Build custom command-line tools from YAML configs or OpenAPI specs. Single static binary, zero runtime dependencies, automatic tab completion, nested subcommands, and trust-based security. Works on Linux, macOS, and Windows.

## What Makes ring-cli Different

ring-cli is a **CLI generator** that turns YAML configs and OpenAPI specs into complete, production-ready command-line tools -- delivered as a single portable binary with zero dependencies.

No interpreters. No package managers. No frameworks. Drop it on any machine and start building CLIs immediately. The binary has no network capabilities -- it never phones home, never downloads anything on its own, and carries zero attack surface. Safe to run on production servers, CI runners, and air-gapped environments.

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

Install as a shell alias:

```bash
ring-cli init --alias ops --config-path deploy.yml --description "Deployment tools"
```

Use it immediately:

```bash
$ ops
Deployment tools

Usage: ops [OPTIONS] [COMMAND]

Commands:
  deploy                 Deployment operations
  refresh-configuration  Re-read and trust updated configuration

$ ops deploy staging --branch main
$ ops <TAB>                   # tab completion at every level
```

## Quick Start: From OpenAPI

Turn any OpenAPI 3.0 spec into a CLI:

```bash
ring-cli init --alias petstore \
  --config-path openapi:https://petstore3.swagger.io/api/v3/openapi.json \
  --description "Petstore API client"
```

```bash
$ petstore
Petstore API client

Usage: petstore [OPTIONS] [COMMAND]

Commands:
  petstore               Generated from OpenAPI spec: Petstore
  refresh-configuration  Re-read and trust updated configuration

$ petstore petstore pets list
$ petstore petstore pets get --pet-id 5
$ petstore petstore pets create --name "Buddy" --tag "dog"
```

Paths become commands. Parameters become flags. Request bodies become dot-notation flags. Authentication via environment variables. curl/wget for execution.

## Multi-Config Composition

Combine multiple configs into one alias -- each becomes a top-level subcommand:

```bash
ring-cli init --alias infra \
  --config-path deploy.yml \
  --config-path db.yml \
  --config-path monitoring.yml \
  --description "Infrastructure management"
```

```
$ infra
Infrastructure management

Usage: infra [OPTIONS] [COMMAND]

Commands:
  deploy                 Build and deploy services to any environment
  db                     Database migrations, backups, and connectivity
  monitoring             Dashboards, alerts, and log queries
  refresh-configuration  Re-read and trust updated configuration

$ infra deploy staging --service api --branch main
$ infra db migrate --env production
$ infra monitoring alerts --team backend
```

Or use a references file to manage configs together:

```yaml
# .ring-cli/references.yml
description: "Infrastructure management"
banner: "infra-cli v2.0"
configs:
  - deploy.yml
  - db.yml
  - monitoring.yml
```

## Features

- **Single Static Binary, Zero Dependencies** -- One portable executable. No Java, Python, Node.js, or interpreters required. Drop on any server and start using immediately.

- **YAML-Driven CLI Generation** -- Define commands, flags, and subcommands in plain YAML. Supports shell commands, scripts, multi-step execution, and environment variable substitution.

- **OpenAPI 3.0 Support** -- Point ring-cli at an OpenAPI spec (local file or remote URL) and get a working CLI automatically. Paths become commands, parameters become flags, request bodies become dot-notation flags, curl/wget for execution.

- **Tab Completion** -- Bash, Zsh, Fish, and PowerShell completions installed automatically. Works at every level: top-level commands, nested subcommands, and flags.

- **Multi-Config Composition** -- Combine multiple YAML configs or OpenAPI specs into one alias. Each config becomes a top-level subcommand. Use references files to manage them together.

- **Alias Descriptions** -- Set a description for your alias via `--description` flag or `description` field in references files. Shown in help output when running the alias.

- **Trust-Based Security** -- Configs are cached with SHA-256 hashes and only run from your trusted cache. Use `refresh-configuration` to review changes before accepting them.

- **Zero Network Footprint** -- No HTTP client in the binary. OpenAPI specs are fetched via your own curl/wget with explicit consent. No callbacks, no analytics, no phone-home. Safe for production servers and air-gapped environments.

- **Built for Automation** -- Stdout/stderr separation for reliable piping. `-q` quiet mode, `--yes` for CI/CD, ASCII-only output, nonzero exit codes on error. Pre-built for Linux (x86_64, aarch64, ARM), macOS (Intel, Apple Silicon), and Windows (x86_64, ARM64).

- **Variable Substitution** -- `${{flag_name}}` for command flags, `${{env.VAR_NAME}}` for environment variables in your shell commands.

- **Nested Subcommands** -- Unlimited nesting depth. Organize complex CLIs into natural hierarchies.

- **AI Agent Integration** -- Generate CLIs from natural language using Claude Code's `/ring-cli-builder` skill. Convert MCP server tools into executable shell commands.

- **Standards Compliant** -- Respects `NO_COLOR` env var. `--color=always|never|auto` override. `-v` verbose mode. Configurable banners on stderr.

## Use Cases

- **DevOps Automation** -- Build multi-command deployment tools from YAML without shell script complexity
- **API Gateway CLI** -- Convert OpenAPI specs into self-documenting CLI tools for your APIs
- **Platform Engineering** -- Distribute internal tools as single binaries to your team
- **CI/CD Helpers** -- Custom CLI tools for GitHub Actions, GitLab CI, or Kubernetes hooks
- **MCP / AI Agent Tools** -- Expose CLI operations to Claude, ChatGPT, or local LLMs
- **SRE Automation** -- Quick incident response tools without learning new frameworks
- **Infrastructure-as-Code** -- Generate CLI wrappers around Terraform, CloudFormation, or Ansible

## Documentation

- [Getting Started Guide](docs/getting-started.md) -- Detailed walkthrough and configuration format
- [Configuration Reference](docs/configuration-reference.md) -- Complete YAML schema and field descriptions
- [OpenAPI Guide](docs/openapi-guide.md) -- OpenAPI specs, flag mapping, authentication, limitations
- [CLI Builder Guide](docs/ring-cli-builder-guide.md) -- Using Claude Code `/ring-cli-builder` skill
- [Setup Guide](docs/setup-guide.md) -- Installation from source, platform notes, troubleshooting

## License

MIT
