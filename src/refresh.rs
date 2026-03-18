use crate::{cache, models, openapi, style};
use std::fs;

/// Fetch the raw content for a config entry, handling both regular file paths
/// and `openapi:` prefixed sources.
///
/// For regular paths the file is read directly.
/// For `openapi:` sources the spec is re-fetched/re-read and re-transformed;
/// the *raw spec content* (before transformation) is returned as the hash
/// source, and the serialised YAML of the transformed `Configuration` is
/// returned as the cache content.
///
/// Returns `(yaml_content_for_cache, raw_content_for_hash)`.
fn fetch_source_content(
    entry: &cache::ConfigEntry,
    meta: &cache::AliasMetadata,
    yes: bool,
) -> Result<Option<(String, String)>, anyhow::Error> {
    if let Some(oa_source) = entry.source_path.strip_prefix("openapi:") {
        let tool = match &meta.http_tool {
            Some(t) => t.clone(),
            None => openapi::http_tool::detect_http_tool()?,
        };
        match openapi::process_openapi_source(oa_source, &tool, yes, false) {
            Ok((config, raw_content)) => {
                let yaml = serde_saphyr::to_string(&config).map_err(|e| {
                    anyhow::anyhow!(
                        "Failed to serialise OpenAPI config '{}': {e}",
                        config.name
                    )
                })?;
                Ok(Some((yaml, raw_content)))
            }
            Err(e) => {
                eprintln!(
                    "{}",
                    style::error(&format!(
                        "Failed to refresh OpenAPI source '{}': {e}",
                        entry.source_path
                    ))
                );
                Ok(None)
            }
        }
    } else {
        match fs::read_to_string(&entry.source_path) {
            Ok(content) => Ok(Some((content.clone(), content))),
            Err(_) => Ok(None),
        }
    }
}

pub(crate) fn handle_check_updates(alias_name: &str, yes: bool) -> Result<(), anyhow::Error> {
    let (_, metadata) = match cache::load_trusted_configs(alias_name) {
        Ok(data) => data,
        Err(_) => return Ok(()), // Silently skip if no cache — don't block shell startup
    };

    let mut changed: Vec<&cache::ConfigEntry> = Vec::new();
    for entry in &metadata.configs {
        let raw_content = if entry.source_path.starts_with("openapi:") {
            match fetch_source_content(entry, &metadata, yes) {
                Ok(Some((_, raw))) => raw,
                _ => continue,
            }
        } else {
            match fs::read_to_string(&entry.source_path) {
                Ok(c) => c,
                Err(_) => continue, // Source gone — skip silently
            }
        };
        let current_hash = cache::compute_hash(&raw_content);
        if current_hash != entry.hash {
            changed.push(entry);
        }
    }

    if changed.is_empty() {
        return Ok(());
    }

    // Initialize color for the prompt
    style::init(style::ColorMode::Auto);

    println!(
        "{}",
        style::warn(&format!(
            "Configuration updates available for '{alias_name}':"
        ))
    );
    for entry in &changed {
        println!("  - {} ({})", entry.name, entry.source_path);
    }

    eprint!("Do you want to update? [y/N] ");
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    if input.trim().to_lowercase() != "y" {
        return Ok(());
    }

    // Re-read all configs and trust the updated versions
    let mut updated_configs: Vec<(String, String, String)> = Vec::new();
    let mut raw_overrides: Vec<(usize, String)> = Vec::new(); // (index, raw_hash_content)

    for (i, entry) in metadata.configs.iter().enumerate() {
        match fetch_source_content(entry, &metadata, yes) {
            Ok(Some((yaml_content, raw_content))) => {
                // Validate before trusting (for regular YAML configs)
                if !entry.source_path.starts_with("openapi:") {
                    let _: models::Configuration = serde_saphyr::from_str(&yaml_content)
                        .map_err(|e| {
                            anyhow::anyhow!(
                                "Updated configuration '{}' is invalid: {e}",
                                entry.name
                            )
                        })?;
                }
                if entry.source_path.starts_with("openapi:") {
                    raw_overrides.push((i, raw_content));
                }
                updated_configs.push((entry.name.clone(), entry.source_path.clone(), yaml_content));
            }
            _ => {
                // Fall back to cached copy
                let dir = cache::alias_dir(alias_name);
                let cached =
                    fs::read_to_string(dir.join(format!("{}.yml", entry.name)))?;
                updated_configs.push((
                    entry.name.clone(),
                    entry.source_path.clone(),
                    cached,
                ));
            }
        }
    }

    cache::save_trusted_configs(
        alias_name,
        &updated_configs,
        metadata.description.clone(),
        metadata.banner.clone(),
        metadata.http_tool.clone(),
    )?;

    // Patch hashes for openapi: sources
    if !raw_overrides.is_empty() {
        let dir = cache::alias_dir(alias_name);
        let meta_path = dir.join("metadata.json");
        let meta_str = fs::read_to_string(&meta_path)?;
        let mut new_meta: cache::AliasMetadata = serde_json::from_str(&meta_str)?;
        for (idx, raw) in raw_overrides {
            if let Some(e) = new_meta.configs.get_mut(idx) {
                e.hash = cache::compute_hash(&raw);
            }
        }
        fs::write(meta_path, serde_json::to_string_pretty(&new_meta)?)?;
    }

    println!("{}", style::success("Configuration updated and trusted."));

    Ok(())
}

