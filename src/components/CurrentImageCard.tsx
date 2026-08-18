import { openUrl } from "@tauri-apps/plugin-opener";
import type { Applied } from "../types";
import { apodPageUrl, watchableVideoUrl } from "../links";
import { ExternalLinkIcon, PlayIcon } from "./Icons";

/**
 * What is on the desktop right now: title, date, credits and explanation.
 *
 * `loading` distinguishes the moment before the first state has arrived from
 * a backend that has answered and has no image to report.
 */
export function CurrentImageCard({
  current,
  loading,
}: {
  current: Applied | null;
  loading: boolean;
}) {
  if (current === null) {
    return (
      <section className="card">
        <h1>{loading ? "Loading..." : "No image loaded"}</h1>
        <p className="meta">
          <span>-</span>
        </p>
        <p className="explanation"></p>
      </section>
    );
  }

  return (
    <section className="card">
      <h1>{current.title}</h1>
      <p className="meta">
        <span>{current.date}</span>
        <span className="copyright">
          {current.copyright ? `(c) ${current.copyright}` : "NASA (public domain)"}
        </span>
      </p>

      {current.media_type === "video" && (
        <div className="video-notice">
          <PlayIcon />
          <span>This APOD is a video: its thumbnail is used as the wallpaper.</span>
          <button
            className="link-btn"
            type="button"
            onClick={() => {
              if (current.video_url) void openUrl(watchableVideoUrl(current.video_url));
            }}
          >
            Watch the video
          </button>
        </div>
      )}

      <p className="explanation">{current.explanation}</p>

      <button
        className="link-btn"
        type="button"
        onClick={() => void openUrl(apodPageUrl(current.date))}
      >
        <ExternalLinkIcon />
        Open the APOD page
      </button>
    </section>
  );
}
