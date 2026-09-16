import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

// @ts-expect-error process is a nodejs global
const env = process.env as Record<string, string | undefined>;

/**
 * Where the site will live.
 *
 * GitHub Pages serves this project from a subdirectory, so that is the
 * default; a deployment at the root of a domain, which is what the Docker
 * image does, passes `SITE_BASE=/`. Every asset URL, and the `/fr/` page the
 * prerender writes, is derived from this one value.
 */
const base = normalise(env.SITE_BASE ?? "/apod-wallpaper/");

/**
 * A base Vite and the prerender can both rely on: one leading slash, one
 * trailing slash, "/" for a site at the root of its host. `configure-pages`
 * hands out "/apod-wallpaper" without the trailing slash, and every URL this
 * site writes is built by concatenation, so normalising once here is what
 * keeps `/fr/` from becoming `/frfr`.
 */
function normalise(value: string): string {
  const trimmed = value.replace(/^\/+|\/+$/g, "");
  return trimmed === "" ? "/" : `/${trimmed}/`;
}

/**
 * `pnpm dev` serves one shell and no `fr/` directory, since the French page
 * only exists once the prerender has run. This hands the shell back for that
 * address so the language switch works in development too: the client finds
 * no `data-locale`, reads the choice that was just remembered, and renders
 * French.
 */
function frenchInDev() {
  return {
    name: "french-page-in-dev",
    apply: "serve" as const,
    configureServer(server: { middlewares: { use: (fn: unknown) => void } }) {
      server.middlewares.use((req: { url?: string }, _res: unknown, next: () => void) => {
        if (req.url && req.url.startsWith(`${base}fr/`)) req.url = base;
        next();
      });
    },
  };
}

export default defineConfig({
  base,
  plugins: [react(), tailwindcss(), frenchInDev()],

  build: {
    // The prerender imports the server bundle from here; the client build
    // writes `dist/` as usual.
    emptyOutDir: true,
  },

  server: {
    port: 5173,
    strictPort: true,
    // Bound to every interface so the container publishes it.
    host: true,
  },

  preview: {
    port: 4173,
    strictPort: true,
    host: true,
  },
});
