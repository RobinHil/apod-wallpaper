import { useEffect, useState } from "react";
import { shell } from "../classes";
import { useCopy } from "../i18n";
import { RELEASES, REPO } from "../links";
import { GithubIcon } from "./Icons";
import { Logo } from "./Logo";
import { LocaleSwitch } from "./LocaleSwitch";

/**
 * The bar stays transparent over the hero and takes its glass on the first
 * scroll, so the top of the page is sky and nothing else.
 */
export function Nav() {
  const copy = useCopy();
  const [scrolled, setScrolled] = useState(false);

  useEffect(() => {
    const onScroll = () => setScrolled(window.scrollY > 24);
    onScroll();
    window.addEventListener("scroll", onScroll, { passive: true });
    return () => window.removeEventListener("scroll", onScroll);
  }, []);

  return (
    <header
      className={`fixed inset-x-0 top-0 z-50 transition-[background-color,border-color,backdrop-filter] duration-500 ${
        scrolled
          ? "border-b border-border bg-bg/80 backdrop-blur-[16px]"
          : "border-b border-transparent"
      }`}
    >
      <nav className={`${shell} flex h-[64px] items-center justify-between gap-[16px]`}>
        <a href="#top" className="flex items-center gap-[9px] font-semibold">
          <Logo className="size-[22px] shrink-0" />
          <span>APOD Wallpaper</span>
        </a>

        <div className="hidden items-center gap-[26px] text-[14px] text-text-dim lg:flex">
          {copy.nav.sections.map((section) => (
            <a key={section.href} href={section.href} className="transition hover:text-text">
              {section.label}
            </a>
          ))}
        </div>

        <div className="flex items-center gap-[10px]">
          <LocaleSwitch />
          <a
            href={REPO}
            target="_blank"
            rel="noreferrer"
            aria-label={copy.nav.source}
            className="hidden size-[38px] items-center justify-center rounded-[10px] border border-border bg-plate text-text-dim transition hover:border-accent hover:text-text sm:inline-flex"
          >
            <GithubIcon className="size-[17px]" />
          </a>
          <a
            href={RELEASES}
            target="_blank"
            rel="noreferrer"
            className="inline-flex items-center gap-[8px] rounded-[10px] border border-accent bg-accent px-[15px] py-[9px] text-[14px] font-semibold text-accent-text transition hover:brightness-[1.1]"
          >
            {copy.nav.download}
          </a>
        </div>
      </nav>
    </header>
  );
}
