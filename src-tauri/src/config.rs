//! Runtime configuration and on-disk locations.
//!
//! The API key is read from `config.json` in the app data directory, with an
//! environment override for development. It is never sent to the webview and
//! never logged.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct FileConfig {
    #[serde(default)]
    pub steamgriddb_api_key: String,
}

pub struct Config {
    pub data_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub api_key: String,
}

impl Config {
    pub fn load(data_dir: PathBuf) -> Self {
        Self::load_with_env(data_dir, std::env::var("PIRATELINK_SGDB_KEY").ok())
    }

    /// Explicit env value, so tests do not depend on the ambient process
    /// environment (`cargo test` with the key exported must not change behaviour).
    pub fn load_with_env(data_dir: PathBuf, env_key: Option<String>) -> Self {
        let cache_dir = data_dir.join("cache");

        let api_key = env_key
            .filter(|k| !k.trim().is_empty())
            .or_else(|| read_key(&data_dir))
            .unwrap_or_default();

        Self {
            data_dir,
            cache_dir,
            api_key,
        }
    }

    pub fn art_dir(&self) -> PathBuf {
        self.cache_dir.join("art")
    }
}

fn read_key(data_dir: &Path) -> Option<String> {
    let text = std::fs::read_to_string(data_dir.join("config.json")).ok()?;
    let parsed: FileConfig = serde_json::from_str(&text).ok()?;
    let key = parsed.steamgriddb_api_key.trim().to_string();
    (!key.is_empty()).then_some(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_key_from_config_file() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("config.json"),
            r#"{"steamgriddb_api_key":"  abc123  "}"#,
        )
        .unwrap();

        let config = Config::load_with_env(tmp.path().to_path_buf(), None);
        assert_eq!(config.api_key, "abc123");
        assert_eq!(config.art_dir(), tmp.path().join("cache").join("art"));
    }

    #[test]
    fn env_key_wins_over_file() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("config.json"),
            r#"{"steamgriddb_api_key":"from-file"}"#,
        )
        .unwrap();

        let config = Config::load_with_env(tmp.path().to_path_buf(), Some("from-env".into()));
        assert_eq!(config.api_key, "from-env");
    }

    #[test]
    fn missing_or_broken_config_means_no_key() {
        let tmp = tempfile::tempdir().unwrap();
        assert_eq!(
            Config::load_with_env(tmp.path().to_path_buf(), None).api_key,
            ""
        );

        std::fs::write(tmp.path().join("config.json"), "{ not json").unwrap();
        assert_eq!(
            Config::load_with_env(tmp.path().to_path_buf(), None).api_key,
            ""
        );
    }

    #[test]
    fn blank_env_key_is_ignored() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("config.json"),
            r#"{"steamgriddb_api_key":"from-file"}"#,
        )
        .unwrap();
        assert_eq!(
            Config::load_with_env(tmp.path().to_path_buf(), Some("   ".into())).api_key,
            "from-file"
        );
    }
}
