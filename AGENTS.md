# Ring-CLI

Ring-CLI generates custom CLIs from YAML configuration files. You define commands, flags, and subcommands in YAML, then install them as a shell command with tab completion and a trust-based security model.

**Start here:** See the [README](README.md) for getting started, configuration format, and complete feature documentation including banners, references files, tab completion, and the `--force` flag.

## Quick Reference

**Install:** `cargo install --path .` (requires Rust toolchain)

**Create an alias:**
```bash
ring-cli init --alias <name> --config-path <file.yml> [--config-path <file2.yml>] [--force] [--check-for-updates]

# Or with a references file:
ring-cli init --alias <name> --references <references.yml> [--force]
```

**Config format (v2.0):**
```yaml
version: "2.0"
name: "<group-name>"
description: "<description>"
base-dir: ".."  # relative to this file, or absolute
banner: "optional startup message"
commands:
  <command-name>:
    description: "<description>"
    flags:
      - name: "<flag-name>"
        short: "<char>"
        description: "<description>"
    cmd:
      run:
        - "<shell command using ${{flag_name}} and ${{env.VAR}}>"
```

Each command must have exactly one of `cmd` or `subcommands`, not both.

**Variable substitution:** `${{flag_name}}` for flag values, `${{env.VAR}}` for environment variables.

**Full documentation:** See [README.md](README.md) for all features and the [setup guide](docs/setup-guide.md) for step-by-step installation, YAML schema reference, validation rules, examples, and troubleshooting.

## Project Structure

```
src/
  main.rs      — Entry point, alias installation, shell hooks, argument dispatch
  cli.rs       — CLI construction (clap builder API), command execution
  models.rs    — YAML data structures (Configuration, Command, Flag, CmdType)
  cache.rs     — Trusted config storage (~/.ring-cli/aliases/), SHA-256 hashing
  utils.rs     — Config loading, placeholder/env-var replacement
  style.rs     — Color output (ANSI, NO_COLOR, --color flag)
  errors.rs    — Error types
tests/
  integration.rs — End-to-end CLI tests (init, completions, live shell tests)
  fixtures/      — Test YAML configs
```

## Key Concepts

- **Two modes:** Installer mode (`ring-cli init`) and alias mode (`ring-cli --alias-mode <name>`)
- **Multi-config:** Multiple `--config-path` flags per alias; each config's `name` becomes a subcommand
- **References file:** `--references` flag loads a YAML manifest listing config paths and an optional top-level banner
- **Banner:** Optional message displayed on stderr when alias is invoked; suppressed with `-q`
- **Trust system:** Configs are cached with SHA-256 hashes in `~/.ring-cli/aliases/<name>/`
- **Update checking:** `--check-for-updates` installs a shell startup hook; `refresh-configuration` re-trusts changed configs
- **Shell support:** Bash, Zsh, Fish, PowerShell (shell functions + tab completion)
- **`--force` flag:** Required to overwrite an existing alias during init; cleans old entries from all shell configs before re-installing
- **Shell functions (not aliases):** Uses `name() { ring-cli --alias-mode name "$@"; }` so tab completion works correctly with zsh/bash

## Git Conventions

- **No co-authoring:** Do not add `Co-Authored-By` trailers to commit messages.
