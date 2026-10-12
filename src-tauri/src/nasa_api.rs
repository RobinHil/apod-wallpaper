use crate::apod_html::{self, Media};
use chrono::NaiveDate;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use std::fmt;
use std::time::Duration;

/// APOD's feed on science.nasa.gov, where APOD moved on 29 September 2026.
/// The WordPress route behind the site itself: no key, one entry by date at
/// `/AAMMJJ`, the newest first when listed. It replaces the api.nasa.gov
/// endpoint, which went on scraping the old site after it moved and since then
/// answers every date with the NASA logo.
const ENDPOINT: &str = "https://science.nasa.gov/wp-json/wp/v2/apod-basic";

/// Vimeo's oEmbed endpoint, the one place a Vimeo video's thumbnail can be
/// read from: the APOD feed publishes none.
const VIMEO_OEMBED: &str = "https://vimeo.com/api/oembed.json";

/// Width asked of oEmbed for the thumbnail. Asked for nothing it answers with
/// a 200x150 one; asked for any width it answers with 1280 pixels at most.
const VIMEO_THUMBNAIL_WIDTH: &str = "1280";

/// Deadline for the metadata requests. Much shorter than the client-wide
/// timeout, which exists for image downloads: an entry is about ten kilobytes,
/// and every panel action waits behind it.
const METADATA_TIMEOUT: Duration = Duration::from_secs(30);

/// Ceiling on anything downloaded: an image, or a video to take a frame out
/// of. Large APOD originals reach a hundred megabytes; the cap only exists so
/// a misbehaving endpoint cannot make the app buffer an unbounded body in
/// memory, and an original over it falls back to the next source.
const MAX_DOWNLOAD_BYTES: usize = 128 * 1024 * 1024;

/// Host of every APOD image and video since the move. It serves each file two
/// ways: as stored, under `content/dam/`, and through a resizing service under
/// `dynamicimage/assets/`, which is what the feed's own URLs point at.
const ASSETS: &str = "https://assets.science.nasa.gov/";

/// Folder of the generic pictures the feed puts in `hdurl` when an entry has
/// no image it knows about -- most videos, and the GIFs of the early archive.
/// Two have been seen there, `news-thumbnail.png` and the NASA logo
/// `NASA-Meatball-Feature5.png`; the folder is matched rather than either name
/// because one more would cost a day of wallpaper.
const PLACEHOLDER_DIR: &str = "/science/astro/programs/cosmic-origins/images/misc/";

/// Largest original worth downloading whole. A 5K screen is 14.7 megapixels,
/// and this leaves room for a crop; the archive holds originals of up to 712,
/// and 120 MB for one, which would be downloaded only to be shrunk or refused
/// by the decoder.
const MAX_ORIGINAL_PIXELS: u64 = 40_000_000;

/// Longest side the resizing service renders. Asked for more, it silently
/// falls back to 1280 pixels. It also enlarges what is smaller than the size
/// asked for, which is why it is only ever asked for originals over
/// [`MAX_ORIGINAL_PIXELS`], whose long side is necessarily beyond this one.
const MAX_RENDITION: u32 = 4096;

/// reqwest appends `" for url (...)"` to most of its error messages. Every
/// error here ends up in the panel, where a full URL is noise at best, so it
/// is stripped before the message is ever read.
fn network(e: reqwest::Error) -> ApiError {
    ApiError::Network(e.without_url().to_string())
}

/// One entry of the feed, as it arrives. Everything but the date can be
/// missing, `null`, or an empty string: all three happen in the archive.
#[derive(Debug, Deserialize)]
struct Entry {
    date: String,
    title: Option<String>,
    media_type: Option<String>,
    explanation: Option<String>,
    copyright: Option<String>,
    hdurl: Option<String>,
    permalink: Option<String>,
    basic_html: Option<String>,
}

/// An APOD, with its text made plain and its media read off its page.
#[derive(Debug, Clone)]
pub struct Apod {
    pub date: String,
    pub title: String,
    pub explanation: String,
    /// Who the picture is credited to. The feed no longer says whether that
    /// is a copyright notice or a public-domain credit -- its `copyright`
    /// field is the credit line, NASA's own images included -- so it is shown
    /// as a credit and nothing is ever called public domain.
    pub credit: Option<String>,
    /// The entry's article on science.nasa.gov.
    pub page_url: Option<String>,
    media: Media,
    /// The feed's own word for the media, used only when the page has none.
    media_type: Option<String>,
    /// `hdurl`, unless it is a placeholder.
    hdurl: Option<String>,
    /// A Vimeo video's thumbnail, filled in by [`fetch_apod`].
    thumbnail: Option<String>,
}

impl Apod {
    fn from_entry(entry: Entry) -> Apod {
        let text =
            |field: Option<String>| field.map(|f| apod_html::to_text(&f)).unwrap_or_default();
        Apod {
            date: entry.date,
            title: strip_page_title(text(entry.title)),
            explanation: strip_labels(text(entry.explanation)),
            credit: strip_credit_label(text(entry.copyright)),
            page_url: entry.permalink.filter(|u| !u.trim().is_empty()),
            media: entry
                .basic_html
                .as_deref()
                .map(apod_html::media)
                .unwrap_or(Media::Nothing),
            media_type: entry.media_type,
            hdurl: entry
                .hdurl
                .filter(|u| !u.trim().is_empty() && !u.contains(PLACEHOLDER_DIR)),
            thumbnail: None,
        }
    }

