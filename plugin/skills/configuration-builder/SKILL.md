---
name: configuration-builder
description: Use when creating CLI tools from scratch using ring-cli, converting MCP server tools into shell commands, or generating ring-cli YAML configurations from natural language descriptions
---

# ring-cli Builder

Create CLI tools powered by ring-cli. Two modes:

1. **Create a CLI from scratch** - user describes commands, you generate ring-cli YAML configs
2. **Convert MCP tools to CLI** - read MCP server definitions, generate equivalent ring-cli configs

## About ring-cli

ring-cli is a CLI generator that turns YAML configs into complete command-line tools. Single static binary, zero runtime dependencies.

- **Repository:** https://github.com/MichaelCereda/ring-cli
- **Documentation:** https://github.com/MichaelCereda/ring-cli/blob/master/docs/configuration-reference.md

## Prerequisites

Verify ring-cli is installed before generating configs:

```bash
ring-cli --version
```

If not installed, ask the user if they'd like you to install it. If they agree, detect the platform and use the appropriate method:

- **macOS/Linux (recommended):**
  ```bash
  curl -fsSL https://raw.githubusercontent.com/MichaelCereda/ring-cli/master/install.sh | sh
  ```
- **Homebrew:**
  ```bash
  brew install michaelcereda/ring-cli/ring-cli
  ```
- **From source (requires Rust):**
  ```bash
  cargo install ring-cli
  ```

Do NOT install without the user's explicit consent.

## Mode Detection

Ask the user:

> What would you like to build?
> 1. **Create a CLI from scratch** - describe the commands you need
> 2. **Convert MCP tools to CLI** - turn MCP server tools into shell commands

If the user's message already makes their intent clear, skip the question.

---

## Mode 1: Create CLI from Scratch

### Flow

1. Ask the user to describe their CLI: what it does, what commands it needs
2. For each command, understand: description, flags (with types/descriptions), and the shell command to run
3. If hierarchical commands, use `subcommands` nesting
4. Generate valid ring-cli YAML config (version 2.0)
5. Show config for review
6. Ask whether to install it

### YAML Generation Rules

```yaml
version: "2.0"
name: "<name>"
description: "<description>"
commands:
  <command-name>:
    description: "<what it does>"
    flags:
      - name: "<flag-name>"
        short: "<single-char>"        # optional
        description: "<flag help text>"
    cmd:
      run:
        - "<shell command using ${{flag_name}} and ${{env.VAR}}>"
```

Rules:
- Each command must have either `cmd` or `subcommands`, never both
- Use `${{flag_name}}` for flag value placeholders
- Use `${{env.VAR_NAME}}` for environment variable placeholders
- Flag names: kebab-case (e.g., `target-env`, not `targetEnv`)
- Multi-step commands: multiple entries in `run` list

### Example

User says: "I need a Docker stack manager with start, stop, logs, and deploy. Deploy needs an env flag."

```yaml
version: "2.0"
name: "stack"
description: "Docker stack management"
commands:
  start:
    description: "Start the stack"
    flags: []
    cmd:
      run:
        - "docker compose up -d"
  stop:
    description: "Stop the stack"
    flags: []
    cmd:
      run:
        - "docker compose down"
  logs:
    description: "View stack logs"
    flags: []
    cmd:
      run:
        - "docker compose logs -f"
  deploy:
    description: "Deploy to an environment"
    flags:
      - name: "env"
        short: "e"
        description: "Target environment (e.g., staging, production)"
    cmd:
      run:
        - "docker compose -f docker-compose.${{env}}.yml up -d"
```

---

## Mode 2: Convert MCP Tools to CLI

### Discovery

Find MCP server configurations automatically:

1. Read `.mcp.json` in the current project directory
2. Read `~/.claude/.mcp.json` (global config)
3. If neither found, ask the user to describe or paste their MCP tool definitions

Many `.mcp.json` files only contain connection info (command, args, env) without inline tool definitions. In that case, ask the user to list the tools they want to convert.

### Mapping Rules

For each MCP server:
- Server name → config `name`
- Each tool → a command
- Tool description → command description
- `inputSchema` properties → flags:
  - Property name: convert camelCase to kebab-case
  - Property description: flag description
  - Required properties: append "(required)" to description
  - Nested objects: flatten with dot-notation (e.g., `config.timeout`)
  - Arrays: note "(can be specified multiple times)" in description

### Shell Command Generation

MCP tools run inside Claude, not in the shell. When generating `cmd.run`:

- **Obvious shell equivalent exists** → generate real command:
  - GitHub tools → `gh` CLI
  - Docker tools → `docker`
  - Kubernetes tools → `kubectl`
  - Database tools → appropriate CLI client
  - File operations → standard shell commands

- **Tool wraps an HTTP API** (user provides base URL) → generate curl:
  ```yaml
  cmd:
    run:
      - "curl -s -X GET 'https://api.example.com/resource/${{id}}' -H 'Authorization: Bearer ${{env.API_TOKEN}}'"
  ```

- **No obvious equivalent** → placeholder with explanation:
  ```yaml
  cmd:
    run:
      - "echo 'TODO: Replace with shell command for <tool-name>. Params: ${{param1}} ${{param2}}'"
  ```
  Tell the user which commands need manual replacement.

### Example

MCP server "github" with tools `list-issues` and `create-issue`:

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
      - name: "limit"
        short: "l"
        description: "Maximum number of results"
    cmd:
      run:
        - "gh issue list --repo ${{repo}} --state ${{state}} --limit ${{limit}}"
  create-issue:
    description: "Create a new issue"
    flags:
      - name: "repo"
        short: "r"
        description: "Repository name in owner/repo format (required)"
      - name: "title"
        short: "t"
        description: "Issue title (required)"
      - name: "body"
        short: "b"
        description: "Issue body"
    cmd:
      run:
        - "gh issue create --repo ${{repo}} --title '${{title}}' --body '${{body}}'"
```

---

## Output and Installation

### Saving

Save configs to `.ring-cli/<name>.yml` in the current working directory. Create `.ring-cli/` if needed. Use a different path if the user specifies one.

### After Generation

Show the complete YAML, then:

> Config saved to `.ring-cli/<name>.yml`.
> Want me to install this as a shell alias? I'll run:
> `ring-cli init --alias <name> --config-path .ring-cli/<name>.yml`

If yes, run init. If alias exists, add `--force`.

If no:
> To install later: `ring-cli init --alias <name> --config-path .ring-cli/<name>.yml`

### Multiple Configs

For multiple MCP servers, generate one config per server and suggest a references file:

```yaml
# .ring-cli/references.yml
configs:
  - github.yml
  - docker.yml
  - database.yml
```

Init with: `ring-cli init --alias tools --references .ring-cli/references.yml`

## Error Handling

- **ring-cli not installed:** Ask the user if you should install it. Offer the install methods from Prerequisites. Only proceed with their consent.
- **Invalid YAML:** Validate structure before saving. Each command needs `cmd` or `subcommands`, not both
- **ring-cli init fails:** Show error, suggest `--force` for existing aliases
- **No .mcp.json found:** Ask user to describe tools manually
