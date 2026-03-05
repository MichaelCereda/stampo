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

/// Load a single `Configuration` from a file path.
///
/// Reads, parses and validates the YAML at `config_path`, returning the
/// resulting `Configuration` or a `RingError` on any failure.
pub fn load_configuration(config_path: &str) -> Result<Configuration, RingError> {
    let path = std::path::Path::new(config_path);
    let path_str = path.display().to_string();
    let content = fs::read_to_string(path).map_err(|e| RingError::Io {
        path: path_str.clone(),
        source: e,
    })?;
    let config: Configuration = serde_saphyr::from_str(&content).map_err(|e| RingError::YamlParse {
        path: path_str,
        source: Box::new(e),
    })?;
    for (cmd_name, cmd) in &config.commands {
        cmd.validate(cmd_name)?;
    }
    Ok(config)
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
        // SAFETY: test runs in isolation; no other thread reads this env var.
        unsafe { std::env::set_var("RING_TEST_VAR", "test_value") };
        let result = replace_env_vars("Value: ${{env.RING_TEST_VAR}}", false)
            .expect("should succeed");
        assert_eq!(result, "Value: test_value");
        unsafe { std::env::remove_var("RING_TEST_VAR") };
    }

    #[test]
    fn test_replace_env_var_not_set() {
        // SAFETY: test runs in isolation; no other thread reads this env var.
        unsafe { std::env::remove_var("RING_TEST_MISSING_VAR") };
        let err = replace_env_vars("Value: ${{env.RING_TEST_MISSING_VAR}}", false)
            .expect_err("should fail");
        assert!(err.to_string().contains("RING_TEST_MISSING_VAR"), "error was: {err}");
    }

    #[test]
    fn test_load_configuration_single_file() {
        let dir = tempfile::TempDir::new().unwrap();
        let file_path = dir.path().join("test.yml");
        let yaml = r#"
version: "2.0"
name: "test"
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
        let config = load_configuration(file_path.to_str().unwrap())
            .expect("should load");
        assert_eq!(config.description, "Temp CLI");
    }

    #[test]
    fn test_load_configuration_nonexistent_errors() {
        let result = load_configuration("/nonexistent/path/config.yml");
        assert!(result.is_err());
    }

    #[test]
    fn test_load_configuration_invalid_yaml_shows_path() {
        let dir = tempfile::TempDir::new().unwrap();
        let file_path = dir.path().join("bad.yml");
        std::fs::write(&file_path, "not: valid: yaml: [unclosed").unwrap();
        let err = load_configuration(file_path.to_str().unwrap())
            .expect_err("should fail");
        assert!(err.to_string().contains("bad.yml"), "error was: {err}");
    }

    #[test]
    fn test_load_configuration_validates() {
        let dir = tempfile::TempDir::new().unwrap();
        let file_path = dir.path().join("invalid.yml");
        let yaml = r#"
version: "2.0"
name: "test"
description: "Invalid"
commands:
  bad:
    description: "neither cmd nor subcommands"
    flags: []
"#;
        std::fs::write(&file_path, yaml).unwrap();
        let err = load_configuration(file_path.to_str().unwrap())
            .expect_err("should fail validation");
        assert!(err.to_string().contains("must be present"), "error was: {err}");
    }
}
