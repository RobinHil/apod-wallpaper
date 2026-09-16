import type { ReactNode } from "react";
import { eyebrow, heading, lede, primary, secondary, shell } from "../classes";
import { useCopy } from "../i18n";
import { API_KEY_SIGNUP, README, RELEASES, REPO } from "../links";
import { AppleIcon, CheckIcon, DownloadIcon, GithubIcon, TerminalIcon } from "./Icons";
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

                  <pre className="mt-[12px] rounded-[10px] border border-border bg-bg/80 px-[14px] py-[11px] text-[12.5px] leading-[1.6] break-words whitespace-pre-wrap text-text-dim">
                    <code>{platform.command}</code>
                  </pre>
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
