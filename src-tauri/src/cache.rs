use crate::nasa_api::Apod;
use crate::settings::FitMode;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Nombre maximum d'images conservees dans l'historique local.
const MAX_ENTRIES: usize = 60;

/// Une image APOD telechargee avec ses metadonnees. Le champ `copyright` est
/// conserve tel quel : quand il est present, l'image n'est pas dans le
/// domaine public et l'attribution est obligatoire.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    pub date: String,
    pub title: String,
    pub explanation: String,
    #[serde(default)]
    pub copyright: Option<String>,
    pub source_url: String,
    /// Nom du fichier image original dans `images/`.
    pub image_file: String,
    pub fetched_at: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct MetadataFile {
    entries: Vec<CacheEntry>,
}

/// Historique local des images. Structure sur disque :
///   <app_data>/cache/metadata.json
///   <app_data>/cache/images/<date>.<ext>        (originaux)
///   <app_data>/cache/wallpapers/apod-<date>-<fit>.jpg  (compositions finales)
pub struct Cache {
    root: PathBuf,
    entries: Vec<CacheEntry>,
}

impl Cache {
    pub fn load(root: PathBuf) -> Cache {
        let entries = fs::read_to_string(root.join("metadata.json"))
            .ok()
            .and_then(|raw| serde_json::from_str::<MetadataFile>(&raw).ok())
            .map(|m| m.entries)
            .unwrap_or_default();
        Cache { root, entries }
    }

    fn images_dir(&self) -> PathBuf {
        self.root.join("images")
    }

    fn wallpapers_dir(&self) -> PathBuf {
        self.root.join("wallpapers")
    }

    pub fn get(&self, date: &str) -> Option<&CacheEntry> {
        self.entries.iter().find(|e| e.date == date)
    }

    /// Entree la plus recente par date de publication : c'est la "derniere
    /// image chargee avec succes" utilisee comme repli hors-ligne.
    pub fn latest(&self) -> Option<&CacheEntry> {
        self.entries.iter().max_by(|a, b| a.date.cmp(&b.date))
    }

    /// Tirage aleatoire dans le cache (repli du mode aleatoire hors-ligne).
    pub fn random(&self) -> Option<&CacheEntry> {
        use rand::RngExt;
        if self.entries.is_empty() {
            return None;
        }
        let i = rand::rng().random_range(0..self.entries.len());
        self.entries.get(i)
    }

    pub fn image_path(&self, entry: &CacheEntry) -> PathBuf {
        self.images_dir().join(&entry.image_file)
    }

    /// Le fichier compose porte la date et le mode d'ajustement dans son nom :
    /// un chemin different a chaque changement force les bureaux qui mettent
    /// le fond d'ecran en cache par chemin (macOS notamment) a le recharger.
    pub fn wallpaper_path(&self, date: &str, fit: FitMode) -> PathBuf {
        let suffix = match fit {
            FitMode::BlurFill => "blur",
            FitMode::CropFill => "crop",
        };
        self.wallpapers_dir()
            .join(format!("apod-{date}-{suffix}.jpg"))
    }

    /// Enregistre l'image originale et ses metadonnees, puis purge les plus
    /// anciennes entrees au-dela de la limite.
    pub fn store(
        &mut self,
        apod: &Apod,
        source_url: &str,
        bytes: &[u8],
    ) -> Result<CacheEntry, String> {
        let ext = match image::guess_format(bytes) {
            Ok(image::ImageFormat::Png) => "png",
            Ok(image::ImageFormat::Gif) => "gif",
            Ok(image::ImageFormat::WebP) => "webp",
            _ => "jpg",
        };
        let file_name = format!("{}.{}", apod.date, ext);

        fs::create_dir_all(self.images_dir())
            .map_err(|e| format!("Création du dossier de cache impossible : {e}"))?;
        fs::write(self.images_dir().join(&file_name), bytes)
            .map_err(|e| format!("Écriture de l'image en cache impossible : {e}"))?;

        let entry = CacheEntry {
            date: apod.date.clone(),
            title: apod.title.clone(),
            explanation: apod.explanation.clone(),
            copyright: apod.copyright.clone(),
            source_url: source_url.to_string(),
            image_file: file_name,
            fetched_at: chrono::Local::now().to_rfc3339(),
        };

        self.entries.retain(|e| e.date != entry.date);
        self.entries.push(entry.clone());
        self.prune(&entry.date);
        self.save_metadata()?;
        Ok(entry)
    }

    fn prune(&mut self, keep_date: &str) {
        if self.entries.len() <= MAX_ENTRIES {
            return;
        }
        // Les plus anciens telechargements partent en premier, sauf l'entree
        // qu'on vient d'appliquer.
        self.entries.sort_by(|a, b| a.fetched_at.cmp(&b.fetched_at));
        while self.entries.len() > MAX_ENTRIES {
            let Some(pos) = self.entries.iter().position(|e| e.date != keep_date) else {
                break;
            };
            let removed = self.entries.remove(pos);
            let _ = fs::remove_file(self.images_dir().join(&removed.image_file));
            for fit in [FitMode::BlurFill, FitMode::CropFill] {
                let _ = fs::remove_file(self.wallpaper_path(&removed.date, fit));
            }
        }
    }

    fn save_metadata(&self) -> Result<(), String> {
        fs::create_dir_all(&self.root)
            .map_err(|e| format!("Création du dossier de cache impossible : {e}"))?;
        let meta = MetadataFile {
            entries: self.entries.clone(),
        };
        let raw = serde_json::to_string_pretty(&meta)
            .map_err(|e| format!("Sérialisation des métadonnées impossible : {e}"))?;
        fs::write(self.root.join("metadata.json"), raw)
            .map_err(|e| format!("Écriture de metadata.json impossible : {e}"))
    }

    pub fn ensure_dirs(&self) -> Result<(), String> {
        for dir in [self.images_dir(), self.wallpapers_dir()] {
            fs::create_dir_all(&dir)
                .map_err(|e| format!("Création du dossier {} impossible : {e}", dir.display()))?;
        }
        Ok(())
    }
}
