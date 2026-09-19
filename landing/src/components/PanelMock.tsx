import type { CSSProperties } from "react";
import { ExternalLinkIcon, GlobeIcon, MonitorIcon } from "./Icons";

/**
 * The settings panel, as it looks on a machine running the dark system
 * theme.
 *
 * It is a picture, not the panel: the real one is rendered by the
 * application and driven entirely by its backend. What is reproduced here is
 * its palette, token for token, its 10px plates and hairline borders, and the
 * cards in the order they actually appear -- so that what the page promises
 * and what the download opens are recognisably the same window. Its header
 * carries the panel's globe rather than the application icon, because that is
 * what the real window shows, and its labels stay in English whatever
 * language the page is read in, because the application is English.
 */

/** The panel's own dark palette, kept out of the page's token namespace. */
const PALETTE = {
  "--p-bg": "#15171d",
  "--p-card": "#1e212a",
  "--p-text": "#e8eaf0",
  "--p-dim": "#9aa0ae",
  "--p-border": "#2c303c",
  "--p-accent": "#4c85f0",
  "--p-online": "#5fd39a",
  "--p-online-bg": "#143324",
} as CSSProperties;

const CARD =
  "rounded-[10px] border border-[var(--p-border)] bg-[var(--p-card)] px-[16px] py-[14px]";
const SECTION_TITLE =
  "mb-[6px] text-[12px] font-semibold tracking-[0.04em] text-[var(--p-dim)] uppercase";
const SEGMENT =
  "relative -mr-px inline-flex flex-1 items-center justify-center gap-[7px] " +
  "border px-[6px] py-[7px] text-[13px] whitespace-nowrap " +
  "first:rounded-l-[8px] last:mr-0 last:rounded-r-[8px]";
const IDLE = "border-[var(--p-border)] bg-[var(--p-card)] text-[var(--p-text)]";
const ACTIVE = "z-[1] border-[var(--p-accent)] bg-[var(--p-accent)] text-white";

function Segmented({ options, active }: { options: string[]; active: string }) {
  return (
    <div className="flex">
      {options.map((option) => (
        <span
          key={option}
          className={`${SEGMENT} ${option === active ? ACTIVE : IDLE}`}
        >
          {option}
        </span>
      ))}
    </div>
  );
}

export function PanelMock({
  title,
  date,
  credit,
  explanation,
}: {
  title: string;
  date: string;
  credit: string;
  explanation: string;
}) {
  return (
    <div
      style={PALETTE}
      className="w-[330px] overflow-hidden rounded-[12px] border border-[var(--p-border)] bg-[var(--p-bg)] font-sans text-[14px] text-[var(--p-text)] shadow-[0_40px_90px_-30px_rgba(0,0,0,0.9)]"
      aria-hidden="true"
    >
      <header className="flex items-center justify-between border-b border-b-[var(--p-border)] bg-[var(--p-card)] px-[16px] py-[12px]">
        <div className="flex items-center gap-[8px] font-semibold">
          <GlobeIcon className="size-[18px] shrink-0 text-[var(--p-accent)]" />
          <span>APOD Wallpaper</span>
        </div>
        <span className="rounded-[99px] bg-[var(--p-online-bg)] px-[10px] py-[3px] text-[11px] font-semibold text-[var(--p-online)]">
          Online
        </span>
      </header>

      <div className="flex flex-col gap-[12px] p-[12px]">
        <section className={CARD}>
          <h3 className="mb-[4px] text-[16px] leading-[1.25] font-[650]">{title}</h3>
          <p className="mb-[8px] flex flex-wrap gap-x-[12px] gap-y-[4px] text-[12px] text-[var(--p-dim)]">
            <span>{date}</span>
            <span className="italic">{credit}</span>
          </p>
          <p className="line-clamp-4 text-[13px] leading-[1.5] text-[var(--p-text)]">
            {explanation}
          </p>
          <span className="mt-[8px] inline-flex items-center gap-[7px] py-[4px] text-[12px] text-[var(--p-accent)]">
            <ExternalLinkIcon className="size-[15px] shrink-0" />
            Open the APOD page
          </span>
        </section>

        <section className={CARD}>
          <h4 className={SECTION_TITLE}>Image</h4>
          <Segmented options={["Today", "Random", "Date"]} active="Today" />

          <h4 className={`${SECTION_TITLE} mt-[14px]`}>Fit to screen</h4>
          <Segmented options={["Blurred fill", "Crop"]} active="Blurred fill" />

          <div className="mt-[14px] inline-flex w-full items-center justify-center gap-[7px] rounded-[8px] border border-[var(--p-accent)] bg-[var(--p-accent)] px-[12px] py-[7px] font-semibold text-white">
            Refresh now
          </div>
        </section>

        <footer className="flex items-center justify-between px-[2px] text-[12px] text-[var(--p-dim)]">
          <span className="inline-flex items-center gap-[6px]">
            <MonitorIcon className="size-[14px] shrink-0" />
            2560 x 1440
          </span>
          <span>v0.2.2</span>
        </footer>
      </div>
    </div>
  );
}
