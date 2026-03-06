# Banner Feature Design

**Goal:** Show an optional banner when running an aliased CLI, suppressible with `-q`.

## YAML Schema

Per-config:
```yaml
version: "2.0"
name: "services"
description: "..."
banner: "Services CLI v2.0"
```

References file:
```yaml
banner: "OpenStudio Infra Tools"
configs:
  - services.yml
  - db.yml
```

## Priority

- If the references file has a `banner`, show only that.
- Otherwise, show banners from individual configs (in load order).
- If no banners exist anywhere, show nothing.

## Display

- Banner prints to stderr (doesn't pollute piped output).
- Suppressed when `-q` / `--quiet` is passed.
- Printed before command execution in alias mode.

## Changes

- `Configuration` struct: add optional `banner` field.
- `References` struct: add optional `banner` field.
- Cache the top-level banner in `metadata.json` so alias mode can access it without re-reading the references file.
- Print banner in the alias-mode dispatch path, before command matching.

## Testing

- Unit test for banner field deserialization.
- Integration test verifying banner shows in output and `-q` suppresses it.
