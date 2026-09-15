//! The AVFoundation half of [`crate::video_frame`].
//!
//! AVFoundation is part of macOS and is linked the same way AppKit already is
//! for setting the desktop picture, so this adds nothing for anyone to
//! install: the container and codec support is whatever the system can play.

use image::RgbaImage;
use objc2::rc::Retained;
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

/// A video opened once and asked for several frames.
///
/// The asset is held alongside the generator it was built from: the generator
/// is what decodes, and keeping the asset next to it makes the lifetime
/// obvious rather than relying on what the generator retains internally.
pub struct Decoder {
    generator: Retained<AVAssetImageGenerator>,
    asset: Retained<AVURLAsset>,
}

impl Decoder {
    pub fn open(path: &Path) -> Result<Self, String> {
        let (generator, asset) = unsafe {
            let url = NSURL::fileURLWithPath(&NSString::from_str(
                path.to_str().ok_or("The video path is not valid UTF-8.")?,
            ));
            let asset = AVURLAsset::URLAssetWithURL_options(&url, None);
            let generator = AVAssetImageGenerator::assetImageGeneratorWithAsset(&asset);
            // Videos shot in portrait carry their rotation as track metadata;
            // without this the frame comes out on its side.
            generator.setAppliesPreferredTrackTransform(true);
            // The default tolerance is infinite, which lets the generator
            // answer with whatever keyframe it likes -- including one far from
            // the instant asked for, which defeats the probes. Half a second
            // is still loose enough to avoid decoding a long run of frames.
            let half = CMTime::with_seconds(0.5, TIMESCALE);
            generator.setRequestedTimeToleranceBefore(half);
            generator.setRequestedTimeToleranceAfter(half);
            (generator, asset)
        };

        Ok(Decoder { generator, asset })
    }

    /// Length of the video in seconds. An invalid or indefinite duration reads
    /// back as NaN, which the caller treats as "unknown".
    pub fn duration(&self) -> Option<f64> {
        Some(unsafe { self.asset.duration().seconds() })
    }

    /// Decodes the frame at one instant and converts it to RGBA.
    pub fn frame_at(&self, seconds: f64) -> Result<RgbaImage, String> {
        let time = unsafe { CMTime::with_seconds(seconds, TIMESCALE) };
        // The asynchronous variant that replaced this one takes a completion
        // block, which buys nothing here: this already runs on the blocking
        // pool, where waiting is the point.
        #[allow(deprecated)]
        let image = unsafe {
            self.generator
                .copyCGImageAtTime_actualTime_error(time, std::ptr::null_mut())
        }
        .map_err(|e| format!("No frame at {seconds:.1}s: {e}"))?;
        to_rgba(&image)
    }
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
