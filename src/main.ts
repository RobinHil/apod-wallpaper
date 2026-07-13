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
    if (state.current.copyright) {
      copyright.textContent = `© ${state.current.copyright}`;
      copyright.hidden = false;
    } else {
      copyright.textContent = "NASA (domaine public)";
      copyright.hidden = false;
    }
    explanation.textContent = state.current.explanation;
    openPage.hidden = false;
  } else {
    currentDate = null;
    title.textContent = "Aucune image chargée";
    date.textContent = "-";
    copyright.hidden = true;
    explanation.textContent = state.status_message ?? "";
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

  const refresh = el<HTMLButtonElement>("refresh");
  refresh.disabled = false;
  refresh.classList.remove("busy");
}

async function refreshState(): Promise<void> {
  try {
    render(await invoke<UiState>("get_state"));
  } catch (e) {
    console.error("get_state a échoué :", e);
  }
}

window.addEventListener("DOMContentLoaded", () => {
  void refreshState();
  void listen<UiState>("state-updated", (event) => render(event.payload));

  el<HTMLButtonElement>("mode-daily").addEventListener("click", () => {
    void invoke("set_mode", { mode: "daily" });
  });
  el<HTMLButtonElement>("mode-random").addEventListener("click", () => {
    void invoke("set_mode", { mode: "random" });
  });
  el<HTMLButtonElement>("fit-blur").addEventListener("click", () => {
    void invoke("set_fit_mode", { fit: "blur_fill" });
  });
  el<HTMLButtonElement>("fit-crop").addEventListener("click", () => {
    void invoke("set_fit_mode", { fit: "crop_fill" });
  });

  el<HTMLButtonElement>("refresh").addEventListener("click", (e) => {
    const btn = e.currentTarget as HTMLButtonElement;
    btn.disabled = true;
    btn.classList.add("busy");
    void invoke("refresh_now");
  });

  el<HTMLFormElement>("key-form").addEventListener("submit", (e) => {
    e.preventDefault();
    const key = el<HTMLInputElement>("api-key").value;
    void invoke("set_api_key", { key });
  });

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
