/** Where every outbound link on the page points. */

export const REPO = "https://github.com/RobinHil/apod-wallpaper";
export const RELEASES = `${REPO}/releases/latest`;
export const LICENCE = `${REPO}/blob/main/LICENSE`;
export const README = `${REPO}#readme`;
export const APOD_SITE = "https://apod.nasa.gov/apod/astropix.html";
export const API_KEY_SIGNUP = "https://api.nasa.gov";

/** Official page of an APOD: https://apod.nasa.gov/apod/apYYMMDD.html */
export function apodPageUrl(date: string): string {
  const [y, m, d] = date.split("-");
  return `https://apod.nasa.gov/apod/ap${y.slice(2)}${m}${d}.html`;
}
