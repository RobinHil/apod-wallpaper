use crate::settings::FitMode;
use image::imageops::FilterType;
use image::{DynamicImage, RgbImage};
use std::fs;
use std::io::BufWriter;
use std::path::Path;

/// Ratio difference below which we fill the screen directly: the blurred
/// backdrop would be invisible behind the image anyway.
const RATIO_TOLERANCE: f32 = 0.03;

/// Composes the final image at the exact screen size. No text is burned in:
/// the wallpaper is the APOD as published, and the metadata (date, copyright)
/// stays visible in the tray and the panel.
pub fn compose_wallpaper(
    original: &DynamicImage,
    screen_w: u32,
    screen_h: u32,
    fit: FitMode,
) -> RgbImage {
    let screen_w = screen_w.max(640);
    let screen_h = screen_h.max(400);

    match fit {
        FitMode::CropFill => original
            .resize_to_fill(screen_w, screen_h, FilterType::Lanczos3)
            .to_rgb8(),
        FitMode::BlurFill => blur_fill(original, screen_w, screen_h),
    }
}

/// Backdrop = image cropped to fill the screen, then blurred and darkened.
/// Foreground = whole image, aspect preserved, centred on top.
fn blur_fill(original: &DynamicImage, screen_w: u32, screen_h: u32) -> RgbImage {
    let img_ratio = original.width() as f32 / original.height().max(1) as f32;
    let screen_ratio = screen_w as f32 / screen_h as f32;

    if ((img_ratio - screen_ratio) / screen_ratio).abs() < RATIO_TOLERANCE {
        return original
            .resize_to_fill(screen_w, screen_h, FilterType::Lanczos3)
            .to_rgb8();
    }

    // Heavy gaussian blur on the cheap: blur a 1/8 scale copy and scale it back
    // up, letting interpolation smooth out the rest.
    let small = original.resize_to_fill(
        (screen_w / 8).max(1),
        (screen_h / 8).max(1),
        FilterType::Triangle,
    );
    let blurred = small.blur(6.0).to_rgb8();
    let mut background =
        image::imageops::resize(&blurred, screen_w, screen_h, FilterType::Triangle);
    darken(&mut background, 0.72);

    let foreground = original
        .resize(screen_w, screen_h, FilterType::Lanczos3)
        .to_rgb8();
    let x = (i64::from(screen_w) - i64::from(foreground.width())) / 2;
    let y = (i64::from(screen_h) - i64::from(foreground.height())) / 2;
    image::imageops::overlay(&mut background, &foreground, x, y);
    background
}

/// Slightly darkens the blurred backdrop so the sharp centred image stands out
/// whatever the photo. Every byte of an RGB buffer is a colour channel, so
/// this walks the samples directly.
fn darken(img: &mut RgbImage, factor: f32) {
    for sample in img.iter_mut() {
        *sample = (f32::from(*sample) * factor) as u8;
    }
}

/// Saves the composition as JPEG (quality 92): far smaller than PNG for
/// photographs, with no visible loss as a wallpaper.
///
/// The buffer is already RGB, so it is handed to the encoder as is. Composing
/// in RGBA used to force a full-size clone plus a conversion here, two extra
/// screen-sized buffers for an alpha channel a wallpaper cannot use.
pub fn save_jpeg(img: &RgbImage, path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Could not create the output directory: {e}"))?;
    }
    let file =
        fs::File::create(path).map_err(|e| format!("Could not create the wallpaper file: {e}"))?;
    let mut writer = BufWriter::new(file);
    let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut writer, 92);
    img.write_with_encoder(encoder)
        .map_err(|e| format!("JPEG encoding failed: {e}"))
}
