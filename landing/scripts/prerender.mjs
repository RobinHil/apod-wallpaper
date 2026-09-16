/**
 * Turns the built bundle into one static page per language.
 *
 * Vite has already written `dist/index.html`, an empty shell, and `.ssr/`,
 * the same application compiled for Node. This renders the application once
 * per language, writes the result into that shell along with the head every
 * language needs of its own, and leaves `dist/` holding two complete pages:
 * the root in English and `fr/` in French. The client bundle hydrates them.
 *
 * Two values steer it, both matching what vite.config.ts was given:
 *
 *   SITE_ORIGIN   scheme and host, no path   (default: GitHub Pages)
 *   SITE_BASE     the path the site sits at  (default: /apod-wallpaper/)
 */
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { pathToFileURL } from "node:url";

const DIST = "dist";
const SSR = ".ssr/entry-server.js";

const origin = (process.env.SITE_ORIGIN ?? "https://robinhil.github.io").replace(/\/+$/, "");
// One leading slash, one trailing slash, "/" at the root of a host: the same
// normalisation vite.config.ts applies, since `configure-pages` hands out a
// base path without its trailing slash and everything here concatenates.
const rawBase = process.env.SITE_BASE ?? "/apod-wallpaper/";
const trimmed = rawBase.replace(/^\/+|\/+$/g, "");
const base = trimmed === "" ? "/" : `/${trimmed}/`;

const { render, meta, LOCALES } = await import(pathToFileURL(SSR).href);
const shell = await readFile(join(DIST, "index.html"), "utf8");

/** The directory a language is written to, and the path it is served at. */
const pathFor = (locale) => (locale === "en" ? base : `${base}${locale}/`);
const urlFor = (locale) => `${origin}${pathFor(locale)}`;

function escapeAttribute(value) {
  return value.replace(/&/g, "&amp;").replace(/"/g, "&quot;").replace(/</g, "&lt;");
}

/**
 * The head of one page: what it is, where it is, what the other languages
 * are, and what a crawler that does not run JavaScript should show when the
 * page is shared.
 */
function head(locale) {
  const { title, description } = meta(locale);
  const url = urlFor(locale);
  const image = `${origin}${base}og-${locale}.png`;
  const ogLocale = locale === "fr" ? "fr_FR" : "en_US";
  const alternates = LOCALES.map(
    (other) => `<link rel="alternate" hreflang="${other}" href="${urlFor(other)}" />`,
  );
  alternates.push(`<link rel="alternate" hreflang="x-default" href="${urlFor("en")}" />`);

  // A free, downloadable desktop application: the one schema.org type that
  // describes this, with the fields Google actually reads.
  const jsonLd = {
    "@context": "https://schema.org",
    "@type": "SoftwareApplication",
    name: "APOD Wallpaper",
    url,
    description,
    applicationCategory: "UtilitiesApplication",
    operatingSystem: "macOS 13.3+, Linux (GNOME 42+)",
    image,
    inLanguage: locale,
    license: "https://github.com/RobinHil/apod-wallpaper/blob/main/LICENSE",
    author: { "@type": "Person", name: "Robin HILAIRE" },
    offers: { "@type": "Offer", price: "0", priceCurrency: "USD" },
  };

  return [
    `<title>${escapeAttribute(title)}</title>`,
    `<meta name="description" content="${escapeAttribute(description)}" />`,
    `<link rel="canonical" href="${url}" />`,
    ...alternates,
    `<meta property="og:type" content="website" />`,
    `<meta property="og:site_name" content="APOD Wallpaper" />`,
    `<meta property="og:locale" content="${ogLocale}" />`,
    ...LOCALES.filter((other) => other !== locale).map(
      (other) =>
        `<meta property="og:locale:alternate" content="${other === "fr" ? "fr_FR" : "en_US"}" />`,
    ),
    `<meta property="og:url" content="${url}" />`,
    `<meta property="og:title" content="${escapeAttribute(title)}" />`,
    `<meta property="og:description" content="${escapeAttribute(description)}" />`,
    `<meta property="og:image" content="${image}" />`,
    `<meta property="og:image:width" content="1200" />`,
    `<meta property="og:image:height" content="630" />`,
    `<meta name="twitter:card" content="summary_large_image" />`,
    `<meta name="twitter:title" content="${escapeAttribute(title)}" />`,
    `<meta name="twitter:description" content="${escapeAttribute(description)}" />`,
    `<meta name="twitter:image" content="${image}" />`,
    `<script type="application/ld+json">${JSON.stringify(jsonLd)}</script>`,
  ].join("\n    ");
}

for (const locale of LOCALES) {
  const { lang } = meta(locale);

  const page = shell
    .replace('<html lang="en"', `<html lang="${lang}"`)
    .replace(/<!--seo:start-->[\s\S]*?<!--seo:end-->/, head(locale))
    .replace(
      '<div id="app"></div>',
      `<div id="app" data-locale="${locale}">${render(locale)}</div>`,
    );

  const directory = locale === "en" ? DIST : join(DIST, locale);
  await mkdir(directory, { recursive: true });
  await writeFile(join(directory, "index.html"), page);
  console.log(`prerendered ${pathFor(locale)}`);
}

/**
 * The page GitHub Pages serves for an address that is not there.
 *
 * Written by hand rather than rendered: it has to work when the application
 * does not, so it carries no JavaScript and inlines what it needs. It does
 * borrow the bundle's stylesheet, which is where the two typefaces are
 * declared, and says `noindex` because a 404 has nothing to be found in.
 */
const stylesheet = shell.match(/href="([^"]+\.css)"/)?.[1] ?? "";

