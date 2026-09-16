/**
 * The utility strings more than one section needs.
 *
 * Same rule as the application panel they are borrowed from: they are only
 * ever spread onto an existing element, and they are never merged. Two
 * utilities setting the same property are resolved by their order in the
 * generated stylesheet, not by their order in the attribute, so a variant
 * spells out its own colours instead of layering them over a base.
 */

/** The column every section hangs from. */
export const shell = "mx-auto w-full max-w-[1120px] px-[24px]";

/** The small capitalised line above a section heading. */
export const eyebrow =
  "text-[12px] font-semibold tracking-[0.18em] text-text-dim uppercase";

/** A section heading: the display face, large, with room to breathe. */
export const heading =
  "font-display text-[38px] leading-[1.08] tracking-[-0.01em] sm:text-[52px]";

/** Body copy under a heading. */
export const lede = "max-w-[62ch] text-[17px] leading-[1.65] text-text-dim";

/** A plate on the void: hairline border, faint fill, generous radius. */
export const plate = "rounded-[16px] border border-border bg-card/60 p-[22px]";

/** The filled call to action. */
export const primary =
  "group inline-flex items-center justify-center gap-[9px] rounded-[10px] " +
  "border border-accent bg-accent px-[20px] py-[11px] text-[15px] font-semibold " +
  "text-accent-text transition hover:brightness-[1.1] " +
  "shadow-[0_10px_40px_-12px_var(--color-accent)]";

/** The quiet one beside it. */
export const secondary =
  "group inline-flex items-center justify-center gap-[9px] rounded-[10px] " +
  "border border-border bg-plate px-[20px] py-[11px] text-[15px] font-semibold " +
  "text-text transition hover:border-accent hover:bg-accent-soft";

/** A rounded label, used for platforms and statuses. */
export const pill =
  "inline-flex items-center gap-[6px] rounded-[99px] border border-border " +
  "bg-plate px-[11px] py-[4px] text-[12px] font-semibold text-text-dim";
