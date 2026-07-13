use crate::settings::FitMode;
use image::imageops::FilterType;
use image::{DynamicImage, RgbaImage};
use std::fs;
use std::io::BufWriter;
use std::path::Path;

/// Ecart de ratio en dessous duquel on remplit directement l'ecran :
/// le fond flou serait invisible derriere l'image.
const RATIO_TOLERANCE: f32 = 0.03;

/// Compose l'image finale a la taille exacte de l'ecran. Aucun texte n'est
/// incruste : le fond d'ecran est l'APOD telle quelle, les metadonnees
/// (date, copyright) restent visibles dans le tray et le panneau.
pub fn compose_wallpaper(
    original: &DynamicImage,
    screen_w: u32,
    screen_h: u32,
    fit: FitMode,
) -> RgbaImage {
    let screen_w = screen_w.max(640);
    let screen_h = screen_h.max(400);

    match fit {
        FitMode::CropFill => original
            .resize_to_fill(screen_w, screen_h, FilterType::Lanczos3)
            .to_rgba8(),
        FitMode::BlurFill => blur_fill(original, screen_w, screen_h),
    }
}

/// Fond = image recadree pour remplir l'ecran puis floutee et assombrie.
/// Premier plan = image entiere (ratio preserve) centree par-dessus.
fn blur_fill(original: &DynamicImage, screen_w: u32, screen_h: u32) -> RgbaImage {
    let img_ratio = original.width() as f32 / original.height().max(1) as f32;
    let screen_ratio = screen_w as f32 / screen_h as f32;

    if ((img_ratio - screen_ratio) / screen_ratio).abs() < RATIO_TOLERANCE {
        return original
            .resize_to_fill(screen_w, screen_h, FilterType::Lanczos3)
            .to_rgba8();
    }

    // Flou gaussien prononce a moindre cout : on floute une version reduite
    // (1/8) puis on la re-agrandit, l'interpolation lisse le reste.
    let small = original.resize_to_fill(
        (screen_w / 8).max(1),
        (screen_h / 8).max(1),
        FilterType::Triangle,
    );
    let blurred = small.blur(6.0).to_rgba8();
    let mut background =
        image::imageops::resize(&blurred, screen_w, screen_h, FilterType::Triangle);
    darken(&mut background, 0.72);

    let foreground = original
        .resize(screen_w, screen_h, FilterType::Lanczos3)
        .to_rgba8();
    let x = (i64::from(screen_w) - i64::from(foreground.width())) / 2;
    let y = (i64::from(screen_h) - i64::from(foreground.height())) / 2;
    image::imageops::overlay(&mut background, &foreground, x, y);
    background
}

/// Assombrit legerement le fond flou pour que l'image nette centree ressorte
/// quelle que soit la photo.
fn darken(img: &mut RgbaImage, factor: f32) {
    for p in img.pixels_mut() {
        for c in 0..3 {
            p[c] = (f32::from(p[c]) * factor) as u8;
        }
    }
}

/// Enregistre la composition en JPEG (qualite 92) : bien plus compact qu'un
/// PNG pour des photos, sans perte visible en fond d'ecran.
pub fn save_jpeg(img: &RgbaImage, path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Création du dossier de sortie impossible : {e}"))?;
    }
    let rgb = DynamicImage::ImageRgba8(img.clone()).to_rgb8();
    let file = fs::File::create(path)
        .map_err(|e| format!("Création du fichier fond d'écran impossible : {e}"))?;
    let mut writer = BufWriter::new(file);
    let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut writer, 92);
    rgb.write_with_encoder(encoder)
        .map_err(|e| format!("Encodage JPEG impossible : {e}"))
}
