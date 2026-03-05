use std::collections::HashMap;
use std::process::Command as ShellCommand;

use crate::errors::RingError;
use crate::models::{CmdType, Command as RingCommand, Configuration, Http};
use crate::utils::{replace_env_vars, replace_placeholders};

fn extract_flag_values(
    flags: &[crate::models::Flag],
    matches: &clap::ArgMatches,
) -> HashMap<String, String> {
    let mut values = HashMap::new();
    for flag in flags {
        if let Some(val) = matches.get_one::<String>(&flag.name) {
            values.insert(flag.name.clone(), val.clone());
        }
    }
    values
}

fn build_arg(flag: &crate::models::Flag) -> clap::Arg {
    let mut arg = clap::Arg::new(flag.name.clone())
        .long(flag.name.clone())
        .help(flag.description.clone());
    if let Some(short_form) = &flag.short {
        if let Some(c) = short_form.chars().next() {
            arg = arg.short(c);
        }
    }
    arg
}

pub fn add_subcommands_to_cli(
    command: &RingCommand,
    cmd_subcommand: clap::Command,
) -> clap::Command {
    let mut updated_subcommand = cmd_subcommand;
    if let Some(subcommands) = &command.subcommands {
        for (sub_name, sub_cmd) in subcommands {
            let mut sub_cli = clap::Command::new(sub_name.to_owned())
                .about(sub_cmd.description.to_owned());
            for flag in &sub_cmd.flags {
                sub_cli = sub_cli.arg(build_arg(flag));
            }
            sub_cli = add_subcommands_to_cli(sub_cmd, sub_cli);
            updated_subcommand = updated_subcommand.subcommand(sub_cli);
        }
    }
    updated_subcommand
}

/// Build the CLI for alias mode. Takes a slice of `Configuration` values and
/// registers each as a named subcommand. Commands within each config are
/// nested under the config's `name` subcommand. Adds `--quiet`, `--verbose`,
/// and `--color` flags. Includes the `refresh-configuration` built-in
/// subcommand.
///
/// The `--config` / `--base-dir` flags are intentionally absent here: in alias
/// mode the alias name is passed via `--alias-mode <name>` and parsed before
/// clap sees the argument list.
pub fn build_cli(configs: &[Configuration]) -> clap::Command {
    let mut app = clap::Command::new("ring-cli")
        .version(env!("CARGO_PKG_VERSION"))
        .arg(
            clap::Arg::new("quiet")
                .short('q')
                .long("quiet")
                .help("Suppress error messages")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            clap::Arg::new("verbose")
                .short('v')
                .long("verbose")
                .help("Print verbose output")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            clap::Arg::new("color")
                .long("color")
                .value_name("WHEN")
                .help("Color output")
                .value_parser(["auto", "always", "never"])
                .default_value("auto"),
        );

    for config in configs {
        let mut config_cmd = clap::Command::new(config.name.to_owned())
            .about(config.description.to_owned());

        for (cmd_name, cmd) in &config.commands {
            let mut cmd_subcommand =
                clap::Command::new(cmd_name.to_owned()).about(cmd.description.to_owned());
            for flag in &cmd.flags {
                cmd_subcommand = cmd_subcommand.arg(build_arg(flag));
            }
            cmd_subcommand = add_subcommands_to_cli(cmd, cmd_subcommand);
            config_cmd = config_cmd.subcommand(cmd_subcommand);
        }

        app = app.subcommand(config_cmd);
    }

    app = app.subcommand(
        clap::Command::new("refresh-configuration")
            .about("Re-read and trust updated configuration"),
    );

    app
}

/// Build the CLI for installer mode (`ring-cli` binary direct invocation).
/// Exposes only the `init` subcommand with `--alias`, `--config-path` (repeatable),
/// and `--warn-only-on-conflict` flags.
pub fn build_ring_cli() -> clap::Command {
    clap::Command::new("ring-cli")
        .version(env!("CARGO_PKG_VERSION"))
        .about("CLI generator from YAML configurations")
        .subcommand(
            clap::Command::new("init")
                .about("Create a new configuration and install as a shell alias")
                .arg(
                    clap::Arg::new("config-path")
                        .long("config-path")
                        .value_name("PATH")
                        .help("Path to a configuration file (can be specified multiple times)")
                        .action(clap::ArgAction::Append),
                )
                .arg(
                    clap::Arg::new("alias")
                        .long("alias")
                        .value_name("NAME")
                        .help("Shell alias name to install")
                        .required(true),
                )
                .arg(
                    clap::Arg::new("warn-only-on-conflict")
                        .long("warn-only-on-conflict")
                        .help("Warn instead of error on command name conflicts")
                        .action(clap::ArgAction::SetTrue),
                ),
        )
}

