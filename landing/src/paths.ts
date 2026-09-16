import type { Locale } from "./i18n/locale";

/**
 * The address of each language.
 *
 * English is the site root and French is a directory under it, which is what
 * the prerender writes and what `hreflang` points at. Both are derived from
 * Vite's `BASE_URL`, so the same code works at the root of a domain and under
 * the subdirectory GitHub Pages serves the project from.
 */
export const BASE = import.meta.env.BASE_URL;

export function localePath(locale: Locale): string {
  return locale === "fr" ? `${BASE}fr/` : BASE;
}

/** The asset URLs that are written by hand rather than imported. */
export function asset(name: string): string {
  return `${BASE}${name}`;
}
