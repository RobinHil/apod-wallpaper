use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::Duration;

const ENDPOINT: &str = "https://api.nasa.gov/planetary/apod";

/// Deadline for the metadata request. Much shorter than the client-wide
/// timeout, which exists for image downloads: this one request is a couple of
/// kilobytes, and every panel action waits behind it.
const METADATA_TIMEOUT: Duration = Duration::from_secs(30);

/// Ceiling on anything downloaded: an image, or a video to take a frame out
/// of. The largest APOD originals sit an order of magnitude below this, and
/// its videos are tens of megabytes; the cap only exists so a misbehaving
/// endpoint cannot make the app buffer an unbounded body in memory.
const MAX_DOWNLOAD_BYTES: usize = 128 * 1024 * 1024;

/// reqwest appends `" for url (...)"` to most of its error messages, and our
/// request URL carries the user's API key as a query parameter. Every error
/// here ends up in the panel, so the URL is stripped before the message is
/// ever read.
fn network(e: reqwest::Error) -> ApiError {
    ApiError::Network(e.without_url().to_string())
}

/// APOD API response. `copyright` is absent for public-domain images produced
/// by NASA; when present it must be preserved and shown everywhere (store,
/// menu bar, panel).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Apod {
    pub date: String,
    pub title: String,
    pub explanation: String,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub hdurl: Option<String>,
    pub media_type: String,
    #[serde(default)]
    pub copyright: Option<String>,
    #[serde(default)]
    pub thumbnail_url: Option<String>,
}

impl Apod {
    pub fn is_video(&self) -> bool {
        self.media_type == "video"
    }

    /// True when some wallpaper can be produced for this day.
    pub fn has_wallpaper(&self) -> bool {
        !self.sources().is_empty()
    }

    /// Everything worth downloading for this APOD, best first. The caller
    /// walks the list and keeps the first one that yields a usable image, so
    /// every entry is a fallback for the ones before it.
    ///
    /// A video is never a wallpaper by itself, and reaches this list one of
    /// two ways. YouTube and Vimeo APODs are embeds, and the API hands back a
    /// thumbnail for those. The ones served as a plain file carry no thumbnail
    /// at all -- asked for one the API answers with an empty string -- so the
    /// file itself is downloaded and a frame taken out of it.
    pub fn sources(&self) -> Vec<Source> {
        let mut sources = Vec::new();
        let thumb = self.thumbnail_url.as_deref();
        match self.media_type.as_str() {
            "image" => {
                push(&mut sources, self.hdurl.clone().map(Source::Image));
                push(&mut sources, self.url.clone().map(Source::Image));
            }
            "video" => {
                // Each larger size is a guess -- YouTube does not generate all
                // of them for every video -- so they are tried biggest first
                // and the thumbnail as published stays the last resort.
                for size in YOUTUBE_SIZES {
                    push(
                        &mut sources,
                        thumb.and_then(|t| youtube_size(t, size)).map(Source::Image),
                    );
                }
                push(&mut sources, thumb.map(str::to_string).map(Source::Image));
                push(&mut sources, self.video_file());
            }
            // An unfamiliar media_type is not a reason to skip the day: what
            // the URL points at decides instead, and a download that turns out
            // to be neither an image nor a video fails the way a corrupt one
            // does.
            _ => {
                push(&mut sources, self.video_file());
                push(&mut sources, self.url.clone().map(Source::Image));
            }
        }
        sources
    }

    /// The APOD as a video file, when that is what `url` points at. An embed
    /// link is not one: nothing can be decoded from a YouTube page.
    fn video_file(&self) -> Option<Source> {
        self.url
            .clone()
            .filter(|u| is_video_file(u))
            .map(Source::Video)
    }

