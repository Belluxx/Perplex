use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::PathBuf;

const SETTINGS_FILE_NAME: &str = ".perplex_settings.json";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Settings {
    pub model_path: Option<String>,
}

impl Settings {
    fn config_file_path() -> PathBuf {
        let home = env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."));
        home.join(SETTINGS_FILE_NAME)
    }

    pub fn load() -> Self {
        let path = Self::config_file_path();
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                match serde_json::from_str::<Settings>(&content) {
                    Ok(settings) => return settings,
                    Err(e) => log::warn!("Failed to parse settings file: {}", e),
                }
            }
        }

        Self::default()
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let path = Self::config_file_path();
        let content = serde_json::to_string_pretty(self)?;
        fs::write(path, content)?;
        Ok(())
    }
}
