use crate::settings::FitMode;
use image::imageops::FilterType;
use image::{DynamicImage, RgbImage};
use std::fs;
use std::io::{BufWriter, Cursor, Write};
use std::path::Path;

/// Decodes an image file, determining the format from its contents rather than
/// from its file name. `image::open` trusts the extension alone, and so trusts
/// whatever the server happened to call the file. Every decode in this app
/// goes through here or through [`decode_bytes`].
pub fn decode(path: &Path) -> Result<DynamicImage, String> {
    image::ImageReader::open(path)
        .map_err(|e| format!("Could not open the image file: {e}"))?
        .with_guessed_format()
        .map_err(|e| format!("Could not read the image file: {e}"))?
        .decode()
        .map_err(|e| format!("Not a usable image: {e}"))
}

/// Decodes a payload still in memory, sniffing the format the same way
/// [`decode`] does. This is what validates a fresh download: a truncated
/// transfer, a non-image body, or an original too large for the decoder's
/// allocation limit all fail here -- before anything has been written to disk,
/// so the caller is still free to fall back to another URL.
pub fn decode_bytes(bytes: &[u8]) -> Result<DynamicImage, String> {
    image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| format!("Could not read the downloaded image: {e}"))?
        .decode()
        .map_err(|e| format!("Not a usable image: {e}"))
}

/// Smallest wallpaper this module will compose, and therefore the floor
/// [`crate::screen_size`] clamps a monitor report to. Declared once because
/// the two must agree: a screen measured below it would otherwise be recorded
/// at one size and composed at another, and every later comparison would see a
/// resolution change that never happened.
pub const MIN_SCREEN: (u32, u32) = (640, 400);

/// Ratio difference below which we fill the screen directly: the blurred
/// backdrop would be invisible behind the image anyway.
const RATIO_TOLERANCE: f32 = 0.03;

/// Filter used to bring the original down to screen size.
///
/// Measured on a 29-megapixel APOD scaled to 2560x1440: Lanczos3 takes 293 ms,
/// CatmullRom 206 ms, and the two results differ by a mean of 0.74 levels out
/// of 255 -- a third of a percent, on an image looked at as a desktop
/// background. The 30% is worth more than the difference.
const DOWNSCALE: FilterType = FilterType::CatmullRom;

/// Composes the final image at the exact screen size. No text is burned in:
/// the wallpaper is the APOD as published, and the metadata (date, copyright)
/// stays visible in the menu bar and the panel.
///
/// Every conversion below uses `into_rgb8` rather than `to_rgb8`: the value is
/// owned and already RGB8, so the latter would copy a screen-sized buffer for
/// nothing.
pub fn compose_wallpaper(
    original: &DynamicImage,
    screen_w: u32,
    screen_h: u32,
    fit: FitMode,
) -> RgbImage {
    let screen_w = screen_w.max(MIN_SCREEN.0);
    let screen_h = screen_h.max(MIN_SCREEN.1);

    match fit {
        FitMode::CropFill => original
            .resize_to_fill(screen_w, screen_h, DOWNSCALE)
            .into_rgb8(),
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
            .resize_to_fill(screen_w, screen_h, DOWNSCALE)
            .into_rgb8();
    }

    // The one and only pass over the full-resolution original.
    let foreground = original.resize(screen_w, screen_h, DOWNSCALE).into_rgb8();

    // The backdrop is built from the foreground, not from the original a
    // second time. It ends up blurred past recognition and darkened to 72%,
    // so the extra detail a 29-megapixel source would carry into a 1/8 scale
    // copy is thrown away either way -- and reading the original twice was
    // costing more than the blur, the upscale and the darkening combined.
    //
    // Heavy gaussian blur on the cheap: blur a 1/8 scale copy and scale it
    // back up, letting interpolation smooth out the rest.
    let small = fill_shrink(&foreground, (screen_w / 8).max(1), (screen_h / 8).max(1));
    let blurred = DynamicImage::ImageRgb8(small).blur(6.0).into_rgb8();
    let mut background =
        image::imageops::resize(&blurred, screen_w, screen_h, FilterType::Triangle);
    darken(&mut background, 0.72);

    let x = (i64::from(screen_w) - i64::from(foreground.width())) / 2;
    let y = (i64::from(screen_h) - i64::from(foreground.height())) / 2;
    image::imageops::overlay(&mut background, &foreground, x, y);
    background
}

