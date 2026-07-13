use crate::settings::FitMode;
use ab_glyph::{Font, FontRef, PxScale, ScaleFont};
use image::imageops::FilterType;
use image::{DynamicImage, Pixel, Rgba, RgbaImage};
use imageproc::drawing::draw_text_mut;
use std::fs;
use std::io::BufWriter;
use std::path::Path;

/// Police embarquee dans le binaire pour que le rendu soit identique sur
/// toutes les plateformes (licence libre, voir assets/DejaVuSans-LICENSE.txt).
static FONT_BYTES: &[u8] = include_bytes!("../assets/DejaVuSans.ttf");

/// Ecart de ratio en dessous duquel on remplit directement l'ecran :
/// le fond flou serait invisible derriere l'image.
const RATIO_TOLERANCE: f32 = 0.03;

/// Compose l'image finale a la taille exacte de l'ecran puis incruste la date
/// et le copyright en bas a droite.
pub fn compose_wallpaper(
    original: &DynamicImage,
    screen_w: u32,
    screen_h: u32,
    fit: FitMode,
    date: &str,
    copyright: Option<&str>,
) -> RgbaImage {
    let screen_w = screen_w.max(640);
    let screen_h = screen_h.max(400);

    let mut canvas = match fit {
        FitMode::CropFill => original
            .resize_to_fill(screen_w, screen_h, FilterType::Lanczos3)
            .to_rgba8(),
        FitMode::BlurFill => blur_fill(original, screen_w, screen_h),
    };

    draw_credits(&mut canvas, date, copyright);
    canvas
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
    let mut background = image::imageops::resize(&blurred, screen_w, screen_h, FilterType::Triangle);
    darken(&mut background, 0.72);

    let foreground = original
        .resize(screen_w, screen_h, FilterType::Lanczos3)
        .to_rgba8();
    let x = (i64::from(screen_w) - i64::from(foreground.width())) / 2;
    let y = (i64::from(screen_h) - i64::from(foreground.height())) / 2;
    image::imageops::overlay(&mut background, &foreground, x, y);
    background
}

/// Assombrit legerement le fond flou pour que l'image nette et le texte
/// incruste restent lisibles quelle que soit la photo.
fn darken(img: &mut RgbaImage, factor: f32) {
    for p in img.pixels_mut() {
        for c in 0..3 {
            p[c] = (f32::from(p[c]) * factor) as u8;
        }
    }
}

/// Incruste la date et le copyright (si present) en bas a droite, en blanc
/// sur un cartouche sombre semi-transparent pour rester lisible sur toute
/// image, claire ou sombre.
fn draw_credits(canvas: &mut RgbaImage, date: &str, copyright: Option<&str>) {
    let font = FontRef::try_from_slice(FONT_BYTES).expect("police embarquee invalide");
    let (w, h) = canvas.dimensions();

    let size = (h as f32 * 0.016).clamp(15.0, 38.0);
    let scale = PxScale::from(size);
    let line_height = size * 1.4;
    let pad = size * 0.6;
    let margin = size * 1.1;

    let mut lines: Vec<String> = Vec::new();
    if let Some(c) = copyright {
        lines.push(truncate_chars(&format!("© {c}"), 80));
    }
    lines.push(date.to_string());

    let widest = lines
        .iter()
        .map(|l| text_width(&font, scale, l))
        .fold(0.0_f32, f32::max);
    let box_w = widest + pad * 2.0;
    let box_h = size * 1.25 + line_height * (lines.len() as f32 - 1.0) + pad * 2.0;

    let x0 = (w as f32 - margin - box_w).max(0.0);
    let y0 = (h as f32 - margin - box_h).max(0.0);

    fill_rect_blend(
        canvas,
        x0 as i32,
        y0 as i32,
        box_w.ceil() as u32,
        box_h.ceil() as u32,
        Rgba([12, 12, 18, 140]),
    );

    for (i, line) in lines.iter().enumerate() {
        let lw = text_width(&font, scale, line);
        let tx = (x0 + box_w - pad - lw) as i32;
        let ty = (y0 + pad + i as f32 * line_height) as i32;
        draw_text_mut(canvas, Rgba([255, 255, 255, 235]), tx, ty, scale, &font, line);
    }
}

fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max.saturating_sub(3)).collect();
    out.push_str("...");
    out
}

fn text_width(font: &impl Font, scale: PxScale, text: &str) -> f32 {
    let scaled = font.as_scaled(scale);
    let mut width = 0.0;
    let mut prev = None;
    for c in text.chars() {
        let id = scaled.glyph_id(c);
        if let Some(p) = prev {
            width += scaled.kern(p, id);
        }
        width += scaled.h_advance(id);
        prev = Some(id);
    }
    width
}

/// Remplit un rectangle en melangeant la couleur (alpha) avec les pixels
/// existants ; imageproc remplace les pixels au lieu de les fusionner.
fn fill_rect_blend(img: &mut RgbaImage, x0: i32, y0: i32, w: u32, h: u32, color: Rgba<u8>) {
    let (iw, ih) = img.dimensions();
    let x_end = (x0 + w as i32).min(iw as i32);
    let y_end = (y0 + h as i32).min(ih as i32);
    for y in y0.max(0)..y_end {
        for x in x0.max(0)..x_end {
            img.get_pixel_mut(x as u32, y as u32).blend(&color);
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
