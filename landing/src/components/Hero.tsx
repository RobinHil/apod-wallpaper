import { eyebrow, lede, primary, secondary, shell } from "../classes";
import { useCopy } from "../i18n";
import { apodPageUrl, REPO, RELEASES } from "../links";
import { apodStill, type Apod } from "../useApod";
import { Desktop } from "./Desktop";
import { PanelMock } from "./PanelMock";
import { Reveal } from "./Reveal";
import { ArrowRightIcon, DownloadIcon, ExternalLinkIcon } from "./Icons";

const STAND_IN = {
  title: "The Great Nebula in Orion",
  date: "1995-06-16",
  credit: "NASA (public domain)",
  explanation:
    "The Great Nebula in Orion, an immense, nearby starbirth region, is probably the most famous of all astronomical nebulas. Here, glowing gas surrounds hot young stars at the edge of an immense interstellar molecular cloud only 1500 light-years away.",
};

export function Hero({ apod }: { apod: Apod | null }) {
  const copy = useCopy();
  const still = apod ? apodStill(apod) : null;

  // The panel is a picture of the application, and the application is in
  // English: what it shows is whatever the API returned, and the stand-in
  // used before the API answers is written the way the panel would write it,
  // ISO date included, in both languages.
  const shown = apod
    ? {
        title: apod.title,
        date: apod.date,
        credit: apod.copyright ? `(c) ${apod.copyright.trim()}` : "NASA (public domain)",
        explanation: apod.explanation,
      }
    : STAND_IN;

  return (
    <section id="top" className="relative pt-[132px] pb-[80px] sm:pt-[156px]">
      {/* The sky fades into the page here rather than in the fixed starfield,
          where a band measured in viewport units would leave a hard edge
          across the screen for the whole scroll. */}
      <div className="pointer-events-none absolute inset-x-0 bottom-0 -z-[1] h-[45%] bg-[linear-gradient(to_bottom,transparent,var(--color-bg))]" />

      <div className={`${shell} grid items-center gap-[64px] lg:grid-cols-[minmax(0,1fr)_minmax(0,1.05fr)]`}>
        <div>
          <Reveal>
            <p className={eyebrow}>{copy.hero.eyebrow}</p>
          </Reveal>

          <Reveal delay={80}>
            <h1 className="mt-[18px] font-display text-[clamp(44px,7vw,78px)] leading-[1.02] tracking-[-0.02em] text-balance">
              <span className="text-gradient">{copy.hero.titleLead}</span>
              <br />
              {copy.hero.titleMiddle}
              <br />
              <span className="italic">{copy.hero.titleAccent}</span>
            </h1>
          </Reveal>

          <Reveal delay={160}>
            <p className={`${lede} mt-[26px]`}>{copy.hero.lede}</p>
          </Reveal>

          <Reveal delay={240}>
            <div className="mt-[34px] flex flex-wrap items-center gap-[12px]">
              <a className={primary} href={RELEASES} target="_blank" rel="noreferrer">
                <DownloadIcon className="size-[17px]" />
                {copy.hero.download}
              </a>
              <a className={secondary} href={REPO} target="_blank" rel="noreferrer">
                {copy.hero.source}
                <ArrowRightIcon className="size-[17px] transition-transform group-hover:translate-x-[3px]" />
              </a>
            </div>
          </Reveal>

          <Reveal delay={320}>
            <ul className="mt-[34px] flex flex-wrap gap-x-[22px] gap-y-[10px] text-[13px] text-text-dim">
              {copy.hero.proof.map((item) => (
                <li key={item} className="inline-flex items-center gap-[8px]">
                  <span className="size-[5px] rounded-full bg-accent" />
                  {item}
                </li>
              ))}
            </ul>
          </Reveal>
        </div>

        <Reveal delay={200}>
          <div className="relative">
            <Desktop
              still={still}
              fit="blur_fill"
              className="aspect-[16/10] w-full rounded-[18px] border border-hairline shadow-[0_50px_120px_-40px_rgba(0,0,0,0.95)]"
            />

            {/* The panel, floating at the corner of the desktop it is
                describing: anchored bottom left, scaled down and spilling out
                of the frame, so it covers a third of the picture and no more.
                Below `sm` it is dropped rather than shrunk further. */}
            <div className="pointer-events-none absolute bottom-0 left-0 hidden origin-bottom-left translate-x-[-44px] translate-y-[52px] scale-[0.72] sm:block">
              <PanelMock {...shown} />
            </div>

            <div className="mt-[22px] flex justify-end sm:mt-[76px]">
              {apod ? (
                <a
                  href={apodPageUrl(apod.date)}
                  target="_blank"
                  rel="noreferrer"
                  className="inline-flex max-w-[60%] items-center gap-[8px] text-[13px] text-text-dim transition hover:text-text"
                >
                  <span className="truncate">
                    {copy.hero.liveAbove(apod.title, apod.date)}
                  </span>
                  <ExternalLinkIcon className="size-[14px] shrink-0" />
                </a>
              ) : (
                <span className="max-w-[60%] text-right text-[13px] text-text-dim">
                  {copy.hero.waitingForNasa}
                </span>
              )}
            </div>
          </div>
        </Reveal>
      </div>
    </section>
  );
}
