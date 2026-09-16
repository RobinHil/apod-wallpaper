import { eyebrow, heading, shell } from "../classes";
import { useCopy } from "../i18n";
import { PlusIcon } from "./Icons";
import { Reveal } from "./Reveal";

export function Faq() {
  const copy = useCopy();

  return (
    <section id="faq" className="relative py-[100px]">
      <div className={shell}>
        <Reveal>
          <p className={eyebrow}>{copy.faq.eyebrow}</p>
          <h2 className={`${heading} mt-[16px] max-w-[18ch]`}>{copy.faq.heading}</h2>
        </Reveal>

        <div className="mt-[44px] grid gap-[12px] lg:grid-cols-2">
          {copy.faq.items.map((item, index) => (
            <Reveal key={item.q} delay={index * 60}>
              <details className="group h-full rounded-[14px] border border-border bg-card/50 p-[20px] transition open:bg-card hover:border-accent/50">
                <summary className="flex cursor-pointer list-none items-start justify-between gap-[16px] text-[16px] font-semibold [&::-webkit-details-marker]:hidden">
                  {item.q}
                  <PlusIcon className="mt-[3px] size-[16px] shrink-0 text-accent transition-transform duration-300 group-open:rotate-45" />
                </summary>
                <p className="mt-[12px] text-[14px] leading-[1.7] text-text-dim">{item.a}</p>
              </details>
            </Reveal>
          ))}
        </div>
      </div>
    </section>
  );
}
