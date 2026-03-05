use std::collections::HashMap;
use std::fs;

use crate::errors::RingError;
use crate::models::Configuration;

pub fn replace_placeholders(
    template: &str,
    flag_values: &HashMap<String, String>,
    verbose: bool,
) -> String {
    let mut result = template.to_string();
    for (flag_name, flag_value) in flag_values {
        if verbose {
            println!("Replacing placeholder for {}: {}", flag_name, flag_value);
        }
        result = result.replace(&format!("${{{{{}}}}}", flag_name), flag_value);
    }
    result
}

pub fn replace_env_vars(template: &str, verbose: bool) -> Result<String, RingError> {
    let mut result = template.to_string();
    let mut pos = 0;
    loop {
        let search = &result[pos..];
        let Some(offset) = search.find("${{env.") else { break };
        let start = pos + offset;
        let rest = &result[start + 7..];
        let end = rest.find("}}").ok_or_else(|| RingError::Config(
            format!("Unclosed placeholder starting at position {}", start),
        ))?;
        let var_name = rest[..end].to_string();
        let var_value = std::env::var(&var_name).map_err(|_| RingError::EnvVar {
            name: var_name.clone(),
        })?;
        if verbose {
            println!("Replacing env var {}: ***", var_name);
        }
        let placeholder = format!("${{{{env.{}}}}}", var_name);
        result = format!("{}{}{}", &result[..start], var_value, &result[start + placeholder.len()..]);
        pos = start + var_value.len();
    }
    Ok(result)
}

pub fn load_configurations(
    config_path: Option<&str>,
) -> Result<Vec<Configuration>, RingError> {
    let mut configurations = Vec::new();

    let default_config_dir = dirs::home_dir()
        .ok_or_else(|| RingError::Config("Unable to determine home directory".to_string()))?
        .join(".ring-cli/configurations");

    let using_default = config_path.is_none();
    let config_dir = if let Some(path) = config_path {
        std::path::PathBuf::from(path)
    } else {
        default_config_dir
    };

    // When the default directory simply doesn't exist yet, return empty configs
    // rather than an error so that `--help` and `init` still work.
    if using_default && !config_dir.exists() {
        return Ok(configurations);
    }

    if config_dir.is_file() {
        let path_str = config_dir.display().to_string();
        let content = fs::read_to_string(&config_dir).map_err(|e| RingError::Io {
            path: path_str.clone(),
            source: e,
        })?;
        let config: Configuration = serde_yml::from_str(&content).map_err(|e| {
            RingError::YamlParse {
                path: path_str,
                source: e,
            }
        })?;
        configurations.push(config);
    } else if config_dir.is_dir() {
        let paths = fs::read_dir(&config_dir).map_err(|e| RingError::Io {
            path: config_dir.display().to_string(),
            source: e,
        })?;
        for entry in paths {
            let entry = entry.map_err(|e| RingError::Io {
                path: config_dir.display().to_string(),
                source: e,
            })?;
            let path = entry.path();
            match path.extension().and_then(|e| e.to_str()) {
                Some("yml") | Some("yaml") => {}
                _ => continue,
            }
            let path_str = path.display().to_string();
            let content = fs::read_to_string(&path).map_err(|e| RingError::Io {
                path: path_str.clone(),
                source: e,
            })?;
            let config: Configuration =
                serde_yml::from_str(&content).map_err(|e| RingError::YamlParse {
                    path: path_str,
                    source: e,
                })?;
            configurations.push(config);
        }
    } else {
        return Err(RingError::Config(format!(
            "Config path '{}' is neither a file nor a directory",
            config_dir.display()
        )));
    }

    for config in &configurations {
        for (cmd_name, cmd) in &config.commands {
            cmd.validate(&format!("{} > {}", config.slug, cmd_name))?;
        }
    }

    Ok(configurations)
}
