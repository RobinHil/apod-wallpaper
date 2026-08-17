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
    /// Reads the settings, falling back to the defaults when there is nothing
    /// to read.
    ///
    /// A file that exists but does not parse is *kept*, renamed out of the
    /// way, rather than left in place: the caller carries on with the
    /// defaults and saves them at the next settings change, which would
    /// otherwise overwrite the only copy of the user's API key with no
    /// warning. Recovering it by hand is then a matter of opening the file
    /// next to it.
    pub fn load(path: &Path) -> Settings {
        let Ok(raw) = fs::read_to_string(path) else {
            return Settings::default();
        };
        match serde_json::from_str(&raw) {
            Ok(settings) => settings,
            Err(e) => {
                let kept = path.with_extension("json.unreadable");
                eprintln!(
                    "{} could not be parsed ({e}); keeping it as {} and starting from the defaults",
                    path.display(),
                    kept.display()
                );
                let _ = fs::rename(path, &kept);
                Settings::default()
            }
        }
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        let raw = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Could not serialise settings: {e}"))?;
        crate::store::write_atomic(path, raw.as_bytes())
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

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("apod-wallpaper-settings-{name}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir.join("settings.json")
    }

    #[test]
    fn a_missing_file_is_simply_the_defaults() {
        let path = scratch("missing");
        let loaded = Settings::load(&path);

        assert_eq!(loaded.mode, Mode::Daily);
        assert_eq!(loaded.effective_api_key(), DEMO_KEY);
        // Nothing to rescue, so nothing is written either.
        assert!(!path.with_extension("json.unreadable").exists());
    }

    #[test]
    fn an_unreadable_file_is_kept_instead_of_being_overwritten() {
        let path = scratch("unreadable");
        // A truncated file: valid JSON up to the point it stops.
        let corrupt = r#"{"api_key": "PERSONAL-KEY-WORTH-KEEPING", "mode": "#;
        fs::write(&path, corrupt).unwrap();

        let loaded = Settings::load(&path);
        assert_eq!(
            loaded.effective_api_key(),
            DEMO_KEY,
            "must not invent a key"
        );

        // The point of the whole exercise: saving the defaults now must not be
        // able to destroy the only copy of the user's key.
        loaded.save(&path).unwrap();
        let kept = path.with_extension("json.unreadable");
        assert_eq!(fs::read_to_string(&kept).unwrap(), corrupt);

        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn a_readable_file_round_trips() {
        let path = scratch("round-trip");
        let saved = Settings {
            api_key: "  spaced-key  ".to_string(),
            mode: Mode::Specific,
            fit_mode: FitMode::CropFill,
            specific_date: "2001-02-03".to_string(),
        };
        saved.save(&path).unwrap();

        let loaded = Settings::load(&path);
        assert_eq!(loaded.mode, Mode::Specific);
        assert_eq!(loaded.fit_mode, FitMode::CropFill);
        assert_eq!(loaded.specific_date, "2001-02-03");
        // Stored as typed, trimmed only where it is used.
        assert_eq!(loaded.effective_api_key(), "spaced-key");

        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn a_key_of_only_spaces_falls_back_to_the_demo_key() {
        let settings = Settings {
            api_key: "   ".to_string(),
            ..Settings::default()
        };
        assert_eq!(settings.effective_api_key(), DEMO_KEY);
    }
}