    /// Repairs the two ways the API states a field loosely: URLs it reports
    /// as present but empty, and copyright lines carrying stray newlines.
    fn normalize(mut self) -> Self {
        blank_to_none(&mut self.url);
        blank_to_none(&mut self.hdurl);
        blank_to_none(&mut self.thumbnail_url);
        if let Some(c) = self.copyright.take() {
            let cleaned = c.split_whitespace().collect::<Vec<_>>().join(" ");
            if !cleaned.is_empty() {
                self.copyright = Some(cleaned);
            }
        }
        self
    }
}

/// One candidate download, and what has to be done with it to get a wallpaper.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// A still image, usable once decoded.
    Image(String),
    /// A video file, usable once a frame has been taken out of it.
    Video(String),
}

impl Source {
    pub fn url(&self) -> &str {
        match self {
            Source::Image(u) | Source::Video(u) => u,
        }
    }
}

/// Appends a candidate, skipping the ones that are absent or already listed.
/// The duplicates are real: an APOD whose `hdurl` and `url` are the same file
/// would otherwise be downloaded twice before the day was declared a failure.
fn push(sources: &mut Vec<Source>, candidate: Option<Source>) {
    if let Some(source) = candidate
        && !sources.contains(&source)
    {
        sources.push(source);
    }
}

/// Containers the system decoder opens, and that APOD actually publishes.
/// Matched on the URL rather than on `media_type`, which says "video" for an
/// embed link just as it does for a file.
const VIDEO_EXTENSIONS: [&str; 5] = ["mp4", "mov", "m4v", "m2v", "mpg"];

pub fn is_video_file(url: &str) -> bool {
    // A query string or fragment is not part of the file name.
    let path = url.split(['?', '#']).next().unwrap_or(url);
    let name = path.rsplit('/').next().unwrap_or(path);
    match name.rsplit_once('.') {
        Some((_, ext)) => VIDEO_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()),
        None => false,
    }
}

/// An absent field is not always `null`: asked for thumbnails, the API answers
/// `"thumbnail_url": ""` for a video it cannot thumbnail. An empty string is
/// not a URL, and deserializes to `Some("")`, which every `is_some()` check
/// here would read as a usable image. Left alone it reaches reqwest, which
/// rejects it before sending anything as a "builder error" -- a plain
/// `reqwest::Error`, indistinguishable from a connection failure, which would
/// put the app in offline mode over a response that arrived perfectly well.
fn blank_to_none(field: &mut Option<String>) {
    if field.as_deref().is_some_and(|s| s.trim().is_empty()) {
        *field = None;
    }
}

/// Larger copies of the published thumbnail to try first, biggest last resort
/// last. YouTube stores one picture at several sizes, and does not generate
/// the big ones for every video: `maxresdefault` (1280x720) is missing for
/// older or low-definition uploads, and `sddefault` (640x480) is the next size
/// down. It is the same picture either way -- the point is only to start from
/// more pixels, since the wallpaper is composed at screen size and whatever is
/// missing has to be invented by upscaling.
const YOUTUBE_SIZES: [&str; 2] = ["maxresdefault", "sddefault"];

