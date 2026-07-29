use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::Duration;

const ENDPOINT: &str = "https://api.nasa.gov/planetary/apod";

/// Deadline for the metadata request. Much shorter than the client-wide
/// timeout, which exists for image downloads: this one request is a couple of
/// kilobytes, and every panel action waits behind it.
const METADATA_TIMEOUT: Duration = Duration::from_secs(30);

/// Ceiling on a downloaded image. The largest APOD originals sit an order of
/// magnitude below this; the cap only exists so a misbehaving endpoint cannot
/// make the app buffer an unbounded body in memory.
const MAX_IMAGE_BYTES: usize = 128 * 1024 * 1024;

/// reqwest appends `" for url (...)"` to most of its error messages, and our
/// request URL carries the user's API key as a query parameter. Every error
/// here ends up in the panel, so the URL is stripped before the message is
/// ever read.
fn network(e: reqwest::Error) -> ApiError {
    ApiError::Network(e.without_url().to_string())
}

/// APOD API response. `copyright` is absent for public-domain images produced
/// by NASA; when present it must be preserved and shown everywhere (store,
/// tray, panel).
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

    /// True when a usable wallpaper image exists: the image itself, or the
    /// thumbnail for a video. The API serves no video file (only a YouTube or
    /// Vimeo embed link), so the thumbnail is the only possible representation
    /// of a video.
    pub fn has_image(&self) -> bool {
        match self.media_type.as_str() {
            "image" => self.hdurl.is_some() || self.url.is_some(),
            "video" => self.thumbnail_url.is_some(),
            _ => false,
        }
    }

    /// Preferred download URL: high definition for an image, highest
    /// resolution thumbnail for a YouTube video.
    pub fn preferred_download_url(&self) -> Option<String> {
        match self.media_type.as_str() {
            "image" => self
                .hdurl
                .as_deref()
                .or(self.url.as_deref())
                .map(str::to_string),
            "video" => {
                let thumb = self.thumbnail_url.as_deref()?;
                Some(youtube_maxres(thumb).unwrap_or_else(|| thumb.to_string()))
            }
            _ => None,
        }
    }

    /// Fallback URL when the first download fails: the standard URL for an
    /// image, the original thumbnail for a video (the maxres variant does not
    /// exist for older or low-definition videos).
    pub fn fallback_download_url(&self) -> Option<String> {
        match self.media_type.as_str() {
            "image" => match (self.hdurl.as_deref(), self.url.as_deref()) {
                (Some(_), Some(u)) => Some(u.to_string()),
                _ => None,
            },
            "video" => {
                let thumb = self.thumbnail_url.as_deref()?;
                // A fallback only makes sense if we tried the maxres variant.
                youtube_maxres(thumb).map(|_| thumb.to_string())
            }
            _ => None,
        }
    }

    /// The copyright field returned by the API often contains stray newlines,
    /// so collapse whitespace into single spaces.
    fn normalize(mut self) -> Self {
        if let Some(c) = self.copyright.take() {
            let cleaned = c.split_whitespace().collect::<Vec<_>>().join(" ");
            if !cleaned.is_empty() {
                self.copyright = Some(cleaned);
            }
        }
        self
    }
}

/// For a standard YouTube thumbnail (0.jpg, hqdefault.jpg, ...), builds the URL
/// of the highest resolution thumbnail (maxresdefault.jpg, usually 1280x720).
/// Returns None when the URL is not a known YouTube thumbnail, in which case
/// the caller keeps the original URL.
fn youtube_maxres(url: &str) -> Option<String> {
    if !url.contains("img.youtube.com") && !url.contains("ytimg.com") {
        return None;
    }
    let (head, tail) = url.rsplit_once('/')?;
    let name = tail.strip_suffix(".jpg")?;
    const VARIANTS: [&str; 8] = [
        "0",
        "1",
        "2",
        "3",
        "default",
        "mqdefault",
        "hqdefault",
        "sddefault",
    ];
    if VARIANTS.contains(&name) {
        Some(format!("{head}/maxresdefault.jpg"))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upgrades_standard_youtube_thumbnails() {
        assert_eq!(
            youtube_maxres("https://img.youtube.com/vi/abc123/0.jpg").as_deref(),
            Some("https://img.youtube.com/vi/abc123/maxresdefault.jpg")
        );
        assert_eq!(
            youtube_maxres("https://i.ytimg.com/vi/abc123/hqdefault.jpg").as_deref(),
            Some("https://i.ytimg.com/vi/abc123/maxresdefault.jpg")
        );
    }

    #[test]
    fn leaves_other_urls_untouched() {
        assert_eq!(youtube_maxres("https://vimeo.com/thumb/42.jpg"), None);
        assert_eq!(
            youtube_maxres("https://img.youtube.com/vi/abc123/maxresdefault.jpg"),
            None
        );
        assert_eq!(
            youtube_maxres("https://img.youtube.com/vi/abc123/0.png"),
            None
        );
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
        let error = download_image(&client, &format!("{REFUSED}/i.jpg?token=SECRET-TOKEN"))
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
                "The image is larger than {} MB and was not downloaded.",
                MAX_IMAGE_BYTES / (1024 * 1024)
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

/// Downloads the raw bytes of an image, refusing a body over
/// [`MAX_IMAGE_BYTES`]. The body is read chunk by chunk rather than in one
/// call so an endpoint that lies about (or omits) its content length still
/// cannot grow the buffer without bound.
pub async fn download_image(client: &reqwest::Client, url: &str) -> Result<Vec<u8>, ApiError> {
    let mut response = client.get(url).send().await.map_err(network)?;

    let status = response.status();
    if !status.is_success() {
        return Err(ApiError::Http(status.as_u16()));
    }

    let declared = response.content_length();
    if declared.is_some_and(|n| n > MAX_IMAGE_BYTES as u64) {
        return Err(ApiError::TooLarge);
    }

    // Trust the declared length only as an allocation hint, and only up to a
    // size worth reserving up front.
    let mut bytes = Vec::with_capacity(declared.unwrap_or(0).min(16 * 1024 * 1024) as usize);
    while let Some(chunk) = response.chunk().await.map_err(network)? {
        if bytes.len() + chunk.len() > MAX_IMAGE_BYTES {
            return Err(ApiError::TooLarge);
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}
