import { useState } from "react";

import { useCopy } from "../i18n";

/**
 * A screen with a wallpaper on it.
 *
 * The two fit modes are composed the way the application composes them, with
 * the browser standing in for the Rust image pipeline: `blur_fill` centres
 * the whole picture, undistorted, over a blurred and darkened copy of itself,
 * and `crop` fills the frame and lets the edges go.
 *
 * `still` is null whenever the APOD API has not answered -- an exhausted
 * quota, no network, an outage -- and the frame falls back to a sky of its
 * own rather than to an empty plate.
 *
 * That sky stays painted underneath while the picture arrives, and the
 * picture fades in only once it has loaded. A JPEG straight from NASA is
 * baseline, not progressive, so without this the frame paints itself band by
 * band from the top over however long the download takes.
 */
export function Desktop({
  still,
  fit,
  className = "",
}: {
  still: string | null;
  fit: "blur_fill" | "crop";
  className?: string;
}) {
  const copy = useCopy();

  // Keyed on the address, so a new picture goes back behind the sky rather
  // than showing half of itself over the previous one.
  const [loaded, setLoaded] = useState<string | null>(null);
  const shown = loaded === still;

  return (
    <div className={`relative overflow-hidden bg-[#05070c] ${className}`}>
      <FallbackSky />

      {still !== null && (
        <div
          className={`absolute inset-0 transition-opacity duration-700 ${shown ? "opacity-100" : "opacity-0"}`}
        >
          {fit === "blur_fill" ? (
            <>
              <img
                src={still}
                alt=""
                aria-hidden="true"
                decoding="async"
                className="absolute inset-0 size-full scale-[1.15] object-cover brightness-[0.45] blur-[26px]"
              />
              <img
                src={still}
                alt={copy.hero.altBlurFill}
                decoding="async"
                onLoad={() => setLoaded(still)}
                className="relative size-full object-contain"
              />
            </>
          ) : (
            <img
              src={still}
              alt={copy.hero.altCrop}
              decoding="async"
              onLoad={() => setLoaded(still)}
              className="size-full object-cover"
            />
          )}
        </div>
      )}

      {/* The glass of the screen: one highlight across the top, one vignette. */}
      <div className="pointer-events-none absolute inset-0 bg-[radial-gradient(120%_80%_at_50%_-10%,rgba(255,255,255,0.10),transparent_60%)]" />
      <div className="pointer-events-none absolute inset-0 shadow-[inset_0_0_80px_rgba(0,0,0,0.55)]" />
    </div>
  );
}

/** What the frame shows when there is no picture to show: a sky, painted. */
function FallbackSky() {
  return (
    <div className="absolute inset-0 animate-breathe bg-[radial-gradient(60%_80%_at_25%_30%,rgba(165,131,245,0.55),transparent_60%),radial-gradient(70%_70%_at_75%_65%,rgba(76,133,240,0.45),transparent_62%),radial-gradient(40%_40%_at_60%_20%,rgba(240,180,95,0.28),transparent_70%),linear-gradient(160deg,#0a0f1e,#05070c)]" />
  );
}
