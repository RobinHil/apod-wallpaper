import { SpinnerIcon } from "./Icons";

/** Covers the whole panel while a command is in flight. */
export function Overlay({ label }: { label: string }) {
  return (
    <div className="fixed inset-0 z-10 flex items-center justify-center bg-scrim backdrop-blur-[2px]">
      <div className="flex items-center gap-[10px] rounded-[10px] border border-border bg-card px-[20px] py-[14px] text-[13px] font-semibold shadow-[0_6px_24px_rgba(0,0,0,0.18)]">
        <SpinnerIcon className="size-[18px] animate-spin text-accent" />
        <span>{label}</span>
      </div>
    </div>
  );
}
