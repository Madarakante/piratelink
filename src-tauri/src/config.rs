//! Runtime configuration and on-disk locations.
//!
//! The API key is read from `config.json` in the app data directory, with an
//! environment override for development. It is never sent to the webview and
//! never logged.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The file as it is on disk. Kept separate from the resolved values below so a
/// save never writes an environment-provided API key into the file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileConfig {
    #[serde(default)]
    pub steamgriddb_api_key: String,
    /// Optional path to the Switch emulator. Empty means "find it".
    #[serde(default)]
    pub ryujinx_path: String,
    /// `fit` shows the whole hero above the shelf; `fill` lets it cover the window.
    #[serde(default)]
    pub hero_fit: String,
    /// `auto`, `xbox` or `playstation`: which glyphs the hint bar prints.
    #[serde(default)]
    pub glyphs: String,
    /// UI cue sounds on or off.
    #[serde(default = "sound_default")]
    pub sound: bool,
}

fn sound_default() -> bool {
    true
}

impl Default for FileConfig {
    /// Matches the serde defaults: a fresh install is `fit` hero, `auto` glyphs and
    /// sound on. `Default::default()` does not run `#[serde(default = ...)]`, and
    /// that difference is exactly the kind of thing a settings page would ship.
    fn default() -> Self {
        Self {
            steamgriddb_api_key: String::new(),
            ryujinx_path: String::new(),
            hero_fit: "fit".to_string(),
            glyphs: "auto".to_string(),
            sound: true,
        }
    }
}

pub struct Config {
    pub data_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub api_key: String,
    /// Configured emulator, if the file names one.
    pub ryujinx: Option<PathBuf>,
    /// `"fit"` or `"fill"`.
    pub hero_fit: String,
    /// `"auto"`, `"xbox"` or `"playstation"`.
    pub glyphs: String,
    pub sound: bool,
    /// What will be written back on save.
    file: FileConfig,
}

impl Config {
    pub fn load(data_dir: PathBuf) -> Self {
        Self::load_with_env(data_dir, std::env::var("PIRATELINK_SGDB_KEY").ok())
    }

    /// Explicit env value, so tests do not depend on the ambient process
    /// environment (`cargo test` with the key exported must not change behaviour).
    pub fn load_with_env(data_dir: PathBuf, env_key: Option<String>) -> Self {
        let cache_dir = data_dir.join("cache");
        let file = read_file(&data_dir).unwrap_or_default();

        let api_key = env_key
            .filter(|k| !k.trim().is_empty())
            .or_else(|| {
                let key = file.steamgriddb_api_key.trim().to_string();
                (!key.is_empty()).then_some(key)
            })
            .unwrap_or_default();

        let ryujinx = {
            let path = file.ryujinx_path.trim();
            (!path.is_empty()).then(|| PathBuf::from(path))
        };

        let hero_fit = match file.hero_fit.trim() {
            "fill" => "fill".to_string(),
            _ => "fit".to_string(),
        };

        let glyphs = match file.glyphs.trim() {
            "xbox" => "xbox".to_string(),
            "playstation" => "playstation".to_string(),
            _ => "auto".to_string(),
        };

        Self {
            data_dir,
            cache_dir,
            api_key,
            ryujinx,
            hero_fit,
            glyphs,
            sound: file.sound,
            file,
        }
    }

    pub fn art_dir(&self) -> PathBuf {
        self.cache_dir.join("art")
    }

    pub fn config_path(&self) -> PathBuf {
        self.data_dir.join("config.json")
    }

    pub fn log_path(&self) -> PathBuf {
        self.data_dir.join("piratelink.log")
    }

    /// Applies a new key and remembers it for the next save.
    pub fn set_api_key(&mut self, key: &str) {
        let key = key.trim().to_string();
        self.file.steamgriddb_api_key = key.clone();
        self.api_key = key;
    }

    /// Empty path means "search for the emulator yourself".
    pub fn set_ryujinx(&mut self, path: &str) {
        let path = path.trim().to_string();
        self.file.ryujinx_path = path.clone();
        self.ryujinx = (!path.is_empty()).then(|| PathBuf::from(path));
    }

    /// Unknown values fall back to the default rather than being written out.
    pub fn set_hero_fit(&mut self, value: &str) {
        self.hero_fit = if value == "fill" { "fill" } else { "fit" }.to_string();
        self.file.hero_fit = self.hero_fit.clone();
    }

    pub fn set_glyphs(&mut self, value: &str) {
        self.glyphs = match value {
            "xbox" => "xbox",
            "playstation" => "playstation",
            _ => "auto",
        }
        .to_string();
        self.file.glyphs = self.glyphs.clone();
    }

    pub fn set_sound(&mut self, on: bool) {
        self.sound = on;
        self.file.sound = on;
    }

