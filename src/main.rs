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
