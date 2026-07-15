import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { openUrl } from "@tauri-apps/plugin-opener";

type Mode = "daily" | "random" | "specific";
type FitMode = "blur_fill" | "crop_fill";

interface CacheEntry {
  date: string;
  title: string;
  explanation: string;
  copyright: string | null;
  media_type: string;
  video_url: string | null;
  source_url: string;
  image_file: string;
  fetched_at: string;
}

interface UiState {
  mode: Mode;
  fit_mode: FitMode;
  api_key: string;
  using_demo_key: boolean;
  specific_date: string;
  offline: boolean;
  status_message: string | null;
  last_check: string | null;
  current: CacheEntry | null;
}

function el<T extends HTMLElement>(id: string): T {
  const node = document.getElementById(id);
  if (!node) throw new Error(`Élément introuvable : ${id}`);
  return node as T;
}

/** Page officielle d'une APOD : https://apod.nasa.gov/apod/apYYMMDD.html */
function apodPageUrl(date: string): string {
  const [y, m, d] = date.split("-");
  return `https://apod.nasa.gov/apod/ap${y.slice(2)}${m}${d}.html`;
}

/**
 * L'API fournit des liens d'integration (embed) ; on les convertit en pages
 * regardables directement dans le navigateur. Les URLs inconnues sont
 * ouvertes telles quelles.
 */
function watchableVideoUrl(url: string): string {
  const youtube = url.match(/youtube\.com\/embed\/([A-Za-z0-9_-]+)/);
  if (youtube) return `https://www.youtube.com/watch?v=${youtube[1]}`;
  const vimeo = url.match(/player\.vimeo\.com\/video\/(\d+)/);
  if (vimeo) return `https://vimeo.com/${vimeo[1]}`;
  return url;
}

let currentDate: string | null = null;
let currentVideoUrl: string | null = null;
/** Le selecteur de date reste visible apres un clic sur "Date précise",
 *  meme tant que le mode reel n'a pas encore bascule. */
let datePickerRequested = false;

// -----------------------------------------------------------------------
// Blocage de l'interface et affichage des erreurs.
//
// Chaque action passe par run() : l'interface entière est masquée par un
// overlay tant que le backend n'a pas terminé (ou échoué), puis l'état
// renvoyé par la commande est affiché. Toute erreur apparaît dans un
// bandeau : rien n'échoue en silence.
// -----------------------------------------------------------------------

let pending = false;

function setBlocked(blocked: boolean, label?: string): void {
  el<HTMLSpanElement>("overlay-label").textContent = label ?? "Application en cours...";
  el<HTMLDivElement>("overlay").hidden = !blocked;
}

function showError(message: string): void {
  el<HTMLSpanElement>("error-text").textContent = message;
  el<HTMLDivElement>("error-banner").hidden = false;
}

function hideError(): void {
  el<HTMLDivElement>("error-banner").hidden = true;
}

async function run(command: string, args?: Record<string, unknown>, label?: string): Promise<void> {
  if (pending) return;
  pending = true;
  setBlocked(true, label);
  hideError();
  try {
    render(await invoke<UiState>(command, args));
  } catch (error) {
    showError(String(error));
    // Resynchronise l'affichage avec l'etat reel du backend apres un echec.
    try {
      render(await invoke<UiState>("get_state"));
    } catch {
      // Backend injoignable : on garde l'affichage courant et l'erreur visible.
    }
  } finally {
    pending = false;
    setBlocked(false);
  }
}

// -----------------------------------------------------------------------
// Rendu
// -----------------------------------------------------------------------

