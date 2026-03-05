mod cli;
mod errors;
mod models;
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

fn alias_line_bash_zsh(alias_name: &str, config_path: &str) -> String {
    format!("alias {alias_name}='ring-cli -c {config_path}' # ring-cli")
}

fn alias_line_fish(alias_name: &str, config_path: &str) -> String {
    format!("alias {alias_name} 'ring-cli -c {config_path}' # ring-cli")
}

fn alias_line_powershell(alias_name: &str, config_path: &str) -> String {
    format!("function {alias_name} {{ ring-cli -c {config_path} @args }} # ring-cli")
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

fn handle_init(path: Option<&String>) -> Result<(), anyhow::Error> {
    let target = if let Some(p) = path {
        PathBuf::from(p)
    } else {
        let dir = default_config_dir();
        fs::create_dir_all(&dir)?;
        dir.join("example.yml")
    };

    if target.exists() {
        anyhow::bail!("File already exists: {}", target.display());
    }

    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }

    let template = r#"# Ring-CLI Configuration
# See https://github.com/user/ring-cli for documentation

version: "1.0"
description: "My custom CLI"
slug: "mycli"
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

  # Example HTTP command:
  # api-status:
  #   description: "Check API status"
  #   flags: []
  #   cmd:
  #     http:
  #       method: "GET"
  #       url: "https://httpbin.org/get"

  # Example with environment variables:
  # deploy:
  #   description: "Deploy with auth"
  #   flags:
  #     - name: "target"
  #       short: "t"
  #       description: "Deploy target"
  #   cmd:
  #     run:
  #       - "curl -H 'Authorization: Bearer ${{env.API_TOKEN}}' https://${{target}}/deploy"

  # Example with subcommands:
  # db:
  #   description: "Database operations"
  #   flags: []
  #   subcommands:
  #     migrate:
  #       description: "Run migrations"
  #       flags: []
  #       cmd:
  #         run:
  #           - "echo Running migrations..."
  #     seed:
  #       description: "Seed database"
  #       flags: []
  #       cmd:
  #         run:
  #           - "echo Seeding database..."
"#;

    fs::write(&target, template)?;
    println!("Created configuration at: {}", target.display());
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() >= 2 && args[1] == "init" {
        let path = args.iter().position(|a| a == "--config-path").and_then(|i| args.get(i + 1));
        return handle_init(path);
    }

    let config_path = args.iter()
        .find(|arg| arg.starts_with("--config="))
        .and_then(|arg| arg.split('=').nth(1).map(String::from))
        .or_else(|| {
            args.iter()
                .position(|a| a == "-c" || a == "--config")
                .and_then(|i| args.get(i + 1).cloned())
        });

    let configurations = utils::load_configurations(config_path.as_deref())?;

    let matches = cli::build_cli_from_configs(&configurations).get_matches();

    let is_quiet = matches.get_flag("quiet");
    let is_verbose = matches.get_flag("verbose");
    let base_dir = matches.get_one::<String>("base_dir").map(|s| s.as_str());

    for config in &configurations {
        if let Some(submatches) = matches.subcommand_matches(&config.slug) {
            for (cmd_name, cmd) in &config.commands {
                if let Some(cmd_matches) = submatches.subcommand_matches(cmd_name) {
                    if let Err(e) = cli::execute_command(cmd, cmd_matches, is_verbose, base_dir) {
                        if !is_quiet {
                            eprintln!("Error: {}", e);
                        }
                        std::process::exit(1);
                    }
                }
            }
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
        assert_eq!(line, "alias my-tool='ring-cli -c /home/user/config.yml' # ring-cli");
    }

    #[test]
    fn test_fish_alias_line() {
        let line = alias_line_fish("my-tool", "/home/user/config.yml");
        assert_eq!(line, "alias my-tool 'ring-cli -c /home/user/config.yml' # ring-cli");
    }

    #[test]
    fn test_powershell_alias_line() {
        let line = alias_line_powershell("my-tool", "/home/user/config.yml");
        assert_eq!(line, "function my-tool { ring-cli -c /home/user/config.yml @args } # ring-cli");
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
}