    pub fn is_video(&self) -> bool {
        match self.media {
            Media::VideoFile(_) | Media::YouTube(_) | Media::Vimeo(_) | Media::OtherEmbed(_) => {
                true
            }
            Media::Image { .. } => false,
            Media::Nothing => matches!(self.media_type.as_deref(), Some("video" | "iframe")),
        }
    }

    /// "video" or "image": the two kinds the rest of the app tells apart. The
    /// feed's own `media_type` has a third, "iframe", for what is a Vimeo
    /// video as often as not.
    pub fn kind(&self) -> &'static str {
        if self.is_video() { "video" } else { "image" }
    }

    /// Where the video can be found, in the shapes the panel turns into a
    /// page to watch it on: an embed link, or the file.
    pub fn video_url(&self) -> Option<String> {
        match &self.media {
            Media::VideoFile(url) | Media::OtherEmbed(url) => Some(url.clone()),
            Media::YouTube(id) => Some(format!("https://www.youtube.com/embed/{id}")),
            Media::Vimeo(id) => Some(format!("https://player.vimeo.com/video/{id}")),
            Media::Image { .. } | Media::Nothing => None,
        }
    }

    /// True when some wallpaper can be produced for this day.
    pub fn has_wallpaper(&self) -> bool {
        !self.sources().is_empty()
    }

    /// Everything worth downloading for this APOD, best first. The caller
    /// walks the list and keeps the first one that yields a usable image, so
    /// every entry is a fallback for the ones before it.
    ///
    /// A video is never a wallpaper by itself. NASA now publishes a still of
    /// its own for some recent ones, in `hdurl`, and that is the picture the
    /// editors chose, so it comes first. After it: YouTube's thumbnail, read
    /// from the video id; Vimeo's, from its oEmbed answer; and for a video
    /// served as a file, a frame taken out of the file itself.
    pub fn sources(&self) -> Vec<Source> {
        let mut sources = Vec::new();
        let declared = self.hdurl.as_deref().and_then(declared_pixels);
        let hdurl = self.hdurl.as_deref();
        match &self.media {
            Media::Image { link, img } => {
                push_image(&mut sources, link.as_deref(), declared);
                push_image(&mut sources, hdurl, declared);
                push_image(&mut sources, img.as_deref(), declared);
            }
            Media::VideoFile(file) => {
                push_image(&mut sources, hdurl, declared);
                if is_video_file(file) {
                    push(&mut sources, Source::Video(file.clone()));
                }
            }
            Media::YouTube(id) => {
                push_image(&mut sources, hdurl, declared);
                for size in YOUTUBE_SIZES {
                    let thumbnail = format!("https://img.youtube.com/vi/{id}/{size}.jpg");
                    push(&mut sources, Source::Image(thumbnail));
                }
            }
            Media::Vimeo(_) => {
                push_image(&mut sources, hdurl, declared);
                push_image(&mut sources, self.thumbnail.as_deref(), None);
            }
            // An entry migrated with an empty page can still have a real
            // `hdurl`, and an embed nothing can be taken from may still come
            // with a still of NASA's.
            Media::OtherEmbed(_) | Media::Nothing => push_image(&mut sources, hdurl, declared),
        }
        sources
    }
}

/// A few entries were migrated with the old page's title as their own:
/// "APOD: 2012 May 23 - SpaceX Dragon Launches to the Space Station", with an
/// en dash. Only that shape is cut, a date and all.
fn strip_page_title(title: String) -> String {
    let rest = title
        .strip_prefix("APOD: ")
        .filter(|rest| rest.starts_with(|c: char| c.is_ascii_digit()))
        .and_then(|rest| rest.split_once(" \u{2013} "))
        .map(|(_, rest)| rest.trim());
    match rest {
        Some(rest) if !rest.is_empty() => rest.to_string(),
        _ => title,
    }
}

/// The explanation as the panel shows it: without the "Explanation:" label
/// the page prints before it, and without the teaser for the next day that
/// the feed now appends, which says nothing about this picture.
fn strip_labels(explanation: String) -> String {
    let text = explanation
        .strip_prefix("Explanation:")
        .unwrap_or(&explanation)
        .trim_start();
    let end = ["Tomorrow's picture:", "Tomorrow\u{2019}s picture:"]
        .iter()
        .filter_map(|marker| text.find(marker))
        .min()
        .unwrap_or(text.len());
    text[..end].trim_end().to_string()
}

