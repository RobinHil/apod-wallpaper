//! A still frame out of a video file, so a video APOD can still become a
//! wallpaper.
//!
//! Most video APODs are YouTube or Vimeo embeds, and for those the API hands
//! back a thumbnail. Some are served as a plain file instead -- an `.mp4` on
//! apod.nasa.gov -- and the API has no thumbnail for those at all: asked for
//! one it answers with an empty string. Those days used to be skipped.
//!
//! Decoding is done by AVFoundation, which is part of macOS. It is linked the
//! same way AppKit already is for setting the desktop picture, so the frame
//! costs the app no bundled decoder, and the user nothing to install: the
//! container and codec support is whatever the system itself can play.

use image::RgbaImage;
use objc2_av_foundation::{AVAssetImageGenerator, AVURLAsset};
use objc2_core_foundation::{CGPoint, CGRect, CGSize};
use objc2_core_graphics::{
    CGBitmapContextCreate, CGColorSpace, CGContext, CGImage, CGImageAlphaInfo,
};
use objc2_core_media::CMTime;
use objc2_foundation::{NSString, NSURL};
use std::path::Path;

/// Timescale for the instants we ask for, in ticks per second. Frame-accurate
/// for any sane frame rate, and far below what a `i64` tick count could
/// overflow for a video of any plausible length.
const TIMESCALE: i32 = 600;

/// Where in the video to look, as fractions of its duration.
///
/// Not the first frame: an APOD video often opens on black, on a fade-in, or
/// on a title card, none of which make a wallpaper. These are tried in order
/// and the search stops at the first frame that clears [`GOOD_ENOUGH`], so the
/// common case costs a single decode.
const PROBES: [f64; 4] = [0.15, 0.4, 0.65, 0.85];

/// Contrast score above which a frame is taken as the wallpaper without
/// looking at the remaining probes. Well below what any real photograph
/// scores; the point is only to walk past frames that are flat black, flat
/// white, or a solid title card.
const GOOD_ENOUGH: f64 = 12.0;

/// Instant used when the duration cannot be read. A video too short for it
/// still yields its last frame rather than an error.
const FALLBACK_SECONDS: f64 = 1.0;

/// Pulls the most usable still frame out of a video file.
///
/// Blocking and CPU-bound: the caller runs it on the blocking pool, like every
/// other decode in the app.
pub fn extract(path: &Path) -> Result<RgbaImage, String> {
    let generator = unsafe {
        let url = NSURL::fileURLWithPath(&NSString::from_str(
            path.to_str().ok_or("The video path is not valid UTF-8.")?,
        ));
        let asset = AVURLAsset::URLAssetWithURL_options(&url, None);
        let generator = AVAssetImageGenerator::assetImageGeneratorWithAsset(&asset);
        // Videos shot in portrait carry their rotation as track metadata; without
        // this the frame comes out on its side.
        generator.setAppliesPreferredTrackTransform(true);
        // The default tolerance is infinite, which lets the generator answer
        // with whatever keyframe it likes -- including one far from the
        // instant asked for, which defeats the probes below. Half a second is
        // still loose enough to avoid decoding a long run of frames.
        let half = CMTime::with_seconds(0.5, TIMESCALE);
        generator.setRequestedTimeToleranceBefore(half);
        generator.setRequestedTimeToleranceAfter(half);
        (generator, asset)
    };
    let (generator, asset) = generator;

    // An invalid or indefinite duration reads back as NaN.
    let duration = unsafe { asset.duration().seconds() };
    let instants: Vec<f64> = if duration.is_finite() && duration > 0.0 {
        PROBES.iter().map(|f| f * duration).collect()
    } else {
        vec![FALLBACK_SECONDS]
    };

    let mut best: Option<(f64, RgbaImage)> = None;
    let mut last_error = String::from("The video yielded no frame.");

    for seconds in instants {
        match frame_at(&generator, seconds) {
            Ok(frame) => {
                let score = contrast(&frame);
                if score >= GOOD_ENOUGH {
                    return Ok(frame);
                }
                if best.as_ref().is_none_or(|(b, _)| score > *b) {
                    best = Some((score, frame));
                }
            }
            Err(e) => last_error = e,
        }
    }

    // Every probe was flat, so the video really does look like that: the least
    // flat of them is still the best wallpaper available.
    best.map(|(_, frame)| frame).ok_or(last_error)
}

