use super::models::{Configuration, Http};
use clap::ArgMatches;
use dirs;
use std::{collections::HashMap, fs, path::PathBuf};

pub fn replace_placeholders<'a>(
    template: &str,
    flags: &'a clap::ArgMatches<'a>,
    verbose: bool,
) -> String {
    let mut result = template.to_string();
    for (flag_name, values) in flags.args.iter() {
        let flag_value = values.vals[0].to_str().unwrap_or_default();
        if verbose {
            println!("Replacing placeholder for {}: {}", flag_name, flag_value);
        }
        result = result.replace(&format!("${{{{{}}}}}", flag_name), flag_value);
    }
    result
}

pub fn load_configurations(
    config_path: Option<&str>,
) -> Result<Vec<Configuration>, Box<dyn std::error::Error>> {
    let mut configurations = Vec::new();

    // Set the default config directory to ~/.ring-cli/configurations
    let default_config_dir = dirs::home_dir()
        .ok_or("Unable to determine home directory")?
        .join(".ring-cli/configurations");

    // If a custom config path is provided, use it. Otherwise, use the default directory.
    let config_dir = if let Some(path) = config_path {
        std::path::PathBuf::from(path)
    } else {
        default_config_dir
    };

    if config_dir.is_file() {
        let content = fs::read_to_string(&config_dir)?;
        let config: Configuration = serde_yaml::from_str(&content)?;
        configurations.push(config);
    } else if config_dir.is_dir() {
        let paths = fs::read_dir(config_dir)?;
        for path in paths {
            let content = fs::read_to_string(path?.path())?;
            let config: Configuration = serde_yaml::from_str(&content)?;
            configurations.push(config);
        }
    } else {
        return Err(Box::from(
            "Provided config path is neither a file nor a directory",
        ));
    }

    for config in &configurations {
        for (_, cmd) in &config.commands {
            cmd.validate()?; // Validate each command after loading
        }
    }

    Ok(configurations)
}
