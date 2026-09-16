import { shell } from "../classes";
import { useCopy } from "../i18n";
import { APOD_SITE, LICENCE, REPO } from "../links";
import { GithubIcon } from "./Icons";
import { Logo } from "./Logo";
import { LocaleSwitch } from "./LocaleSwitch";

export function Footer() {
  const copy = useCopy();

  return (
    <footer className="relative border-t border-border py-[52px]">
      <div className={`${shell} flex flex-col gap-[28px]`}>
        <div className="flex flex-wrap items-end justify-between gap-[24px]">
          <div>
            <p className="flex items-center gap-[9px] font-semibold">
              <Logo className="size-[22px] shrink-0" />
              APOD Wallpaper
            </p>
            <p className="mt-[10px] max-w-[46ch] font-display text-[22px] leading-[1.3] text-text-dim">
              {copy.footer.tagline}
            </p>
          </div>

          <div className="flex flex-wrap items-center gap-[22px] text-[14px] text-text-dim">
            <a
              className="inline-flex items-center gap-[7px] transition hover:text-text"
              href={REPO}
              target="_blank"
              rel="noreferrer"
            >
              <GithubIcon className="size-[16px]" />
              {copy.footer.source}
            </a>
            <a
              className="transition hover:text-text"
              href={LICENCE}
              target="_blank"
              rel="noreferrer"
            >
              {copy.footer.licence}
            </a>
            <a
              className="transition hover:text-text"
              href={APOD_SITE}
              target="_blank"
              rel="noreferrer"
            >
              {copy.footer.apod}
            </a>
            <LocaleSwitch />
          </div>
        </div>

        <p className="max-w-[80ch] text-[12.5px] leading-[1.7] text-text-dim">
          {copy.footer.legal}
        </p>
      </div>
    </footer>
  );
}
