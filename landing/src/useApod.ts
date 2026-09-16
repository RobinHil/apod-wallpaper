import { useEffect, useState } from "react";

/** The fields of the APOD API this page shows, named as the API names them. */
export interface Apod {
  date: string;
  title: string;
  explanation: string;
  copyright?: string;
  media_type: string;
  url: string;
  hdurl?: string;
  thumbnail_url?: string;
}

/**
 * `DEMO_KEY` is rate limited per address, which is exactly the right default
 * here: the page has to survive being handed nothing. Set `VITE_NASA_API_KEY`
 * at build time to use a key of your own.
 */
const KEY = import.meta.env.VITE_NASA_API_KEY ?? "DEMO_KEY";

/** The picture the hero shows when the API answers in time, and null otherwise. */
export function useApod(): Apod | null {
  const [apod, setApod] = useState<Apod | null>(null);

  useEffect(() => {
    const abort = new AbortController();

    // `thumbs=true` is what makes a video entry usable here: the API returns
    // the published thumbnail alongside the embed URL, which is the same
    // still the application would put on the desktop.
    fetch(`https://api.nasa.gov/planetary/apod?thumbs=true&api_key=${KEY}`, {
      signal: abort.signal,
    })
      .then((response) => (response.ok ? response.json() : null))
      .then((data: Apod | null) => {
        if (data && typeof data.url === "string") setApod(data);
      })
      // A quota that ran out, a network that is not there, an outage: the
      // page keeps its own sky and says nothing about it.
      .catch(() => undefined);

    return () => abort.abort();
  }, []);

  return apod;
}

/** The still for an entry, which is the thumbnail when the entry is a video. */
export function apodStill(apod: Apod): string {
  if (apod.media_type === "video") return apod.thumbnail_url ?? apod.url;
  return apod.hdurl ?? apod.url;
}