/// The credit line, without the label some entries open it with ("Image
/// Credit:", "Credit & Copyright:", "Illustrations Credit & Copyright:"), and
/// `None` when nothing is left. A label that is not about credit stays, since
/// it is part of what is credited: "Image: NASA, ESA; Image Processing: ...".
fn strip_credit_label(credit: String) -> Option<String> {
    let text = match credit.split_once(':') {
        Some((label, rest))
            if label.len() <= 40 && label.to_ascii_lowercase().contains("credit") =>
        {
            rest.trim()
        }
        _ => credit.trim(),
    };
    (!text.is_empty()).then(|| text.to_string())
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

/// Appends a candidate unless it is already listed. The duplicates are real:
/// the link around a picture, the picture shown and `hdurl` are usually the
/// same file, which would otherwise be downloaded three times before the day
/// was declared a failure.
fn push(sources: &mut Vec<Source>, candidate: Source) {
    if !sources.contains(&candidate) {
        sources.push(candidate);
    }
}

/// Appends a picture, preceded by the better copy of it the asset host has.
fn push_image(sources: &mut Vec<Source>, url: Option<&str>, declared: Option<u64>) {
    for url in url.map(|u| renditions(u, declared)).unwrap_or_default() {
        push(sources, Source::Image(url));
    }
}

/// The copies of one asset worth trying, best first, ending with the URL as
/// published.
///
/// Every URL the feed publishes goes through the resizing service, which
/// recompresses heavily -- the picture of 2026-10-12 comes back at 993x1280
/// and 70 kB, against 3105x4000 and 1.4 MB as stored -- and caps what it
/// renders, so an original larger than [`MAX_RENDITION`] comes back at 1280
/// pixels whatever the URL asks for. The stored file is therefore tried first,
/// unless it is one of the giants, where a rendition at the service's largest
/// size is enough for any screen.
fn renditions(url: &str, declared: Option<u64>) -> Vec<String> {
    let mut out = Vec::with_capacity(2);
    if let Some(path) = asset_path(url) {
        if declared.is_some_and(|pixels| pixels > MAX_ORIGINAL_PIXELS) {
            out.push(format!(
                "{ASSETS}dynamicimage/assets/{path}?w={MAX_RENDITION}&h={MAX_RENDITION}&fit=clip"
            ));
        } else {
            out.push(format!("{ASSETS}content/dam/{path}"));
        }
    }
    out.push(url.to_string());
    out
}

/// The asset's path on the host, the same under both prefixes.
fn asset_path(url: &str) -> Option<&str> {
    let rest = url.strip_prefix(ASSETS)?.split(['?', '#']).next()?;
    rest.strip_prefix("content/dam/")
        .or_else(|| rest.strip_prefix("dynamicimage/assets/"))
        .filter(|path| !path.is_empty())
}

/// The original's size, as `hdurl` declares it in its `w` and `h` parameters.
/// `w=0&h=0`, which the newest entries carry, is a size the feed does not
/// know.
fn declared_pixels(url: &str) -> Option<u64> {
    let (_, query) = url.split_once('?')?;
    let (mut width, mut height) = (None, None);
    for pair in query.split('&') {
        match pair.split_once('=') {
            Some(("w", v)) => width = v.parse::<u64>().ok(),
            Some(("h", v)) => height = v.parse::<u64>().ok(),
            _ => {}
        }
    }
    Some(width? * height?).filter(|&pixels| pixels > 0)
}

/// Containers the system decoder opens, and that APOD actually publishes. The
/// archive also holds two `.wmv` and an `.asx`, which neither desktop's own
/// framework can be counted on to read.
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

/// The sizes of a YouTube video's thumbnail to try, biggest first. It is one
/// picture at several sizes, and YouTube does not generate the big ones for
/// every video: `maxresdefault` (1280x720) is missing for older or
/// low-definition uploads, `sddefault` (640x480) is the next size down, and
/// `hqdefault` (480x360) always exists. Starting from more pixels only means
/// less upscaling on the way to a screen-sized wallpaper.
const YOUTUBE_SIZES: [&str; 3] = ["maxresdefault", "sddefault", "hqdefault"];

#[derive(Debug)]
pub enum ApiError {
    /// Network error: no connection, DNS, timeout... treated as offline.
    Network(String),
    /// The server is turning requests away (HTTP 429).
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
                "NASA's server is turning requests away for now (HTTP 429). Will retry automatically."
            ),
            ApiError::NotFound => write!(
                f,
                "No APOD was published on that date (the archive has a few days with no entry)."
            ),
            ApiError::Http(code) => write!(f, "HTTP error {code} from the server."),
            ApiError::Parse(e) => write!(f, "Unreadable answer from NASA's APOD feed: {e}"),
            ApiError::TooLarge => write!(
                f,
                "The file is larger than {} MB and was not downloaded.",
                MAX_DOWNLOAD_BYTES / (1024 * 1024)
            ),
        }
    }
}

/// Fetches an APOD. `date: None` returns the most recently published one,
/// which avoids time-zone mistakes: the feed decides what "today" is rather
/// than the local clock.
pub async fn fetch_apod(
    client: &reqwest::Client,
    date: Option<NaiveDate>,
) -> Result<Apod, ApiError> {
    fetch_apod_from(client, ENDPOINT, date).await
}

