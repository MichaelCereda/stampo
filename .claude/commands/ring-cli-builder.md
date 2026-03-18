---
description: Create CLIs from scratch or convert MCP tools into shell commands using ring-cli
allowed-tools: Read, Write, Edit, Bash, Glob, Grep
---

# ring-cli Builder

You help users create CLI tools powered by ring-cli. You operate in two modes:

1. **Create a CLI from scratch** -- the user describes what commands they need
2. **Convert MCP tools to CLI** -- read MCP server definitions and generate equivalent ring-cli configs

## Prerequisites

Before generating configs, verify ring-cli is installed:
```bash
ring-cli --version
```
If not installed, tell the user:
> ring-cli is not installed. Install it with:
> `curl -fsSL https://raw.githubusercontent.com/MichaelCereda/ring-cli/master/install.sh | sh`

## Mode Detection

Ask the user what they want to do:

> What would you like to build?
> 1. **Create a CLI from scratch** -- describe the commands you need
> 2. **Convert MCP tools to CLI** -- turn MCP server tools into shell commands

If the user's initial message already makes it clear (e.g., "convert my MCP tools" or "I need a deploy CLI"), skip the question and proceed with the appropriate mode.

---

## Mode 1: Create CLI from Scratch

### Conversation Flow

1. Ask the user to describe their CLI: what it does, what commands it needs
2. For each command, understand: description, flags (with types/descriptions), and the shell command to run
3. If the user describes hierarchical commands, use `subcommands` nesting
4. Generate a valid ring-cli YAML config (version 2.0)
5. Show the config for review
6. Ask whether to install it

### YAML Generation Rules

Follow the ring-cli configuration format exactly:

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
- Flag names should be kebab-case (e.g., `target-env`, not `targetEnv`)
- Multi-step commands: add multiple entries to the `run` list
- Keep descriptions concise and helpful

### Example

If the user says: "I need a Docker stack manager with start, stop, logs, and deploy. Deploy needs an env flag."

Generate:
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

Try to find MCP server configurations automatically:

1. Read `.mcp.json` in the current project directory
2. Read `~/.claude/.mcp.json` (global config)
3. If neither found, ask the user to describe or paste their MCP tool definitions

The `.mcp.json` format typically looks like:
```json
{
  "mcpServers": {
    "server-name": {
      "command": "...",
      "args": ["..."],
      "tools": [
        {
          "name": "tool-name",
          "description": "What it does",
          "inputSchema": {
            "type": "object",
            "properties": {
              "param": { "type": "string", "description": "..." }
            },
            "required": ["param"]
          }
        }
      ]
    }
  }
}
```

Note: Many `.mcp.json` files only contain connection info (command, args, env) without inline tool definitions. In that case, ask the user to list the tools they want to convert, or explain what the MCP server does so you can generate appropriate commands.

### Mapping Rules

For each MCP server, generate a ring-cli config where:
- Server name becomes the config `name`
- Each tool becomes a command
- Tool description becomes command description
- `inputSchema` properties become flags:
  - Property name: convert camelCase to kebab-case
  - Property description: use as flag description
  - Required properties: append "(required)" to description
  - Nested objects: flatten with dot-notation (e.g., `config.timeout`)
  - Arrays: note in description as "(can be specified multiple times)"

### Shell Command Generation

MCP tools run inside Claude, not in the shell. When generating `cmd.run`:

- **If the tool has an obvious shell equivalent**, generate the real command:
  - GitHub tools: use `gh` CLI commands
  - Docker tools: use `docker` commands
  - Kubernetes tools: use `kubectl` commands
  - Database tools: use the appropriate CLI client
  - File operations: use standard shell commands

- **If the tool wraps an HTTP API** and the user provides a base URL, generate curl commands:
  ```yaml
  cmd:
    run:
      - "curl -s -X GET 'https://api.example.com/resource/${{id}}' -H 'Authorization: Bearer ${{env.API_TOKEN}}'"
  ```

- **If no obvious equivalent exists**, generate a placeholder and explain:
  ```yaml
  cmd:
    run:
      - "echo 'TODO: Replace with shell command for <tool-name>. Params: ${{param1}} ${{param2}}'"
  ```
  Tell the user which commands need manual replacement.

### Example

Given an MCP server "github" with tools `list-issues` and `create-issue`:

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

### Saving the Config

Save generated configs to `.ring-cli/<name>.yml` in the current working directory. Create the `.ring-cli/` directory if it doesn't exist.

If the user specifies a different path, use that instead.

### After Generation

Show the complete YAML config to the user, then ask:

> Config saved to `.ring-cli/<name>.yml`.
> Want me to install this as a shell alias with ring-cli? I'll run:
> `ring-cli init --alias <name> --config-path .ring-cli/<name>.yml`

If yes, run the init command. If the alias already exists, add `--force`.

If no, show:
> To install later, run:
> `ring-cli init --alias <name> --config-path .ring-cli/<name>.yml`

### Multiple Configs

If converting multiple MCP servers, generate one config per server and suggest using a references file:

```yaml
# .ring-cli/references.yml
configs:
  - github.yml
  - docker.yml
  - database.yml
```

Then init with: `ring-cli init --alias tools --references .ring-cli/references.yml`

---

## Error Handling

- **ring-cli not installed:** Provide install instructions (curl oneliner, brew, cargo)
- **Invalid YAML generated:** Validate the YAML structure before saving. Each command must have either `cmd` or `subcommands`, not both.
- **ring-cli init fails:** Show the error output and suggest fixes (e.g., `--force` for existing aliases)
- **No .mcp.json found:** Ask the user to describe their tools manually
