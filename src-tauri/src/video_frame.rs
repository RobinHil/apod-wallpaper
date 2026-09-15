//! A still frame out of a video file, so a video APOD can still become a
//! wallpaper.
//!
//! Most video APODs are YouTube or Vimeo embeds, and for those the API hands
//! back a thumbnail. Some are served as a plain file instead -- an `.mp4` on
//! apod.nasa.gov -- and the API has no thumbnail for those at all: asked for
//! one it answers with an empty string. Those days used to be skipped.
//!
//! Decoding is done by whatever the desktop already ships -- AVFoundation on
//! macOS, GStreamer on GNOME -- so the frame costs the app no bundled decoder
//! and the user nothing to install. The formats that work are the ones the
//! system itself can play, on both, which is a limitation worth stating rather
//! than papering over: a distribution that ships no H.264 decoder will not
//! produce a frame, and the day keeps the wallpaper already in place.
//!
//! Choosing *which* frame is the same problem everywhere, so it lives here.
//! Only opening the file and decoding one instant differ, and that is the
//! whole of the per-platform backend.

use image::RgbaImage;
use std::path::Path;

#[cfg_attr(target_os = "macos", path = "video_frame_macos.rs")]
#[cfg_attr(target_os = "linux", path = "video_frame_gnome.rs")]
mod backend;

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
    let decoder = backend::Decoder::open(path)?;

    // An invalid or indefinite duration reads back as `None` on GNOME and as
    // NaN on macOS; the backend normalises both away, and either way one
    // instant near the start is the best guess left.
    let instants: Vec<f64> = match decoder.duration() {
        Some(duration) if duration.is_finite() && duration > 0.0 => {
            PROBES.iter().map(|fraction| fraction * duration).collect()
        }
        _ => vec![FALLBACK_SECONDS],
    };

    let mut best: Option<(f64, RgbaImage)> = None;
    let mut last_error = String::from("The video yielded no frame.");

    for seconds in instants {
        match decoder.frame_at(seconds) {
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

    #[test]
    fn the_probes_stay_inside_the_video() {
        // Each probe is a fraction of the duration, so one at or past 1.0
        // would seek past the end and the backend would report no frame --
        // turning a video that decodes perfectly well into a skipped day.
        assert!(PROBES.iter().all(|f| *f > 0.0 && *f < 1.0));
        // In order, because the search stops at the first frame that clears
        // the bar and "earliest usable frame" is the intent.
        assert!(PROBES.windows(2).all(|pair| pair[0] < pair[1]));
    }
}
