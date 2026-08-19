import { openUrl } from "@tauri-apps/plugin-opener";
import type { Applied } from "../types";
import { apodPageUrl, isVideoFile, watchableVideoUrl } from "../links";
import { card, linkButton } from "../classes";
import { ExternalLinkIcon, PlayIcon } from "./Icons";

const TITLE = "mb-[4px] text-[16px] font-[650]";
const META = "mb-[8px] flex flex-wrap gap-x-[12px] gap-y-[4px] text-[12px] text-text-dim";
const EXPLANATION =
  "max-h-[130px] cursor-text overflow-y-auto text-[13px] text-text select-text";

/**
 * Which still ended up on the desktop. A video the API has a thumbnail for
 * uses that; one published as a plain file has a frame taken out of it, and
 * `source_url` is the video itself rather than a picture.
 */
function stillSource(current: Applied): string {
  return isVideoFile(current.source_url) ? "a frame from it" : "its thumbnail";
}

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
      <section className={card}>
        <h1 className={TITLE}>{loading ? "Loading..." : "No image loaded"}</h1>
        <p className={META}>
          <span>-</span>
        </p>
        <p className={EXPLANATION}></p>
      </section>
    );
  }

  return (
    <section className={card}>
      <h1 className={TITLE}>{current.title}</h1>
      <p className={META}>
        <span>{current.date}</span>
        <span className="italic">
          {current.copyright ? `(c) ${current.copyright}` : "NASA (public domain)"}
        </span>
      </p>

      {current.media_type === "video" && (
        <div className="mt-[2px] mb-[10px] flex flex-wrap items-center gap-[6px] rounded-[8px] border border-accent bg-accent-soft px-[10px] py-[8px] text-[12px]">
          <PlayIcon className="size-[14px] shrink-0 text-accent" />
          <span className="min-w-[180px] flex-1">
            This APOD is a video: {stillSource(current)} is used as the wallpaper.
          </span>
          <button
            className={`${linkButton} font-semibold`}
            type="button"
            onClick={() => {
              if (current.video_url)
                void openUrl(watchableVideoUrl(current.video_url, current.date));
            }}
          >
            Watch the video
          </button>
        </div>
      )}

      <p className={EXPLANATION}>{current.explanation}</p>

      <button
        className={linkButton}
        type="button"
        onClick={() => void openUrl(apodPageUrl(current.date))}
      >
        <ExternalLinkIcon />
        Open the APOD page
      </button>
    </section>
  );
}
