/** External URLs the panel opens in the user's browser. */

import type { Applied } from "./types";

/**
 * Where APOD's entries can be browsed. A record written before APOD moved to
 * science.nasa.gov has no article address, and the apod.nasa.gov page it was
 * published on now redirects to the home page; this is the nearest thing.
 */
const APOD_ARCHIVE = "https://science.nasa.gov/apod/archive/";

/** The entry's article on science.nasa.gov. */
export function apodPageUrl(current: Applied): string {
  return current.page_url ?? APOD_ARCHIVE;
}

/**
 * Where to send someone who wants to watch a video APOD.
 *
 * The backend reports embed links, which are converted to the pages a browser
 * can play them on. Anything else -- a video published as a plain file, a
 * player the old site embedded -- goes to the APOD entry: a raw `.mp4` would
 * drop the viewer into a bare player with none of the explanation around it,
 * and the old players are gone with the site.
 */
export function watchableVideoUrl(current: Applied): string {
  const url = current.video_url ?? "";
  const youtube = url.match(/youtube\.com\/embed\/([A-Za-z0-9_-]+)/);
  if (youtube) return `https://www.youtube.com/watch?v=${youtube[1]}`;
  const vimeo = url.match(/player\.vimeo\.com\/video\/(\d+)/);
  if (vimeo) return `https://vimeo.com/${vimeo[1]}`;
  return apodPageUrl(current);
}

/**
 * Mirrors the check the backend makes on the same URL: a file APOD decodes a
 * frame from, as opposed to an embed link it cannot.
 */
export function isVideoFile(url: string): boolean {
  const name = url.split(/[?#]/)[0].split("/").pop() ?? "";
  return /\.(mp4|mov|m4v|m2v|mpg)$/i.test(name);
}