function render(state: UiState): void {
  const pill = el<HTMLSpanElement>("status-pill");
  if (state.offline) {
    pill.textContent = "Hors-ligne";
    pill.className = "pill offline";
  } else if (state.current) {
    pill.textContent = "En ligne";
    pill.className = "pill online";
  } else {
    pill.textContent = "Chargement";
    pill.className = "pill";
  }

  const title = el<HTMLHeadingElement>("apod-title");
  const date = el<HTMLSpanElement>("apod-date");
  const copyright = el<HTMLSpanElement>("apod-copyright");
  const explanation = el<HTMLParagraphElement>("apod-explanation");
  const openPage = el<HTMLButtonElement>("open-page");

  const videoNotice = el<HTMLDivElement>("video-notice");

  if (state.current) {
    currentDate = state.current.date;
    currentVideoUrl = state.current.video_url;
    title.textContent = state.current.title;
    date.textContent = state.current.date;
    copyright.textContent = state.current.copyright
      ? `© ${state.current.copyright}`
      : "NASA (domaine public)";
    copyright.hidden = false;
    explanation.textContent = state.current.explanation;
    openPage.hidden = false;
    videoNotice.hidden = state.current.media_type !== "video";
  } else {
    currentDate = null;
    currentVideoUrl = null;
    title.textContent = "Aucune image chargée";
    date.textContent = "-";
    copyright.hidden = true;
    explanation.textContent = "";
    openPage.hidden = true;
    videoNotice.hidden = true;
  }

  el<HTMLButtonElement>("mode-daily").classList.toggle("active", state.mode === "daily");
  el<HTMLButtonElement>("mode-random").classList.toggle("active", state.mode === "random");
  el<HTMLButtonElement>("mode-specific").classList.toggle("active", state.mode === "specific");
  el<HTMLButtonElement>("fit-blur").classList.toggle("active", state.fit_mode === "blur_fill");
  el<HTMLButtonElement>("fit-crop").classList.toggle("active", state.fit_mode === "crop_fill");

  el<HTMLDivElement>("date-picker").hidden = state.mode !== "specific" && !datePickerRequested;
  const dateInput = el<HTMLInputElement>("specific-date");
  if (document.activeElement !== dateInput && state.specific_date) {
    dateInput.value = state.specific_date;
  }

  const keyInput = el<HTMLInputElement>("api-key");
  if (document.activeElement !== keyInput) {
    keyInput.value = state.api_key;
  }

  const parts: string[] = [];
  if (state.status_message) parts.push(state.status_message);
  if (state.last_check) parts.push(`Dernière vérification : ${state.last_check}`);
  el<HTMLSpanElement>("last-check").textContent = parts.join(" — ");
}

async function refreshState(): Promise<void> {
  try {
    render(await invoke<UiState>("get_state"));
  } catch (error) {
    showError(String(error));
  }
}

// -----------------------------------------------------------------------
// Cablage
// -----------------------------------------------------------------------

window.addEventListener("DOMContentLoaded", () => {
  void refreshState();

  // Mises a jour poussees par le backend (verification quotidienne,
  // demarrage...) : on ne rafraichit pas pendant une action en cours pour
  // ne pas perturber le blocage.
  void listen<UiState>("state-updated", (event) => {
    if (!pending) render(event.payload);
  });

  // Borne haute du selecteur : aujourd'hui (heure locale).
  const now = new Date();
  const today = `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, "0")}-${String(now.getDate()).padStart(2, "0")}`;
  el<HTMLInputElement>("specific-date").max = today;

  el<HTMLButtonElement>("mode-daily").addEventListener("click", () => {
    datePickerRequested = false;
    void run("set_mode", { mode: "daily" }, "Passage en mode image du jour...");
  });
  el<HTMLButtonElement>("mode-random").addEventListener("click", () => {
    datePickerRequested = false;
    void run("set_mode", { mode: "random" }, "Tirage d'une image aléatoire...");
  });
  el<HTMLButtonElement>("mode-specific").addEventListener("click", () => {
    // Pas d'appel backend ici : on devoile le selecteur, le mode ne bascule
    // reellement qu'une fois une date choisie et appliquee.
    datePickerRequested = true;
    el<HTMLDivElement>("date-picker").hidden = false;
    el<HTMLInputElement>("specific-date").focus();
  });
  el<HTMLFormElement>("date-form").addEventListener("submit", (e) => {
    e.preventDefault();
    const date = el<HTMLInputElement>("specific-date").value;
    if (!date) {
      showError("Choisissez d'abord une date.");
      return;
    }
    void run("set_specific_date", { date }, `Chargement de l'APOD du ${date}...`);
  });
  el<HTMLButtonElement>("fit-blur").addEventListener("click", () => {
    void run("set_fit_mode", { fit: "blur_fill" }, "Recomposition du fond d'écran...");
  });
  el<HTMLButtonElement>("fit-crop").addEventListener("click", () => {
    void run("set_fit_mode", { fit: "crop_fill" }, "Recomposition du fond d'écran...");
  });
  el<HTMLButtonElement>("refresh").addEventListener("click", () => {
    void run("refresh_now", undefined, "Vérification de la dernière image...");
  });

  el<HTMLFormElement>("key-form").addEventListener("submit", (e) => {
    e.preventDefault();
    const key = el<HTMLInputElement>("api-key").value;
    void run("set_api_key", { key }, "Enregistrement de la clé...");
  });

  el<HTMLButtonElement>("error-close").addEventListener("click", hideError);

  el<HTMLButtonElement>("open-page").addEventListener("click", () => {
    if (currentDate) void openUrl(apodPageUrl(currentDate));
  });
  el<HTMLButtonElement>("open-video").addEventListener("click", () => {
    if (currentVideoUrl) void openUrl(watchableVideoUrl(currentVideoUrl));
  });
  el<HTMLButtonElement>("open-nasa").addEventListener("click", () => {
    void openUrl("https://api.nasa.gov/");
  });

  el<HTMLButtonElement>("quit").addEventListener("click", () => {
    void invoke("quit_app");
  });
});
