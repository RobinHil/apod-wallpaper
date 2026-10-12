//! Reads the HTML the APOD feed hands back.
//!
//! Since APOD moved to science.nasa.gov, its feed carries HTML in two places:
//! the text fields (title, explanation, credit) are fragments with links and
//! entities in them, and `basic_html` is the whole entry rendered the way the
//! old apod.nasa.gov page was. That page is the only place that says what was
//! actually published: the feed's own `hdurl` is a generic placeholder for
//! about one entry in seven, and it has no field at all for a video.
//!
//! The shapes read here are few and fixed -- they are the page template of a
//! single site -- so this is a small scanner rather than an HTML parser, which
//! would be a large dependency for a process that reads one page a day.

/// What an entry publishes, as its page shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Media {
    /// A picture: `link` is the `<a href>` around it, which the APOD template
    /// has always pointed at the highest resolution available, and `img` the
    /// copy shown on the page.
    Image {
        link: Option<String>,
        img: Option<String>,
    },
    /// A video served as a file of its own, from a `<video><source>`.
    VideoFile(String),
    /// A YouTube embed, by video id.
    YouTube(String),
    /// A Vimeo embed, by video id.
    Vimeo(String),
    /// Any other `<iframe>`: a Flash player, an interactive page, another
    /// site's viewer. Nothing a still can be taken from.
    OtherEmbed(String),
    /// Nothing at all: some entries were migrated with an empty page.
    Nothing,
}

/// The media an entry's `basic_html` shows: the first image, video or embed
/// in the block between the date line and the title.
///
/// The search stops at the first `</center>` after `<body`, which is where
/// the template closes the block holding the media. Everything after it --
/// the explanation, the navigation -- is full of links that are not the
/// picture.
pub fn media(page: &str) -> Media {
    let Some(start) = page.find("<body") else {
        return Media::Nothing;
    };
    let body = &page[start..];
    let block = &body[..body.find("</center>").unwrap_or(body.len())];

    // The block also opens with the template's own link to the archive, so an
    // anchor only counts as the picture's link while it is still open when the
    // `<img>` arrives.
    let mut anchor = None;
    for tag in tags(block) {
        match tag.name.as_str() {
            "a" => anchor = tag.url("href"),
            "/a" => anchor = None,
            "img" => {
                if let Some(img) = tag.url("src") {
                    return Media::Image {
                        link: anchor,
                        img: Some(img),
                    };
                }
            }
            // `<video>` itself names no file in this template, its `<source>`
            // does; both are read in case that ever changes.
            "video" | "source" => {
                if let Some(src) = tag.url("src") {
                    return Media::VideoFile(src);
                }
            }
            "iframe" => {
                if let Some(src) = tag.url("src") {
                    return embed(src);
                }
            }
            _ => {}
        }
    }
    Media::Nothing
}

/// Tells the two embeds a still can be found for from everything else.
fn embed(src: String) -> Media {
    let path = src.split(['?', '#']).next().unwrap_or(&src);
    let after = |marker: &str| path.split_once(marker).map(|(_, rest)| rest);

    let youtube = after("youtube.com/embed/")
        .or_else(|| after("youtube-nocookie.com/embed/"))
        .or_else(|| after("youtu.be/"));
    if let Some(id) = youtube
        .and_then(|rest| leading_id(rest, |c| c.is_ascii_alphanumeric() || c == '_' || c == '-'))
    {
        return Media::YouTube(id);
    }
    if let Some(id) =
        after("player.vimeo.com/video/").and_then(|rest| leading_id(rest, |c| c.is_ascii_digit()))
    {
        return Media::Vimeo(id);
    }
    Media::OtherEmbed(src)
}

/// The run of id characters a path segment starts with, if there is one.
fn leading_id(rest: &str, allowed: impl Fn(char) -> bool) -> Option<String> {
    let end = rest.find(|c: char| !allowed(c)).unwrap_or(rest.len());
    (end > 0).then(|| rest[..end].to_string())
}

