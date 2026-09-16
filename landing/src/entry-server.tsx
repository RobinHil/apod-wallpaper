import { renderToString } from "react-dom/server";
import { App } from "./App";
import { copyFor } from "./i18n";
import { LOCALES, type Locale } from "./i18n/locale";

/**
 * What the prerender calls, once per language.
 *
 * Nothing here touches the network or the document: `useApod` and the
 * starfield are effects, so the HTML that comes out is the page with its
 * painted sky, which is also what a visitor without JavaScript keeps.
 */
export function render(locale: Locale): string {
  return renderToString(<App locale={locale} />);
}

/** The per-language head values, read from the same dictionaries. */
export function meta(locale: Locale): {
  lang: string;
  title: string;
  description: string;
} {
  const copy = copyFor(locale);
  return {
    lang: copy.htmlLang,
    title: copy.documentTitle,
    description: copy.documentDescription,
  };
}

export { LOCALES };