pub(crate) fn handle_refresh_configuration(
    alias_name: &str,
    yes: bool,
) -> Result<(), anyhow::Error> {
    let (_, metadata) = cache::load_trusted_configs(alias_name).map_err(|_| {
        anyhow::anyhow!(
            "No cached configuration found for alias '{alias_name}'. Run 'ring-cli init' first."
        )
    })?;

    let mut any_changed = false;
    let mut updated_configs: Vec<(String, String, String)> = Vec::new();
    let mut raw_overrides: Vec<(usize, String)> = Vec::new(); // (index, raw_hash_content)

    for (i, entry) in metadata.configs.iter().enumerate() {
        match fetch_source_content(entry, &metadata, yes) {
            Ok(Some((yaml_content, raw_content))) => {
                let current_hash = cache::compute_hash(&raw_content);

                if current_hash == entry.hash {
                    // Unchanged — keep as-is
                    updated_configs.push((
                        entry.name.clone(),
                        entry.source_path.clone(),
                        yaml_content,
                    ));
                    continue;
                }

                // Validate regular YAML configs before prompting
                if !entry.source_path.starts_with("openapi:") {
                    let _: models::Configuration = serde_saphyr::from_str(&yaml_content)
                        .map_err(|e| {
                            anyhow::anyhow!(
                                "New configuration '{}' is invalid: {e}",
                                entry.name
                            )
                        })?;
                }

                println!(
                    "{}",
                    style::warn(&format!("Configuration '{}' has changed.", entry.name))
                );
                println!("Source: {}", entry.source_path);

                let accepted = if yes {
                    true
                } else {
                    eprint!("Trust this configuration? [y/N] ");
                    let mut input = String::new();
                    std::io::stdin().read_line(&mut input)?;
                    input.trim().to_lowercase() == "y"
                };

                if accepted {
                    if entry.source_path.starts_with("openapi:") {
                        raw_overrides.push((i, raw_content));
                    }
                    updated_configs.push((
                        entry.name.clone(),
                        entry.source_path.clone(),
                        yaml_content,
                    ));
                    any_changed = true;
                } else {
                    println!("Keeping previous version of '{}'.", entry.name);
                    let dir = cache::alias_dir(alias_name);
                    let cached =
                        fs::read_to_string(dir.join(format!("{}.yml", entry.name)))?;
                    updated_configs.push((
                        entry.name.clone(),
                        entry.source_path.clone(),
                        cached,
                    ));
                }
            }
            _ => {
                eprintln!(
                    "{}",
                    style::error(&format!(
                        "Source '{}' not found at '{}'. Using cached copy.",
                        entry.name, entry.source_path
                    ))
                );
                let dir = cache::alias_dir(alias_name);
                let cached =
                    fs::read_to_string(dir.join(format!("{}.yml", entry.name)))?;
                updated_configs.push((
                    entry.name.clone(),
                    entry.source_path.clone(),
                    cached,
                ));
            }
        }
    }

    if any_changed {
        cache::save_trusted_configs(
            alias_name,
            &updated_configs,
            metadata.description.clone(),
            metadata.banner.clone(),
            metadata.http_tool.clone(),
        )?;

        // Patch hashes for openapi: sources
        if !raw_overrides.is_empty() {
            let dir = cache::alias_dir(alias_name);
            let meta_path = dir.join("metadata.json");
            let meta_str = fs::read_to_string(&meta_path)?;
            let mut new_meta: cache::AliasMetadata = serde_json::from_str(&meta_str)?;
            for (idx, raw) in raw_overrides {
                if let Some(e) = new_meta.configs.get_mut(idx) {
                    e.hash = cache::compute_hash(&raw);
                }
            }
            fs::write(meta_path, serde_json::to_string_pretty(&new_meta)?)?;
        }

        println!("{}", style::success("Configuration updated and trusted."));
    } else {
        println!("{}", style::success("Configuration is up to date."));
    }

    Ok(())
}