/// The body of [`fetch_apod`], with the endpoint injected so the tests can
/// point it at an address that fails.
async fn fetch_apod_from(
    client: &reqwest::Client,
    endpoint: &str,
    date: Option<NaiveDate>,
) -> Result<Apod, ApiError> {
    let entry = match date {
        // Listed newest first, so the first entry of a one-entry page is the
        // latest publication.
        None => get_json::<Vec<Entry>>(client.get(endpoint).query(&[("per_page", "1")]))
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| ApiError::Parse("the feed listed no entry".to_string()))?,
        Some(d) => {
            let entry: Entry =
                get_json(client.get(format!("{endpoint}/{}", d.format("%y%m%d")))).await?;
            check_date(&entry, d)?;
            entry
        }
    };

    let mut apod = Apod::from_entry(entry);
    if let Media::Vimeo(id) = &apod.media {
        apod.thumbnail = vimeo_thumbnail(client, id).await;
    }
    Ok(apod)
}

/// An entry for the date asked for, or an error saying it is not.
///
/// The feed ignores what it does not understand and answers anyway: asked for
/// `?date=`, it returns its latest entries with a 200. The api.nasa.gov
/// endpoint did the same with a date after APOD moved, and that is how it came
/// to serve one wrong picture for every day. A date that comes back different
/// is therefore a failure, not something to apply.
fn check_date(entry: &Entry, asked: NaiveDate) -> Result<(), ApiError> {
    let asked = asked.format("%Y-%m-%d").to_string();
    if entry.date == asked {
        Ok(())
    } else {
        Err(ApiError::Parse(format!(
            "asked for {asked}, received {}",
            entry.date
        )))
    }
}

async fn get_json<T: DeserializeOwned>(request: reqwest::RequestBuilder) -> Result<T, ApiError> {
    let response = request
        .timeout(METADATA_TIMEOUT)
        .send()
        .await
        .map_err(network)?;
    match response.status().as_u16() {
        200 => response
            .json::<T>()
            .await
            .map_err(|e| ApiError::Parse(e.without_url().to_string())),
        429 => Err(ApiError::RateLimited),
        // The feed's answer for a day with no publication. A malformed date
        // is a 400, which cannot happen here: dates are validated before.
        404 => Err(ApiError::NotFound),
        code => Err(ApiError::Http(code)),
    }
}

#[derive(Deserialize)]
struct OEmbed {
    thumbnail_url: Option<String>,
}

