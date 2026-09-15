//! The GStreamer half of [`crate::video_frame`].
//!
//! GStreamer is what GNOME already plays video with, so the codecs are the
//! system's and nothing is bundled here -- the same bargain AVFoundation makes
//! on macOS, and the same consequence: a machine with no H.264 decoder
//! installed produces no frame, and the day keeps the wallpaper in place.
//!
//! `playbin` does the work rather than a hand-built pipeline. It picks the
//! demuxer and decoder for whatever the file turns out to be, and its
//! `convert-sample` signal hands back one frame already converted to the
//! format asked for, which is the same service Core Graphics performs on the
//! other side.

use gstreamer as gst;
use gstreamer::prelude::*;
use image::RgbaImage;
use std::path::Path;
use std::sync::OnceLock;

/// How long to wait for the pipeline to settle, on opening and after each
/// seek. Bounded because this runs on the blocking pool inside an update a
/// panel action may be queued behind: a file GStreamer cannot make sense of
/// has to fail rather than hang.
const SETTLE: gst::ClockTime = gst::ClockTime::from_seconds(10);

/// A video opened once and asked for several frames.
///
/// Held in `Paused`: prerolled, so the duration is known and a seek lands, but
/// with no clock running and nothing being rendered.
pub struct Decoder {
    playbin: gst::Element,
}

impl Decoder {
    pub fn open(path: &Path) -> Result<Self, String> {
        init()?;

        let uri = glib::filename_to_uri(path, None)
            .map_err(|e| format!("Could not build a file URI for the video: {e}"))?;
        let playbin = gst::ElementFactory::make("playbin")
            .property("uri", uri.as_str())
            .build()
            .map_err(|e| format!("GStreamer has no playbin element: {e}"))?;

        // Nothing is ever played. Both branches end in a sink that discards
        // what reaches it: the pipeline exists to preroll and to be seeked,
        // and the frame is taken with `convert-sample` rather than off a sink.
        // `sync` is off so neither branch waits on a clock that is not running.
        for property in ["audio-sink", "video-sink"] {
            let sink = gst::ElementFactory::make("fakesink")
                .property("sync", false)
                .build()
                .map_err(|e| format!("GStreamer has no fakesink element: {e}"))?;
            playbin.set_property(property, &sink);
        }

        // Videos shot in portrait carry their rotation as a tag rather than in
        // the pixels; `automatic` reads that tag, which is what
        // `setAppliesPreferredTrackTransform` does on macOS. It lives in
        // gst-plugins-good, and a build without it merely loses the rotation,
        // so a missing element is stepped over rather than refused.
        match gst::ElementFactory::make("videoflip")
            .property_from_str("method", "automatic")
            .build()
        {
            Ok(flip) => playbin.set_property("video-filter", &flip),
            Err(e) => eprintln!("video rotation will not be corrected: {e}"),
        }

        let decoder = Decoder { playbin };
        decoder
            .playbin
            .set_state(gst::State::Paused)
            .map_err(|e| format!("Could not open the video: {e}"))?;

        // Prerolling is what makes the duration readable and the first seek
        // possible. Blocking for it is the point of running on the blocking
        // pool.
        decoder
            .playbin
            .state(SETTLE)
            .0
            .map_err(|e| format!("The video did not open in time: {e}"))?;

        Ok(decoder)
    }

    /// Length of the video in seconds, or `None` for a stream that will not
    /// say -- which the caller treats the same as macOS's NaN.
    pub fn duration(&self) -> Option<f64> {
        self.playbin
            .query_duration::<gst::ClockTime>()
            .map(|duration| duration.nseconds() as f64 / 1e9)
    }

    /// Seeks to one instant and converts whatever frame lands there to RGBA.
    pub fn frame_at(&self, seconds: f64) -> Result<RgbaImage, String> {
        let position = gst::ClockTime::from_nseconds((seconds.max(0.0) * 1e9) as u64);

        // `KEY_UNIT` accepts the nearest keyframe instead of decoding forward
        // to the exact instant, which is the same trade the macOS side makes
        // with its half-second tolerance: the probes want a frame from around
        // there, not that precise frame.
        self.playbin
            .seek_simple(gst::SeekFlags::FLUSH | gst::SeekFlags::KEY_UNIT, position)
            .map_err(|e| format!("No frame at {seconds:.1}s: {e}"))?;

        // The seek is asynchronous, and the new frame only exists once the
        // pipeline has prerolled again.
        self.playbin
            .state(SETTLE)
            .0
            .map_err(|e| format!("No frame at {seconds:.1}s: {e}"))?;

        let caps = gst::Caps::builder("video/x-raw")
            .field("format", "RGBA")
            .build();
        let sample = self
            .playbin
            .emit_by_name::<Option<gst::Sample>>("convert-sample", &[&caps])
            .ok_or_else(|| format!("No frame at {seconds:.1}s."))?;

        to_rgba(&sample)
    }
}

impl Drop for Decoder {
    /// A pipeline left in `Paused` holds its decoder, its file handle and its
    /// buffers. This runs once per video APOD, but the buffers are frame-sized
    /// and there is no reason to wait for the process to exit.
    fn drop(&mut self) {
        let _ = self.playbin.set_state(gst::State::Null);
    }
}

/// Copies the sample into a tightly packed RGBA buffer.
///
/// GStreamer pads each row out to an alignment of its choosing, so the rows
/// are copied one at a time: handing the mapped buffer to `image` whole would
/// shear the picture on any width that is not already aligned.
fn to_rgba(sample: &gst::Sample) -> Result<RgbaImage, String> {
    let caps = sample.caps().ok_or("The frame arrived without a format.")?;
    let info = gstreamer_video::VideoInfo::from_caps(caps)
        .map_err(|e| format!("Could not read the frame's format: {e}"))?;
    let buffer = sample.buffer().ok_or("The frame arrived empty.")?;
    let frame = gstreamer_video::VideoFrameRef::from_buffer_ref_readable(buffer, &info)
        .map_err(|e| format!("Could not map the frame: {e}"))?;

    let width = frame.width() as usize;
    let height = frame.height() as usize;
    if width == 0 || height == 0 {
        return Err("The extracted frame has no pixels.".to_string());
    }

    let stride = frame.plane_stride()[0] as usize;
    let row = width
        .checked_mul(4)
        .ok_or("The extracted frame is implausibly large.")?;
    if stride < row {
        return Err("The frame's rows are shorter than its width.".to_string());
    }
    let data = frame
        .plane_data(0)
        .map_err(|e| format!("Could not read the frame's pixels: {e}"))?;

    let mut pixels = Vec::with_capacity(
        row.checked_mul(height)
            .ok_or("The extracted frame is implausibly large.")?,
    );
    for y in 0..height {
        let start = y * stride;
        let end = start + row;
        pixels.extend_from_slice(
            data.get(start..end)
                .ok_or("The frame did not fill its buffer.")?,
        );
    }

    RgbaImage::from_raw(width as u32, height as u32, pixels)
        .ok_or_else(|| "The frame did not fill its buffer.".to_string())
}

/// Starts GStreamer once per process.
///
/// It is only ever needed for a video APOD, a handful of days a year, so it is
/// started on first use rather than at launch: an application that sets a
/// picture has no reason to carry a media framework's registry in memory for
/// the other three hundred and sixty days.
fn init() -> Result<(), String> {
    static STARTED: OnceLock<Result<(), String>> = OnceLock::new();
    STARTED
        .get_or_init(|| gst::init().map_err(|e| format!("Could not start GStreamer: {e}")))
        .clone()
}
