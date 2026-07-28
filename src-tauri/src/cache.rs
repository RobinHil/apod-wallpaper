use crate::nasa_api::Apod;
use crate::settings::FitMode;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Maximum number of images kept in the local history.
const MAX_ENTRIES: usize = 60;

/// A downloaded APOD image with its metadata. The `copyright` field is kept
/// verbatim: when present the image is not public domain and attribution is
/// mandatory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    pub date: String,
    pub title: String,
    pub explanation: String,
    #[serde(default)]
    pub copyright: Option<String>,
    /// "image" or "video"; for a video the stored file is the thumbnail.
    #[serde(default = "default_media_type")]
    pub media_type: String,
    /// Video link (YouTube/Vimeo) when media_type == "video", so the panel can
    /// open it directly.
    #[serde(default)]
    pub video_url: Option<String>,
    pub source_url: String,
    /// Name of the original image file inside `images/`.
    pub image_file: String,
    pub fetched_at: String,
}

fn default_media_type() -> String {
    "image".to_string()
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct MetadataFile {
    entries: Vec<CacheEntry>,
}

/// Local image history. On-disk layout:
///   <app_data>/cache/metadata.json
///   <app_data>/cache/images/<date>.<ext>                (originals)
///   <app_data>/cache/wallpapers/wall-<date>-<fit>.jpg   (final compositions)
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

    /// Most recent entry by publication date: the "last successfully loaded
    /// image" used as the offline fallback.
    pub fn latest(&self) -> Option<&CacheEntry> {
        self.entries.iter().max_by(|a, b| a.date.cmp(&b.date))
    }

    /// Random pick from the store (offline fallback for random mode).
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

    /// The composed file carries the date and the fit mode in its name: a
    /// different path on every change forces desktops that cache the wallpaper
    /// by path (macOS in particular) to reload it. The "wall-" prefix
    /// distinguishes the current format (no burned-in text) from older
    /// "apod-" compositions, which are therefore ignored.
    pub fn wallpaper_path(&self, date: &str, fit: FitMode) -> PathBuf {
        let suffix = match fit {
            FitMode::BlurFill => "blur",
            FitMode::CropFill => "crop",
        };
        self.wallpapers_dir()
            .join(format!("wall-{date}-{suffix}.jpg"))
    }

    /// Stores the original image and its metadata, then prunes the oldest
    /// entries beyond the limit.
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
            .map_err(|e| format!("Could not create the store directory: {e}"))?;
        fs::write(self.images_dir().join(&file_name), bytes)
            .map_err(|e| format!("Could not write the image to the store: {e}"))?;

        let entry = CacheEntry {
            date: apod.date.clone(),
            title: apod.title.clone(),
            explanation: apod.explanation.clone(),
            copyright: apod.copyright.clone(),
            media_type: apod.media_type.clone(),
            video_url: if apod.is_video() {
                apod.url.clone()
            } else {
                None
            },
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
        // Oldest downloads go first, except the entry we just applied.
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
            .map_err(|e| format!("Could not create the store directory: {e}"))?;
        let meta = MetadataFile {
            entries: self.entries.clone(),
        };
        let raw = serde_json::to_string_pretty(&meta)
            .map_err(|e| format!("Could not serialise metadata: {e}"))?;
        fs::write(self.root.join("metadata.json"), raw)
            .map_err(|e| format!("Could not write metadata.json: {e}"))
    }

    pub fn ensure_dirs(&self) -> Result<(), String> {
        for dir in [self.images_dir(), self.wallpapers_dir()] {
            fs::create_dir_all(&dir)
                .map_err(|e| format!("Could not create directory {}: {e}", dir.display()))?;
        }
        Ok(())
    }
}