fn run_shell_commands(
    commands: &[String],
    flag_values: &HashMap<String, String>,
    verbose: bool,
    base_dir: Option<&str>,
) -> Result<String, RingError> {
    let mut output_text = String::new();
    for cmd in commands {
        let replaced_cmd = replace_placeholders(cmd, flag_values, verbose);
        let replaced_cmd = replace_env_vars(&replaced_cmd, verbose)?;

        let mut command = ShellCommand::new("sh");
        command.arg("-c").arg(&replaced_cmd);
        if let Some(dir) = base_dir {
            command.current_dir(dir);
        }

        let output = command.output().map_err(|e| RingError::ShellCommand {
            command: replaced_cmd.clone(),
            code: -1,
            stderr: e.to_string(),
        })?;

        if output.status.success() {
            output_text.push_str(&String::from_utf8_lossy(&output.stdout));
        } else {
            return Err(RingError::ShellCommand {
                command: replaced_cmd,
                code: output.status.code().unwrap_or(-1),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            });
        }
    }
    Ok(output_text)
}

pub async fn execute_http_request(
    http: &Http,
    flag_values: &HashMap<String, String>,
    verbose: bool,
) -> Result<String, RingError> {
    let client = reqwest::Client::new();

    let replace = |template: &str| -> Result<String, RingError> {
        let result = replace_placeholders(template, flag_values, verbose);
        replace_env_vars(&result, verbose)
    };

    let url = replace(&http.url)?;
    let body = if let Some(ref body_content) = http.body {
        Some(replace(body_content)?)
    } else {
        None
    };

    let request_builder = match http.method.as_str() {
        "GET" => client.get(&url),
        "POST" => client.post(&url).body(body.unwrap_or_default()),
        "PUT" => client.put(&url).body(body.unwrap_or_default()),
        "DELETE" => client.delete(&url),
        "PATCH" => client.patch(&url).body(body.unwrap_or_default()),
        "HEAD" => client.head(&url),
        _ => return Err(RingError::UnsupportedMethod(http.method.clone())),
    };

    let mut request_with_headers = request_builder;
    if let Some(header_map) = &http.headers {
        for (header_name, header_value) in header_map.iter() {
            let replaced_value = replace(header_value)?;
            request_with_headers = request_with_headers.header(header_name, replaced_value);
        }
    }

    let response = request_with_headers.send().await.map_err(|e| RingError::Http {
        method: http.method.clone(),
        url: url.clone(),
        message: e.to_string(),
    })?;

    let text = response.text().await.map_err(|e| RingError::Http {
        method: http.method.clone(),
        url: url.clone(),
        message: e.to_string(),
    })?;

    Ok(text)
}

