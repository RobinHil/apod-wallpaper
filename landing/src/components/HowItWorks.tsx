import { eyebrow, heading, lede, pill, shell } from "../classes";
import { useCopy } from "../i18n";
import { Reveal } from "./Reveal";

export function HowItWorks() {
  const copy = useCopy();

  return (
    <section id="how" className="relative py-[100px]">
      {/* A faint band behind the section, which is what separates it from the
          void above and below without drawing a line. It is in the flow, so
          it scrolls with the text it belongs to. */}
      <div className="pointer-events-none absolute inset-x-0 top-0 h-full bg-[linear-gradient(to_bottom,transparent,var(--color-surface)_25%,var(--color-surface)_75%,transparent)] opacity-60" />

      <div className={`${shell} relative`}>
        <Reveal>
          <p className={eyebrow}>{copy.how.eyebrow}</p>
          <h2 className={`${heading} mt-[16px] max-w-[18ch]`}>{copy.how.heading}</h2>
          <p className={`${lede} mt-[20px]`}>{copy.how.lede}</p>
        </Reveal>

        <ol className="mt-[54px] grid gap-[1px] overflow-hidden rounded-[18px] border border-border bg-border sm:grid-cols-2">
          {copy.how.steps.map((item, index) => (
            <Reveal key={item.title} delay={index * 90} className="bg-surface">
              <li className="flex h-full flex-col gap-[12px] p-[28px]">
                <span className="font-display text-[30px] leading-none text-accent">
                  {String(index + 1).padStart(2, "0")}
                </span>
                <h3 className="text-[18px] font-semibold">{item.title}</h3>
                <p className="text-[14px] leading-[1.7] text-text-dim">{item.body}</p>
              </li>
            </Reveal>
          ))}
        </ol>

        <Reveal delay={120}>
          <div className="mt-[28px] flex flex-wrap items-center gap-[10px]">
            <span className="text-[14px] text-text-dim">{copy.how.wakesLabel}</span>
            {copy.how.wakes.map((wake) => (
              <span key={wake} className={pill}>
                {wake}
              </span>
            ))}
          </div>
        </Reveal>
      </div>
    </section>
  );
}
