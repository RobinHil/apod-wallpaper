import { asset } from "../paths";

/**
 * The application's own icon, served as a file rather than inlined: the SVG
 * declares its gradients by id, and two inlined copies on one page would
 * fight over those ids.
 */
export function Logo({ className = "size-[22px]" }: { className?: string }) {
  return (
    <img src={asset("app-icon.svg")} alt="" aria-hidden="true" className={className} />
  );
}
