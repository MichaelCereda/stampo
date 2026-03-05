use thiserror::Error;

#[derive(Debug, Error)]
pub enum RingError {
    #[error("Config error: {0}")]
    Config(String),

    #[error("Error parsing '{path}': {source}")]
    YamlParse {
        path: String,
        source: Box<serde_saphyr::Error>,
    },

    #[error("IO error reading '{path}': {source}")]
    Io {
        path: String,
        source: std::io::Error,
    },

    #[error("{context}: {message}")]
    Validation { context: String, message: String },

    #[error("Command '{command}' failed with exit code {code}: {stderr}")]
    ShellCommand {
        command: String,
        code: i32,
        stderr: String,
    },

    #[error("{method} {url}: {message}")]
    Http {
        method: String,
        url: String,
        message: String,
    },

    #[error("Environment variable '{name}' is not set")]
    EnvVar { name: String },

    #[error("Unsupported HTTP method '{0}'")]
    UnsupportedMethod(String),
}
