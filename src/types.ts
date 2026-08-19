/**
 * The panel's view of the backend.
 *
 * `UiState` is the whole contract: every command returns one, and the backend
 * pushes one on `state-updated` whenever it changes something on its own (the
 * daily update, a screen change, a wake from sleep). The panel never derives
 * application state of its own from it -- it renders it.
 *
 * These declarations mirror `src-tauri/src/lib.rs`; the field names are the
 * ones serde emits, hence the snake_case.
 */

export type Mode = "daily" | "random" | "specific";

export type FitMode = "blur_fill" | "crop_fill";

/** The wallpaper currently applied, as reported by the backend. */
export interface Applied {
  date: string;
  title: string;
  explanation: string;
  copyright: string | null;
  media_type: string;
  video_url: string | null;
  /** URL the wallpaper was downloaded from: an image, or the video a frame
   *  was taken out of. */
  source_url: string;
}

export interface UiState {
  mode: Mode;
  fit_mode: FitMode;
  api_key: string;
  specific_date: string;
  offline: boolean;
  status_message: string | null;
  last_check: string | null;
  current: Applied | null;
}
