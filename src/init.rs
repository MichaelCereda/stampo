use crate::{cache, models, openapi, shell, style};
use std::fs;
use std::path::PathBuf;

fn default_config_dir() -> PathBuf {
    dirs::home_dir()
        .expect("Unable to determine home directory")
        .join(".ring-cli/configurations")
}

pub(crate) fn validate_alias_name(name: &str) -> Result<(), anyhow::Error> {
    if name.is_empty() {
        anyhow::bail!("Alias name cannot be empty");
    }
    if !name.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
        anyhow::bail!(
            "Alias name '{}' contains invalid characters. Only alphanumeric, '-', and '_' are allowed.",
            name
        );
    }
    Ok(())
}

/// Resolve a relative `base_dir` in a configuration to an absolute path,
/// using the parent directory of the given config file path as the anchor.
pub(crate) fn resolve_base_dir(config: &mut models::Configuration, config_file_path: &str) {
    if let Some(ref dir) = config.base_dir {
        let p = std::path::Path::new(dir);
        if p.is_relative() {
            let config_parent = std::path::Path::new(config_file_path)
                .parent()
                .unwrap_or(std::path::Path::new("."));
            let resolved = config_parent.join(p);
            // Use canonicalize if the path exists, otherwise just use the joined path
            config.base_dir = Some(
                resolved
                    .canonicalize()
                    .unwrap_or(resolved)
                    .display()
                    .to_string(),
            );
        }
    }
}