/// Plain text out of an HTML fragment: tags removed, entities decoded, runs of
/// whitespace collapsed to a single space.
///
/// Tags that end a line in a browser become a space, so two paragraphs do not
/// run into one word; the inline ones (`<a>`, `<b>`, `<em>`...) vanish
/// without one, since the text around them already carries its own spacing.
pub fn to_text(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(start) = rest.find('<') {
        out.push_str(&rest[..start]);
        let tag = &rest[start + 1..];
        let Some(end) = tag.find('>') else {
            rest = "";
            break;
        };
        let name = tag[..end]
            .trim_start_matches('/')
            .split(|c: char| !c.is_ascii_alphanumeric())
            .next()
            .unwrap_or("");
        if BREAKING.iter().any(|b| b.eq_ignore_ascii_case(name)) {
            out.push(' ');
        }
        rest = &tag[end + 1..];
    }
    out.push_str(rest);
    decode_entities(&out)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Tags whose text does not continue on the same line.
const BREAKING: [&str; 13] = [
    "br", "p", "div", "li", "ul", "ol", "tr", "td", "hr", "center", "h1", "h2", "h3",
];

/// Decodes character references: numeric ones in both bases, and the named
/// ones these pages actually use. An unknown name is left as it was written,
/// which reads better than a guess.
pub fn decode_entities(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        let tail = &rest[amp + 1..];
        let decoded = tail
            .find(';')
            .filter(|&n| n > 0 && n <= 10)
            .and_then(|n| entity(&tail[..n]).map(|c| (c, n)));
        match decoded {
            Some((c, n)) => {
                out.push(c);
                rest = &tail[n + 1..];
            }
            None => {
                out.push('&');
                rest = tail;
            }
        }
    }
    out.push_str(rest);
    out
}

fn entity(name: &str) -> Option<char> {
    if let Some(number) = name.strip_prefix('#') {
        let code = match number.strip_prefix(['x', 'X']) {
            Some(hex) => u32::from_str_radix(hex, 16).ok()?,
            None => number.parse().ok()?,
        };
        return char::from_u32(code);
    }
    Some(match name {
        "amp" => '&',
        "lt" => '<',
        "gt" => '>',
        "quot" => '"',
        "apos" => '\'',
        "nbsp" => '\u{a0}',
        "ndash" => '\u{2013}',
        "mdash" => '\u{2014}',
        "lsquo" => '\u{2018}',
        "rsquo" => '\u{2019}',
        "ldquo" => '\u{201c}',
        "rdquo" => '\u{201d}',
        "hellip" => '\u{2026}',
        _ => return None,
    })
}

/// One tag: its lower-cased name, `/` first for a closing tag, and the raw
/// text of its attributes.
struct Tag<'a> {
    name: String,
    attrs: &'a str,
}

impl Tag<'_> {
    /// An attribute holding an absolute URL. Entities are decoded (the feed
    /// writes `&#038;` between query parameters) and a protocol-relative
    /// `//host/...` is given the `https:` a browser would. Anything else,
    /// relative paths included, is not a URL this app could download.
    fn url(&self, wanted: &str) -> Option<String> {
        let value = attribute(self.attrs, wanted)?;
        let value = decode_entities(value.trim());
        let value = match value.strip_prefix("//") {
            Some(rest) => format!("https://{rest}"),
            None => value,
        };
        (value.starts_with("https://") || value.starts_with("http://")).then_some(value)
    }
}

/// Every tag in `html`, in order. Comments, doctypes and anything else that
/// does not start with a name are stepped over.
fn tags(html: &str) -> impl Iterator<Item = Tag<'_>> {
    let mut rest = html;
    std::iter::from_fn(move || {
        loop {
            let start = rest.find('<')?;
            let inside = &rest[start + 1..];
            let end = inside.find('>').unwrap_or(inside.len());
            let (tag, after) = (&inside[..end], inside.get(end + 1..).unwrap_or(""));
            rest = after;

            let (closing, tag) = match tag.strip_prefix('/') {
                Some(t) => (true, t),
                None => (false, tag),
            };
            let name_len = tag
                .find(|c: char| !c.is_ascii_alphanumeric())
                .unwrap_or(tag.len());
            if name_len == 0 {
                continue;
            }
            let name = tag[..name_len].to_ascii_lowercase();
            return Some(Tag {
                name: if closing { format!("/{name}") } else { name },
                attrs: &tag[name_len..],
            });
        }
    })
}

