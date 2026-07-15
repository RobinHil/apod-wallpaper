use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// Cle utilisee quand l'utilisateur n'a pas fourni la sienne.
/// Limites DEMO_KEY : 30 requetes/heure, 50/jour.
pub const DEMO_KEY: &str = "DEMO_KEY";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// Image APOD du jour courant.
    Daily,
    /// Date aleatoire dans l'historique APOD, tiree a chaque demarrage
    /// (et a chaque rafraichissement manuel).
    Random,
    /// Date fixe choisie par l'utilisateur (champ `specific_date`).
    Specific,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FitMode {
    /// Image entiere centree sur un fond flou qui remplit l'ecran (defaut).
    BlurFill,
    /// Recadrage de l'image pour remplir l'ecran, sans flou.
    CropFill,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Cle API NASA personnelle. Chaine vide = DEMO_KEY.
    pub api_key: String,
    pub mode: Mode,
    pub fit_mode: FitMode,
    /// Date choisie pour le mode `Specific`, au format AAAA-MM-JJ.
    /// Chaine vide tant que l'utilisateur n'a jamais choisi de date.
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
                .map_err(|e| format!("Creation du dossier de configuration impossible : {e}"))?;
        }
        let raw = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Serialisation des parametres impossible : {e}"))?;
        fs::write(path, raw)
            .map_err(|e| format!("Ecriture du fichier de parametres impossible : {e}"))
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
