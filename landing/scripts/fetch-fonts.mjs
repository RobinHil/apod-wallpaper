/**
 * Brings the two typefaces into the repository.
 *
 * Self-hosted rather than linked: a stylesheet from fonts.googleapis.com
 * blocks the first paint on a third-party connection, and it tells Google who
 * is reading the page. This asks the same API for the same faces, saves the
 * woff2 files under src/fonts, and writes src/fonts.css pointing at them.
 *
 * Under src rather than public so the bundler owns them: it hashes each file
 * and rewrites the url() to sit under whatever `base` the site is built for,
 * which a path written by hand could not follow.
 *
 * Run it again when a face or a weight changes:
 *
 *   node scripts/fetch-fonts.mjs
 *
 * It does not touch src/fonts/OFL.txt, which is the licence both typefaces
 * are under and which has to keep travelling with the files. Adding a third
 * face means adding its copyright line there by hand.
 */
import { mkdir, writeFile } from "node:fs/promises";
import { basename, join } from "node:path";

// Inter as a variable font, which is one file instead of four, and
// Instrument Serif upright and italic.
const API =
  "https://fonts.googleapis.com/css2" +
  "?family=Instrument+Serif:ital@0;1&family=Inter:wght@400..700&display=swap";

// A modern browser's user agent, or the API answers with ttf.
const UA =
  "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0 Safari/537.36";

// The page is in English and French: the other subsets are weight nobody
// downloads. The comment above each block is what names the subset.
const KEEP = new Set(["latin", "latin-ext"]);

const OUT_DIR = "src/fonts";
const CSS = "src/fonts.css";

const css = await fetch(API, { headers: { "User-Agent": UA } }).then((r) => {
  if (!r.ok) throw new Error(`Google Fonts answered ${r.status}`);
  return r.text();
});

await mkdir(OUT_DIR, { recursive: true });

const blocks = css.split("/*").slice(1);
const kept = [];

for (const block of blocks) {
  const subset = block.slice(0, block.indexOf("*/")).trim();
  if (!KEEP.has(subset)) continue;

  const rule = block.slice(block.indexOf("*/") + 2);
  const url = rule.match(/url\((https:[^)]+)\)/)?.[1];
  if (!url) continue;

  const file = basename(new URL(url).pathname);
  const bytes = Buffer.from(await fetch(url).then((r) => r.arrayBuffer()));
  await writeFile(join(OUT_DIR, file), bytes);

  kept.push(`/* ${subset} */${rule.replace(url, `./fonts/${file}`)}`);
  console.log(`${file}  ${(bytes.length / 1024).toFixed(1)} kB`);
}

await writeFile(
  CSS,
  `/* Written by scripts/fetch-fonts.mjs. Do not edit by hand: run the script.\n` +
    `   The files these point at live in src/fonts and are hashed into the\n` +
    `   bundle, so no browser reading this page ever talks to Google. */\n\n` +
    kept.join("\n").trim() +
    "\n",
);
console.log(`wrote ${CSS}`);