pub fn execute_command(
    command: &RingCommand,
    cmd_matches: &clap::ArgMatches,
    verbose: bool,
    base_dir: Option<&str>,
) -> Result<(), RingError> {
    let flag_values = extract_flag_values(&command.flags, cmd_matches);

    if verbose {
        println!("Executing command with flags: {:?}", flag_values);
    }

    if let Some(actual_cmd) = &command.cmd {
        match actual_cmd {
            CmdType::Http { http } => {
                let rt = tokio::runtime::Runtime::new()
                    .map_err(|e| RingError::Config(format!("Failed to create async runtime: {}", e)))?;
                let output = rt.block_on(execute_http_request(http, &flag_values, verbose))?;
                println!("{}", output);
            }
            CmdType::Run { run } => {
                match run_shell_commands(run, &flag_values, verbose, base_dir) {
                    Ok(output) => {
                        if !output.trim().is_empty() {
                            println!("{}", output);
                        }
                    }
                    Err(e) => return Err(e),
                }
            }
        }
    }

    if let Some(subcommands) = &command.subcommands {
        for (sub_name, sub_cmd) in subcommands {
            if let Some(sub_cmd_matches) = cmd_matches.subcommand_matches(sub_name) {
                execute_command(sub_cmd, sub_cmd_matches, verbose, base_dir)?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{CmdType, Command as RingCommand, Configuration, Flag};

    fn make_test_config() -> Configuration {
        let mut commands = HashMap::new();
        commands.insert(
            "greet".to_string(),
            RingCommand {
                description: "Greet a user".to_string(),
                flags: vec![Flag {
                    name: "name".to_string(),
                    short: Some("n".to_string()),
                    description: "Name of the user".to_string(),
                }],
                cmd: Some(CmdType::Run { run: vec!["echo Hello, ${{name}}!".to_string()] }),
                subcommands: None,
            },
        );
        Configuration {
            version: "1.0".to_string(),
            name: "test".to_string(),
            description: "Test CLI".to_string(),
            base_dir: None,
            commands,
        }
    }

    #[test]
    fn test_build_cli_has_config_subcommand() {
        let config = make_test_config();
        let app = build_cli(&[config]);
        let matches = app
            .try_get_matches_from(["ring-cli", "test", "greet", "--name", "Alice"])
            .expect("should parse");
        let test_matches = matches.subcommand_matches("test").expect("test subcommand");
        let greet_matches = test_matches.subcommand_matches("greet").expect("greet subcommand");
        let name = greet_matches.get_one::<String>("name").expect("name flag");
        assert_eq!(name, "Alice");
    }

    #[test]
    fn test_build_cli_quiet_and_verbose_flags() {
        let empty_config = Configuration {
            version: "2.0".to_string(),
            name: "empty".to_string(),
            description: "Empty".to_string(),
            base_dir: None,
            commands: HashMap::new(),
        };
        let app = build_cli(&[empty_config]);
        let matches = app
            .try_get_matches_from(["ring-cli", "-q", "-v"])
            .expect("should parse");
        assert!(matches.get_flag("quiet"));
        assert!(matches.get_flag("verbose"));
    }

    #[test]
    fn test_build_cli_nested_subcommands() {
        let mut migrate_subs = HashMap::new();
        migrate_subs.insert(
            "migrate".to_string(),
            RingCommand {
                description: "Run migrations".to_string(),
                flags: vec![],
                cmd: Some(CmdType::Run { run: vec!["echo migrating".to_string()] }),
                subcommands: None,
            },
        );
        let mut commands = HashMap::new();
        commands.insert(
            "db".to_string(),
            RingCommand {
                description: "Database operations".to_string(),
                flags: vec![],
                cmd: None,
                subcommands: Some(migrate_subs),
            },
        );
        let config = Configuration {
            version: "1.0".to_string(),
            name: "nested".to_string(),
            description: "Nested CLI".to_string(),
            base_dir: None,
            commands,
        };
        let app = build_cli(&[config]);
        let matches = app
            .try_get_matches_from(["ring-cli", "nested", "db", "migrate"])
            .expect("should parse nested subcommands");
        let nested_matches = matches.subcommand_matches("nested").expect("nested subcommand");
        let db_matches = nested_matches.subcommand_matches("db").expect("db subcommand");
        assert!(db_matches.subcommand_matches("migrate").is_some());
    }

    #[test]
    fn test_extract_flag_values() {
        let flags = vec![Flag {
            name: "name".to_string(),
            short: Some("n".to_string()),
            description: "Name".to_string(),
        }];
        // Build a minimal CLI and parse to get ArgMatches
        let app = clap::Command::new("test-app").arg(
            clap::Arg::new("name")
                .long("name")
                .short('n'),
        );
        let matches = app
            .try_get_matches_from(["test-app", "--name", "Bob"])
            .expect("should parse");
        let flag_values = extract_flag_values(&flags, &matches);
        assert_eq!(flag_values.get("name").map(String::as_str), Some("Bob"));
    }

    #[test]
    fn test_build_ring_cli_has_init() {
        let app = build_ring_cli();
        let matches = app
            .try_get_matches_from(["ring-cli", "init", "--alias", "my-tool"])
            .expect("should parse");
        let init_matches = matches.subcommand_matches("init").expect("init subcommand");
        let alias = init_matches.get_one::<String>("alias").expect("alias flag");
        assert_eq!(alias, "my-tool");
    }

    #[test]
    fn test_build_ring_cli_init_accepts_multiple_config_paths() {
        let app = build_ring_cli();
        let matches = app
            .try_get_matches_from([
                "ring-cli",
                "init",
                "--alias",
                "my-tool",
                "--config-path",
                "/a.yml",
                "--config-path",
                "/b.yml",
            ])
            .expect("should parse multiple config paths");
        let init_matches = matches.subcommand_matches("init").expect("init subcommand");
        let paths: Vec<&String> = init_matches
            .get_many::<String>("config-path")
            .expect("config paths")
            .collect();
        assert_eq!(paths.len(), 2);
    }

    #[test]
    fn test_build_cli_has_refresh_configuration() {
        let config = make_test_config();
        let app = build_cli(&[config]);
        let matches = app
            .try_get_matches_from(["ring-cli", "refresh-configuration"])
            .expect("should parse");
        assert!(matches.subcommand_matches("refresh-configuration").is_some());
    }

    #[test]
    fn test_build_cli_color_flag() {
        let config = make_test_config();
        let app = build_cli(&[config]);
        let matches = app
            .try_get_matches_from(["ring-cli", "--color=never"])
            .expect("should parse");
        let color = matches.get_one::<String>("color").expect("color flag");
        assert_eq!(color, "never");
    }
}
