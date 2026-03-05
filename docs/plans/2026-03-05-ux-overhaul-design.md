# ring-cli UX Overhaul Design

## Context

ring-cli is a tool that generates custom CLIs from YAML config files. Users create a config, run `ring-cli init --alias XXX`, and then use `XXX` as a fully functional CLI. This overhaul improves the end-to-end experience: completions, color, security, and polish.

## Architecture

Two distinct personas:

- **ring-cli** (the installer): `ring-cli init --alias XXX --config-path <path>`. Sets up the alias, caches the config, installs completions. This is the only direct interaction users have with `ring-cli`.
- **XXX** (the user's tool): Entirely defined by YAML config. Commands, flags, subcommands come from the config. The only built-in command exposed is `refresh-configuration`.

## Config Format v2

Remove `slug`. Commands are top-level. Version bumped to `2.0`. Old v1 configs with `slug` are rejected with a clear error.

```yaml
version: "2.0"
description: "Infrastructure management tools"
commands:
  deploy:
    description: "Deployment operations"
    flags: []
    subcommands:
      staging:
        description: "Deploy to staging"
        flags:
          - name: "branch"
            short: "b"
            description: "Branch to deploy"
        cmd:
          run:
            - "echo Deploying ${{branch}} to staging"
      prod:
        description: "Deploy to production"
        flags:
          - name: "tag"
            short: "t"
            description: "Release tag"
        cmd:
          run:
            - "echo Deploying ${{tag}} to prod"
  db:
    description: "Database operations"
    flags: []
    subcommands:
      migrate:
        description: "Run migrations"
        flags: []
        cmd:
          run:
            - "echo Running migrations..."
      seed:
        description: "Seed database"
        flags: []
        cmd:
          run:
            - "echo Seeding database..."
```

## Completion Cache + Trust System

### Flow

1. `ring-cli init --alias XXX --config-path <path>` reads the YAML, stores a trusted copy + SHA-256 hash + completion data in `~/.ring-cli/aliases/XXX/`. Auto-trusted since the user just created or pointed to it. Installs completion hook in shell configs alongside the alias.

2. `XXX <command>` loads from the cached/trusted config. Does NOT read the original YAML file at runtime. Fast and safe.

3. `XXX refresh-configuration` reads the original YAML file, compares hash to trusted version. If changed: shows what changed (new/removed/modified commands), asks "Trust this configuration? [y/N]". On yes: updates cached config + hash. On no: keeps old trusted version. If unchanged: prints "Configuration is up to date."

4. If the original YAML is deleted or moved, `XXX` still works from cache. `refresh-configuration` reports the source file is missing.

### Cache Structure

```
~/.ring-cli/
  aliases/
    XXX/
      config.yml          # trusted copy of the config
      metadata.json       # { source_path, hash, trusted_at }
```

### What This Protects Against

Someone (or a script) modifying the YAML to inject malicious commands. The user must explicitly refresh and trust before changes take effect.

## Color Output

- Auto-detect TTY via `stdout.is_terminal()`. Color on if terminal, off if piped.
- `NO_COLOR` env var disables all color regardless of TTY.
- `--color=<auto|always|never>` flag overrides auto-detection. Default is `auto`.
- No new color crate — use ANSI escape codes directly. Small palette: red (errors), yellow (warnings), green (success), bold, dim.
- Command output (echo, HTTP responses) is never colored — always pass-through.
- Clap 4.5 has built-in help styling via the `color` feature.

## Unified CLI Through Clap

`init` moves into clap proper (currently parsed manually). `refresh-configuration` is a built-in subcommand added to the alias CLI.

### ring-cli help

```
ring-cli - CLI generator from YAML configurations

Usage: ring-cli [COMMAND]

Commands:
  init   Create a new configuration and install as a shell alias
  help   Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version
```

### XXX help (the alias)

```
Infrastructure management tools

Usage: XXX [OPTIONS] [COMMAND]

Commands:
  deploy                  Deployment operations
  db                      Database operations
  refresh-configuration   Re-read and trust updated configuration
  help                    Print this message or the help of the given subcommand(s)

Options:
      --color <WHEN>  Color output [default: auto] [auto, always, never]
  -v, --verbose       Print verbose output
  -q, --quiet         Suppress error messages
  -h, --help          Print help
  -V, --version       Print version
```

## Test Suite

### Unit Tests

- Config v2 parsing (no slug, commands at top level)
- Color output functions (enabled/disabled, TTY detection)
- Trust hash computation and comparison
- Completion data generation from config
- Alias name validation

### Integration Tests

- `ring-cli init --alias XXX --config-path <path>` creates cache structure
- `XXX --help` shows commands from YAML, not ring-cli internals
- `XXX <command>` runs from cached config
- `XXX refresh-configuration` detects changes, prompts for trust
- `XXX refresh-configuration` with unchanged config says "up to date"
- Tab completion returns correct commands and flags
- Color output present when TTY, absent when piped
- `NO_COLOR` env var disables color
- `--color=never` disables, `--color=always` forces color
- Config with missing source file still works from cache
- Old v1 configs with `slug` are rejected with clear error

### Edge Cases

- Empty config (no commands)
- Deeply nested subcommands completion
- Config path with spaces
- Flag names that could collide with built-in flags

## Files Impacted

- `src/models.rs` — remove slug, bump version
- `src/main.rs` — rewrite init flow, add refresh-configuration, trust system
- `src/cli.rs` — unified clap build, completion generation, color support
- `src/utils.rs` — cache read/write, hash computation
- New: `src/style.rs` — color output helpers
- New: `src/cache.rs` — trust/cache management
- `tests/integration.rs` — rewrite for v2, new test cases
- `Cargo.toml` — add `clap_complete`, `sha2`

## Breaking Changes

- Config version `2.0` required
- `slug` field removed
- Old v1 configs rejected with clear error message