/// A Vimeo video's thumbnail, or `None`. A failure here is not one of the
/// update's: NASA may have published a still of its own, and when it has not,
/// the update ends the way any video with nothing to show does.
async fn vimeo_thumbnail(client: &reqwest::Client, id: &str) -> Option<String> {
    let response = client
        .get(VIMEO_OEMBED)
        .timeout(METADATA_TIMEOUT)
        .query(&[
            ("url", format!("https://vimeo.com/{id}").as_str()),
            ("width", VIMEO_THUMBNAIL_WIDTH),
        ])
        .send()
        .await
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    let answer = response.json::<OEmbed>().await.ok()?;
    answer.thumbnail_url.filter(|u| !u.trim().is_empty())
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const PLACEHOLDER: &str = "https://assets.science.nasa.gov/dynamicimage/assets/science/astro/programs/cosmic-origins/images/misc/news-thumbnail.png?w=594&h=516&fit=clip&crop=faces%2Cfocalpoint";

    /// An entry as the feed sends it, around one media block of its page.
    fn entry(date: &str, media_type: Option<&str>, hdurl: &str, block: &str) -> Apod {
        let basic_html = format!(
            "<!doctype html>\n<html>\n<body BGCOLOR=\"#F4F4FF\">\n<center>\n\
             <h1> Astronomy Picture of the Day </h1>\n\
             <a href=\"https://science.nasa.gov/apod/archive/\">Discover the cosmos!</a>\n\
             <p>\n{date}<br>\n{block}\n</center>\n</body>\n</html>"
        );
        let value = json!({
            "date": date,
            "title": "Somewhere",
            "media_type": media_type,
            "explanation": "...",
            "copyright": "",
            "hdurl": hdurl,
            "permalink": format!("https://science.nasa.gov/image-article/apod-{date}/"),
            "basic_html": basic_html,
        });
        Apod::from_entry(serde_json::from_value(value).expect("the fixture must deserialize"))
    }

    fn images(urls: &[&str]) -> Vec<Source> {
        urls.iter().map(|u| Source::Image(u.to_string())).collect()
    }

    /// 2026-10-12, the text fields verbatim but shortened.
    #[test]
    fn the_text_fields_arrive_as_plain_text() {
        let value = json!({
            "date": "2026-10-12",
            "post_id": 1481216,
            "title": "Jupiter and the Moons",
            "media_type": "image",
            "explanation": "<strong>Explanation:</strong> How many moons can you see? The best answer is five: four orbiting <a href=\"https://science.nasa.gov/jupiter/\">Jupiter</a> and <a href=\"https://science.nasa.gov/moon/\">Luna</a> orbiting Earth.<br><br><strong>Tomorrow's picture:</strong> stairway to heaven",
            "credit": "<a href=\"https://www.instagram.com/chiragupreti/\">Chirag Upreti</a>",
            "copyright": "<a href=\"https://www.instagram.com/chiragupreti/\">Chirag Upreti</a>",
            "url": "https://science.nasa.gov/image-article/apod-2026-october-12-jupiter-and-the-moons/",
            "hdurl": "https://assets.science.nasa.gov/dynamicimage/assets/science/cds/apod/apod/2026/october/JupiterMoons_Upreti_3105.jpg?w=0&h=0&fit=clip&crop=faces%2Cfocalpoint",
            "permalink": "https://science.nasa.gov/image-article/apod-2026-october-12-jupiter-and-the-moons/",
            "basic_html_url": "https://science.nasa.gov/wp-json/wp/v2/apod-basic/261012/html",
        });
        let apod = Apod::from_entry(serde_json::from_value(value).unwrap());

        assert_eq!(apod.title, "Jupiter and the Moons");
        assert_eq!(
            apod.explanation,
            "How many moons can you see? The best answer is five: four orbiting Jupiter and Luna orbiting Earth."
        );
        assert_eq!(apod.credit.as_deref(), Some("Chirag Upreti"));
        assert_eq!(
            apod.page_url.as_deref(),
            Some(
                "https://science.nasa.gov/image-article/apod-2026-october-12-jupiter-and-the-moons/"
            )
        );
        assert_eq!(apod.kind(), "image");
    }

    #[test]
    fn credit_labels_are_dropped_and_other_labels_kept() {
        let credit = |raw: &str| strip_credit_label(apod_html::to_text(raw));
        // 2026-03-12, 2009-04-13, 2012-05-01, 2026-07-26, verbatim openings.
        assert_eq!(
            credit("<b> Video Credit: </b> ESA/Webb, NASA, CSA <br /> <b>Text:</b> <a href=\"x\">Keighley Rockcliffe</a>").as_deref(),
            Some("ESA/Webb, NASA, CSA Text: Keighley Rockcliffe")
        );
        assert_eq!(
            credit("<b> Credit &amp; <a href=\"x\">Copyright</a>: </b> <a href=\"y\">Jean-Charles Cuillandre</a>").as_deref(),
            Some("Jean-Charles Cuillandre")
        );
        assert_eq!(
            credit("<b> Illustrations Credit &amp; Copyright: </b> <a href=\"x\">Jorge Cham</a>")
                .as_deref(),
            Some("Jorge Cham")
        );
        assert_eq!(
            credit("<strong>Video Credit:</strong> <a href=\"x\">IllustrisTNG Project</a>")
                .as_deref(),
            Some("IllustrisTNG Project")
        );
        // 2026-08-04: "Image:" is part of the credit, not a label for it.
        assert_eq!(
            credit("Image: NASA, ESA, CSA, STScI; Image Processing: Alyssa Pagan (STScI)")
                .as_deref(),
            Some("Image: NASA, ESA, CSA, STScI; Image Processing: Alyssa Pagan (STScI)")
        );
        // 2012-05-23, and the 33 others like it.
        assert_eq!(credit(""), None);
    }

    /// 2012-05-23 and 2020-08-13 carry the old page title, entity and all.
    #[test]
    fn a_page_title_is_cut_down_to_the_title() {
        let title = |raw: &str| strip_page_title(apod_html::to_text(raw));
        assert_eq!(
            title(
                "APOD: 2020 August 13 &#8211; Jupiter and Saturn Rising Beyond Alien Throne Rock"
            ),
            "Jupiter and Saturn Rising Beyond Alien Throne Rock"
        );
        assert_eq!(
            title("Meteor &amp; Milky Way over the Mediterranean"),
            "Meteor & Milky Way over the Mediterranean"
        );
        assert_eq!(
            title("APOD: a title that only looks like one"),
            "APOD: a title that only looks like one"
        );
    }

    /// 2026-10-12: the stored original first, the published copies after it.
    #[test]
    fn a_picture_is_downloaded_as_stored_first() {
        let apod = entry(
            "2026-10-12",
            Some("image"),
            "https://assets.science.nasa.gov/dynamicimage/assets/science/cds/apod/apod/2026/october/JupiterMoons_Upreti_3105.jpg?w=0&h=0&fit=clip&crop=faces%2Cfocalpoint",
            "<a href=\"https://assets.science.nasa.gov/dynamicimage/assets/science/cds/apod/apod/2026/october/JupiterMoons_Upreti_3105.jpg\">\n\
             <IMG SRC=\"https://assets.science.nasa.gov/dynamicimage/assets/science/cds/apod/apod/2026/october/JupiterMoons_Upreti_3105.jpg\" alt=\"Earth&#039;s Moon\"></a>",
        );
        assert_eq!(
            apod.sources(),
            images(&[
                "https://assets.science.nasa.gov/content/dam/science/cds/apod/apod/2026/october/JupiterMoons_Upreti_3105.jpg",
                "https://assets.science.nasa.gov/dynamicimage/assets/science/cds/apod/apod/2026/october/JupiterMoons_Upreti_3105.jpg",
                "https://assets.science.nasa.gov/dynamicimage/assets/science/cds/apod/apod/2026/october/JupiterMoons_Upreti_3105.jpg?w=0&h=0&fit=clip&crop=faces%2Cfocalpoint",
            ])
        );
    }

    /// 2026-08-04: an 11124x8064 PNG of 120 MB. The published `hdurl` comes
    /// back at 1280 pixels, so the service's largest rendition goes first.
    #[test]
    fn a_giant_original_is_replaced_by_the_largest_rendition() {
        let apod = entry(
            "2026-08-04",
            Some("image"),
            "https://assets.science.nasa.gov/dynamicimage/assets/science/missions/webb/science/2026/01/STScI-01KCMAMZ8ZSMRTR9KKD8Y6EMRJ.png?w=11124&h=8064&fit=clip&crop=faces%2Cfocalpoint",
            "<a href=\"https://assets.science.nasa.gov/dynamicimage/assets/science/missions/webb/science/2026/01/STScI-01KCMAMZ8ZSMRTR9KKD8Y6EMRJ.png\">\n\
             <IMG SRC=\"https://assets.science.nasa.gov/dynamicimage/assets/science/missions/webb/science/2026/01/STScI-01KCMAMZ8ZSMRTR9KKD8Y6EMRJ.png\" alt=\"A closeup\"></a>",
        );
        let sources = apod.sources();
        assert_eq!(
            sources.first(),
            Some(&Source::Image(
                "https://assets.science.nasa.gov/dynamicimage/assets/science/missions/webb/science/2026/01/STScI-01KCMAMZ8ZSMRTR9KKD8Y6EMRJ.png?w=4096&h=4096&fit=clip".to_string()
            ))
        );
        assert!(!sources.iter().any(|s| s.url().contains("/content/dam/")));
    }

    /// 1995-06-20: a GIF, with the placeholder as `hdurl`.
    #[test]
    fn a_placeholder_is_never_downloaded() {
        let apod = entry(
            "1995-06-20",
            Some("image"),
            PLACEHOLDER,
            "<a href=\"https://assets.science.nasa.gov/content/dam/science/cds/apod/apod/1995/june/pleiades2.gif\">\n\
             <IMG SRC=\"https://assets.science.nasa.gov/content/dam/science/cds/apod/apod/1995/june/pleiades2.gif\" alt=\"See Explanation.\"></a>",
        );
        assert_eq!(
            apod.sources(),
            images(&[
                "https://assets.science.nasa.gov/content/dam/science/cds/apod/apod/1995/june/pleiades2.gif"
            ])
        );
    }

    /// 2026-07-26: the other placeholder, the NASA logo, on a video.
    #[test]
    fn the_nasa_logo_is_a_placeholder_too() {
        let apod = entry(
            "2026-07-26",
            Some("video"),
            "https://assets.science.nasa.gov/dynamicimage/assets/science/astro/programs/cosmic-origins/images/misc/NASA-Meatball-Feature5.png?w=594&h=516&fit=clip&crop=faces%2Cfocalpoint",
            "<video controls title=\"\">\n\
             <source src=\"https://assets.science.nasa.gov/content/dam/science/cds/apod/apod/2026/july/ClusterFormation_TNG50.mp4\" type=\"video/mp4\">\n</video>",
        );
        assert_eq!(
            apod.sources(),
            vec![Source::Video(
                "https://assets.science.nasa.gov/content/dam/science/cds/apod/apod/2026/july/ClusterFormation_TNG50.mp4".to_string()
            )]
        );
    }

    /// 2000-04-01: no `hdurl` at all, an empty string in its place.
    #[test]
    fn a_picture_with_no_hdurl_is_read_off_its_page() {
        let apod = entry(
            "2000-04-01",
            Some("image"),
            "",
            "<a href=\"https://assets.science.nasa.gov/content/dam/science/cds/apod/apod/2000/april/first_tiros1_big.gif\">\n\
             <IMG SRC=\"https://assets.science.nasa.gov/content/dam/science/cds/apod/apod/2000/april/first_tiros1_big.gif\" alt=\"See Explanation.\"></a>",
        );
        assert_eq!(
            apod.sources(),
            images(&[
                "https://assets.science.nasa.gov/content/dam/science/cds/apod/apod/2000/april/first_tiros1_big.gif"
            ])
        );
    }

    /// 2000-01-24: a TIFF from Hubble's releases, outside the APOD folders.
    #[test]
    fn a_tiff_original_comes_before_its_jpeg_rendition() {
        let apod = entry(
            "2000-01-24",
            Some("image"),
            "https://assets.science.nasa.gov/dynamicimage/assets/science/missions/hubble/releases/2000/01/STScI-01EVVP8RZA90C7S8VQPYR6DMEN.tif?w=1500&h=1500&fit=clip&crop=faces%2Cfocalpoint",
            "<a href=\"https://assets.science.nasa.gov/dynamicimage/assets/science/missions/hubble/releases/2000/01/STScI-01EVVP8RZA90C7S8VQPYR6DMEN.tif\">\n\
             <IMG SRC=\"https://assets.science.nasa.gov/dynamicimage/assets/science/missions/hubble/releases/2000/01/STScI-01EVVP8RZA90C7S8VQPYR6DMEN.tif\" alt=\"NGC 2392\"></a>",
        );
        assert_eq!(
            apod.sources().first(),
            Some(&Source::Image(
                "https://assets.science.nasa.gov/content/dam/science/missions/hubble/releases/2000/01/STScI-01EVVP8RZA90C7S8VQPYR6DMEN.tif".to_string()
            ))
        );
    }

    /// 2026-08-19: a video file, and a still of it NASA published alongside.
    #[test]
    fn a_video_with_a_still_of_nasas_uses_it_before_its_frames() {
        let apod = entry(
            "2026-08-19",
            Some("video"),
            "https://assets.science.nasa.gov/dynamicimage/assets/science/cds/apod/apod/2026/august/perseids_eclipse_mystery_frame.png?w=2216&h=1506&fit=clip&crop=faces%2Cfocalpoint",
            "<video controls title=\"\">\n\
             <source src=\"https://assets.science.nasa.gov/content/dam/science/cds/apod/apod/2026/august/perseids_eclipse_mystery.mp4\" type=\"video/mp4\">\n</video>",
        );
        assert!(apod.is_video());
        assert_eq!(
            apod.sources(),
            vec![
                Source::Image("https://assets.science.nasa.gov/content/dam/science/cds/apod/apod/2026/august/perseids_eclipse_mystery_frame.png".to_string()),
                Source::Image("https://assets.science.nasa.gov/dynamicimage/assets/science/cds/apod/apod/2026/august/perseids_eclipse_mystery_frame.png?w=2216&h=1506&fit=clip&crop=faces%2Cfocalpoint".to_string()),
                Source::Video("https://assets.science.nasa.gov/content/dam/science/cds/apod/apod/2026/august/perseids_eclipse_mystery.mp4".to_string()),
            ]
        );
        assert_eq!(
            apod.video_url().as_deref(),
            Some(
                "https://assets.science.nasa.gov/content/dam/science/cds/apod/apod/2026/august/perseids_eclipse_mystery.mp4"
            )
        );
    }

    /// 2017-11-13: a video file and nothing else.
    #[test]
    fn a_video_file_alone_is_downloaded_for_its_frame() {
        let apod = entry(
            "2017-11-13",
            Some("video"),
            PLACEHOLDER,
            "<video controls title=\"\">\n\
             <source src=\"https://assets.science.nasa.gov/content/dam/science/cds/apod/apod/2017/november/CometMachholz2017_SOHO_big.mp4\" type=\"video/mp4\">\n</video>",
        );
        assert_eq!(
            apod.sources(),
            vec![Source::Video(
                "https://assets.science.nasa.gov/content/dam/science/cds/apod/apod/2017/november/CometMachholz2017_SOHO_big.mp4".to_string()
            )]
        );
    }

    /// A container no system decoder can be counted on: no frame is tried.
    #[test]
    fn a_video_in_a_foreign_container_yields_nothing() {
        let apod = entry(
            "2003-01-01",
            Some("video"),
            PLACEHOLDER,
            "<video controls>\n<source src=\"https://assets.science.nasa.gov/content/dam/a.wmv\">\n</video>",
        );
        assert!(apod.is_video());
        assert!(!apod.has_wallpaper());
    }

    /// 2026-03-12: YouTube has no `maxresdefault` for this one, which is why
    /// `sddefault` sits between it and the size that always exists.
    #[test]
    fn a_youtube_video_tries_its_thumbnails_biggest_first() {
        let apod = entry(
            "2026-03-12",
            Some("video"),
            PLACEHOLDER,
            "<iframe width=\"960\" height=\"540\" src=\"//www.youtube.com/embed/3jsn1829OPw?rel=0\" frameborder=\"0\" allowfullscreen></iframe>",
        );
        assert_eq!(
            apod.sources(),
            images(&[
                "https://img.youtube.com/vi/3jsn1829OPw/maxresdefault.jpg",
                "https://img.youtube.com/vi/3jsn1829OPw/sddefault.jpg",
                "https://img.youtube.com/vi/3jsn1829OPw/hqdefault.jpg",
            ])
        );
        assert_eq!(
            apod.video_url().as_deref(),
            Some("https://www.youtube.com/embed/3jsn1829OPw")
        );
    }

    /// 2012-05-01: "iframe" in the feed, a Vimeo video on the page.
    #[test]
    fn a_vimeo_video_uses_the_thumbnail_vimeo_published() {
        let mut apod = entry(
            "2012-05-01",
            Some("iframe"),
            PLACEHOLDER,
            "<iframe src=\"https://player.vimeo.com/video/41038445?portrait=0&#038;color=c8b3df\" width=\"960\" height=\"720\" frameBorder=\"0\"></iframe>",
        );
        assert_eq!(apod.kind(), "video");
        assert_eq!(
            apod.video_url().as_deref(),
            Some("https://player.vimeo.com/video/41038445")
        );
        // Before the oEmbed answer, and if it never comes, nothing to show.
        assert!(!apod.has_wallpaper());

        apod.thumbnail =
            Some("https://i.vimeocdn.com/video/284014229-892c-d_1280?region=us".to_string());
        assert_eq!(
            apod.sources(),
            images(&["https://i.vimeocdn.com/video/284014229-892c-d_1280?region=us"])
        );
    }

    /// 2009-04-13: a Flash player from the old site.
    #[test]
    fn an_embed_with_nothing_to_show_yields_nothing() {
        let apod = entry(
            "2009-04-13",
            Some("iframe"),
            PLACEHOLDER,
            "<iframe src=\"https://apod.nasa.gov/image/0904/flvplayer.swf\" width=\"960\" height=\"700\" frameBorder=\"0\"></iframe>",
        );
        assert!(apod.is_video());
        assert!(!apod.has_wallpaper());
    }

    /// 2020-08-13: migrated with an empty page and a `media_type` of null, but
    /// a real `hdurl`.
    #[test]
    fn an_empty_page_falls_back_to_hdurl() {
        let apod = entry(
            "2020-08-13",
            None,
            "https://assets.science.nasa.gov/content/dam/science/cds/apod/apod/2020/august/AlienThrone_Zajac_3807.jpg?w=3807&h=3657&fit=clip&crop=faces%2Cfocalpoint",
            "",
        );
        assert_eq!(apod.kind(), "image");
        assert_eq!(
            apod.sources().first(),
            Some(&Source::Image(
                "https://assets.science.nasa.gov/content/dam/science/cds/apod/apod/2020/august/AlienThrone_Zajac_3807.jpg".to_string()
            ))
        );
    }

    /// 2012-05-23: an empty page and the placeholder. Nothing to be had.
    #[test]
    fn an_empty_page_and_a_placeholder_yield_nothing() {
        let apod = entry("2012-05-23", None, PLACEHOLDER, "");
        assert!(!apod.has_wallpaper());
    }

    #[test]
    fn renditions_only_rewrite_the_asset_host() {
        assert_eq!(
            renditions(
                "https://assets.science.nasa.gov/content/dam/science/a.jpg?w=10&h=10",
                None
            ),
            vec![
                "https://assets.science.nasa.gov/content/dam/science/a.jpg",
                "https://assets.science.nasa.gov/content/dam/science/a.jpg?w=10&h=10",
            ]
        );
        assert_eq!(
            renditions("https://i.vimeocdn.com/video/1-d_1280", Some(u64::MAX)),
            vec!["https://i.vimeocdn.com/video/1-d_1280"]
        );
        // At the threshold the original is still fetched; above it, not.
        let at = renditions(
            "https://assets.science.nasa.gov/content/dam/a.jpg",
            Some(MAX_ORIGINAL_PIXELS),
        );
        assert_eq!(at[0], "https://assets.science.nasa.gov/content/dam/a.jpg");
        let above = renditions(
            "https://assets.science.nasa.gov/content/dam/a.jpg",
            Some(MAX_ORIGINAL_PIXELS + 1),
        );
        assert_eq!(
            above[0],
            "https://assets.science.nasa.gov/dynamicimage/assets/a.jpg?w=4096&h=4096&fit=clip"
        );
    }

    #[test]
    fn the_declared_size_is_read_from_the_query() {
        assert_eq!(
            declared_pixels("https://x/a.png?w=11124&h=8064&fit=clip"),
            Some(89_703_936)
        );
        assert_eq!(declared_pixels("https://x/a.jpg?w=0&h=0&fit=clip"), None);
        assert_eq!(declared_pixels("https://x/a.jpg"), None);
    }

    /// Asking for a date and accepting another is what broke the old API.
    #[test]
    fn an_entry_for_another_date_is_refused() {
        let entry = |date: &str| -> Entry {
            serde_json::from_value(json!({ "date": date })).expect("a date is enough")
        };
        let asked = NaiveDate::from_ymd_opt(2026, 8, 19).unwrap();
        assert!(check_date(&entry("2026-08-19"), asked).is_ok());
        assert!(matches!(
            check_date(&entry("2026-10-12"), asked),
            Err(ApiError::Parse(_))
        ));
    }

    #[test]
    fn video_files_are_told_apart_from_embed_links() {
        assert!(is_video_file(
            "https://assets.science.nasa.gov/content/dam/a.mp4"
        ));
        assert!(is_video_file(
            "https://assets.science.nasa.gov/content/dam/A.MOV"
        ));
        assert!(is_video_file("https://example.com/a.mp4?v=2#t=10"));
        assert!(!is_video_file("https://www.youtube.com/embed/abc123"));
        assert!(!is_video_file("https://player.vimeo.com/video/12345"));
        assert!(!is_video_file(
            "https://assets.science.nasa.gov/content/dam/a.jpg"
        ));
        assert!(!is_video_file(
            "https://assets.science.nasa.gov/content/dam/a.wmv"
        ));
        // A dot in a directory name is not an extension.
        assert!(!is_video_file("https://example.com/v1.2/watch"));
    }

    /// Port 1 on the loopback: nothing listens there, so the requests below
    /// fail locally and immediately.
    const REFUSED: &str = "http://127.0.0.1:1";

    #[tokio::test]
    async fn a_refused_feed_reads_as_offline() {
        let client = reqwest::Client::new();
        let error = fetch_apod_from(&client, &format!("{REFUSED}/apod-basic"), None)
            .await
            .expect_err("a refused connection must produce an error");
        assert!(error.is_offline(), "{error}");
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