    /// Writes the file, not the resolved values: an API key that came from the
    /// environment must not be persisted by opening the settings page.
    ///
    /// The other fields are written from their resolved values, so the file always
    /// holds `"fit"`/`"auto"` rather than an empty string that only means "default".
    pub fn save(&self) -> std::io::Result<()> {
        let file = FileConfig {
            steamgriddb_api_key: self.file.steamgriddb_api_key.clone(),
            ryujinx_path: self.file.ryujinx_path.clone(),
            hero_fit: self.hero_fit.clone(),
            glyphs: self.glyphs.clone(),
            sound: self.sound,
        };
        std::fs::create_dir_all(&self.data_dir)?;
        let text = serde_json::to_string_pretty(&file)
            .map_err(|err| std::io::Error::other(err.to_string()))?;
        std::fs::write(self.config_path(), text + "\n")
    }
}

fn read_file(data_dir: &Path) -> Option<FileConfig> {
    let text = std::fs::read_to_string(data_dir.join("config.json")).ok()?;
    serde_json::from_str(&text).ok()
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
    fn reads_emulator_path_from_config_file() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("config.json"),
            r#"{"steamgriddb_api_key":"abc123","ryujinx_path":"D:\\ryujinx-stable-1.3.3-windows_x64\\Ryujinx.exe"}"#,
        )
        .unwrap();

        let config = Config::load_with_env(tmp.path().to_path_buf(), None);
        assert_eq!(
            config.ryujinx.as_deref(),
            Some(Path::new(
                r"D:\ryujinx-stable-1.3.3-windows_x64\Ryujinx.exe"
            ))
        );
    }

    #[test]
    fn blank_emulator_path_means_auto_detect() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("config.json"),
            r#"{"steamgriddb_api_key":"abc123","ryujinx_path":"   "}"#,
        )
        .unwrap();

        let config = Config::load_with_env(tmp.path().to_path_buf(), None);
        assert!(config.ryujinx.is_none());
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

    #[test]
    fn settings_survive_a_save_and_reload() {
        let tmp = tempfile::tempdir().unwrap();
        let mut config = Config::load_with_env(tmp.path().to_path_buf(), None);
        config.set_api_key("abc123");
        config.set_ryujinx(r"D:\ryujinx\Ryujinx.exe");
        config.set_hero_fit("fill");
        config.set_glyphs("playstation");
        config.set_sound(false);
        config.save().unwrap();

        let reloaded = Config::load_with_env(tmp.path().to_path_buf(), None);
        assert_eq!(reloaded.api_key, "abc123");
        assert_eq!(
            reloaded.ryujinx.as_deref(),
            Some(Path::new(r"D:\ryujinx\Ryujinx.exe"))
        );
        assert_eq!(reloaded.hero_fit, "fill");
        assert_eq!(reloaded.glyphs, "playstation");
        assert!(!reloaded.sound);
    }

    #[test]
    fn saving_does_not_persist_an_environment_key() {
        let tmp = tempfile::tempdir().unwrap();
        let mut config = Config::load_with_env(tmp.path().to_path_buf(), Some("from-env".into()));
        config.set_hero_fit("fit");
        config.save().unwrap();

        let text = std::fs::read_to_string(tmp.path().join("config.json")).unwrap();
        assert!(!text.contains("from-env"));
    }

    #[test]
    fn save_writes_normalised_preferences() {
        let tmp = tempfile::tempdir().unwrap();
        // A file from before the settings page existed: only the key.
        std::fs::write(
            tmp.path().join("config.json"),
            r#"{"steamgriddb_api_key":"from-file"}"#,
        )
        .unwrap();
        let config = Config::load_with_env(tmp.path().to_path_buf(), None);
        config.save().unwrap();

        let text = std::fs::read_to_string(tmp.path().join("config.json")).unwrap();
        assert!(text.contains("\"hero_fit\": \"fit\""), "{text}");
        assert!(text.contains("\"glyphs\": \"auto\""), "{text}");
        assert!(text.contains("\"sound\": true"), "{text}");
        assert!(text.contains("from-file"), "{text}");
    }

    #[test]
    fn defaults_are_fit_auto_and_sound_on() {
        let tmp = tempfile::tempdir().unwrap();
        let config = Config::load_with_env(tmp.path().to_path_buf(), None);
        assert_eq!(config.hero_fit, "fit");
        assert_eq!(config.glyphs, "auto");
        assert!(config.sound);
    }

    #[test]
    fn unknown_preferences_fall_back_to_the_default() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("config.json"),
            r#"{"hero_fit":"stretch","glyphs":"nintendo"}"#,
        )
        .unwrap();
        let config = Config::load_with_env(tmp.path().to_path_buf(), None);
        assert_eq!(config.hero_fit, "fit");
        assert_eq!(config.glyphs, "auto");
    }
}
