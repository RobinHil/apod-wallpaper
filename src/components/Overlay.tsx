import { SpinnerIcon } from "./Icons";

/** Covers the whole panel while a command is in flight. */
export function Overlay({ label }: { label: string }) {
  return (
    <div className="overlay">
      <div className="overlay-box">
        <SpinnerIcon />
        <span>{label}</span>
      </div>
    </div>
  );
}
