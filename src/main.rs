use serde_derive::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
struct Configuration {
    version: String,
    description: String,
    slug: String,
    commands: std::collections::HashMap<String, Command>,
}

#[derive(Debug, Deserialize, Serialize)]
struct Command {
    description: String,
    flags: Vec<Flag>,
    cmd: CmdType,
}

#[derive(Debug, Deserialize, Serialize)]
struct Flag {
    name: String,
    #[serde(default)]
    short: Option<String>,
    description: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
// #[serde(tag = "type")]
enum CmdType {
    Http { http: Http },
    Run { run: Vec<String> },
}

#[derive(Debug, Deserialize, Serialize)]
struct Http {
    method: String,
    url: String,
    headers: Option<HashMap<String, String>>,
    #[serde(default)]
    body: Option<String>,
}

use std::{fs, collections::HashMap};
use dirs;

fn replace_placeholders<'a>(template: &str, flags: &'a clap::ArgMatches<'a>) -> String {
    let mut result = template.to_string();
    for (flag_name, values) in flags.args.iter() {
        let flag_value = values.vals[0].to_str().unwrap_or_default();
        result = result.replace(&format!("${{{{{}}}}}", flag_name), flag_value);
    }
    result
}

fn load_configurations() -> Result<Vec<Configuration>, Box<dyn std::error::Error>> {
    let mut configurations = Vec::new();
    
    let config_dir = dirs::home_dir()
        .ok_or("Unable to determine home directory")?
        .join(".ring-cli/configurations");
    
    let paths = fs::read_dir(config_dir)?;

    for path in paths {
        let content = fs::read_to_string(path?.path())?;
        
        let config: Configuration = serde_yaml::from_str(&content)?;
        configurations.push(config);
    }
    Ok(configurations)
}

use clap::{App, Arg, SubCommand};

fn build_cli_from_configs(configs: &Vec<Configuration>) -> App {
    let mut app = App::new("ring-cli")
        .version("1.0")
        .about("Ring CLI Tool powered by YAML configurations");
    for config in configs {
        let mut subcommand = SubCommand::with_name(&config.slug)
            .about(config.description.as_str())
            .version(config.version.as_str());
        for (cmd_name, cmd) in &config.commands {
            let mut cmd_subcommand = SubCommand::with_name(cmd_name)
                .about(cmd.description.as_str());
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
            subcommand = subcommand.subcommand(cmd_subcommand);
        }
        app = app.subcommand(subcommand);
    }
    app
}
use std::process::Command as ShellCommand;

fn run_shell_commands(commands: &Vec<String>, flags: &clap::ArgMatches) -> Result<String, String> {
    let mut output_text = String::new();
    for cmd in commands {
        let replaced_cmd = replace_placeholders(cmd, flags);
        
        // Running the command using a shell
        let output = ShellCommand::new("sh")
            .arg("-c")
            .arg(&replaced_cmd)
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

use reqwest;

async fn execute_http_request<'a>(http: &Http, flags: &'a clap::ArgMatches<'a>) -> Result<String, String> {
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

    let response = request_with_headers.send().await
        .map_err(|e| format!("Failed to send request: {}", e))?;
    
    let text = response.text().await
        .map_err(|e| format!("Failed to read response: {}", e))?;
    
    Ok(text)
}


fn main() {
    let configurations = load_configurations().unwrap_or_else(|e| {
        eprintln!("Error loading configurations: {}", e);
        std::process::exit(1);
    });
    let matches = build_cli_from_configs(&configurations).get_matches();

    for config in &configurations {
        if let Some(submatches) = matches.subcommand_matches(&config.slug) {
            for (cmd_name, cmd) in &config.commands {
                if let Some(cmd_matches) = submatches.subcommand_matches(cmd_name) {
                    match &cmd.cmd {
                        CmdType::Http { http } => {
                            match tokio::runtime::Runtime::new()
                                .unwrap()
                                .block_on(execute_http_request(http, &cmd_matches)) {
                                    Ok(output) => println!("{}", output),
                                    Err(e) => eprintln!("Error executing HTTP request: {}", e),
                                }
                        },
                        CmdType::Run { run } => {
                            match run_shell_commands(run, &cmd_matches) {
                                Ok(output) => println!("{}", output),
                                Err(e) => eprintln!("Error executing shell command: {}", e),
                            }
                        },
                    }
                }
            }
        }
    }
}
