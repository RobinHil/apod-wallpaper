import { StrictMode } from "react";
import "./styles.css";
import { createRoot, hydrateRoot } from "react-dom/client";
import { App } from "./App";
import { isLocale, preferred, remembered } from "./i18n/locale";

const container = document.getElementById("app");
if (!container) throw new Error("Mount point #app not found");

/**
 * Which language this document is.
 *
 * A prerendered page says so on the mount point, and that is the only answer
 * that may be used: hydration has to render exactly what the HTML already
 * contains. `pnpm dev` serves an empty shell instead, and there the browser's
 * preference decides.
 */
const declared = container.dataset.locale;
const locale = isLocale(declared) ? declared : (remembered() ?? preferred());

const page = (
  <StrictMode>
    <App locale={locale} />
  </StrictMode>
);

// An empty container is the development shell; anything else came from the
// prerender and is hydrated rather than thrown away.
if (container.firstChild) hydrateRoot(container, page);
else createRoot(container).render(page);
