use serde_derive::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Deserialize, Serialize)]
pub struct Configuration {
    pub version: String,
    pub description: String,
    pub slug: String,
    pub commands: HashMap<String, Command>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Command {
    pub description: String,
    pub flags: Vec<Flag>,
    pub cmd: Option<CmdType>,
    pub subcommands: Option<HashMap<String, Command>>,
}

impl Command {
    pub fn validate(&self) -> Result<(), String> {
        match (&self.cmd, &self.subcommands) {
            (Some(_), Some(_)) => {
                return Err("Only 'cmd' or 'subcommands' should be present, not both.".to_string())
            }
            (None, None) => {
                return Err("Either 'cmd' or 'subcommands' must be present.".to_string())
            }
            _ => (),
        }

        if let Some(subcommands) = &self.subcommands {
            for (_, sub_cmd) in subcommands {
                sub_cmd.validate()?; // Recursive call
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
