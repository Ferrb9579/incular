//! State persistence and safe recovery for Incular Studio.
//! Uses directories standard config paths without hardcoding home directory paths.

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RestorationData {
    pub sidebar_width: f32,
    pub inspector_width: f32,
    pub bottom_panel_height: f32,
    pub sidebar_visible: bool,
    pub inspector_visible: bool,
    pub bottom_panel_visible: bool,
    pub active_tab_index: usize,
    pub theme_mode: String,
    pub locale_code: String,
    pub editor_font_size: f32,
    pub word_wrap: bool,
    pub tab_size: u32,
    pub auto_save: bool,
    pub zoom_level: f32,
}

impl Default for RestorationData {
    fn default() -> Self {
        Self {
            sidebar_width: 260.0,
            inspector_width: 280.0,
            bottom_panel_height: 200.0,
            sidebar_visible: true,
            inspector_visible: true,
            bottom_panel_visible: false,
            active_tab_index: 0,
            theme_mode: "Dark".to_owned(),
            locale_code: "en-US".to_owned(),
            editor_font_size: 14.0,
            word_wrap: false,
            tab_size: 4,
            auto_save: true,
            zoom_level: 1.0,
        }
    }
}

impl RestorationData {
    #[must_use]
    pub fn config_path() -> Option<PathBuf> {
        let proj_dirs = ProjectDirs::from("com", "incular", "incular-studio")?;
        let config_dir = proj_dirs.config_dir();
        Some(config_dir.join("workspace_state.json"))
    }

    #[must_use]
    pub fn load_or_default() -> Self {
        let Some(path) = Self::config_path() else {
            return Self::default();
        };

        if !path.exists() {
            return Self::default();
        }

        match fs::read_to_string(&path) {
            Ok(content) => match serde_json::from_str::<Self>(&content) {
                Ok(data) => data,
                Err(err) => {
                    eprintln!(
                        "Warning: Failed to parse restoration state ({err}); using defaults."
                    );
                    Self::default()
                }
            },
            Err(err) => {
                eprintln!("Warning: Failed to read restoration file ({err}); using defaults.");
                Self::default()
            }
        }
    }

    pub fn save(&self) -> Result<(), std::io::Error> {
        let Some(path) = Self::config_path() else {
            return Ok(());
        };

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let json = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        let tmp_path = path.with_extension("tmp");
        fs::write(&tmp_path, json)?;
        fs::rename(tmp_path, path)?;
        Ok(())
    }
}
