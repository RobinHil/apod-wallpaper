import type { ReactNode } from "react";

/** What every icon accepts: the size and colour its surroundings want. */
interface IconProps {
  className?: string;
}

/** The size icons take unless their caller asks for another: the one buttons use. */
const DEFAULT_CLASS = "size-[15px] shrink-0";

/**
 * Every icon in the panel but the application's own is the same thing: a
 * 24x24 stroked outline that takes its colour from the text around it and
 * its size from its caller.
 */
function Icon({
  children,
  className = DEFAULT_CLASS,
}: {
  children: ReactNode;
  className?: string;
}) {
  return (
    <svg
      className={className}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={2}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      {children}
    </svg>
  );
}

/**
 * The application's own icon, the one the desktop shows. The only icon here
 * that is not a stroked outline: it carries its own colours, so it takes no
 * colour from its surroundings. It is a copy of `src-tauri/icons/app-icon.svg`,
 * which the packaged icons are generated from; change one and change the other.
 * Its gradient ids are prefixed, since inlining puts them in the page's
 * namespace alongside every other definition.
 */
export function AppIcon({ className }: IconProps) {
  return (
    <svg
      className={className}
      viewBox="0 0 1024 1024"
      fill="none"
      aria-hidden="true"
    >
      <defs>
        <radialGradient id="app-icon-planet" cx="38%" cy="32%" r="85%">
          <stop offset="0%" stopColor="#9cc2ff" />
          <stop offset="45%" stopColor="#4c85f0" />
          <stop offset="100%" stopColor="#1d3d8f" />
        </radialGradient>
        <linearGradient id="app-icon-ring" x1="0%" y1="0%" x2="100%" y2="0%">
          <stop offset="0%" stopColor="#c7d8f7" stopOpacity="0.35" />
          <stop offset="50%" stopColor="#dce8ff" stopOpacity="0.95" />
          <stop offset="100%" stopColor="#c7d8f7" stopOpacity="0.35" />
        </linearGradient>
        <clipPath id="app-icon-lower-half">
          <rect x="-360" y="0" width="720" height="360" />
        </clipPath>
      </defs>
      <g transform="translate(512 512) scale(1.35) rotate(-22)">
        <ellipse
          cx="0"
          cy="0"
          rx="332"
          ry="108"
          stroke="url(#app-icon-ring)"
          strokeWidth="34"
          opacity="0.85"
        />
        <circle cx="0" cy="0" r="196" fill="url(#app-icon-planet)" />
        <path
          d="M -196 0 a 196 196 0 0 0 392 0 a 240 196 0 0 1 -392 0 z"
          fill="#0a0d1c"
          opacity="0.35"
        />
        <ellipse
          cx="0"
          cy="0"
          rx="332"
          ry="108"
          stroke="url(#app-icon-ring)"
          strokeWidth="34"
          clipPath="url(#app-icon-lower-half)"
        />
      </g>
      <circle cx="912" cy="539" r="35" fill="#8ea6cf" />
      <circle cx="901" cy="528" r="11" fill="#b9c9e8" opacity="0.8" />
    </svg>
  );
}

export function AlertIcon({ className }: IconProps) {
  return (
    <Icon className={className}>
      <circle cx="12" cy="12" r="10" />
      <line x1="12" y1="8" x2="12" y2="12" />
      <line x1="12" y1="16" x2="12.01" y2="16" />
    </Icon>
  );
}

export function CloseIcon({ className }: IconProps) {
  return (
    <Icon className={className}>
      <line x1="18" y1="6" x2="6" y2="18" />
      <line x1="6" y1="6" x2="18" y2="18" />
    </Icon>
  );
}

export function PlayIcon({ className }: IconProps) {
  return (
    <Icon className={className}>
      <polygon points="5 3 19 12 5 21 5 3" />
    </Icon>
  );
}

export function ExternalLinkIcon({ className }: IconProps) {
  return (
    <Icon className={className}>
      <path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6" />
      <polyline points="15 3 21 3 21 9" />
      <line x1="10" y1="14" x2="21" y2="3" />
    </Icon>
  );
}

export function CalendarIcon({ className }: IconProps) {
  return (
    <Icon className={className}>
      <rect x="3" y="4" width="18" height="18" rx="2" ry="2" />
      <line x1="16" y1="2" x2="16" y2="6" />
      <line x1="8" y1="2" x2="8" y2="6" />
      <line x1="3" y1="10" x2="21" y2="10" />
    </Icon>
  );
}

export function ShuffleIcon({ className }: IconProps) {
  return (
    <Icon className={className}>
      <polyline points="16 3 21 3 21 8" />
      <line x1="4" y1="20" x2="21" y2="3" />
      <polyline points="21 16 21 21 16 21" />
      <line x1="15" y1="15" x2="21" y2="21" />
      <line x1="4" y1="4" x2="9" y2="9" />
    </Icon>
  );
}

/** A calendar with a day singled out: the "specific date" mode. */
export function CalendarPickIcon({ className }: IconProps) {
  return (
    <Icon className={className}>
      <rect x="3" y="4" width="18" height="18" rx="2" ry="2" />
      <line x1="16" y1="2" x2="16" y2="6" />
      <line x1="8" y1="2" x2="8" y2="6" />
      <line x1="3" y1="10" x2="21" y2="10" />
      <circle cx="12" cy="16" r="2.5" />
    </Icon>
  );
}

export function RefreshIcon({ className }: IconProps) {
  return (
    <Icon className={className}>
      <polyline points="23 4 23 10 17 10" />
      <polyline points="1 20 1 14 7 14" />
      <path d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15" />
    </Icon>
  );
}

export function KeyIcon({ className }: IconProps) {
  return (
    <Icon className={className}>
      <path d="M21 2l-2 2m-7.61 7.61a5.5 5.5 0 1 1-7.778 7.778 5.5 5.5 0 0 1 7.777-7.777zm0 0L15.5 7.5m0 0l3 3L22 7l-3-3m-3.5 3.5L19 4" />
    </Icon>
  );
}

export function PowerIcon({ className }: IconProps) {
  return (
    <Icon className={className}>
      <path d="M18.36 6.64a9 9 0 1 1-12.73 0" />
      <line x1="12" y1="2" x2="12" y2="12" />
    </Icon>
  );
}

export function SpinnerIcon({ className }: IconProps) {
  return (
    <Icon className={className}>
      <path d="M21 12a9 9 0 1 1-9-9" />
    </Icon>
  );
}
