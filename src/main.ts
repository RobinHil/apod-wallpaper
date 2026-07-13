import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { openUrl } from "@tauri-apps/plugin-opener";

type Mode = "daily" | "random";
type FitMode = "blur_fill" | "crop_fill";

interface CacheEntry {
  date: string;
  title: string;
  explanation: string;
  copyright: string | null;
  source_url: string;
  image_file: string;
  fetched_at: string;
}

interface UiState {
  mode: Mode;
  fit_mode: FitMode;
  api_key: string;
  using_demo_key: boolean;
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

let currentDate: string | null = null;

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

  if (state.current) {
    currentDate = state.current.date;
    title.textContent = state.current.title;
    date.textContent = state.current.date;
    copyright.textContent = state.current.copyright
      ? `© ${state.current.copyright}`
      : "NASA (domaine public)";
    copyright.hidden = false;
    explanation.textContent = state.current.explanation;
    openPage.hidden = false;
  } else {
    currentDate = null;
    title.textContent = "Aucune image chargée";
    date.textContent = "-";
    copyright.hidden = true;
    explanation.textContent = "";
    openPage.hidden = true;
  }

  el<HTMLButtonElement>("mode-daily").classList.toggle("active", state.mode === "daily");
  el<HTMLButtonElement>("mode-random").classList.toggle("active", state.mode === "random");
  el<HTMLButtonElement>("fit-blur").classList.toggle("active", state.fit_mode === "blur_fill");
  el<HTMLButtonElement>("fit-crop").classList.toggle("active", state.fit_mode === "crop_fill");

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

  el<HTMLButtonElement>("mode-daily").addEventListener("click", () => {
    void run("set_mode", { mode: "daily" }, "Passage en mode image du jour...");
  });
  el<HTMLButtonElement>("mode-random").addEventListener("click", () => {
    void run("set_mode", { mode: "random" }, "Tirage d'une image aléatoire...");
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
  el<HTMLButtonElement>("open-nasa").addEventListener("click", () => {
    void openUrl("https://api.nasa.gov/");
  });

  el<HTMLButtonElement>("quit").addEventListener("click", () => {
    void invoke("quit_app");
  });
});
