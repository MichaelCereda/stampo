# CLI Builder Guide

The `/ring-cli-builder` command for Claude Code helps you create ring-cli configurations from natural language descriptions or convert MCP server tools into standalone shell commands.

## Prerequisites

- [ring-cli](../README.md) installed (`ring-cli --version` to check)
- [Claude Code](https://claude.com/claude-code) CLI

## Creating a CLI from Scratch

Run `/ring-cli-builder` in Claude Code and describe the commands you need:

```
> /ring-cli-builder
> I need a CLI for managing my Kubernetes deployments. Commands:
> - deploy: deploy to a cluster, needs --env and --image flags
> - rollback: rollback to previous version, needs --env flag
> - status: check deployment status, needs --env flag
```

The skill generates a ring-cli YAML config:

```yaml
version: "2.0"
name: "k8s"
description: "Kubernetes deployment management"
commands:
  deploy:
    description: "Deploy to a cluster"
    flags:
      - name: "env"
        short: "e"
        description: "Target environment (e.g., staging, production)"
      - name: "image"
        short: "i"
        description: "Docker image to deploy"
    cmd:
      run:
        - "kubectl set image deployment/app app=${{image}} -n ${{env}}"
  rollback:
    description: "Rollback to previous version"
    flags:
      - name: "env"
        short: "e"
        description: "Target environment"
    cmd:
      run:
        - "kubectl rollout undo deployment/app -n ${{env}}"
  status:
    description: "Check deployment status"
    flags:
      - name: "env"
        short: "e"
        description: "Target environment"
    cmd:
      run:
        - "kubectl rollout status deployment/app -n ${{env}}"
```

The config is saved to `.ring-cli/k8s.yml` and the skill asks whether to install it. If you say yes:

```bash
ring-cli init --alias k8s --config-path .ring-cli/k8s.yml
```

After restarting your shell:

```bash
k8s deploy --env staging --image myapp:v2.1
k8s rollback --env production
k8s status --env staging
k8s <TAB>              # tab completion works
```

## Converting MCP Tools to CLI

If you have MCP servers configured in Claude Code, the skill can convert their tools into shell commands.

```
> /ring-cli-builder
> Convert my MCP tools to a CLI
```

The skill reads your `.mcp.json` configuration, discovers the available tools, and generates ring-cli configs for each server.

### How Tools Map to Commands

| MCP Concept | ring-cli Equivalent |
|---|---|
| Server name | Config `name` (top-level subcommand) |
| Tool name | Command name |
| Tool description | Command description |
| inputSchema property | Flag (`--flag-name`) |
| Required property | Flag with "(required)" in description |
| Nested object | Dot-notation flag (e.g., `--config.timeout`) |
| camelCase parameter | kebab-case flag (e.g., `userId` -> `--user-id`) |

### Shell Command Generation

MCP tools run inside Claude, not in the shell. The skill handles this by:

- **Generating real shell commands** when the tool has an obvious equivalent (e.g., GitHub tools use `gh`, Docker tools use `docker`, Kubernetes tools use `kubectl`)
- **Generating curl commands** when the tool wraps an HTTP API and you provide a base URL
- **Generating placeholder commands** when no shell equivalent exists -- you replace these manually

### Example: GitHub MCP Server

If your `.mcp.json` has a GitHub MCP server with `list-issues` and `create-issue` tools, the skill generates:

```yaml
version: "2.0"
name: "github"
description: "GitHub operations"
commands:
  list-issues:
    description: "List issues in a repository"
    flags:
      - name: "repo"
        short: "r"
        description: "Repository name in owner/repo format (required)"
      - name: "state"
        short: "s"
        description: "Filter by state: open, closed, all"
    cmd:
      run:
        - "gh issue list --repo ${{repo}} --state ${{state}}"
  create-issue:
    description: "Create a new issue"
    flags:
      - name: "repo"
        short: "r"
        description: "Repository name in owner/repo format (required)"
      - name: "title"
        short: "t"
        description: "Issue title (required)"
    cmd:
      run:
        - "gh issue create --repo ${{repo}} --title '${{title}}'"
```

After installation:

```bash
github list-issues --repo myorg/myrepo --state open
github create-issue --repo myorg/myrepo --title "Fix login bug"
```

### Multiple MCP Servers

If you have several MCP servers, the skill generates one config per server and suggests a references file:

```yaml
# .ring-cli/references.yml
configs:
  - github.yml
  - docker.yml
  - database.yml
```

Install all at once:

```bash
ring-cli init --alias tools --references .ring-cli/references.yml
tools github list-issues --repo myorg/myrepo
tools docker ps
tools database query --table users
```

## Customizing Generated Configs

The generated YAML files are plain text. Edit them to:

- Change shell commands
- Add or remove flags
- Add `base-dir` to set the working directory
- Add `banner` for a startup message
- Nest commands using `subcommands`
- Add environment variable placeholders with `${{env.VAR_NAME}}`

See the [Configuration Reference](configuration-reference.md) for all available options.

After editing, refresh the cached version:

```bash
<alias> refresh-configuration
```

## Manual Installation

If you don't have the ring-cli repo cloned, you can install the skill manually:

1. Create the commands directory:
   ```bash
   mkdir -p .claude/commands
   ```

2. Download the skill file:
   ```bash
   curl -fsSL https://raw.githubusercontent.com/MichaelCereda/ring-cli/master/.claude/commands/ring-cli-builder.md \
     -o .claude/commands/ring-cli-builder.md
   ```

3. Use it in Claude Code:
   ```
   > /ring-cli-builder
   ```

## Troubleshooting

**ring-cli not found**
Install ring-cli first:
```bash
curl -fsSL https://raw.githubusercontent.com/MichaelCereda/ring-cli/master/install.sh | sh
```

**No MCP tools found**
The skill looks for `.mcp.json` in the current directory and `~/.claude/.mcp.json`. If your MCP configuration is elsewhere, describe the tools manually when prompted.

**Alias already exists**
If `ring-cli init` fails because the alias already exists, the skill will add `--force` to overwrite it. You can also run manually:
```bash
ring-cli init --alias <name> --config-path .ring-cli/<name>.yml --force
```

**Generated command doesn't work**
The skill generates best-effort shell commands. For MCP tools without obvious shell equivalents, it creates placeholders. Edit the generated `.ring-cli/<name>.yml` file and replace the placeholder with the correct command.