await writeFile(
  join(DIST, "404.html"),
  `<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <meta name="color-scheme" content="dark" />
    <meta name="theme-color" content="#07090f" />
    <meta name="robots" content="noindex" />
    <title>Page not found - APOD Wallpaper</title>
    <link rel="icon" href="${base}app-icon.svg" type="image/svg+xml" />
    ${stylesheet ? `<link rel="stylesheet" href="${stylesheet}" />` : ""}
    <style>
      body {
        margin: 0;
        min-height: 100vh;
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
        gap: 18px;
        padding: 24px;
        text-align: center;
        background: radial-gradient(60% 60% at 50% 0%, #131a2e 0%, #07090f 70%);
        color: #e9ebf2;
        font-family: "Inter", -apple-system, BlinkMacSystemFont, sans-serif;
      }
      img { width: 56px; height: 56px; }
      h1 { margin: 0; font-family: "Instrument Serif", serif; font-weight: 400;
           font-size: 44px; line-height: 1.1; }
      p { margin: 0; color: #8f96a8; font-size: 15px; }
      nav { display: flex; gap: 10px; margin-top: 8px; }
      a { border: 1px solid #222736; border-radius: 10px; padding: 9px 16px;
          color: #e9ebf2; text-decoration: none; font-size: 14px;
          font-weight: 600; background: rgba(255, 255, 255, 0.04); }
      a:hover { border-color: #4c85f0; }
    </style>
  </head>
  <body>
    <img src="${base}app-icon.svg" alt="" />
    <h1>This corner of the sky is empty</h1>
    <p>Page not found. Cette page n'existe pas.</p>
    <nav>
      <a href="${pathFor("en")}">English</a>
      <a href="${pathFor("fr")}">Français</a>
    </nav>
  </body>
</html>
`,
);
console.log("wrote 404.html");

// The sitemap names both pages and says they are translations of each other.
const sitemap = `<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9"
        xmlns:xhtml="http://www.w3.org/1999/xhtml">
${LOCALES.map(
  (locale) => `  <url>
    <loc>${urlFor(locale)}</loc>
${LOCALES.map(
  (other) =>
    `    <xhtml:link rel="alternate" hreflang="${other}" href="${urlFor(other)}" />`,
).join("\n")}
    <xhtml:link rel="alternate" hreflang="x-default" href="${urlFor("en")}" />
  </url>`,
).join("\n")}
</urlset>
`;
await writeFile(join(DIST, "sitemap.xml"), sitemap);

// Only read when the site owns the root of its host, which is the case for
// the Docker image and not for a GitHub Pages project site. Harmless there.
await writeFile(
  join(DIST, "robots.txt"),
  `User-agent: *\nAllow: /\n\nSitemap: ${origin}${base}sitemap.xml\n`,
);
console.log("wrote sitemap.xml and robots.txt");
