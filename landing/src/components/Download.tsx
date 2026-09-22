import { useEffect, useRef, useState, type ReactNode } from "react";
import { eyebrow, heading, lede, primary, secondary, shell } from "../classes";
import { selectContents, writeClipboard } from "../clipboard";
import { useCopy } from "../i18n";
import { API_KEY_SIGNUP, README, RELEASES, REPO } from "../links";
import {
  AppleIcon,
  CheckIcon,
  CopyIcon,
  DownloadIcon,
  GithubIcon,
  TerminalIcon,
} from "./Icons";
import { Reveal } from "./Reveal";

/**
 * What does not translate: the icon, and the command, which is the same
 * command whatever language the page is read in.
 */
const PLATFORMS: { id: "macos" | "linux"; icon: ReactNode; command: string }[] = [
  {
    id: "macos",
    icon: <AppleIcon className="size-[20px]" />,
    command: 'xattr -d com.apple.quarantine "/Applications/APOD Wallpaper.app"',
  },
  {
    id: "linux",
    icon: <TerminalIcon className="size-[20px]" />,
    command: "sudo apt install ./APOD*.deb",
  },
];

/**
 * A command, and the button that copies it.
 *
 * The button lives inside the frame so the block stays one object, and the
 * text is padded on its right by the room the button takes, which is what
 * keeps a wrapped line from running underneath it.
 *
 * Both clipboards can be refused, so both are tried, and when neither answers
 * the command is selected instead: a button that silently does nothing reads
 * as broken, and one that leaves the keyboard a line away from the same
 * result is still a button that worked.
 */
function Command({ command }: { command: string }) {
  const copy = useCopy();
  const [state, setState] = useState<"idle" | "copied" | "selected" | "failed">("idle");
  const code = useRef<HTMLElement>(null);

  useEffect(() => {
    if (state === "idle") return;

    const timer = window.setTimeout(() => setState("idle"), 1800);
    return () => window.clearTimeout(timer);
  }, [state]);

  const label = {
    idle: copy.download.copyCommand,
    copied: copy.download.copied,
    selected: copy.download.copySelected,
    failed: copy.download.copyFailed,
  }[state];

  async function run() {
    if (await writeClipboard(command)) {
      setState("copied");
      return;
    }

    setState(selectContents(code.current) ? "selected" : "failed");
  }

  return (
    <div className="relative mt-[12px]">
      <pre className="rounded-[10px] border border-border bg-bg/80 py-[11px] pl-[14px] pr-[52px] text-[12.5px] leading-[1.6] break-words whitespace-pre-wrap text-text-dim">
        <code ref={code}>{command}</code>
      </pre>
      <button
        type="button"
        className="absolute top-[8px] right-[8px] inline-flex size-[30px] items-center justify-center rounded-[8px] border border-border bg-plate text-text-dim transition hover:border-accent hover:text-text"
        aria-label={label}
        title={label}
        onClick={run}
      >
        {state === "copied" ? (
          <CheckIcon className="size-[15px] text-accent" />
        ) : (
          <CopyIcon className={`size-[15px] ${state === "idle" ? "" : "text-ember"}`} />
        )}
      </button>
    </div>
  );
}

export function Download() {
  const copy = useCopy();

  return (
    <section id="download" className="relative py-[100px]">
      <div className={shell}>
        <Reveal>
          <p className={eyebrow}>{copy.download.eyebrow}</p>
          <h2 className={`${heading} mt-[16px] max-w-[20ch]`}>{copy.download.heading}</h2>
          <p className={`${lede} mt-[20px]`}>
            {copy.download.ledeBefore}
            <a
              className="text-text underline decoration-border underline-offset-[4px] transition hover:decoration-accent"
              href={API_KEY_SIGNUP}
              target="_blank"
              rel="noreferrer"
            >
              {copy.download.ledeLink}
            </a>
            {copy.download.ledeAfter}
          </p>
        </Reveal>

        <div className="mt-[48px] grid gap-[18px] md:grid-cols-2">
          {PLATFORMS.map((platform, index) => {
            const text = copy.download.platforms[platform.id];

            return (
              <Reveal key={platform.id} delay={index * 90}>
                <article className="flex h-full flex-col rounded-[18px] border border-border bg-card/60 p-[26px]">
                  <div className="flex items-center gap-[12px]">
                    <span className="inline-flex size-[42px] items-center justify-center rounded-[12px] border border-hairline bg-plate text-text">
                      {platform.icon}
                    </span>
                    <div>
                      <h3 className="text-[19px] font-semibold">{text.name}</h3>
                      <p className="text-[13px] text-text-dim">{text.requires}</p>
                    </div>
                  </div>

                  <ul className="mt-[20px] flex flex-wrap gap-[8px]">
                    {text.formats.map((format) => (
                      <li
                        key={format}
                        className="inline-flex items-center gap-[6px] rounded-[99px] border border-border bg-plate px-[11px] py-[4px] text-[12px] font-semibold text-text-dim"
                      >
                        <CheckIcon className="size-[13px] text-accent" />
                        {format}
                      </li>
                    ))}
                  </ul>

                  <p className="mt-[20px] text-[13px] leading-[1.65] text-text-dim">
                    {text.note}
                  </p>

                  <Command command={platform.command} />
                </article>
              </Reveal>
            );
          })}
        </div>

        <Reveal delay={160}>
          <div className="mt-[18px] flex flex-col items-center gap-[20px] rounded-[18px] border border-border bg-[linear-gradient(120deg,var(--color-accent-soft),var(--color-nebula-soft))] p-[34px] text-center">
            <p className="font-display text-[28px] leading-[1.2] text-balance sm:text-[34px]">
              {copy.download.ctaTitle}
            </p>
            <p className="max-w-[58ch] text-[14px] leading-[1.7] text-text-dim">
              {copy.download.ctaBody}
            </p>
            <div className="flex flex-wrap justify-center gap-[12px]">
              <a className={primary} href={RELEASES} target="_blank" rel="noreferrer">
                <DownloadIcon className="size-[17px]" />
                {copy.download.ctaPrimary}
              </a>
              <a className={secondary} href={REPO} target="_blank" rel="noreferrer">
                <GithubIcon className="size-[17px]" />
                {copy.download.ctaSecondary}
              </a>
            </div>
            <a
              className="text-[13px] text-text-dim underline decoration-border underline-offset-[4px] transition hover:text-text hover:decoration-accent"
              href={README}
              target="_blank"
              rel="noreferrer"
            >
              {copy.download.ctaNotes}
            </a>
          </div>
        </Reveal>
      </div>
    </section>
  );
}
