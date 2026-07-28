use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// Key used when the user has not supplied their own.
/// DEMO_KEY limits: 30 requests/hour, 50/day.
pub const DEMO_KEY: &str = "DEMO_KEY";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// Today's APOD.
    Daily,
    /// A random date from the APOD archive.
    Random,
    /// A fixed date chosen by the user (see `specific_date`).
    Specific,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FitMode {
    /// Whole image centred over a blurred fill of itself (default).
    BlurFill,
    /// Image cropped to fill the screen, no blur.
    CropFill,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Personal NASA API key. Empty string means DEMO_KEY.
    pub api_key: String,
    pub mode: Mode,
    pub fit_mode: FitMode,
    /// Date for `Specific` mode, formatted YYYY-MM-DD. Empty until the user
    /// picks one.
    pub specific_date: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            api_key: String::new(),
            mode: Mode::Daily,
            fit_mode: FitMode::BlurFill,
            specific_date: String::new(),
        }
    }
}

impl Settings {
    pub fn load(path: &Path) -> Settings {
        match fs::read_to_string(path) {
            Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
            Err(_) => Settings::default(),
        }
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Could not create the configuration directory: {e}"))?;
        }
        let raw = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Could not serialise settings: {e}"))?;
        fs::write(path, raw).map_err(|e| format!("Could not write the settings file: {e}"))
    }

    pub fn effective_api_key(&self) -> &str {
        let trimmed = self.api_key.trim();
        if trimmed.is_empty() {
            DEMO_KEY
        } else {
            trimmed
        }
    }
}