/// Decodes the frame at one instant and converts it to RGBA.
fn frame_at(generator: &AVAssetImageGenerator, seconds: f64) -> Result<RgbaImage, String> {
    let time = unsafe { CMTime::with_seconds(seconds, TIMESCALE) };
    // The asynchronous variant that replaced this one takes a completion
    // block, which buys nothing here: this already runs on the blocking pool,
    // where waiting is the point.
    #[allow(deprecated)]
    let image = unsafe { generator.copyCGImageAtTime_actualTime_error(time, std::ptr::null_mut()) }
        .map_err(|e| format!("No frame at {seconds:.1}s: {e}"))?;
    to_rgba(&image)
}

/// Redraws a `CGImage` into a buffer whose layout we chose, which is the point:
/// the frame arrives in whatever pixel format the decoder produced, and Core
/// Graphics converts colour space, alpha and byte order on the way in.
fn to_rgba(image: &CGImage) -> Result<RgbaImage, String> {
    let width = CGImage::width(Some(image));
    let height = CGImage::height(Some(image));
    if width == 0 || height == 0 {
        return Err("The extracted frame has no pixels.".to_string());
    }
    let stride = width
        .checked_mul(4)
        .and_then(|row| row.checked_mul(height))
        .ok_or("The extracted frame is implausibly large.")?;

    let mut pixels = vec![0u8; stride];
    let space = CGColorSpace::new_device_rgb().ok_or("No RGB colour space available.")?;
    let context = unsafe {
        CGBitmapContextCreate(
            pixels.as_mut_ptr().cast(),
            width,
            height,
            8,
            width * 4,
            Some(&space),
            CGImageAlphaInfo::PremultipliedLast.0,
        )
    }
    .ok_or("Could not allocate a bitmap for the frame.")?;

    CGContext::draw_image(
        Some(&context),
        CGRect {
            origin: CGPoint { x: 0.0, y: 0.0 },
            size: CGSize {
                width: width as f64,
                height: height as f64,
            },
        },
        Some(image),
    );
    CGContext::flush(Some(&context));
    // The context borrows `pixels` and writes through that pointer; dropping it
    // here ends the borrow before the buffer is handed to `image`.
    drop(context);

    RgbaImage::from_raw(width as u32, height as u32, pixels)
        .ok_or_else(|| "The frame did not fill its buffer.".to_string())
}

/// Standard deviation of luminance, as a stand-in for "is there a picture
/// here". A frame that is entirely black, white, or one flat colour scores
/// zero however bright it is, which is what separates a fade-in from a
/// photograph of the night sky.
///
/// Sampled rather than measured: a few thousand pixels settle the question,
/// and this runs once per probe.
fn contrast(frame: &RgbaImage) -> f64 {
    const SAMPLES: usize = 4096;
    let pixels = frame.as_raw();
    let count = pixels.len() / 4;
    if count == 0 {
        return 0.0;
    }
    let step = (count / SAMPLES).max(1);

    let mut n = 0.0;
    let mut sum = 0.0;
    let mut sum_squares = 0.0;
    for i in (0..count).step_by(step) {
        let p = &pixels[i * 4..];
        // Rec. 601 luma, the usual cheap approximation.
        let luma = 0.299 * p[0] as f64 + 0.587 * p[1] as f64 + 0.114 * p[2] as f64;
        n += 1.0;
        sum += luma;
        sum_squares += luma * luma;
    }
    let mean = sum / n;
    (sum_squares / n - mean * mean).max(0.0).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    fn flat(luma: u8) -> RgbaImage {
        RgbaImage::from_pixel(64, 64, Rgba([luma, luma, luma, 255]))
    }

    #[test]
    fn a_flat_frame_has_no_contrast() {
        assert!(contrast(&flat(0)) < 0.01, "black");
        assert!(contrast(&flat(255)) < 0.01, "white");
        assert!(contrast(&flat(128)) < 0.01, "mid grey");
    }

    #[test]
    fn a_frame_with_a_picture_in_it_clears_the_bar() {
        // Half black, half white: the loudest thing a frame can be.
        let mut frame = flat(0);
        for y in 0..32 {
            for x in 0..64 {
                frame.put_pixel(x, y, Rgba([255, 255, 255, 255]));
            }
        }
        assert!(
            contrast(&frame) >= GOOD_ENOUGH,
            "scored {}",
            contrast(&frame)
        );
    }
}