/// Rewrites a standard YouTube thumbnail URL (0.jpg, hqdefault.jpg, ...) to
/// another size. Returns None when the URL is not a known YouTube thumbnail,
/// or already is the size asked for, in which case there is nothing to add
/// that the published URL does not already cover.
fn youtube_size(url: &str, size: &str) -> Option<String> {
    if !url.contains("img.youtube.com") && !url.contains("ytimg.com") {
        return None;
    }
    let (head, tail) = url.rsplit_once('/')?;
    let name = tail.strip_suffix(".jpg")?;
    const KNOWN: [&str; 8] = [
        "0",
        "1",
        "2",
        "3",
        "default",
        "mqdefault",
        "hqdefault",
        "sddefault",
    ];
    if KNOWN.contains(&name) && name != size {
        Some(format!("{head}/{size}.jpg"))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewrites_standard_youtube_thumbnails_to_another_size() {
        assert_eq!(
            youtube_size("https://img.youtube.com/vi/abc123/0.jpg", "maxresdefault").as_deref(),
            Some("https://img.youtube.com/vi/abc123/maxresdefault.jpg")
        );
        assert_eq!(
            youtube_size("https://i.ytimg.com/vi/abc123/hqdefault.jpg", "sddefault").as_deref(),
            Some("https://i.ytimg.com/vi/abc123/sddefault.jpg")
        );
    }

    #[test]
    fn leaves_other_urls_untouched() {
        assert_eq!(
            youtube_size("https://vimeo.com/thumb/42.jpg", "sddefault"),
            None
        );
        assert_eq!(
            youtube_size(
                "https://img.youtube.com/vi/abc123/maxresdefault.jpg",
                "maxresdefault"
            ),
            None
        );
        assert_eq!(
            youtube_size("https://img.youtube.com/vi/abc123/0.png", "sddefault"),
            None
        );
    }

    /// A thumbnail already published at one of the sizes we would ask for must
    /// not be listed twice.
    #[test]
    fn a_size_the_thumbnail_already_is_adds_nothing() {
        assert_eq!(
            youtube_size(
                "https://img.youtube.com/vi/abc123/sddefault.jpg",
                "sddefault"
            ),
            None
        );
    }

    fn parse(json: &str) -> Apod {
        serde_json::from_str::<Apod>(json)
            .expect("the fixture must deserialize")
            .normalize()
    }

    /// The APOD of 2026-08-19, verbatim: a video APOD served as an `.mp4` of
    /// its own rather than as an embed. The API has no thumbnail for it and
    /// says so with an empty string instead of omitting the field.
    #[test]
    fn a_self_hosted_video_is_downloaded_for_its_frame() {
        let apod = parse(
            r#"{
                "date": "2026-08-19",
                "title": "The Case of the Mysterious Maybe Meteor",
                "explanation": "Whatdunit?",
                "media_type": "video",
                "service_version": "v1",
                "thumbnail_url": "",
                "url": "https://apod.nasa.gov/apod/image/2608/perseids_eclipse_mystery.mp4"
            }"#,
        );

        // The empty thumbnail must not survive as a URL to download.
        assert_eq!(apod.thumbnail_url, None);
        assert!(apod.has_wallpaper());
        assert_eq!(
            apod.sources(),
            vec![Source::Video(
                "https://apod.nasa.gov/apod/image/2608/perseids_eclipse_mystery.mp4".to_string()
            )]
        );
    }

    /// An embed link is not a file: there is nothing to decode at the end of
    /// it, so a video with no thumbnail and no file yields no wallpaper.
    #[test]
    fn an_embed_with_no_thumbnail_yields_nothing() {
        let apod = parse(
            r#"{
                "date": "2026-01-01",
                "title": "Somewhere",
                "explanation": "...",
                "media_type": "video",
                "thumbnail_url": "",
                "url": "https://www.youtube.com/embed/abc123"
            }"#,
        );

        assert!(!apod.has_wallpaper());
        assert_eq!(apod.sources(), vec![]);
    }

    /// The 2026-03-12 entry is the case this matters for: YouTube has no
    /// `maxresdefault` for that video, so without `sddefault` in between the
    /// app drops straight from 1280x720 to the published 480x360.
    #[test]
    fn a_youtube_video_tries_the_larger_sizes_before_the_published_one() {
        let apod = parse(
            r#"{
                "date": "2026-01-02",
                "title": "Somewhere",
                "explanation": "...",
                "media_type": "video",
                "thumbnail_url": "https://img.youtube.com/vi/abc123/0.jpg",
                "url": "https://www.youtube.com/embed/abc123"
            }"#,
        );

        assert_eq!(
            apod.sources(),
            vec![
                Source::Image("https://img.youtube.com/vi/abc123/maxresdefault.jpg".to_string()),
                Source::Image("https://img.youtube.com/vi/abc123/sddefault.jpg".to_string()),
                Source::Image("https://img.youtube.com/vi/abc123/0.jpg".to_string()),
            ]
        );
    }

    #[test]
    fn a_blank_hdurl_leaves_only_the_standard_url() {
        let apod = parse(
            r#"{
                "date": "2026-08-18",
                "title": "Somewhere",
                "explanation": "...",
                "media_type": "image",
                "hdurl": "",
                "url": "https://apod.nasa.gov/apod/image/2608/some.jpg"
            }"#,
        );

        assert_eq!(
            apod.sources(),
            vec![Source::Image(
                "https://apod.nasa.gov/apod/image/2608/some.jpg".to_string()
            )]
        );
    }

    /// The same file under both keys is one download, not two.
    #[test]
    fn an_image_listed_twice_is_only_downloaded_once() {
        let apod = parse(
            r#"{
                "date": "2026-08-17",
                "title": "Somewhere",
                "explanation": "...",
                "media_type": "image",
                "hdurl": "https://apod.nasa.gov/apod/image/2608/some.jpg",
                "url": "https://apod.nasa.gov/apod/image/2608/some.jpg"
            }"#,
        );

        assert_eq!(apod.sources().len(), 1);
    }

    /// A media type we have never seen is judged on its URL rather than
    /// skipped, so a new one does not silently cost a day.
    #[test]
    fn an_unknown_media_type_falls_back_to_what_the_url_looks_like() {
        let apod = parse(
            r#"{
                "date": "2026-08-20",
                "title": "Somewhere",
                "explanation": "...",
                "media_type": "interactive",
                "url": "https://apod.nasa.gov/apod/image/2608/thing.mov"
            }"#,
        );

        assert_eq!(
            apod.sources().first(),
            Some(&Source::Video(
                "https://apod.nasa.gov/apod/image/2608/thing.mov".to_string()
            ))
        );
    }

    #[test]
    fn video_files_are_told_apart_from_embed_links() {
        assert!(is_video_file("https://apod.nasa.gov/apod/image/2608/a.mp4"));
        assert!(is_video_file("https://apod.nasa.gov/apod/image/2608/A.MOV"));
        assert!(is_video_file("https://example.com/a.mp4?v=2#t=10"));
        assert!(!is_video_file("https://www.youtube.com/embed/abc123"));
        assert!(!is_video_file("https://player.vimeo.com/video/12345"));
        assert!(!is_video_file(
            "https://apod.nasa.gov/apod/image/2608/a.jpg"
        ));
        // A dot in a directory name is not an extension.
        assert!(!is_video_file("https://example.com/v1.2/watch"));
    }

    /// Port 1 on the loopback: nothing listens there, so both requests below
    /// fail locally and immediately, and reqwest attaches the request URL --
    /// query string included -- to the error it hands back.
    const REFUSED: &str = "http://127.0.0.1:1";

    #[tokio::test]
    async fn api_errors_never_carry_the_api_key() {
        let client = reqwest::Client::new();
        let error = fetch_apod_from(
            &client,
            &format!("{REFUSED}/planetary/apod"),
            "SECRET-KEY",
            None,
        )
        .await
        .expect_err("a refused connection must produce an error");

        let message = error.to_string();
        assert!(!message.contains("SECRET-KEY"), "leaked the key: {message}");
        assert!(
            !message.contains("api_key"),
            "leaked the query string: {message}"
        );
    }

    #[tokio::test]
    async fn download_errors_never_carry_the_url() {
        let client = reqwest::Client::new();
        let error = download(&client, &format!("{REFUSED}/i.jpg?token=SECRET-TOKEN"))
            .await
            .expect_err("a refused connection must produce an error");

        let message = error.to_string();
        assert!(!message.contains("SECRET-TOKEN"), "leaked: {message}");
    }
}

