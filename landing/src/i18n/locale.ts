/** The languages the page is written in. English is the default. */
export type Locale = "en" | "fr";

export const LOCALES: Locale[] = ["en", "fr"];

/** Where a visitor's own choice is remembered, so it survives a reload. */
export const STORED = "apod-landing-locale";

export function isLocale(value: unknown): value is Locale {
  return value === "en" || value === "fr";
}

/**
 * The language the browser asks for: the first entry of its ordered
 * preference list that names a language this page has. A visitor asking for
 * `de` and then `fr` reads French rather than English.
 */
export function preferred(): Locale {
  if (typeof navigator === "undefined") return "en";

  const preferences = navigator.languages?.length
    ? navigator.languages
    : [navigator.language];

  for (const preference of preferences) {
    const tag = preference.toLowerCase();
    if (tag.startsWith("fr")) return "fr";
    if (tag.startsWith("en")) return "en";
  }

  return "en";
}

/** The choice made on the page, if one was ever made. */
export function remembered(): Locale | null {
  try {
    const stored = localStorage.getItem(STORED);
    return isLocale(stored) ? stored : null;
  } catch {
    // Private mode, blocked storage: nothing was remembered.
    return null;
  }
}

export function remember(locale: Locale): void {
  try {
    localStorage.setItem(STORED, locale);
  } catch {
    // Not being able to remember it is not a reason to refuse the change.
  }
}
