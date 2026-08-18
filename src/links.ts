/** External URLs the panel opens in the user's browser. */

/** Official page of an APOD: https://apod.nasa.gov/apod/apYYMMDD.html */
export function apodPageUrl(date: string): string {
  const [y, m, d] = date.split("-");
  return `https://apod.nasa.gov/apod/ap${y.slice(2)}${m}${d}.html`;
}

/**
 * The API serves embed links; convert them to pages that can be watched
 * directly in a browser. Unknown URLs are opened as they are.
 */
export function watchableVideoUrl(url: string): string {
  const youtube = url.match(/youtube\.com\/embed\/([A-Za-z0-9_-]+)/);
  if (youtube) return `https://www.youtube.com/watch?v=${youtube[1]}`;
  const vimeo = url.match(/player\.vimeo\.com\/video\/(\d+)/);
  if (vimeo) return `https://vimeo.com/${vimeo[1]}`;
  return url;
}