#[derive(Debug)]
pub enum ApiError {
    /// Network error: no connection, DNS, timeout... treated as offline.
    Network(String),
    /// API key quota exceeded (HTTP 429).
    RateLimited,
    /// No APOD for the requested date (the archive has a few gaps).
    NotFound,
    /// Any other HTTP error.
    Http(u16),
    /// Unreadable response.
    Parse(String),
    /// Body beyond what we are willing to buffer.
    TooLarge,
}

impl ApiError {
    /// True when the error is a temporary outage that warrants retrying in the
    /// background rather than reporting a hard failure.
    pub fn is_offline(&self) -> bool {
        matches!(
            self,
            ApiError::Network(_) | ApiError::RateLimited | ApiError::Http(500..=599)
        )
    }
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ApiError::Network(e) => write!(f, "Network error: {e}"),
            ApiError::RateLimited => write!(
                f,
                "API key quota exceeded (DEMO_KEY allows 30 requests/hour). Will retry automatically."
            ),
            ApiError::NotFound => write!(
                f,
                "No APOD was published on that date (the archive has a few days with no entry)."
            ),
            ApiError::Http(code) => write!(f, "HTTP error {code} from the NASA API."),
            ApiError::Parse(e) => write!(f, "Unreadable API response: {e}"),
            ApiError::TooLarge => write!(
                f,
                "The file is larger than {} MB and was not downloaded.",
                MAX_DOWNLOAD_BYTES / (1024 * 1024)
            ),
        }
    }
}

