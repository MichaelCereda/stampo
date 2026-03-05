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
        let config: Configuration = serde_saphyr::from_str(&content).map_err(|e| {
            RingError::YamlParse {
                path: path_str,
                source: Box::new(e),
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
                serde_saphyr::from_str(&content).map_err(|e| RingError::YamlParse {
                    path: path_str,
                    source: Box::new(e),
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
            cmd.validate(cmd_name)?;
        }
    }

    Ok(configurations)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_replace_single_placeholder() {
        let mut flags = HashMap::new();
        flags.insert("name".to_string(), "Alice".to_string());
        let result = replace_placeholders("Hello, ${{name}}!", &flags, false);
        assert_eq!(result, "Hello, Alice!");
    }

    #[test]
    fn test_replace_multiple_placeholders() {
        let mut flags = HashMap::new();
        flags.insert("first".to_string(), "Hello".to_string());
        flags.insert("second".to_string(), "World".to_string());
        let result = replace_placeholders("${{first}}, ${{second}}!", &flags, false);
        assert_eq!(result, "Hello, World!");
    }

    #[test]
    fn test_replace_missing_placeholder_left_as_is() {
        let flags = HashMap::new();
        let result = replace_placeholders("Hello, ${{name}}!", &flags, false);
        assert_eq!(result, "Hello, ${{name}}!");
    }

    #[test]
    fn test_replace_env_var() {
        std::env::set_var("RING_TEST_VAR", "test_value");
        let result = replace_env_vars("Value: ${{env.RING_TEST_VAR}}", false)
            .expect("should succeed");
        assert_eq!(result, "Value: test_value");
        std::env::remove_var("RING_TEST_VAR");
    }

    #[test]
    fn test_replace_env_var_not_set() {
        std::env::remove_var("RING_TEST_MISSING_VAR");
        let err = replace_env_vars("Value: ${{env.RING_TEST_MISSING_VAR}}", false)
            .expect_err("should fail");
        assert!(err.to_string().contains("RING_TEST_MISSING_VAR"), "error was: {err}");
    }

    #[test]
    fn test_load_single_config_file() {
        let dir = tempfile::TempDir::new().unwrap();
        let file_path = dir.path().join("test.yml");
        let yaml = r#"
version: "2.0"
description: "Temp CLI"
commands:
  run:
    description: "Run something"
    flags: []
    cmd:
      run:
        - "echo hi"
"#;
        std::fs::write(&file_path, yaml).unwrap();
        let configs = load_configurations(Some(file_path.to_str().unwrap()))
            .expect("should load");
        assert_eq!(configs.len(), 1);
        assert_eq!(configs[0].description, "Temp CLI");
    }

    #[test]
    fn test_load_config_directory() {
        let dir = tempfile::TempDir::new().unwrap();
        let yaml = |name: &str| format!(r#"
version: "2.0"
description: "CLI {name}"
commands:
  run:
    description: "run"
    flags: []
    cmd:
      run:
        - "echo {name}"
"#);
        std::fs::write(dir.path().join("a.yml"), yaml("aname")).unwrap();
        std::fs::write(dir.path().join("b.yml"), yaml("bname")).unwrap();
        std::fs::write(dir.path().join("c.txt"), "not yaml").unwrap();

        let configs = load_configurations(Some(dir.path().to_str().unwrap()))
            .expect("should load");
        assert_eq!(configs.len(), 2);
    }

    #[test]
    fn test_load_nonexistent_path_errors() {
        let result = load_configurations(Some("/nonexistent/path/config.yml"));
        assert!(result.is_err());
    }

    #[test]
    fn test_load_invalid_yaml_shows_path() {
        let dir = tempfile::TempDir::new().unwrap();
        let file_path = dir.path().join("bad.yml");
        std::fs::write(&file_path, "not: valid: yaml: [unclosed").unwrap();
        let err = load_configurations(Some(file_path.to_str().unwrap()))
            .expect_err("should fail");
        assert!(err.to_string().contains("bad.yml"), "error was: {err}");
    }

    #[test]
    fn test_load_validates_configs() {
        let dir = tempfile::TempDir::new().unwrap();
        let file_path = dir.path().join("invalid.yml");
        let yaml = r#"
version: "2.0"
description: "Invalid"
commands:
  bad:
    description: "neither cmd nor subcommands"
    flags: []
"#;
        std::fs::write(&file_path, yaml).unwrap();
        let err = load_configurations(Some(file_path.to_str().unwrap()))
            .expect_err("should fail validation");
        assert!(err.to_string().contains("must be present"), "error was: {err}");
    }
}
