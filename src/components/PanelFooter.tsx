import { invoke } from "@tauri-apps/api/core";
import type { UiState } from "../types";
import { PowerIcon } from "./Icons";

export function PanelFooter({ state }: { state: UiState | null }) {
  const parts: string[] = [];
  if (state?.status_message) parts.push(state.status_message);
  if (state?.last_check) parts.push(`Last check: ${state.last_check}`);

  return (
    <footer className="flex items-center justify-between gap-[12px] px-[4px] pt-[2px] pb-[8px]">
      <span className="flex-1 text-[12px] text-text-dim">{parts.join(" - ")}</span>
      {/* Not routed through `run`: quitting has no state to come back to, and
          blocking the UI for it would only flash the overlay. */}
      <button
        className="inline-flex shrink-0 items-center justify-center gap-[7px] rounded-[8px] border-none px-[12px] py-[7px] text-[12px] text-text-dim hover:text-danger"
        type="button"
        onClick={() => void invoke("quit_app")}
      >
        <PowerIcon />
        Quit
      </button>
    </footer>
  );
}
