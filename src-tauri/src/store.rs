use crate::settings::FitMode;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Record of the wallpaper currently applied, persisted in `state.json`.
///
/// Only one image is kept. The previous design kept a 60-image history as an
/// offline fallback; failures now leave the desktop untouched instead, so the
/// history had no remaining purpose.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Applied {
    /// APOD publication date (YYYY-MM-DD).
    pub date: String,
    pub title: String,
    pub explanation: String,
    #[serde(default)]
    pub copyright: Option<String>,
    /// "image" or "video". For a video the stored file is a still: the
    /// thumbnail the API published, or a frame decoded from the video itself
    /// when it was served as a plain file and had no thumbnail.
    pub media_type: String,
    /// The video itself when `media_type` is "video": a YouTube or Vimeo embed
    /// link, or the URL of the file.
    #[serde(default)]
    pub video_url: Option<String>,
    /// URL the original was downloaded from.
    pub source_url: String,
    /// File name of the downloaded original, inside the store directory.
    pub image_file: String,
    /// File name of the composition that was set as the wallpaper.
    pub wallpaper_file: String,
    /// Composition inputs, so we can tell when it has to be redone.
    pub fit: FitMode,
    pub width: u32,
    pub height: u32,
    /// Local calendar day the wallpaper was applied on. Drives the
    /// once-per-day rule in random mode.
    pub applied_on: String,
}

/// On-disk layout, under the OS app-data directory:
///   settings.json
///   state.json
///   current/<date>.<ext>                        downloaded original
///   current/wall-<date>-<fit>-<w>x<h>.jpg       applied composition
pub struct Store {
    root: PathBuf,
    dir: PathBuf,
    state_path: PathBuf,
    applied: Option<Applied>,
}

impl Store {
    pub fn load(root: &Path) -> Store {
        let state_path = root.join("state.json");
        let applied = fs::read_to_string(&state_path)
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok());
        Store {
            root: root.to_path_buf(),
            dir: root.join("current"),
            state_path,
            applied,
        }
    }

    pub fn applied(&self) -> Option<&Applied> {
        self.applied.as_ref()
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn image_path(&self, a: &Applied) -> PathBuf {
        self.dir.join(&a.image_file)
    }

    pub fn wallpaper_path(&self, a: &Applied) -> PathBuf {
        self.dir.join(&a.wallpaper_file)
    }

    /// True when both files backing the record are still on disk and not
    /// empty. Combined with the atomic renames used to install them, this is
    /// enough to rule out a truncated or half-written image.
    pub fn files_present(&self, a: &Applied) -> bool {
        [self.image_path(a), self.wallpaper_path(a)]
            .iter()
            .all(|p| fs::metadata(p).map(|m| m.len() > 0).unwrap_or(false))
    }

    pub fn ensure_dir(&self) -> Result<(), String> {
        fs::create_dir_all(&self.dir)
            .map_err(|e| format!("Could not create {}: {e}", self.dir.display()))
    }

    /// Records the new wallpaper and deletes everything else in the store
    /// directory. Called only after the wallpaper has actually been applied.
    pub fn commit(&mut self, applied: Applied) -> Result<(), String> {
        let raw = serde_json::to_string_pretty(&applied)
            .map_err(|e| format!("Could not serialise the state file: {e}"))?;
        write_atomic(&self.state_path, raw.as_bytes())?;
        self.applied = Some(applied);
        self.prune();
        self.remove_legacy_cache();
        Ok(())
    }

    /// Deletes the 60-image history written by earlier versions. Only called
    /// once a wallpaper has been applied from the current layout, so the file
    /// backing the visible desktop is never pulled out from under it.
    fn remove_legacy_cache(&self) {
        let legacy = self.root.join("cache");
        if legacy.is_dir() {
            let _ = fs::remove_dir_all(legacy);
        }
    }

    /// Removes every file in the store directory that the current record does
    /// not reference: previous images, stale temporary files, and leftovers
    /// from older versions of the app.
    fn prune(&self) {
        let Some(a) = &self.applied else { return };
        let keep = [a.image_file.as_str(), a.wallpaper_file.as_str()];
        let Ok(entries) = fs::read_dir(&self.dir) else {
            return;
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let Some(name) = name.to_str() else { continue };
            if !keep.contains(&name) {
                let _ = fs::remove_file(entry.path());
            }
        }
    }
}

/// Writes a file by way of a temporary file in the same directory followed by
/// a rename, so a crash or a full disk can never leave a half-written file
/// behind. `fs::rename` replaces the destination atomically.
///
/// The temporary file is flushed to the device before the rename, and the
/// directory after it. Without the first, a power loss can leave the rename
/// visible with an empty file under it -- the rename is ordered, its contents
/// are not; without the second, the rename itself can be lost.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("Invalid path: {}", path.display()))?;
    fs::create_dir_all(parent)
        .map_err(|e| format!("Could not create {}: {e}", parent.display()))?;

    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| format!("Invalid file name: {}", path.display()))?;
    let tmp = parent.join(format!(".{file_name}.tmp"));

    let written = (|| -> std::io::Result<()> {
        let mut file = fs::File::create(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()
    })();
    if let Err(e) = written {
        let _ = fs::remove_file(&tmp);
        return Err(format!("Could not write {}: {e}", tmp.display()));
    }

    fs::rename(&tmp, path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        format!("Could not replace {}: {e}", path.display())
    })?;
    sync_dir(parent);
    Ok(())
}

/// Flushes a directory entry, so a rename performed in it survives a power
/// loss and not merely a process crash.
///
/// Best effort by design: a failure here costs durability, never correctness,
/// and there is nothing useful to tell the user about it.
pub fn sync_dir(dir: &Path) {
    if let Ok(handle) = fs::File::open(dir) {
        let _ = handle.sync_all();
    }
}
