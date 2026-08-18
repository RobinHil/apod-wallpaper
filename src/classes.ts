/**
 * The utility strings that more than one component needs.
 *
 * They are constants rather than a shared component because they are only
 * ever spread onto an existing element. They are also never merged: two
 * utilities that set the same property, such as `bg-card` and `bg-accent`,
 * are resolved by their order in the generated stylesheet and not by their
 * order in the attribute, so a variant spells out its own colours instead of
 * layering them over a base.
 */

/** A panel section: white plate, hairline border, rounded corners. */
export const card = "rounded-[10px] border border-border bg-card px-[16px] py-[14px]";

/** The small capitalised heading above a group of controls. */
export const sectionTitle =
  "mb-[6px] text-[12px] font-semibold tracking-[0.04em] text-text-dim uppercase";

/** Explanatory small print under a control. */
export const hint = "mt-[4px] mb-[8px] text-[12px] text-text-dim";

/** An ordinary push button. */
export const button =
  "inline-flex items-center justify-center gap-[7px] rounded-[8px] " +
  "border border-border bg-card px-[12px] py-[7px] hover:border-accent";

/** The one call to action on a card, filled in the accent colour. */
export const primaryButton =
  "mt-[14px] inline-flex w-full items-center justify-center gap-[7px] " +
  "rounded-[8px] border border-accent bg-accent px-[12px] py-[7px] " +
  "font-semibold text-accent-text hover:brightness-[1.08]";

/** A button that reads as a link: no chrome, accent coloured, underlined on hover. */
export const linkButton =
  "inline-flex items-center justify-center gap-[7px] rounded-[8px] " +
  "border-none px-0 py-[4px] text-[12px] text-accent hover:underline";

/** A text input filling the width its form leaves it. */
export const field =
  "flex-1 cursor-text rounded-[8px] border border-border bg-bg px-[10px] " +
  "py-[7px] text-text select-text focus:border-accent focus:outline-none";

/** A row of inputs and the button that submits them. */
export const form = "flex gap-[8px]";

/**
 * One choice in a segmented control. The buttons overlap by a pixel so that
 * neighbours share a border, and the raised `z-index` puts whichever one is
 * hovered or selected on top of that shared edge.
 */
export const segmentedButton =
  "relative -mr-px inline-flex flex-1 items-center justify-center gap-[7px] " +
  "rounded-none border px-[6px] py-[7px] text-[13px] whitespace-nowrap " +
  "first:rounded-l-[8px] last:mr-0 last:rounded-r-[8px] hover:z-[1]";

export const segmentedIdle = "border-border bg-card hover:border-accent";

export const segmentedActive = "z-[1] border-accent bg-accent text-accent-text";
