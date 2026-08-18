import { invoke } from "@tauri-apps/api/core";
import type { UiState } from "../types";
import { PowerIcon } from "./Icons";

export function PanelFooter({ state }: { state: UiState | null }) {
  const parts: string[] = [];
  if (state?.status_message) parts.push(state.status_message);
  if (state?.last_check) parts.push(`Last check: ${state.last_check}`);

  return (
    <footer>
      <span className="hint">{parts.join(" - ")}</span>
      {/* Not routed through `run`: quitting has no state to come back to, and
          blocking the UI for it would only flash the overlay. */}
      <button className="quit-btn" type="button" onClick={() => void invoke("quit_app")}>
        <PowerIcon />
        Quit
      </button>
    </footer>
  );
}
