use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;

/// Metadata stored alongside a cached alias configuration.
#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct AliasMetadata {
    /// Absolute path to the original source configuration file.
    pub source_path: String,
    /// SHA-256 hex digest of the configuration content at trust time.
    pub hash: String,
    /// Unix timestamp (seconds) when the configuration was trusted.
    pub trusted_at: String,
}

/// Returns the directory that holds all cached alias directories.
///
/// # Panics
///
/// Panics if the home directory cannot be determined.
///
/// # Examples
///
/// ```no_run
/// let dir = ring_cli::cache::aliases_dir();
/// assert!(dir.ends_with(".ring-cli/aliases"));
/// ```
pub fn aliases_dir() -> PathBuf {
    dirs::home_dir()
        .expect("Unable to determine home directory")
        .join(".ring-cli/aliases")
}

/// Returns the cache directory for a specific alias.
///
/// # Examples
///
/// ```no_run
/// let dir = ring_cli::cache::alias_dir("my-tool");
/// assert!(dir.ends_with("my-tool"));
/// ```
pub fn alias_dir(alias_name: &str) -> PathBuf {
    aliases_dir().join(alias_name)
}

/// Computes the SHA-256 hex digest of `content`.
///
/// The output is deterministic: identical inputs always produce the same hash.
///
/// # Examples
///
/// ```
/// let h = ring_cli::cache::compute_hash("hello");
/// assert_eq!(h.len(), 64); // 256 bits = 64 hex chars
/// ```
pub fn compute_hash(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Saves a trusted configuration to the local cache.
///
/// Creates `~/.ring-cli/aliases/<alias_name>/config.yml` with the raw
/// configuration content and `metadata.json` with the source path, hash,
/// and trust timestamp.
///
/// # Errors
///
/// Returns an error if the cache directory cannot be created or if any
/// file write fails.
pub fn save_trusted_config(
    alias_name: &str,
    source_path: &str,
    config_content: &str,
) -> Result<(), anyhow::Error> {
    let dir = alias_dir(alias_name);
    fs::create_dir_all(&dir)?;
    fs::write(dir.join("config.yml"), config_content)?;
    let metadata = AliasMetadata {
        source_path: source_path.to_string(),
        hash: compute_hash(config_content),
        trusted_at: chrono_free_timestamp(),
    };
    let json = serde_json::to_string_pretty(&metadata)?;
    fs::write(dir.join("metadata.json"), json)?;
    Ok(())
}

/// Loads a previously cached configuration and its metadata.
///
/// # Errors
///
/// Returns an error if the cache directory does not exist, any file is
/// unreadable, or the metadata JSON is malformed.
pub fn load_trusted_config(alias_name: &str) -> Result<(String, AliasMetadata), anyhow::Error> {
    let dir = alias_dir(alias_name);
    let config = fs::read_to_string(dir.join("config.yml"))?;
    let metadata_str = fs::read_to_string(dir.join("metadata.json"))?;
    let metadata: AliasMetadata = serde_json::from_str(&metadata_str)?;
    Ok((config, metadata))
}

/// Returns the current Unix time as a decimal string without pulling in chrono.
fn chrono_free_timestamp() -> String {
    use std::time::SystemTime;
    let secs = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    secs.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_hash_deterministic() {
        let h1 = compute_hash("hello world");
        let h2 = compute_hash("hello world");
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_compute_hash_changes_with_input() {
        let h1 = compute_hash("hello");
        let h2 = compute_hash("world");
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_save_and_load_trusted_config() {
        let dir = tempfile::TempDir::new().unwrap();
        let alias_path = dir.path().join("test-alias");
        fs::create_dir_all(&alias_path).unwrap();

        let content = "version: \"2.0\"\ndescription: \"test\"";
        fs::write(alias_path.join("config.yml"), content).unwrap();
        let metadata = AliasMetadata {
            source_path: "/tmp/test.yml".to_string(),
            hash: compute_hash(content),
            trusted_at: "12345".to_string(),
        };
        let json = serde_json::to_string_pretty(&metadata).unwrap();
        fs::write(alias_path.join("metadata.json"), json).unwrap();

        let loaded_config = fs::read_to_string(alias_path.join("config.yml")).unwrap();
        let loaded_meta: AliasMetadata = serde_json::from_str(
            &fs::read_to_string(alias_path.join("metadata.json")).unwrap(),
        )
        .unwrap();

        assert_eq!(loaded_config, content);
        assert_eq!(loaded_meta.hash, compute_hash(content));
        assert_eq!(loaded_meta.source_path, "/tmp/test.yml");
    }
}
