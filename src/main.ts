import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { openUrl } from "@tauri-apps/plugin-opener";

type Mode = "daily" | "random" | "specific";
type FitMode = "blur_fill" | "crop_fill";

/** The wallpaper currently applied, as reported by the backend. */
interface Applied {
  date: string;
  title: string;
  explanation: string;
  copyright: string | null;
  media_type: string;
  video_url: string | null;
}

interface UiState {
  mode: Mode;
  fit_mode: FitMode;
  api_key: string;
  specific_date: string;
  offline: boolean;
  status_message: string | null;
  last_check: string | null;
  current: Applied | null;
}

function el<T extends HTMLElement>(id: string): T {
  const node = document.getElementById(id);
  if (!node) throw new Error(`Element not found: ${id}`);
  return node as T;
}

/** Official page of an APOD: https://apod.nasa.gov/apod/apYYMMDD.html */
function apodPageUrl(date: string): string {
  const [y, m, d] = date.split("-");
  return `https://apod.nasa.gov/apod/ap${y.slice(2)}${m}${d}.html`;
}

/**
 * The API serves embed links; convert them to pages that can be watched
 * directly in a browser. Unknown URLs are opened as they are.
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
/** The date picker stays visible after clicking "Specific date", even while
 *  the real mode has not switched over yet. */
let datePickerRequested = false;

// -----------------------------------------------------------------------
// UI blocking and error reporting.
//
// Every action goes through run(): the whole UI is covered by an overlay
// until the backend finishes (or fails), then the state returned by the
// command is rendered. Any error shows up in a banner: nothing fails
// silently.
// -----------------------------------------------------------------------

let pending = false;

function setBlocked(blocked: boolean, label?: string): void {
  el<HTMLSpanElement>("overlay-label").textContent = label ?? "Applying...";
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
    // Resynchronise the display with the real backend state after a failure.
    try {
      render(await invoke<UiState>("get_state"));
    } catch {
      // Backend unreachable: keep the current display and the visible error.
    }
  } finally {
    pending = false;
    setBlocked(false);
  }
}

// -----------------------------------------------------------------------
// Rendering
// -----------------------------------------------------------------------

function render(state: UiState): void {
  const pill = el<HTMLSpanElement>("status-pill");
  if (state.offline) {
    pill.textContent = "Offline";
    pill.className = "pill offline";
  } else if (state.current) {
    pill.textContent = "Online";
    pill.className = "pill online";
  } else {
    pill.textContent = "Loading";
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
      ? `(c) ${state.current.copyright}`
      : "NASA (public domain)";
    copyright.hidden = false;
    explanation.textContent = state.current.explanation;
    openPage.hidden = false;
    videoNotice.hidden = state.current.media_type !== "video";
  } else {
    currentDate = null;
    currentVideoUrl = null;
    title.textContent = "No image loaded";
    date.textContent = "-";
    copyright.hidden = true;
    explanation.textContent = "";
    openPage.hidden = true;
    videoNotice.hidden = true;
  }

  setToggle("mode-daily", state.mode === "daily");
  setToggle("mode-random", state.mode === "random");
  setToggle("mode-specific", state.mode === "specific");
  setToggle("fit-blur", state.fit_mode === "blur_fill");
  setToggle("fit-crop", state.fit_mode === "crop_fill");

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
  if (state.last_check) parts.push(`Last check: ${state.last_check}`);
  el<HTMLSpanElement>("last-check").textContent = parts.join(" - ");
}

/** Reflects a segmented-button selection visually and to assistive tech. */
function setToggle(id: string, active: boolean): void {
  const button = el<HTMLButtonElement>(id);
  button.classList.toggle("active", active);
  button.setAttribute("aria-pressed", String(active));
}

async function refreshState(): Promise<void> {
  try {
    render(await invoke<UiState>("get_state"));
  } catch (error) {
    showError(String(error));
  }
}

// -----------------------------------------------------------------------
// Wiring
// -----------------------------------------------------------------------

window.addEventListener("DOMContentLoaded", () => {
  void refreshState();

  // Updates pushed by the backend (daily check, startup...): do not re-render
  // while an action is running, so the blocking overlay is not disturbed.
  void listen<UiState>("state-updated", (event) => {
    if (!pending) render(event.payload);
  });

  // The action we just asked for is queued behind an update already running --
  // the scheduler's, usually. It can take as long as a download does, so the
  // overlay says what it is waiting on rather than sitting there mute.
  void listen("update-waiting", () => {
    if (pending) setBlocked(true, "Waiting for the update in progress...");
  });

  // Upper bound of the picker: today (local time).
  const now = new Date();
  const today = `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, "0")}-${String(now.getDate()).padStart(2, "0")}`;
  el<HTMLInputElement>("specific-date").max = today;

  el<HTMLButtonElement>("mode-daily").addEventListener("click", () => {
    datePickerRequested = false;
    void run("set_mode", { mode: "daily" }, "Switching to picture of the day...");
  });
  el<HTMLButtonElement>("mode-random").addEventListener("click", () => {
    datePickerRequested = false;
    void run("set_mode", { mode: "random" }, "Drawing a random image...");
  });
  el<HTMLButtonElement>("mode-specific").addEventListener("click", () => {
    // No backend call here: reveal the picker, the mode only switches once a
    // date has been chosen and applied.
    datePickerRequested = true;
    el<HTMLDivElement>("date-picker").hidden = false;
    el<HTMLInputElement>("specific-date").focus();
  });
  el<HTMLFormElement>("date-form").addEventListener("submit", (e) => {
    e.preventDefault();
    const date = el<HTMLInputElement>("specific-date").value;
    if (!date) {
      showError("Pick a date first.");
      return;
    }
    void run("set_specific_date", { date }, `Loading the APOD for ${date}...`);
  });
  el<HTMLButtonElement>("fit-blur").addEventListener("click", () => {
    void run("set_fit_mode", { fit: "blur_fill" }, "Recomposing the wallpaper...");
  });
  el<HTMLButtonElement>("fit-crop").addEventListener("click", () => {
    void run("set_fit_mode", { fit: "crop_fill" }, "Recomposing the wallpaper...");
  });
  el<HTMLButtonElement>("refresh").addEventListener("click", () => {
    void run("refresh_now", undefined, "Checking for the latest image...");
  });

  el<HTMLFormElement>("key-form").addEventListener("submit", (e) => {
    e.preventDefault();
    const key = el<HTMLInputElement>("api-key").value;
    void run("set_api_key", { key }, "Saving the key...");
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
