mod cache;
mod cli;
mod errors;
mod models;
mod style;
mod utils;

use std::fs;
use std::path::PathBuf;

fn default_config_dir() -> PathBuf {
    dirs::home_dir()
        .expect("Unable to determine home directory")
        .join(".ring-cli/configurations")
}

#[derive(Clone, Copy)]
enum ShellKind {
    BashZsh,
    Fish,
    PowerShell,
}

fn validate_alias_name(name: &str) -> Result<(), anyhow::Error> {
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

fn alias_line_bash_zsh(alias_name: &str) -> String {
    format!("alias {alias_name}='ring-cli --alias-mode {alias_name}' # ring-cli")
}

fn alias_line_fish(alias_name: &str) -> String {
    format!("alias {alias_name} 'ring-cli --alias-mode {alias_name}' # ring-cli")
}

fn alias_line_powershell(alias_name: &str) -> String {
    format!("function {alias_name} {{ ring-cli --alias-mode {alias_name} @args }} # ring-cli")
}

fn alias_exists(file_content: &str, alias_name: &str, kind: ShellKind) -> bool {
    let pattern = match kind {
        ShellKind::BashZsh => format!("alias {alias_name}="),
        ShellKind::Fish => format!("alias {alias_name} "),
        ShellKind::PowerShell => format!("function {alias_name}"),
    };
    file_content.contains(&pattern)
}

struct ShellConfig {
    path: PathBuf,
    kind: ShellKind,
    display_name: &'static str,
}

fn detect_shell_configs() -> Vec<ShellConfig> {
    let home = match dirs::home_dir() {
        Some(h) => h,
        None => return vec![],
    };
    let candidates = vec![
        ShellConfig {
            path: home.join(".bashrc"),
            kind: ShellKind::BashZsh,
            display_name: "~/.bashrc",
        },
        ShellConfig {
            path: home.join(".zshrc"),
            kind: ShellKind::BashZsh,
            display_name: "~/.zshrc",
        },
        ShellConfig {
            path: home.join(".config/fish/config.fish"),
            kind: ShellKind::Fish,
            display_name: "~/.config/fish/config.fish",
        },
        ShellConfig {
            path: home.join(".config/powershell/Microsoft.PowerShell_profile.ps1"),
            kind: ShellKind::PowerShell,
            display_name: "~/.config/powershell/Microsoft.PowerShell_profile.ps1",
        },
    ];
    #[cfg(target_os = "windows")]
    let candidates = {
        let mut c = candidates;
        c.push(ShellConfig {
            path: home.join("Documents/PowerShell/Microsoft.PowerShell_profile.ps1"),
            kind: ShellKind::PowerShell,
            display_name: "~/Documents/PowerShell/Microsoft.PowerShell_profile.ps1",
        });
        c
    };
    candidates.into_iter().filter(|sc| sc.path.exists()).collect()
}

fn install_alias(alias_name: &str) -> Result<(), anyhow::Error> {
    validate_alias_name(alias_name)?;
    let shells = detect_shell_configs();
    if shells.is_empty() {
        eprintln!("Warning: No shell config files found. Add the alias manually:");
        eprintln!("  Bash/Zsh: {}", alias_line_bash_zsh(alias_name));
        eprintln!("  Fish:     {}", alias_line_fish(alias_name));
        eprintln!("  PowerShell: {}", alias_line_powershell(alias_name));
        return Ok(());
    }

    let mut modified = Vec::new();
    for shell in &shells {
        let content = fs::read_to_string(&shell.path)?;
        if alias_exists(&content, alias_name, shell.kind) {
            println!("Alias '{}' already exists in {}, skipping.", alias_name, shell.display_name);
            continue;
        }
        let line = match shell.kind {
            ShellKind::BashZsh => alias_line_bash_zsh(alias_name),
            ShellKind::Fish => alias_line_fish(alias_name),
            ShellKind::PowerShell => alias_line_powershell(alias_name),
        };
        let mut file = fs::OpenOptions::new().append(true).open(&shell.path)?;
        use std::io::Write;
        writeln!(file, "\n{}", line)?;
        modified.push(shell.display_name);
    }

    if !modified.is_empty() {
        println!("Added alias '{}' to:", alias_name);
        for name in &modified {
            println!("  {}", name);
        }
        if let Some(first) = modified.first() {
            println!("Restart your terminal or run 'source {}' to use '{}'.", first, alias_name);
        }
    }

    Ok(())
}

fn install_update_check(alias_name: &str) -> Result<(), anyhow::Error> {
    let shells = detect_shell_configs();
    for shell in &shells {
        let content = fs::read_to_string(&shell.path)?;
        let marker = format!("# ring-cli-update-check:{alias_name}");
        if content.contains(&marker) {
            continue;
        }
        let hook = format!("ring-cli --check-updates {alias_name} {marker}");
        let mut file = fs::OpenOptions::new().append(true).open(&shell.path)?;
        use std::io::Write;
        writeln!(file, "{}", hook)?;
    }
    Ok(())
}

fn remove_update_check(alias_name: &str) -> Result<(), anyhow::Error> {
    let shells = detect_shell_configs();
    let marker = format!("# ring-cli-update-check:{alias_name}");
    for shell in &shells {
        let content = fs::read_to_string(&shell.path)?;
        if !content.contains(&marker) {
            continue;
        }
        let filtered: String = content
            .lines()
            .filter(|line| !line.contains(&marker))
            .collect::<Vec<_>>()
            .join("\n");
        // Preserve trailing newline
        let filtered = if content.ends_with('\n') {
            format!("{filtered}\n")
        } else {
            filtered
        };
        fs::write(&shell.path, filtered)?;
    }
    Ok(())
}

fn handle_check_updates(alias_name: &str) -> Result<(), anyhow::Error> {
    let (_, metadata) = match cache::load_trusted_configs(alias_name) {
        Ok(data) => data,
        Err(_) => return Ok(()), // Silently skip if no cache — don't block shell startup
    };

    let mut changed: Vec<&cache::ConfigEntry> = Vec::new();
    for entry in &metadata.configs {
        let source_content = match fs::read_to_string(&entry.source_path) {
            Ok(c) => c,
            Err(_) => continue, // Source gone — skip silently
        };
        let current_hash = cache::compute_hash(&source_content);
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
    for entry in &metadata.configs {
        let source_content = match fs::read_to_string(&entry.source_path) {
            Ok(content) => {
                // Validate before trusting
                let _config: models::Configuration = serde_saphyr::from_str(&content)
                    .map_err(|e| {
                        anyhow::anyhow!("Updated configuration '{}' is invalid: {e}", entry.name)
                    })?;
                content
            }
            Err(_) => {
                // Fall back to cached copy
                let dir = cache::alias_dir(alias_name);
                fs::read_to_string(dir.join(format!("{}.yml", entry.name)))?
            }
        };
        updated_configs.push((entry.name.clone(), entry.source_path.clone(), source_content));
    }

    cache::save_trusted_configs(alias_name, &updated_configs)?;
    println!("{}", style::success("Configuration updated and trusted."));

    Ok(())
}

fn install_completions(alias_name: &str) -> Result<(), anyhow::Error> {
    let shells = detect_shell_configs();
    for shell in &shells {
        let content = fs::read_to_string(&shell.path)?;
        let completion_marker = format!("# ring-cli-completions:{alias_name}");
        if content.contains(&completion_marker) {
            continue;
        }
        let hook = match shell.kind {
            ShellKind::BashZsh => {
                if shell.display_name.contains("zsh") {
                    format!(
                        "eval \"$(ring-cli --generate-completions zsh {alias_name})\" {completion_marker}"
                    )
                } else {
                    format!(
                        "eval \"$(ring-cli --generate-completions bash {alias_name})\" {completion_marker}"
                    )
                }
            }
            ShellKind::Fish => {
                format!(
                    "ring-cli --generate-completions fish {alias_name} | source {completion_marker}"
                )
            }
            ShellKind::PowerShell => {
                format!(
                    "ring-cli --generate-completions powershell {alias_name} | Invoke-Expression {completion_marker}"
                )
            }
        };
        let mut file = fs::OpenOptions::new().append(true).open(&shell.path)?;
        use std::io::Write;
        writeln!(file, "{}", hook)?;
    }
    Ok(())
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
    configs: Vec<String>,
}

fn resolve_references(references_path: &std::path::Path) -> Result<Vec<PathBuf>, anyhow::Error> {
    let content = fs::read_to_string(references_path)
        .map_err(|e| anyhow::anyhow!("Cannot read references file '{}': {e}", references_path.display()))?;
    let refs: References = serde_saphyr::from_str(&content)
        .map_err(|e| anyhow::anyhow!("Invalid references file '{}': {e}", references_path.display()))?;

    let base_dir = references_path.parent().unwrap_or(std::path::Path::new("."));
    let mut paths = Vec::new();
    for config_path in &refs.configs {
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
    Ok(paths)
}

fn handle_init(
    config_paths: Option<clap::parser::ValuesRef<'_, String>>,
    references_path: Option<&String>,
    alias: Option<&String>,
    warn_only_on_conflict: bool,
    check_for_updates: bool,
) -> Result<(), anyhow::Error> {
    let alias_name = alias.ok_or_else(|| anyhow::anyhow!("--alias is required for init"))?;
    validate_alias_name(alias_name)?;

    let paths: Vec<PathBuf> = if let Some(ref_path) = references_path {
        resolve_references(std::path::Path::new(ref_path))?
    } else if let Some(paths) = config_paths {
        paths.map(PathBuf::from).collect()
    } else {
        let dir = default_config_dir();
        fs::create_dir_all(&dir)?;
        let path = dir.join(format!("{alias_name}.yml"));
        if !path.exists() {
            create_default_config(&path)?;
        }
        vec![path]
    };

    // Read and validate all configs
    let mut configs_data: Vec<(String, String, String)> = Vec::new(); // (name, abs_path, content)
    let mut seen_names: std::collections::HashMap<String, String> = std::collections::HashMap::new();

    for path in &paths {
        if !path.exists() {
            create_default_config(path)?;
        }
        let abs_path = fs::canonicalize(path)?;
        let abs_path_str = abs_path.display().to_string();
        let content = fs::read_to_string(&abs_path)?;
        let config: models::Configuration = serde_saphyr::from_str(&content)
            .map_err(|e| anyhow::anyhow!("Invalid configuration at '{}': {e}", abs_path_str))?;

        // Check for name conflicts
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

        configs_data.push((config.name.clone(), abs_path_str, content));
    }

    // Save all trusted configs to cache
    cache::save_trusted_configs(alias_name, &configs_data)?;

    // Install shell alias (only needs to be done once per alias)
    install_alias(alias_name)?;

    // Install completion hooks
    install_completions(alias_name)?;

    // Install or remove update-check shell hook
    if check_for_updates {
        install_update_check(alias_name)?;
    } else {
        remove_update_check(alias_name)?;
    }

    println!("{}", style::success(&format!("Alias '{}' is ready!", alias_name)));

    Ok(())
}

fn handle_refresh_configuration(alias_name: &str) -> Result<(), anyhow::Error> {
    let (_, metadata) = cache::load_trusted_configs(alias_name)
        .map_err(|_| anyhow::anyhow!("No cached configuration found for alias '{alias_name}'. Run 'ring-cli init' first."))?;

    let mut any_changed = false;
    let mut updated_configs: Vec<(String, String, String)> = Vec::new();

    for entry in &metadata.configs {
        let source_content = match fs::read_to_string(&entry.source_path) {
            Ok(content) => content,
            Err(_) => {
                eprintln!("{}", style::error(&format!(
                    "Source '{}' not found at '{}'. Using cached copy.",
                    entry.name, entry.source_path
                )));
                let dir = cache::alias_dir(alias_name);
                let cached = fs::read_to_string(dir.join(format!("{}.yml", entry.name)))?;
                updated_configs.push((entry.name.clone(), entry.source_path.clone(), cached));
                continue;
            }
        };

        let current_hash = cache::compute_hash(&source_content);
        if current_hash == entry.hash {
            // Unchanged — keep as-is
            updated_configs.push((entry.name.clone(), entry.source_path.clone(), source_content));
            continue;
        }

        // Changed — validate before prompting
        let _config: models::Configuration = serde_saphyr::from_str(&source_content)
            .map_err(|e| anyhow::anyhow!("New configuration '{}' is invalid: {e}", entry.name))?;

        println!("{}", style::warn(&format!("Configuration '{}' has changed.", entry.name)));
        println!("Source: {}", entry.source_path);

        eprint!("Trust this configuration? [y/N] ");
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        if input.trim().to_lowercase() == "y" {
            updated_configs.push((entry.name.clone(), entry.source_path.clone(), source_content));
            any_changed = true;
        } else {
            println!("Keeping previous version of '{}'.", entry.name);
            let dir = cache::alias_dir(alias_name);
            let cached = fs::read_to_string(dir.join(format!("{}.yml", entry.name)))?;
            updated_configs.push((entry.name.clone(), entry.source_path.clone(), cached));
        }
    }

    if any_changed {
        cache::save_trusted_configs(alias_name, &updated_configs)?;
        println!("{}", style::success("Configuration updated and trusted."));
    } else {
        println!("{}", style::success("Configuration is up to date."));
    }

    Ok(())
}

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();

    // Handle completion generation (called by shell hooks installed during init)
    if let Some(pos) = args.iter().position(|a| a == "--generate-completions") {
        let shell_name = args
            .get(pos + 1)
            .ok_or_else(|| anyhow::anyhow!("Missing shell name after --generate-completions"))?;
        let alias_name = args
            .get(pos + 2)
            .ok_or_else(|| anyhow::anyhow!("Missing alias name after --generate-completions"))?;

        let shell: clap_complete::Shell = shell_name
            .parse()
            .map_err(|_| anyhow::anyhow!("Unknown shell: {shell_name}"))?;

        let (config_contents, _metadata) = cache::load_trusted_configs(alias_name)?;
        let configs: Vec<models::Configuration> = config_contents
            .iter()
            .map(|c| {
                serde_saphyr::from_str(c)
                    .map_err(|e| anyhow::anyhow!("Cached config invalid: {e}"))
            })
            .collect::<Result<_, _>>()?;

        let mut cmd = cli::build_cli(&configs);
        clap_complete::generate(shell, &mut cmd, alias_name.as_str(), &mut std::io::stdout());
        return Ok(());
    }

    // Handle update check (called by shell startup hook installed via --check-for-updates)
    if let Some(pos) = args.iter().position(|a| a == "--check-updates") {
        let alias_name = args
            .get(pos + 1)
            .ok_or_else(|| anyhow::anyhow!("Missing alias name after --check-updates"))?;
        return handle_check_updates(alias_name);
    }

    // Detect alias mode (--alias-mode <name>)
    let alias_mode = args
        .iter()
        .position(|a| a == "--alias-mode")
        .and_then(|i| args.get(i + 1).cloned());

    // Also support legacy -c / --config for backwards compat
    let config_path = args
        .iter()
        .find(|arg| arg.starts_with("--config="))
        .and_then(|arg| arg.split('=').nth(1).map(String::from))
        .or_else(|| {
            args.iter()
                .position(|a| a == "-c" || a == "--config")
                .and_then(|i| args.get(i + 1).cloned())
        });

    if let Some(ref alias_name) = alias_mode {
        // ALIAS MODE: load all configs from cache for this alias
        let (config_contents, _metadata) = cache::load_trusted_configs(alias_name)?;
        let configs: Vec<models::Configuration> = config_contents
            .iter()
            .map(|c| {
                serde_saphyr::from_str(c)
                    .map_err(|e| anyhow::anyhow!("Invalid cached config: {e}"))
            })
            .collect::<Result<_, _>>()?;

        // Strip --alias-mode and its value from args before clap sees them
        let clap_args: Vec<String> = {
            let mut out = Vec::with_capacity(args.len());
            let mut skip_next = false;
            for arg in &args {
                if skip_next {
                    skip_next = false;
                    continue;
                }
                if arg == "--alias-mode" {
                    skip_next = true;
                    continue;
                }
                out.push(arg.clone());
            }
            out
        };

        let matches = cli::build_cli(&configs).get_matches_from(clap_args);

        // Initialize color mode
        let color_str = matches.get_one::<String>("color").map(|s| s.as_str()).unwrap_or("auto");
        style::init(match color_str {
            "always" => style::ColorMode::Always,
            "never" => style::ColorMode::Never,
            _ => style::ColorMode::Auto,
        });

        let is_quiet = matches.get_flag("quiet");
        let is_verbose = matches.get_flag("verbose");

        // Handle refresh-configuration
        if matches.subcommand_matches("refresh-configuration").is_some() {
            return handle_refresh_configuration(alias_name);
        }

        // Dispatch: match config name subcommand, then command within that config
        for config in &configs {
            if let Some(config_matches) = matches.subcommand_matches(&config.name) {
                for (cmd_name, cmd) in &config.commands {
                    if let Some(cmd_matches) = config_matches.subcommand_matches(cmd_name) {
                        if let Err(e) = cli::execute_command(cmd, cmd_matches, is_verbose, config.base_dir.as_deref()) {
                            if !is_quiet {
                                eprintln!("{}", style::error(&e.to_string()));
                            }
                            std::process::exit(1);
                        }
                    }
                }
            }
        }
    } else if let Some(ref path) = config_path {
        // LEGACY SINGLE-CONFIG MODE: -c <path>
        let config = utils::load_configuration(path)?;
        let configs = vec![config];

        // Strip the config flag (and its value) from args before clap parses them
        let clap_args: Vec<String> = {
            let mut out = Vec::with_capacity(args.len());
            let mut skip_next = false;
            for arg in &args {
                if skip_next {
                    skip_next = false;
                    continue;
                }
                if arg.starts_with("--config=") {
                    continue;
                }
                if arg == "-c" || arg == "--config" {
                    skip_next = true;
                    continue;
                }
                out.push(arg.clone());
            }
            out
        };

        let matches = cli::build_cli(&configs).get_matches_from(clap_args);

        // Initialize color mode
        let color_str = matches.get_one::<String>("color").map(|s| s.as_str()).unwrap_or("auto");
        style::init(match color_str {
            "always" => style::ColorMode::Always,
            "never" => style::ColorMode::Never,
            _ => style::ColorMode::Auto,
        });

        let is_quiet = matches.get_flag("quiet");
        let is_verbose = matches.get_flag("verbose");

        // Handle refresh-configuration
        if matches.subcommand_matches("refresh-configuration").is_some() {
            // In legacy mode we don't have an alias name — inform user to reinitialise
            eprintln!("{}", style::error("refresh-configuration requires --alias-mode. Please re-run 'ring-cli init' to update your alias."));
            std::process::exit(1);
        }

        // Dispatch: commands are nested under config.name subcommand
        for config in &configs {
            if let Some(config_matches) = matches.subcommand_matches(&config.name) {
                for (cmd_name, cmd) in &config.commands {
                    if let Some(cmd_matches) = config_matches.subcommand_matches(cmd_name) {
                        if let Err(e) = cli::execute_command(cmd, cmd_matches, is_verbose, config.base_dir.as_deref()) {
                            if !is_quiet {
                                eprintln!("{}", style::error(&e.to_string()));
                            }
                            std::process::exit(1);
                        }
                    }
                }
            }
        }
    } else {
        // INSTALLER MODE
        let matches = cli::build_ring_cli().get_matches();

        if let Some(init_matches) = matches.subcommand_matches("init") {
            let config_paths = init_matches.get_many::<String>("config-path");
            let references = init_matches.get_one::<String>("references");
            let alias = init_matches.get_one::<String>("alias");
            let warn_only = init_matches.get_flag("warn-only-on-conflict");
            let check_for_updates = init_matches.get_flag("check-for-updates");
            return handle_init(config_paths, references, alias, warn_only, check_for_updates);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bash_alias_line() {
        let line = alias_line_bash_zsh("my-tool");
        assert_eq!(line, "alias my-tool='ring-cli --alias-mode my-tool' # ring-cli");
    }

    #[test]
    fn test_fish_alias_line() {
        let line = alias_line_fish("my-tool");
        assert_eq!(line, "alias my-tool 'ring-cli --alias-mode my-tool' # ring-cli");
    }

    #[test]
    fn test_powershell_alias_line() {
        let line = alias_line_powershell("my-tool");
        assert_eq!(
            line,
            "function my-tool { ring-cli --alias-mode my-tool @args } # ring-cli"
        );
    }

    #[test]
    fn test_alias_already_exists_bash() {
        let content = "# my stuff\nalias my-tool='ring-cli --alias-mode my-tool' # ring-cli\n";
        assert!(alias_exists(content, "my-tool", ShellKind::BashZsh));
        assert!(!alias_exists(content, "other-tool", ShellKind::BashZsh));
    }

    #[test]
    fn test_alias_already_exists_fish() {
        let content = "alias my-tool 'ring-cli --alias-mode my-tool' # ring-cli\n";
        assert!(alias_exists(content, "my-tool", ShellKind::Fish));
        assert!(!alias_exists(content, "other-tool", ShellKind::Fish));
    }

    #[test]
    fn test_alias_already_exists_powershell() {
        let content = "function my-tool { ring-cli --alias-mode my-tool @args } # ring-cli\n";
        assert!(alias_exists(content, "my-tool", ShellKind::PowerShell));
        assert!(!alias_exists(content, "other-tool", ShellKind::PowerShell));
    }

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
