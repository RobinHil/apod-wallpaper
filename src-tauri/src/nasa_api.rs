use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::fmt;

const ENDPOINT: &str = "https://api.nasa.gov/planetary/apod";

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
        "0", "1", "2", "3", "default", "mqdefault", "hqdefault", "sddefault",
    ];
    if VARIANTS.contains(&name) {
        Some(format!("{head}/maxresdefault.jpg"))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::youtube_maxres;

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
    // thumbs=true asks the API to include video thumbnails, the only usable
    // wallpaper representation of a video (no video file is served).
    let mut request = client
        .get(ENDPOINT)
        .query(&[("api_key", api_key), ("thumbs", "true")]);
    if let Some(d) = date {
        request = request.query(&[("date", d.format("%Y-%m-%d").to_string())]);
    }

    let response = request
        .send()
        .await
        .map_err(|e| ApiError::Network(e.to_string()))?;

    match response.status().as_u16() {
        200 => response
            .json::<Apod>()
            .await
            .map(Apod::normalize)
            .map_err(|e| ApiError::Parse(e.to_string())),
        429 => Err(ApiError::RateLimited),
        // The API answers 404 or 400 for dates with no publication.
        400 | 404 => Err(ApiError::NotFound),
        code => Err(ApiError::Http(code)),
    }
}

/// Downloads the raw bytes of an image.
pub async fn download_image(client: &reqwest::Client, url: &str) -> Result<Vec<u8>, ApiError> {
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| ApiError::Network(e.to_string()))?;

    let status = response.status();
    if !status.is_success() {
        return Err(ApiError::Http(status.as_u16()));
    }

    response
        .bytes()
        .await
        .map(|b| b.to_vec())
        .map_err(|e| ApiError::Network(e.to_string()))
}