/// The raw value of one attribute, matched by name without regard to case:
/// the archive's oldest pages write `<IMG SRC=...>`. Quoted with either quote,
/// or not at all.
fn attribute<'a>(attrs: &'a str, wanted: &str) -> Option<&'a str> {
    let mut rest = attrs;
    loop {
        rest = rest.trim_start();
        let name_len = rest
            .find(|c: char| c == '=' || c == '/' || c.is_whitespace())
            .unwrap_or(rest.len());
        if name_len == 0 {
            // A stray `=` or `/`, or the end of the tag.
            rest = rest.get(1..)?;
            continue;
        }
        let name = &rest[..name_len];
        rest = rest[name_len..].trim_start();

        let value = match rest.strip_prefix('=') {
            Some(after) => {
                let after = after.trim_start();
                let (value, remainder) = match after.chars().next() {
                    Some(quote @ ('"' | '\'')) => {
                        let body = &after[1..];
                        match body.find(quote) {
                            Some(close) => (&body[..close], &body[close + 1..]),
                            None => (body, ""),
                        }
                    }
                    _ => {
                        let end = after.find(char::is_whitespace).unwrap_or(after.len());
                        (&after[..end], &after[end..])
                    }
                };
                rest = remainder;
                Some(value)
            }
            None => None,
        };

        if name.eq_ignore_ascii_case(wanted) {
            return value;
        }
        if rest.is_empty() {
            return None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The template every entry is rendered in, cut down to what matters:
    /// the archive link before the date, the media block, and an explanation
    /// full of links after it.
    fn page(block: &str) -> String {
        format!(
            "<!doctype html>\n<html>\n<head>\n<title> APOD</title>\n</head>\n\
             <body BGCOLOR=\"#F4F4FF\" text=\"#000000\">\n<center>\n\
             <h1> Astronomy Picture of the Day </h1>\n<p>\n\
             <a href=\"https://science.nasa.gov/apod/archive/\">Discover the cosmos!</a>\n\
             Each day a different image.\n<p>\n2026 October 12<br>\n{block}\n</center>\n\
             <center>\n<b> Title </b> <br>\n</center>\n<p>\n<b> Explanation: </b>\n\
             <a href=\"https://science.nasa.gov/jupiter/\">Jupiter</a>\n\
             <iframe src=\"https://example.com/not-the-media\"></iframe>\n</body>\n</html>"
        )
    }

    /// 2026-10-12, verbatim: the original behind the link, the same file shown.
    #[test]
    fn a_picture_is_read_with_the_link_around_it() {
        let html = page(
            "<a href=\"https://assets.science.nasa.gov/dynamicimage/assets/science/cds/apod/apod/2026/october/JupiterMoons_Upreti_3105.jpg\">\n\
             <IMG SRC=\"https://assets.science.nasa.gov/dynamicimage/assets/science/cds/apod/apod/2026/october/JupiterMoons_Upreti_3105.jpg\" alt=\"In front of a dark background an image of Earth&#039;s Moon\"></a>",
        );
        let url = "https://assets.science.nasa.gov/dynamicimage/assets/science/cds/apod/apod/2026/october/JupiterMoons_Upreti_3105.jpg";
        assert_eq!(
            media(&html),
            Media::Image {
                link: Some(url.to_string()),
                img: Some(url.to_string()),
            }
        );
    }

    /// The archive link the template opens with is closed long before the
    /// picture, and must not be taken for the picture's own link.
    #[test]
    fn a_picture_without_a_link_of_its_own_borrows_none() {
        let html = page("<IMG SRC=\"https://assets.science.nasa.gov/content/dam/a.gif\">");
        assert_eq!(
            media(&html),
            Media::Image {
                link: None,
                img: Some("https://assets.science.nasa.gov/content/dam/a.gif".to_string()),
            }
        );
    }

    /// 2017-11-13, verbatim.
    #[test]
    fn a_video_file_is_read_from_its_source() {
        let html = page(
            "<video controls title=\"\">\n\
             <source src=\"https://assets.science.nasa.gov/content/dam/science/cds/apod/apod/2017/november/CometMachholz2017_SOHO_big.mp4\" type=\"video/mp4\">\n\
             </video>",
        );
        assert_eq!(
            media(&html),
            Media::VideoFile(
                "https://assets.science.nasa.gov/content/dam/science/cds/apod/apod/2017/november/CometMachholz2017_SOHO_big.mp4"
                    .to_string()
            )
        );
    }

    /// 2026-03-12, verbatim: protocol-relative, with a query.
    #[test]
    fn a_youtube_embed_is_read_down_to_its_id() {
        let html = page(
            "<iframe width=\"960\" height=\"540\" src=\"//www.youtube.com/embed/3jsn1829OPw?rel=0\" frameborder=\"0\" allowfullscreen></iframe>",
        );
        assert_eq!(media(&html), Media::YouTube("3jsn1829OPw".to_string()));
    }

    /// 2012-05-01, verbatim: the query separators arrive as `&#038;`.
    #[test]
    fn a_vimeo_embed_is_read_down_to_its_id() {
        let html = page(
            "<iframe src=\"https://player.vimeo.com/video/41038445?portrait=0&#038;color=c8b3df\" width=\"960\" height=\"720\" frameBorder=\"0\"></iframe>",
        );
        assert_eq!(media(&html), Media::Vimeo("41038445".to_string()));
    }

    /// 2009-04-13, verbatim: a Flash player on the old site, gone with it.
    #[test]
    fn any_other_embed_is_kept_for_what_it_is() {
        let html = page(
            "<iframe src=\"https://apod.nasa.gov/image/0904/flvplayer.swf\" width=\"960\" height=\"700\" frameBorder=\"0\"></iframe>",
        );
        assert_eq!(
            media(&html),
            Media::OtherEmbed("https://apod.nasa.gov/image/0904/flvplayer.swf".to_string())
        );
    }

    /// 2012-05-23: migrated with nothing between the date and the title. The
    /// iframe further down the page is in the explanation, not the media.
    #[test]
    fn an_empty_block_is_nothing() {
        assert_eq!(media(&page("")), Media::Nothing);
        assert_eq!(media(""), Media::Nothing);
    }

    #[test]
    fn relative_and_unquoted_urls_are_told_apart() {
        let html = page("<img src=image/1995/a.gif>");
        assert_eq!(media(&html), Media::Nothing);
        let html = page("<img src=https://example.com/a.gif alt=x>");
        assert_eq!(
            media(&html),
            Media::Image {
                link: None,
                img: Some("https://example.com/a.gif".to_string()),
            }
        );
    }

    #[test]
    fn text_loses_its_tags_and_keeps_its_words() {
        assert_eq!(
            to_text("<strong>Explanation:</strong> <a href=\"x\">Whatdunit</a>? What<br><br>next"),
            "Explanation: Whatdunit? What next"
        );
        assert_eq!(
            to_text("Curious Cometary Knots  in the Helix Nebula"),
            "Curious Cometary Knots in the Helix Nebula"
        );
    }

    #[test]
    fn entities_are_decoded_in_both_bases_and_by_name() {
        assert_eq!(
            to_text("APOD: 2012 May 23 &#8211; SpaceX &amp; Earth&#039;s Moon &#x2019; &rsquo;"),
            "APOD: 2012 May 23 \u{2013} SpaceX & Earth's Moon \u{2019} \u{2019}"
        );
        // Non-breaking spaces, written out or as an entity, are spaces.
        assert_eq!(
            to_text("in\u{a0}<a>planetary</a>&nbsp;nebulas"),
            "in planetary nebulas"
        );
        // Not an entity, or one nobody here writes: left alone.
        assert_eq!(to_text("Q&A &unknown; a & b"), "Q&A &unknown; a & b");
    }
}
