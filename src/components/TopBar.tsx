import type { UiState } from "../types";
import { GlobeIcon } from "./Icons";

/**
 * The status pill: offline whenever the backend says so, online once it has
 * an image to show for it, and neither while the first state is on its way.
 */
function pill(state: UiState | null): { label: string; className: string } {
  if (state === null) return { label: "Loading", className: "pill" };
  if (state.offline) return { label: "Offline", className: "pill offline" };
  if (state.current) return { label: "Online", className: "pill online" };
  return { label: "Loading", className: "pill" };
}

export function TopBar({ state }: { state: UiState | null }) {
  const status = pill(state);

  return (
    <header className="topbar">
      <div className="brand">
        <GlobeIcon />
        <span>APOD Wallpaper</span>
      </div>
      <span className={status.className}>{status.label}</span>
    </header>
  );
}
