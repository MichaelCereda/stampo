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

fn alias_line_bash_zsh(alias_name: &str, config_path: &str) -> String {
    format!("alias {alias_name}='ring-cli -c \"{config_path}\"' # ring-cli")
}

fn alias_line_fish(alias_name: &str, config_path: &str) -> String {
    format!("alias {alias_name} 'ring-cli -c \"{config_path}\"' # ring-cli")
}

fn alias_line_powershell(alias_name: &str, config_path: &str) -> String {
    format!("function {alias_name} {{ ring-cli -c \"{config_path}\" @args }} # ring-cli")
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

fn install_alias(alias_name: &str, config_abs_path: &str) -> Result<(), anyhow::Error> {
    validate_alias_name(alias_name)?;
    let shells = detect_shell_configs();
    if shells.is_empty() {
        eprintln!("Warning: No shell config files found. Add the alias manually:");
        eprintln!("  Bash/Zsh: {}", alias_line_bash_zsh(alias_name, config_abs_path));
        eprintln!("  Fish:     {}", alias_line_fish(alias_name, config_abs_path));
        eprintln!("  PowerShell: {}", alias_line_powershell(alias_name, config_abs_path));
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
            ShellKind::BashZsh => alias_line_bash_zsh(alias_name, config_abs_path),
            ShellKind::Fish => alias_line_fish(alias_name, config_abs_path),
            ShellKind::PowerShell => alias_line_powershell(alias_name, config_abs_path),
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

fn handle_init(config_path: Option<&String>, alias: Option<&String>) -> Result<(), anyhow::Error> {
    let alias_name = alias.ok_or_else(|| anyhow::anyhow!("--alias is required for init"))?;
    validate_alias_name(alias_name)?;

    let target = if let Some(p) = config_path {
        let path = PathBuf::from(p);
        if !path.exists() {
            create_default_config(&path)?;
        }
        path
    } else {
        let dir = default_config_dir();
        fs::create_dir_all(&dir)?;
        let path = dir.join(format!("{alias_name}.yml"));
        if !path.exists() {
            create_default_config(&path)?;
        }
        path
    };

    let abs_path = fs::canonicalize(&target)?;
    let abs_path_str = abs_path.display().to_string();

    // Read and validate config
    let content = fs::read_to_string(&abs_path)?;
    let _config: models::Configuration = serde_saphyr::from_str(&content)
        .map_err(|e| anyhow::anyhow!("Invalid configuration: {e}"))?;

    // Save trusted config to cache
    cache::save_trusted_config(alias_name, &abs_path_str, &content)?;

    // Install shell alias
    install_alias(alias_name, &abs_path_str)?;

    // Install completion hooks
    install_completions(alias_name)?;

    println!("{}", style::success(&format!("Alias '{}' is ready!", alias_name)));

    Ok(())
}

fn find_alias_for_config(config_path: &str) -> Result<String, anyhow::Error> {
    let aliases_dir = cache::aliases_dir();
    if !aliases_dir.exists() {
        anyhow::bail!("No aliases configured");
    }
    let abs_config = fs::canonicalize(config_path)?;
    for entry in fs::read_dir(&aliases_dir)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            let meta_path = entry.path().join("metadata.json");
            if meta_path.exists() {
                let meta_str = fs::read_to_string(&meta_path)?;
                let meta: cache::AliasMetadata = serde_json::from_str(&meta_str)?;
                if std::path::Path::new(&meta.source_path) == abs_config {
                    return Ok(entry.file_name().to_string_lossy().to_string());
                }
            }
        }
    }
    anyhow::bail!("No alias found for config '{config_path}'")
}

fn handle_refresh_configuration(alias_name: &str) -> Result<(), anyhow::Error> {
    let (_, metadata) = cache::load_trusted_config(alias_name)
        .map_err(|_| anyhow::anyhow!("No cached configuration found for alias '{alias_name}'. Run 'ring-cli init' first."))?;

    let source_content = match fs::read_to_string(&metadata.source_path) {
        Ok(content) => content,
        Err(_) => {
            eprintln!("{}", style::error(&format!(
                "Source configuration not found at '{}'. The file may have been moved or deleted. The alias still works from the cached copy.",
                metadata.source_path
            )));
            return Ok(());
        }
    };

    let current_hash = cache::compute_hash(&source_content);
    if current_hash == metadata.hash {
        println!("{}", style::success("Configuration is up to date."));
        return Ok(());
    }

    // Validate new config before prompting
    let _config: models::Configuration = serde_saphyr::from_str(&source_content)
        .map_err(|e| anyhow::anyhow!("New configuration is invalid: {e}"))?;

    println!("{}", style::warn("Configuration has changed."));
    println!("Source: {}", metadata.source_path);

    eprint!("Trust this configuration? [y/N] ");
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    if input.trim().to_lowercase() != "y" {
        println!("Keeping previous trusted configuration.");
        return Ok(());
    }

    cache::save_trusted_config(alias_name, &metadata.source_path, &source_content)?;
    println!("{}", style::success("Configuration updated and trusted."));

    Ok(())
}

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();

    // Detect config path (alias bakes in -c /path or --config=path).
    // We find the config argument first, then strip it from the arg list
    // before handing the rest to clap so that clap never sees --config.
    let config_path = args.iter()
        .find(|arg| arg.starts_with("--config="))
        .and_then(|arg| arg.split('=').nth(1).map(String::from))
        .or_else(|| {
            args.iter()
                .position(|a| a == "-c" || a == "--config")
                .and_then(|i| args.get(i + 1).cloned())
        });

    if let Some(ref path) = config_path {
        // ALIAS MODE: load config, build CLI, dispatch.
        // Strip the config flag (and its value) from args before clap parses them.
        let clap_args: Vec<String> = {
            let mut out = Vec::with_capacity(args.len());
            let mut skip_next = false;
            for arg in &args {
                if skip_next {
                    skip_next = false;
                    continue;
                }
                if arg.starts_with("--config=") {
                    // --config=VALUE form — skip the single token
                    continue;
                }
                if arg == "-c" || arg == "--config" {
                    // separate-value form — skip this token and the next
                    skip_next = true;
                    continue;
                }
                out.push(arg.clone());
            }
            out
        };

        let config = utils::load_configuration(path)?;
        let matches = cli::build_cli(&config).get_matches_from(clap_args);

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
            let alias_name = find_alias_for_config(path)?;
            return handle_refresh_configuration(&alias_name);
        }

        // Dispatch user commands
        for (cmd_name, cmd) in &config.commands {
            if let Some(cmd_matches) = matches.subcommand_matches(cmd_name) {
                if let Err(e) = cli::execute_command(cmd, cmd_matches, is_verbose, None) {
                    if !is_quiet {
                        eprintln!("{}", style::error(&e.to_string()));
                    }
                    std::process::exit(1);
                }
            }
        }
    } else {
        // INSTALLER MODE: ring-cli init
        let matches = cli::build_ring_cli().get_matches();

        if let Some(init_matches) = matches.subcommand_matches("init") {
            let config_path = init_matches.get_one::<String>("config-path");
            let alias = init_matches.get_one::<String>("alias");
            return handle_init(config_path, alias);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bash_alias_line() {
        let line = alias_line_bash_zsh("my-tool", "/home/user/config.yml");
        assert_eq!(line, "alias my-tool='ring-cli -c \"/home/user/config.yml\"' # ring-cli");
    }

    #[test]
    fn test_fish_alias_line() {
        let line = alias_line_fish("my-tool", "/home/user/config.yml");
        assert_eq!(line, "alias my-tool 'ring-cli -c \"/home/user/config.yml\"' # ring-cli");
    }

    #[test]
    fn test_powershell_alias_line() {
        let line = alias_line_powershell("my-tool", "/home/user/config.yml");
        assert_eq!(line, "function my-tool { ring-cli -c \"/home/user/config.yml\" @args } # ring-cli");
    }

    #[test]
    fn test_alias_already_exists_bash() {
        let content = "# my stuff\nalias my-tool='ring-cli -c /old/path' # ring-cli\n";
        assert!(alias_exists(content, "my-tool", ShellKind::BashZsh));
        assert!(!alias_exists(content, "other-tool", ShellKind::BashZsh));
    }

    #[test]
    fn test_alias_already_exists_fish() {
        let content = "alias my-tool 'ring-cli -c /old/path' # ring-cli\n";
        assert!(alias_exists(content, "my-tool", ShellKind::Fish));
        assert!(!alias_exists(content, "other-tool", ShellKind::Fish));
    }

    #[test]
    fn test_alias_already_exists_powershell() {
        let content = "function my-tool { ring-cli -c /old/path @args } # ring-cli\n";
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

    #[test]
    fn test_find_alias_for_config_no_aliases_dir() {
        // If aliases dir doesn't exist, should error
        // This is hard to test without mocking, so just verify the function compiles
        // and the error message is reasonable
        let result = find_alias_for_config("/nonexistent/config.yml");
        assert!(result.is_err());
    }
}