/// Queries the APOD API. `date: None` returns the most recently published
/// image, which avoids time-zone mistakes: the API decides what "today" is
/// rather than the local clock.
pub async fn fetch_apod(
    client: &reqwest::Client,
    api_key: &str,
    date: Option<NaiveDate>,
) -> Result<Apod, ApiError> {
    fetch_apod_from(client, ENDPOINT, api_key, date).await
}

/// The body of [`fetch_apod`], with the endpoint injected so the tests can
/// point it at an address that fails and inspect the resulting message.
async fn fetch_apod_from(
    client: &reqwest::Client,
    endpoint: &str,
    api_key: &str,
    date: Option<NaiveDate>,
) -> Result<Apod, ApiError> {
    // thumbs=true asks the API to include video thumbnails, the only usable
    // wallpaper representation of a video (no video file is served).
    let mut request = client
        .get(endpoint)
        .timeout(METADATA_TIMEOUT)
        .query(&[("api_key", api_key), ("thumbs", "true")]);
    if let Some(d) = date {
        request = request.query(&[("date", d.format("%Y-%m-%d").to_string())]);
    }

    let response = request.send().await.map_err(network)?;

    match response.status().as_u16() {
        200 => response
            .json::<Apod>()
            .await
            .map(Apod::normalize)
            .map_err(|e| ApiError::Parse(e.without_url().to_string())),
        429 => Err(ApiError::RateLimited),
        // The API answers 404 or 400 for dates with no publication.
        400 | 404 => Err(ApiError::NotFound),
        code => Err(ApiError::Http(code)),
    }
}

/// Downloads the raw bytes of one [`Source`] -- an image, or a video a frame
/// is taken out of -- refusing a body over [`MAX_DOWNLOAD_BYTES`]. The body is
/// read chunk by chunk rather than in one call so an endpoint that lies about
/// (or omits) its content length still cannot grow the buffer without bound.
pub async fn download(client: &reqwest::Client, url: &str) -> Result<Vec<u8>, ApiError> {
    let mut response = client.get(url).send().await.map_err(network)?;

    let status = response.status();
    if !status.is_success() {
        return Err(ApiError::Http(status.as_u16()));
    }

    let declared = response.content_length();
    if declared.is_some_and(|n| n > MAX_DOWNLOAD_BYTES as u64) {
        return Err(ApiError::TooLarge);
    }

    // Trust the declared length only as an allocation hint, and only up to a
    // size worth reserving up front.
    let mut bytes = Vec::with_capacity(declared.unwrap_or(0).min(16 * 1024 * 1024) as usize);
    while let Some(chunk) = response.chunk().await.map_err(network)? {
        if bytes.len() + chunk.len() > MAX_DOWNLOAD_BYTES {
            return Err(ApiError::TooLarge);
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}