/// Shrinks `source` to exactly `w` x `h`, cropping it to that aspect ratio
/// first so the result fills the box instead of being stretched into it.
///
/// This is what `DynamicImage::resize_to_fill` does; it exists because the
/// backdrop is now derived from a buffer we already own, and wrapping that
/// buffer in a `DynamicImage` just to call the method would mean giving up
/// ownership of the foreground we still need to overlay.
fn fill_shrink(source: &RgbImage, w: u32, h: u32) -> RgbImage {
    let (source_w, source_h) = source.dimensions();
    // The larger of the two ratios is the one that makes the crop cover the
    // whole box; the other dimension is then the one with something to spare.
    let scale = (w as f32 / source_w as f32).max(h as f32 / source_h as f32);
    let crop_w = ((w as f32 / scale).round() as u32).clamp(1, source_w);
    let crop_h = ((h as f32 / scale).round() as u32).clamp(1, source_h);
    let view = image::imageops::crop_imm(
        source,
        (source_w - crop_w) / 2,
        (source_h - crop_h) / 2,
        crop_w,
        crop_h,
    );
    image::imageops::resize(&*view, w, h, FilterType::Triangle)
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
///
/// The bytes reach the disk before this returns: the caller renames the file
/// into place straight afterwards, and a rename that outlives its own contents
/// would leave an empty wallpaper behind after a power loss.
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
        .map_err(|e| format!("JPEG encoding failed: {e}"))?;
    writer
        .flush()
        .map_err(|e| format!("Could not flush the wallpaper file: {e}"))?;
    writer
        .into_inner()
        .map_err(|e| format!("Could not flush the wallpaper file: {e}"))?
        .sync_all()
        .map_err(|e| format!("Could not commit the wallpaper file to disk: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_files_without_an_extension() {
        // Stored originals are named from a content sniff, so the decoder must
        // never depend on the file name to agree with the payload.
        let dir = std::env::temp_dir().join("apod-wallpaper-decode-test");
        let _ = fs::remove_dir_all(&dir);
        let path = dir.join("no-extension-here");

        save_jpeg(&RgbImage::from_pixel(8, 4, image::Rgb([10, 20, 30])), &path).unwrap();
        let decoded = decode(&path).expect("a JPEG with no file extension must decode");

        assert_eq!((decoded.width(), decoded.height()), (8, 4));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_backdrop_is_cropped_to_fill_not_stretched() {
        // 200x50 down to 10x10: only the central 50x50 square should be
        // sampled. Painting everything outside it red is what tells a crop
        // apart from a stretch, which would drag the red in.
        let mut source = RgbImage::from_pixel(200, 50, image::Rgb([255, 0, 0]));
        for x in 75..125 {
            for y in 0..50 {
                source.put_pixel(x, y, image::Rgb([0, 0, 255]));
            }
        }

        let out = fill_shrink(&source, 10, 10);

        assert_eq!(out.dimensions(), (10, 10));
        for pixel in out.pixels() {
            assert_eq!(pixel.0[0], 0, "red bled in: the crop was a stretch");
            assert_eq!(pixel.0[2], 255);
        }
    }

    #[test]
    fn fill_shrink_hits_the_requested_size_for_any_ratio() {
        for (w, h) in [(640u32, 480u32), (50, 1000), (1000, 50), (7, 3)] {
            let source = RgbImage::from_pixel(w, h, image::Rgb([9, 9, 9]));
            assert_eq!(fill_shrink(&source, 320, 180).dimensions(), (320, 180));
        }
    }

    #[test]
    fn decodes_a_payload_still_in_memory() {
        let dir = std::env::temp_dir().join("apod-wallpaper-decode-bytes-test");
        let _ = fs::remove_dir_all(&dir);
        let path = dir.join("source.jpg");
        save_jpeg(&RgbImage::from_pixel(6, 3, image::Rgb([1, 2, 3])), &path).unwrap();

        let bytes = fs::read(&path).unwrap();
        let decoded = decode_bytes(&bytes).expect("a JPEG payload must decode from memory");
        assert_eq!((decoded.width(), decoded.height()), (6, 3));

        // The validation the download path relies on: garbage is rejected here
        // rather than after it has been installed.
        assert!(decode_bytes(b"<html>404 not found</html>").is_err());
        assert!(decode_bytes(&bytes[..bytes.len() / 2]).is_err());

        let _ = fs::remove_dir_all(&dir);
    }
}
