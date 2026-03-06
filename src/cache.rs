use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;

/// One entry in the alias metadata — corresponds to a single config file.
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct ConfigEntry {
    /// The `name` field from the configuration YAML.
    pub name: String,
    /// Absolute path to the original source configuration file.
    pub source_path: String,
    /// SHA-256 hex digest of the configuration content at trust time.
    pub hash: String,
    /// Unix timestamp (seconds) when the configuration was trusted.
    pub trusted_at: String,
}

/// Metadata stored alongside all cached configs for one alias.
#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct AliasMetadata {
    /// All config entries registered for this alias.
    pub configs: Vec<ConfigEntry>,
    /// Optional banner text to display on CLI invocation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub banner: Option<String>,
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

/// Saves one or more trusted configurations to the local cache.
///
/// Each element of `configs` is `(name, source_path, content)`.  The content
/// is written to `~/.ring-cli/aliases/<alias_name>/<name>.yml` and a combined
/// `metadata.json` records the source path, hash, and trust timestamp for
/// every entry.
///
/// # Errors
///
/// Returns an error if the cache directory cannot be created or if any
/// file write fails.
pub fn save_trusted_configs(
    alias_name: &str,
    configs: &[(String, String, String)], // (name, source_path, content)
    banner: Option<String>,
) -> Result<(), anyhow::Error> {
    let dir = alias_dir(alias_name);
    fs::create_dir_all(&dir)?;

    let mut entries = Vec::new();
    for (name, source_path, content) in configs {
        fs::write(dir.join(format!("{name}.yml")), content)?;
        entries.push(ConfigEntry {
            name: name.clone(),
            source_path: source_path.clone(),
            hash: compute_hash(content),
            trusted_at: chrono_free_timestamp(),
        });
    }

    let metadata = AliasMetadata { configs: entries, banner };
    let json = serde_json::to_string_pretty(&metadata)?;
    fs::write(dir.join("metadata.json"), json)?;
    Ok(())
}

/// Loads all previously cached configurations and their metadata for an alias.
///
/// Returns `(contents, metadata)` where `contents` is the raw YAML for each
/// config in the same order as `metadata.configs`.
///
/// # Errors
///
/// Returns an error if the cache directory does not exist, any file is
/// unreadable, or the metadata JSON is malformed.
pub fn load_trusted_configs(
    alias_name: &str,
) -> Result<(Vec<String>, AliasMetadata), anyhow::Error> {
    let dir = alias_dir(alias_name);
    let metadata_str = fs::read_to_string(dir.join("metadata.json"))?;
    let metadata: AliasMetadata = serde_json::from_str(&metadata_str)?;

    let mut contents = Vec::new();
    for entry in &metadata.configs {
        let content = fs::read_to_string(dir.join(format!("{}.yml", entry.name)))?;
        contents.push(content);
    }

    Ok((contents, metadata))
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
    fn test_save_and_load_trusted_configs() {
        let dir = tempfile::TempDir::new().unwrap();

        // Simulate what save_trusted_configs does but in a temp dir by writing
        // directly so we can control the alias_dir path.
        let alias_path = dir.path().join("test-alias");
        fs::create_dir_all(&alias_path).unwrap();

        let content_a = "version: \"2.0\"\nname: \"alpha\"\ndescription: \"alpha config\"";
        let content_b = "version: \"2.0\"\nname: \"beta\"\ndescription: \"beta config\"";

        fs::write(alias_path.join("alpha.yml"), content_a).unwrap();
        fs::write(alias_path.join("beta.yml"), content_b).unwrap();

        let entries = vec![
            ConfigEntry {
                name: "alpha".to_string(),
                source_path: "/tmp/alpha.yml".to_string(),
                hash: compute_hash(content_a),
                trusted_at: "12345".to_string(),
            },
            ConfigEntry {
                name: "beta".to_string(),
                source_path: "/tmp/beta.yml".to_string(),
                hash: compute_hash(content_b),
                trusted_at: "12345".to_string(),
            },
        ];
        let metadata = AliasMetadata { configs: entries, banner: None };
        let json = serde_json::to_string_pretty(&metadata).unwrap();
        fs::write(alias_path.join("metadata.json"), json).unwrap();

        // Verify round-trip by reading back manually
        let loaded_a = fs::read_to_string(alias_path.join("alpha.yml")).unwrap();
        let loaded_b = fs::read_to_string(alias_path.join("beta.yml")).unwrap();
        let loaded_meta: AliasMetadata = serde_json::from_str(
            &fs::read_to_string(alias_path.join("metadata.json")).unwrap(),
        )
        .unwrap();

        assert_eq!(loaded_a, content_a);
        assert_eq!(loaded_b, content_b);
        assert_eq!(loaded_meta.configs.len(), 2);
        assert_eq!(loaded_meta.configs[0].name, "alpha");
        assert_eq!(loaded_meta.configs[0].hash, compute_hash(content_a));
        assert_eq!(loaded_meta.configs[0].source_path, "/tmp/alpha.yml");
        assert_eq!(loaded_meta.configs[1].name, "beta");
        assert_eq!(loaded_meta.configs[1].hash, compute_hash(content_b));
        assert_eq!(loaded_meta.configs[1].source_path, "/tmp/beta.yml");
    }
}
