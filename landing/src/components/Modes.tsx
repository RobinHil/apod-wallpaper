import { useState, type ReactNode } from "react";
import { eyebrow, heading, lede, shell } from "../classes";
import { useCopy } from "../i18n";
import { apodStill, type Apod } from "../useApod";
import { Desktop } from "./Desktop";
import { CalendarIcon, ShuffleIcon, SunIcon } from "./Icons";
import { Reveal } from "./Reveal";

/** One icon per entry of `copy.modes.items`, in the same order. */
const ICONS: ReactNode[] = [<SunIcon />, <ShuffleIcon />, <CalendarIcon />];

/** The fit modes, named as the backend names them. */
const FITS = ["blur_fill", "crop"] as const;
type Fit = (typeof FITS)[number];

export function Modes({ apod }: { apod: Apod | null }) {
  const copy = useCopy();
  const [fit, setFit] = useState<Fit>("blur_fill");

  return (
    <section id="modes" className="relative py-[100px]">
      <div className={shell}>
        <Reveal>
          <p className={eyebrow}>{copy.modes.eyebrow}</p>
          <h2 className={`${heading} mt-[16px] max-w-[22ch]`}>{copy.modes.heading}</h2>
          <p className={`${lede} mt-[20px]`}>{copy.modes.lede}</p>
        </Reveal>

        <div className="mt-[48px] grid gap-[18px] md:grid-cols-3">
          {copy.modes.items.map((mode, index) => (
            <Reveal key={mode.title} delay={index * 80}>
              <article className="h-full rounded-[16px] border border-border bg-card/50 p-[24px]">
                <span className="inline-flex size-[38px] items-center justify-center rounded-[11px] border border-hairline bg-nebula-soft text-nebula">
                  {ICONS[index]}
                </span>
                <h3 className="mt-[16px] text-[17px] font-semibold">{mode.title}</h3>
                <p className="mt-[8px] text-[14px] leading-[1.65] text-text-dim">
                  {mode.body}
                </p>
              </article>
            </Reveal>
          ))}
        </div>

        <Reveal delay={120}>
          <div className="mt-[28px] grid items-center gap-[32px] rounded-[20px] border border-border bg-card/40 p-[24px] lg:grid-cols-[minmax(0,1fr)_minmax(0,1.25fr)]">
            <div className="order-2 lg:order-1">
              <h3 className="font-display text-[30px] leading-[1.15]">
                {copy.modes.fitHeading}
              </h3>

              <div className="mt-[22px] flex" role="group" aria-label={copy.modes.fitLabel}>
                {FITS.map((option) => (
                  <button
                    key={option}
                    type="button"
                    aria-pressed={option === fit}
                    onClick={() => setFit(option)}
                    className={`relative -mr-px inline-flex flex-1 items-center justify-center border px-[12px] py-[9px] text-[14px] font-semibold transition first:rounded-l-[10px] last:mr-0 last:rounded-r-[10px] ${
                      option === fit
                        ? "z-[1] border-accent bg-accent text-accent-text"
                        : "border-border bg-plate text-text-dim hover:border-accent hover:text-text"
                    }`}
                  >
                    {copy.modes.fits[option].label}
                  </button>
                ))}
              </div>

              <p className="mt-[18px] text-[14px] leading-[1.7] text-text-dim">
                {copy.modes.fits[fit].note}
              </p>
            </div>

            <Desktop
              still={apod ? apodStill(apod) : null}
              fit={fit}
              className="order-1 aspect-[16/10] w-full rounded-[14px] border border-hairline lg:order-2"
            />
          </div>
        </Reveal>
      </div>
    </section>
  );
}