fn create_default_config(path: &std::path::Path) -> Result<(), anyhow::Error> {
    if path.exists() {
        anyhow::bail!("File already exists: {}", path.display());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let template = r#"# Ring-CLI Configuration
version: "2.0"
name: "mycli"
description: "My custom CLI"
commands:
  greet:
    description: "Greet a user"
    flags:
      - name: "name"
        short: "n"
        description: "Name of the user to greet"
    cmd:
      run:
        - "echo Hello, ${{name}}!"
"#;
    fs::write(path, template)?;
    println!("Created configuration at: {}", path.display());
    Ok(())
}

/// A references file that lists config paths relative to its own location.
#[derive(serde::Deserialize)]
struct References {
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    banner: Option<String>,
    configs: Vec<String>,
}

fn resolve_references(references_path: &std::path::Path) -> Result<(Vec<PathBuf>, Option<String>, Option<String>), anyhow::Error> {
    let content = fs::read_to_string(references_path)
        .map_err(|e| anyhow::anyhow!("Cannot read references file '{}': {e}", references_path.display()))?;
    let refs: References = serde_saphyr::from_str(&content)
        .map_err(|e| anyhow::anyhow!("Invalid references file '{}': {e}", references_path.display()))?;

    let base_dir = references_path.parent().unwrap_or(std::path::Path::new("."));
    let mut paths = Vec::new();
    for config_path in &refs.configs {
        // openapi: prefixed paths are handled specially — skip filesystem resolution
        if config_path.starts_with("openapi:") {
            paths.push(PathBuf::from(config_path));
            continue;
        }
        let resolved = base_dir.join(config_path);
        if !resolved.exists() {
            anyhow::bail!(
                "Config '{}' referenced in '{}' does not exist (resolved to '{}')",
                config_path,
                references_path.display(),
                resolved.display()
            );
        }
        paths.push(resolved);
    }
    Ok((paths, refs.description, refs.banner))
}

pub(crate) fn handle_init(
    config_paths: Option<clap::parser::ValuesRef<'_, String>>,
    references_path: Option<&String>,
    alias: Option<&String>,
    warn_only_on_conflict: bool,
    check_for_updates: bool,
    force: bool,
    yes: bool,
    verbose: bool,
    description_override: Option<&String>,
) -> Result<(), anyhow::Error> {
    let alias_name = alias.ok_or_else(|| anyhow::anyhow!("--alias is required for init"))?;
    validate_alias_name(alias_name)?;

    // Check if alias already exists in any shell config
    let shells = shell::detect_shell_configs();
    let exists_in_any = shells.iter().any(|s| {
        fs::read_to_string(&s.path)
            .map(|content| shell::alias_exists(&content, alias_name, s.kind))
            .unwrap_or(false)
    });

    if exists_in_any {
        if !force {
            anyhow::bail!(
                "Alias '{}' already exists. Use --force to overwrite.",
                alias_name
            );
        }
        shell::clean_alias_from_shells(alias_name)?;
    }

    let (paths, refs_description, top_level_banner): (Vec<PathBuf>, Option<String>, Option<String>) = if let Some(ref_path) = references_path {
        resolve_references(std::path::Path::new(ref_path))?
    } else if let Some(paths) = config_paths {
        (paths.map(PathBuf::from).collect(), None, None)
    } else {
        let dir = default_config_dir();
        fs::create_dir_all(&dir)?;
        let path = dir.join(format!("{alias_name}.yml"));
        if !path.exists() {
            create_default_config(&path)?;
        }
        (vec![path], None, None)
    };

    // CLI --description flag overrides references file description.
    // For single-config aliases without either, use the config's own description.
    let description = description_override.cloned().or(refs_description);

    // Detect the HTTP tool once (lazily) for any openapi: paths.
    let mut http_tool_cache: Option<String> = None;
    let mut any_openapi = false;

    // Read and validate all configs
    // Each entry: (name, source_path, yaml_content, raw_content_for_hash)
    let mut configs_data_ext: Vec<(String, String, String, String)> = Vec::new();
    let mut seen_names: std::collections::HashMap<String, String> = std::collections::HashMap::new();

    for path in &paths {
        let path_str = path.to_string_lossy().to_string();

        if let Some(oa_source) = path_str.strip_prefix("openapi:") {
            // --- OpenAPI path ---
            any_openapi = true;
            let tool = if let Some(ref t) = http_tool_cache {
                t.clone()
            } else {
                let detected = openapi::http_tool::detect_http_tool()?;
                http_tool_cache = Some(detected.clone());
                detected
            };

            let (config, raw_content) =
                openapi::process_openapi_source(oa_source, &tool, yes, verbose)?;

            let yaml_content = serde_saphyr::to_string(&config).map_err(|e| {
                anyhow::anyhow!("Failed to serialize OpenAPI configuration '{}': {e}", config.name)
            })?;

            // Conflict check
            if let Some(prev_path) = seen_names.get(&config.name) {
                let msg = format!(
                    "Config name '{}' is used by both '{}' and '{}'",
                    config.name, prev_path, path_str
                );
                if warn_only_on_conflict {
                    eprintln!("{}", style::warn(&msg));
                } else {
                    anyhow::bail!("{}", msg);
                }
            }
            seen_names.insert(config.name.clone(), path_str.clone());
            configs_data_ext.push((config.name.clone(), path_str, yaml_content, raw_content));
        } else {
            // --- Regular YAML path ---
            if !path.exists() {
                create_default_config(path)?;
            }
            let abs_path = fs::canonicalize(path)?;
            let abs_path_str = abs_path.display().to_string();
            let content = fs::read_to_string(&abs_path)?;
            let config: models::Configuration = serde_saphyr::from_str(&content)
                .map_err(|e| anyhow::anyhow!("Invalid configuration at '{}': {e}", abs_path_str))?;

            // Conflict check
            if let Some(prev_path) = seen_names.get(&config.name) {
                let msg = format!(
                    "Config name '{}' is used by both '{}' and '{}'",
                    config.name, prev_path, abs_path_str
                );
                if warn_only_on_conflict {
                    eprintln!("{}", style::warn(&msg));
                } else {
                    anyhow::bail!("{}", msg);
                }
            }
            seen_names.insert(config.name.clone(), abs_path_str.clone());
            // For regular configs the YAML content and the raw content are the same.
            configs_data_ext.push((config.name.clone(), abs_path_str, content.clone(), content));
        }
    }

    // Build the configs_data slice that save_trusted_configs expects.
    // The third element is the YAML content written to the cache file.
    // The hash must be computed from raw_content (not the cached YAML for openapi: sources).
    // We need a custom save step for the hash, so we build the entries manually.
    let configs_data: Vec<(String, String, String)> = configs_data_ext
        .iter()
        .map(|(name, source, yaml, _raw)| (name.clone(), source.clone(), yaml.clone()))
        .collect();

    // Resolve effective banner: top-level (from references) takes priority,
    // otherwise collect per-config banners.
    let banner = if top_level_banner.is_some() {
        top_level_banner
    } else {
        let per_config: Vec<String> = configs_data_ext
            .iter()
            .filter_map(|(_, _, yaml, _)| {
                let config: models::Configuration = serde_saphyr::from_str(yaml).ok()?;
                config.banner
            })
            .collect();
        if per_config.is_empty() {
            None
        } else {
            Some(per_config.join("\n"))
        }
    };

    // For openapi: sources the hash must be over the raw spec content, not the
    // transformed YAML.  We save normally first, then patch the metadata entries.
    let detected_tool = if any_openapi { http_tool_cache.clone() } else { None };
    cache::save_trusted_configs(alias_name, &configs_data, description.clone(), banner.clone(), detected_tool.clone())?;

    // Patch hashes for openapi: sources so that refresh can detect real spec changes.
    if any_openapi {
        let dir = cache::alias_dir(alias_name);
        let meta_path = dir.join("metadata.json");
        let meta_str = fs::read_to_string(&meta_path)?;
        let mut meta: cache::AliasMetadata = serde_json::from_str(&meta_str)?;
        for (i, entry) in meta.configs.iter_mut().enumerate() {
            if entry.source_path.starts_with("openapi:")
                && let Some((_, _, _, raw)) = configs_data_ext.get(i) {
                    entry.hash = cache::compute_hash(raw);
                }
        }
        let patched = serde_json::to_string_pretty(&meta)?;
        fs::write(meta_path, patched)?;
    }

    // Install shell alias (only needs to be done once per alias)
    shell::install_alias(alias_name)?;

    // Install completion hooks
    shell::install_completions(alias_name)?;

    // Install or remove update-check shell hook
    if check_for_updates {
        shell::install_update_check(alias_name)?;
    } else {
        shell::remove_update_check(alias_name)?;
    }

    println!("{}", style::success(&format!("Alias '{}' is ready!", alias_name)));

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_alias_name_valid() {
        assert!(validate_alias_name("my-tool").is_ok());
        assert!(validate_alias_name("my_tool").is_ok());
        assert!(validate_alias_name("mytool123").is_ok());
    }

    #[test]
    fn test_validate_alias_name_invalid() {
        assert!(validate_alias_name("").is_err());
        assert!(validate_alias_name("my tool").is_err());
        assert!(validate_alias_name("my;tool").is_err());
        assert!(validate_alias_name("my'tool").is_err());
    }
}
