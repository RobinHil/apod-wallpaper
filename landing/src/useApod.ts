import { useEffect, useState } from "react";

/** What this page shows of an APOD: plain text, and one picture to draw. */
export interface Apod {
  date: string;
  title: string;
  explanation: string;
  credit: string | null;
  /** The entry's article on science.nasa.gov. */
  page: string;
  /** The picture to draw, or null when the entry has none this page can show. */
  still: string | null;
}

/** The fields of NASA's APOD feed this page reads, named as the feed names them. */
interface Entry {
  date: string;
  title?: string | null;
  explanation?: string | null;
  copyright?: string | null;
  hdurl?: string | null;
  permalink?: string | null;
  basic_html?: string | null;
}

/**
 * APOD's feed on science.nasa.gov, where APOD moved in September 2026: the
 * latest entry, and only the fields read below. It needs no key, and it
 * answers this page's origin with the CORS headers a browser wants.
 */
const FEED =
  "https://science.nasa.gov/wp-json/wp/v2/apod-basic?per_page=1&_fields=date,title,explanation,copyright,hdurl,permalink,basic_html";

/**
 * Folder of the generic pictures the feed gives as `hdurl` when it knows no
 * image for an entry -- most videos, and the GIFs of the early archive.
 */
const PLACEHOLDER_DIR = "/science/astro/programs/cosmic-origins/images/misc/";

/**
 * A few entries were migrated with the old page title as their own: "APOD:
 * 2012 May 23", an en dash, then the title.
 */
const PAGE_TITLE = new RegExp(`^APOD: \\d[^${String.fromCodePoint(0x2013)}]*. `);

/** The picture the hero shows when the feed answers in time, and null otherwise. */
export function useApod(): Apod | null {
  const [apod, setApod] = useState<Apod | null>(null);

  useEffect(() => {
    const abort = new AbortController();

    fetch(FEED, { signal: abort.signal })
      .then((response) => (response.ok ? response.json() : null))
      .then((data: Entry[] | null) => {
        const entry = Array.isArray(data) ? data[0] : undefined;
        if (entry && typeof entry.date === "string") setApod(read(entry));
      })
      // An outage, a network that is not there: the page keeps its own sky
      // and says nothing about it.
      .catch(() => undefined);

    return () => abort.abort();
  }, []);

  return apod;
}

/** An entry made fit to show: its fields arrive as HTML. */
function read(entry: Entry): Apod {
  // "Image Credit:", "Credit & Copyright:" and the like open some credit
  // lines; the panel prints its own label.
  const credit = text(entry.copyright).replace(/^[^:]{0,40}credit[^:]*:\s*/i, "");
  return {
    date: entry.date,
    title: text(entry.title).replace(PAGE_TITLE, ""),
    explanation: text(entry.explanation)
      .replace(/^Explanation:\s*/, "")
      .replace(/\s*Tomorrow['’]s picture:.*$/, ""),
    credit: credit === "" ? null : credit,
    page: entry.permalink || "https://science.nasa.gov/apod/",
    still: still(entry),
  };
}

/** Plain text out of an HTML fragment, with its whitespace collapsed. */
function text(html: string | null | undefined): string {
  if (!html) return "";
  const doc = new DOMParser().parseFromString(html.replace(/<br\s*\/?>/gi, " "), "text/html");
  return (doc.body.textContent ?? "").replace(/\s+/g, " ").trim();
}

/**
 * The still for an entry.
 *
 * `hdurl` as published, and not the stored original the application downloads
 * for a wallpaper: the published one goes through NASA's resizing service and
 * comes back at about 1280 pixels and a few hundred kilobytes, where the
 * original can weigh a hundred megabytes, for a frame this page never draws
 * wider than about 700 CSS pixels.
 *
 * When `hdurl` is a placeholder, the entry's page says what was published: an
 * image, or a YouTube video whose thumbnail stands in for it.
 */
function still(entry: Entry): string | null {
  const hdurl = entry.hdurl?.trim();
  if (hdurl && !hdurl.includes(PLACEHOLDER_DIR)) return hdurl;

  const page = new DOMParser().parseFromString(entry.basic_html ?? "", "text/html");
  // The first block of the page holds the media; the explanation after it is
  // full of links that are not the picture.
  const block = page.querySelector("body > center");
  const embed = block?.querySelector("iframe")?.getAttribute("src") ?? "";
  const youtube = embed.match(/youtube(?:-nocookie)?\.com\/embed\/([A-Za-z0-9_-]+)/);
  if (youtube) return `https://img.youtube.com/vi/${youtube[1]}/hqdefault.jpg`;
  const img = block?.querySelector("img")?.getAttribute("src") ?? "";
  return /^https?:\/\//.test(img) ? img : null;
}
