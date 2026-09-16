import type { ReactNode } from "react";
import { eyebrow, heading, lede, shell } from "../classes";
import { useCopy } from "../i18n";
import {
  FeatherIcon,
  LayersIcon,
  MonitorIcon,
  MoonIcon,
  PlayIcon,
  ShieldIcon,
} from "./Icons";
import { Reveal } from "./Reveal";

/** One icon per entry of `copy.features.items`, in the same order. */
const ICONS: ReactNode[] = [
  <MoonIcon />,
  <MonitorIcon />,
  <ShieldIcon />,
  <LayersIcon />,
  <PlayIcon />,
  <FeatherIcon />,
];

export function Features() {
  const copy = useCopy();

  return (
    <section id="craft" className="relative py-[100px]">
      <div className={shell}>
        <Reveal>
          <p className={eyebrow}>{copy.features.eyebrow}</p>
          <h2 className={`${heading} mt-[16px] max-w-[20ch]`}>{copy.features.heading}</h2>
          <p className={`${lede} mt-[20px]`}>{copy.features.lede}</p>
        </Reveal>

        <div className="mt-[52px] grid gap-[18px] sm:grid-cols-2 lg:grid-cols-3">
          {copy.features.items.map((feature, index) => (
            <Reveal key={feature.title} delay={index * 70}>
              <article className="group h-full rounded-[16px] border border-border bg-card/50 p-[24px] transition duration-500 hover:-translate-y-[3px] hover:border-accent/60 hover:bg-card">
                <span className="inline-flex size-[42px] items-center justify-center rounded-[12px] border border-hairline bg-accent-soft text-accent transition group-hover:bg-accent group-hover:text-accent-text">
                  {ICONS[index]}
                </span>
                <h3 className="mt-[18px] text-[17px] font-semibold">{feature.title}</h3>
                <p className="mt-[9px] text-[14px] leading-[1.65] text-text-dim">
                  {feature.body}
                </p>
              </article>
            </Reveal>
          ))}
        </div>
      </div>
    </section>
  );
}
