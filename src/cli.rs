use crate::{
    models::{CmdType, Http},
    utils::replace_placeholders,
};

use super::models::{Command, Configuration};
use clap::{App, Arg, SubCommand};
use std::process::Command as ShellCommand;

pub fn add_subcommands_to_cli<'a>(
    command: &'a Command,
    cmd_subcommand: clap::App<'a, 'a>,
) -> clap::App<'a, 'a> {
    let mut updated_subcommand = cmd_subcommand;
    if let Some(subcommands) = &command.subcommands {
        for (sub_name, sub_cmd) in subcommands {
            let mut sub_cli = SubCommand::with_name(sub_name).about(sub_cmd.description.as_str());
            for flag in &sub_cmd.flags {
                let mut arg = Arg::with_name(&flag.name)
                    .long(&flag.name)
                    .help(&flag.description)
                    .takes_value(true);
                if let Some(short_form) = &flag.short {
                    arg = arg.short(short_form);
                }
                sub_cli = sub_cli.arg(arg);
            }
            sub_cli = add_subcommands_to_cli(&sub_cmd, sub_cli);
            updated_subcommand = updated_subcommand.subcommand(sub_cli);
        }
    }
    updated_subcommand
}

pub fn build_cli_from_configs(configs: &Vec<Configuration>) -> App {
    let mut app = App::new("ring-cli")
        .version("1.0")
        .about("Ring CLI Tool powered by YAML configurations")
        .arg(
            Arg::with_name("quiet")
                .short("q")
                .long("quiet")
                .help("Suppress error messages"),
        )
        .arg(
            Arg::with_name("verbose")
                .short("v")
                .long("verbose")
                .help("Print verbose output"),
        )
        .arg(
            Arg::with_name("config")
                .short("c")
                .long("config")
                .value_name("PATH")
                .help("Path to a custom configuration file or directory")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("base_dir")
                .short("b")
                .long("base-dir")
                .value_name("PATH")
                .help("Base directory for relative paths")
                .takes_value(true),
        );
    for config in configs {
        let mut subcommand = SubCommand::with_name(&config.slug)
            .about(config.description.as_str())
            .version(config.version.as_str());
        for (cmd_name, cmd) in &config.commands {
            let mut cmd_subcommand =
                SubCommand::with_name(cmd_name).about(cmd.description.as_str());
            for flag in &cmd.flags {
                let mut arg = Arg::with_name(&flag.name)
                    .long(&flag.name)
                    .help(&flag.description)
                    .takes_value(true);
                if let Some(short_form) = &flag.short {
                    arg = arg.short(short_form);
                }
                cmd_subcommand = cmd_subcommand.arg(arg);
            }
            cmd_subcommand = add_subcommands_to_cli(cmd, cmd_subcommand);
            subcommand = subcommand.subcommand(cmd_subcommand);
        }
        app = app.subcommand(subcommand);
    }
    app
}

fn run_shell_commands(
    commands: &Vec<String>,
    flags: &clap::ArgMatches,
    verbose: bool,
    base_dir: Option<&str>,
) -> Result<String, String> {
    let mut output_text = String::new();
    for cmd in commands {
        let replaced_cmd = replace_placeholders(cmd, flags, verbose);

        // Running the command using a shell
        let mut command = ShellCommand::new("sh");
        command.arg("-c").arg(&replaced_cmd);
        // If a base directory is provided, run the command from that directory
        if let Some(dir) = base_dir {
            command.current_dir(dir);
        }

        let output = command
            .output()
            .map_err(|e| format!("Failed to run command '{}': {}", cmd, e))?;

        if output.status.success() {
            output_text.push_str(&String::from_utf8_lossy(&output.stdout));
        } else {
            return Err(String::from_utf8_lossy(&output.stderr).to_string());
        }
    }
    Ok(output_text)
}

pub async fn execute_http_request<'a>(
    http: &Http,
    flags: &'a clap::ArgMatches<'a>,
) -> Result<String, String> {
    let client = reqwest::Client::new();

    let replace_with_flag_values = |template: &str| -> String {
        let mut result = template.to_string();
        for (flag_name, values) in flags.args.iter() {
            let flag_value = values.vals[0].to_str().unwrap_or_default();
            result = result.replace(&format!("${{{{{}}}}}", flag_name), flag_value);
        }
        result
    };

    let url = replace_with_flag_values(&http.url);
    let body = if let Some(ref body_content) = &http.body {
        Some(replace_with_flag_values(body_content))
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
        _ => return Err(format!("Unsupported HTTP method '{}'", http.method)),
    };

    // Adding headers if they exist
    let mut request_with_headers = request_builder;
    if let Some(header_map) = &http.headers {
        for (header_name, header_value) in header_map.iter() {
            request_with_headers = request_with_headers.header(header_name, header_value);
        }
    }

    let response = request_with_headers
        .send()
        .await
        .map_err(|e| format!("Failed to send request: {}", e))?;

    let text = response
        .text()
        .await
        .map_err(|e| format!("Failed to read response: {}", e))?;

    Ok(text)
}

pub fn execute_command(
    command: &Command,
    cmd_matches: &clap::ArgMatches,
    verbose: bool,
    base_dir: Option<&str>,
) -> Result<(), String> {
    if verbose {
        println!("Executing command with flags: {:?}", cmd_matches.args);
    }
    if let Some(actual_cmd) = &command.cmd {
        match actual_cmd {
            CmdType::Http { http } => {
                match tokio::runtime::Runtime::new()
                    .unwrap()
                    .block_on(execute_http_request(http, &cmd_matches))
                {
                    Ok(output) => println!("{}", output),
                    Err(e) => eprintln!("Error executing HTTP request: {}", e),
                }
            }
            CmdType::Run { run } => match run_shell_commands(run, cmd_matches, verbose, base_dir) {
                Ok(output) => {
                    if !output.trim().is_empty() {
                        println!("{}", output);
                    }
                }
                Err(e) => return Err(e),
            },
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
