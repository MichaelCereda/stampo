use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::errors::RingError;

#[derive(Debug, Deserialize, Serialize)]
pub struct Configuration {
    pub version: String,
    pub description: String,
    pub commands: HashMap<String, Command>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Command {
    pub description: String,
    #[serde(default)]
    pub flags: Vec<Flag>,
    pub cmd: Option<CmdType>,
    pub subcommands: Option<HashMap<String, Command>>,
}

impl Command {
    pub fn validate(&self, context: &str) -> Result<(), RingError> {
        match (&self.cmd, &self.subcommands) {
            (Some(_), Some(_)) => {
                return Err(RingError::Validation {
                    context: context.to_string(),
                    message: "Only 'cmd' or 'subcommands' should be present, not both.".to_string(),
                })
            }
            (None, None) => {
                return Err(RingError::Validation {
                    context: context.to_string(),
                    message: "Either 'cmd' or 'subcommands' must be present.".to_string(),
                })
            }
            _ => (),
        }

        if let Some(subcommands) = &self.subcommands {
            for (sub_name, sub_cmd) in subcommands {
                sub_cmd.validate(&format!("{} > {}", context, sub_name))?;
            }
        }

        Ok(())
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Flag {
    pub name: String,
    #[serde(default)]
    pub short: Option<String>,
    pub description: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum CmdType {
    Http { http: Http },
    Run { run: Vec<String> },
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Http {
    pub method: String,
    pub url: String,
    pub headers: Option<HashMap<String, String>>,
    #[serde(default)]
    pub body: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_run_command() {
        let yaml = r#"
version: "2.0"
description: "Test CLI"
commands:
  greet:
    description: "Greet someone"
    flags:
      - name: "name"
        short: "n"
        description: "Name to greet"
    cmd:
      run:
        - "echo Hello, ${{name}}!"
"#;
        let config: Configuration = serde_saphyr::from_str(yaml).expect("valid YAML");
        let greet = config.commands.get("greet").expect("greet command exists");
        assert_eq!(greet.flags.len(), 1);
        assert_eq!(greet.flags[0].name, "name");
        assert!(matches!(&greet.cmd, Some(CmdType::Run { run }) if run.len() == 1));
    }

    #[test]
    fn test_deserialize_http_command() {
        let yaml = r#"
version: "2.0"
description: "HTTP CLI"
commands:
  fetch:
    description: "Fetch a URL"
    flags: []
    cmd:
      http:
        method: "POST"
        url: "https://example.com/api"
        headers:
          Authorization: "Bearer token"
        body: '{"key":"value"}'
"#;
        let config: Configuration = serde_saphyr::from_str(yaml).expect("valid YAML");
        let fetch = config.commands.get("fetch").expect("fetch command exists");
        if let Some(CmdType::Http { http }) = &fetch.cmd {
            assert_eq!(http.method, "POST");
            assert_eq!(http.url, "https://example.com/api");
            assert!(http.headers.is_some());
            let headers = http.headers.as_ref().unwrap();
            assert_eq!(headers.get("Authorization").map(String::as_str), Some("Bearer token"));
            assert_eq!(http.body.as_deref(), Some(r#"{"key":"value"}"#));
        } else {
            panic!("expected Http CmdType");
        }
    }

    #[test]
    fn test_validate_rejects_both_cmd_and_subcommands() {
        let cmd = Command {
            description: "bad".to_string(),
            flags: vec![],
            cmd: Some(CmdType::Run { run: vec!["echo hi".to_string()] }),
            subcommands: Some({
                let mut map = HashMap::new();
                map.insert("sub".to_string(), Command {
                    description: "sub".to_string(),
                    flags: vec![],
                    cmd: Some(CmdType::Run { run: vec!["echo sub".to_string()] }),
                    subcommands: None,
                });
                map
            }),
        };
        let err = cmd.validate("mycli > bad").expect_err("should fail");
        assert!(err.to_string().contains("not both"), "error was: {err}");
    }

    #[test]
    fn test_validate_rejects_neither_cmd_nor_subcommands() {
        let cmd = Command {
            description: "bad".to_string(),
            flags: vec![],
            cmd: None,
            subcommands: None,
        };
        let err = cmd.validate("mycli > bad").expect_err("should fail");
        assert!(err.to_string().contains("must be present"), "error was: {err}");
    }

    #[test]
    fn test_validate_accepts_cmd_only() {
        let cmd = Command {
            description: "ok".to_string(),
            flags: vec![],
            cmd: Some(CmdType::Run { run: vec!["echo ok".to_string()] }),
            subcommands: None,
        };
        assert!(cmd.validate("mycli > ok").is_ok());
    }

    #[test]
    fn test_validate_accepts_subcommands_only() {
        let inner = Command {
            description: "inner".to_string(),
            flags: vec![],
            cmd: Some(CmdType::Run { run: vec!["echo inner".to_string()] }),
            subcommands: None,
        };
        let mut subs = HashMap::new();
        subs.insert("inner".to_string(), inner);
        let cmd = Command {
            description: "parent".to_string(),
            flags: vec![],
            cmd: None,
            subcommands: Some(subs),
        };
        assert!(cmd.validate("mycli > parent").is_ok());
    }

    #[test]
    fn test_validate_error_includes_context_path() {
        let broken = Command {
            description: "broken".to_string(),
            flags: vec![],
            cmd: None,
            subcommands: None,
        };
        let mut deploy_subs = HashMap::new();
        deploy_subs.insert("broken".to_string(), broken);
        let deploy = Command {
            description: "deploy".to_string(),
            flags: vec![],
            cmd: None,
            subcommands: Some(deploy_subs),
        };
        let err = deploy.validate("mycli > deploy").expect_err("should fail");
        let msg = err.to_string();
        assert!(msg.contains("mycli > deploy > broken"), "path not in error: {msg}");
    }

    #[test]
    fn test_deserialize_flags_without_short() {
        let yaml = r#"
version: "2.0"
description: "No short flag"
commands:
  run:
    description: "Run something"
    flags:
      - name: "target"
        description: "The target"
    cmd:
      run:
        - "echo ${{target}}"
"#;
        let config: Configuration = serde_saphyr::from_str(yaml).expect("valid YAML");
        let run_cmd = config.commands.get("run").expect("run command exists");
        assert_eq!(run_cmd.flags[0].short, None);
    }

    #[test]
    fn test_deserialize_v2_config_no_slug() {
        let yaml = r#"
version: "2.0"
description: "My CLI"
commands:
  greet:
    description: "Greet someone"
    flags:
      - name: "name"
        short: "n"
        description: "Name to greet"
    cmd:
      run:
        - "echo Hello, ${{name}}!"
"#;
        let config: Configuration = serde_saphyr::from_str(yaml).expect("valid v2 YAML");
        assert_eq!(config.version, "2.0");
        assert_eq!(config.description, "My CLI");
        assert!(config.commands.contains_key("greet"));
    }

    #[test]
    fn test_empty_commands_map() {
        let yaml = r#"
version: "2.0"
description: "Empty"
commands: {}
"#;
        let config: Configuration = serde_saphyr::from_str(yaml).expect("valid YAML");
        assert!(config.commands.is_empty());
    }
}
