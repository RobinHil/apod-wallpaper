/** External URLs the panel opens in the user's browser. */

/** Official page of an APOD: https://apod.nasa.gov/apod/apYYMMDD.html */
export function apodPageUrl(date: string): string {
  const [y, m, d] = date.split("-");
  return `https://apod.nasa.gov/apod/ap${y.slice(2)}${m}${d}.html`;
}

/**
 * Where to send someone who wants to watch a video APOD.
 *
 * The API serves embed links, which are converted to the pages a browser can
 * play them on. A video published as a plain file has no page of its own, and
 * handing over the raw `.mp4` would drop the viewer into a bare player with
 * none of the explanation around it -- so those go to the APOD entry, which is
 * where the video is meant to be watched.
 */
export function watchableVideoUrl(url: string, date: string): string {
  const youtube = url.match(/youtube\.com\/embed\/([A-Za-z0-9_-]+)/);
  if (youtube) return `https://www.youtube.com/watch?v=${youtube[1]}`;
  const vimeo = url.match(/player\.vimeo\.com\/video\/(\d+)/);
  if (vimeo) return `https://vimeo.com/${vimeo[1]}`;
  if (isVideoFile(url)) return apodPageUrl(date);
  return url;
}

/**
 * Mirrors the check the backend makes on the same URL: a file APOD decodes a
 * frame from, as opposed to an embed link it cannot.
 */
export function isVideoFile(url: string): boolean {
  const name = url.split(/[?#]/)[0].split("/").pop() ?? "";
  return /\.(mp4|mov|m4v|m2v|mpg)$/i.test(name);
}
