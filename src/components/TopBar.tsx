import type { UiState } from "../types";
import { GlobeIcon } from "./Icons";

const PILL = "rounded-[99px] px-[10px] py-[3px] text-[11px] font-semibold";
const LOADING = `${PILL} bg-border text-text-dim`;
const ONLINE = `${PILL} bg-pill-online-bg text-pill-online`;
const OFFLINE = `${PILL} bg-pill-offline-bg text-pill-offline`;

/**
 * The status pill: offline whenever the backend says so, online once it has
 * an image to show for it, and neither while the first state is on its way.
 */
function pill(state: UiState | null): { label: string; className: string } {
  if (state === null) return { label: "Loading", className: LOADING };
  if (state.offline) return { label: "Offline", className: OFFLINE };
  if (state.current) return { label: "Online", className: ONLINE };
  return { label: "Loading", className: LOADING };
}

export function TopBar({ state }: { state: UiState | null }) {
  const status = pill(state);

  return (
    <header className="flex items-center justify-between border-b border-b-border bg-card px-[16px] py-[12px]">
      <div className="flex items-center gap-[8px] font-semibold">
        <GlobeIcon className="size-[18px] text-accent" />
        <span>APOD Wallpaper</span>
      </div>
      <span className={status.className}>{status.label}</span>
    </header>
  );
}
