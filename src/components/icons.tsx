/** Düğmelerdeki basit, özgün simgeler (24x24, currentColor). */

const base = {
  width: 20,
  height: 20,
  viewBox: "0 0 24 24",
  fill: "currentColor",
  "aria-hidden": true,
} as const;

export function EjectIcon() {
  return (
    <svg {...base}>
      <path d="M12 4 4 14h16L12 4Z" />
      <rect x="4" y="16.5" width="16" height="3" rx="1" />
    </svg>
  );
}

export function PlayIcon() {
  return (
    <svg {...base}>
      <path d="M7 4.5v15a1 1 0 0 0 1.5.86l12-7.5a1 1 0 0 0 0-1.72l-12-7.5A1 1 0 0 0 7 4.5Z" />
    </svg>
  );
}

export function PauseIcon() {
  return (
    <svg {...base}>
      <rect x="5.5" y="4.5" width="4.5" height="15" rx="1" />
      <rect x="14" y="4.5" width="4.5" height="15" rx="1" />
    </svg>
  );
}

export function StopIcon() {
  return (
    <svg {...base}>
      <rect x="5.5" y="5.5" width="13" height="13" rx="1.5" />
    </svg>
  );
}

export function PreviousIcon() {
  return (
    <svg {...base}>
      <rect x="4.5" y="5" width="3" height="14" rx="1" />
      <path d="M20 5.9v12.2a1 1 0 0 1-1.55.83L9.6 13.08a1.3 1.3 0 0 1 0-2.16l8.85-5.85A1 1 0 0 1 20 5.9Z" />
    </svg>
  );
}

export function NextIcon() {
  return (
    <svg {...base}>
      <rect x="16.5" y="5" width="3" height="14" rx="1" />
      <path d="M4 5.9v12.2a1 1 0 0 0 1.55.83l8.85-5.85a1.3 1.3 0 0 0 0-2.16L5.55 5.07A1 1 0 0 0 4 5.9Z" />
    </svg>
  );
}

export function FolderPlusIcon() {
  return (
    <svg {...base}>
      <path d="M3 6.5A1.5 1.5 0 0 1 4.5 5h4.6l2 2h8.4A1.5 1.5 0 0 1 21 8.5v9a1.5 1.5 0 0 1-1.5 1.5h-15A1.5 1.5 0 0 1 3 17.5v-11Zm8 5v2H9v1.5h2v2h1.5v-2h2V13.5h-2v-2H11Z" />
    </svg>
  );
}

export function MusicPlusIcon() {
  return (
    <svg {...base}>
      <path d="M14 3v10.55A3.5 3.5 0 1 0 16 16.5V7h4V3h-6Z" />
      <path d="M3 6h8v2H3zM3 10h8v2H3zM3 14h6v2H3z" />
    </svg>
  );
}

export function RefreshIcon() {
  return (
    <svg {...base} fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round">
      <path d="M19 12a7 7 0 1 1-2.05-4.95" />
      <path d="M19.5 4.5v4h-4" />
    </svg>
  );
}

export function CloseIcon() {
  return (
    <svg
      {...base}
      width={14}
      height={14}
      fill="none"
      stroke="currentColor"
      strokeWidth="2.4"
      strokeLinecap="round"
    >
      <path d="M6 6l12 12M18 6 6 18" />
    </svg>
  );
}

export function SearchIcon() {
  return (
    <svg
      {...base}
      width={16}
      height={16}
      fill="none"
      stroke="currentColor"
      strokeWidth="2.2"
      strokeLinecap="round"
    >
      <circle cx="10.5" cy="10.5" r="6" />
      <path d="m15 15 5 5" />
    </svg>
  );
}

const stroke = {
  ...base,
  fill: "none",
  stroke: "currentColor",
  strokeWidth: 1.8,
  strokeLinecap: "round",
  strokeLinejoin: "round",
} as const;

export function LibraryIcon() {
  return (
    <svg {...stroke}>
      <path d="M4 6h16M4 12h16M4 18h10" />
    </svg>
  );
}

export function SlidersIcon() {
  return (
    <svg {...stroke}>
      <path d="M5 20v-8M5 8V4M12 20v-4M12 12V4M19 20v-6M19 10V4" />
      <path d="M3 10h4M10 14h4M17 12h4" />
    </svg>
  );
}

export function SettingsIcon() {
  return (
    <svg {...stroke}>
      <circle cx="12" cy="12" r="3" />
      <path d="M12 2.5v3M12 18.5v3M2.5 12h3M18.5 12h3M5.3 5.3l2.1 2.1M16.6 16.6l2.1 2.1M5.3 18.7l2.1-2.1M16.6 7.4l2.1-2.1" />
    </svg>
  );
}

/** Lyraska işareti: Lyra takımyıldızının beş yıldızı (Vega en parlak). */
export function LyraMark() {
  return (
    <svg width={26} height={26} viewBox="0 0 26 26" aria-hidden className="brand__mark">
      <path
        d="M20 4 14 8 9 12 7 22 13 19 14 8"
        fill="none"
        stroke="currentColor"
        strokeOpacity="0.45"
        strokeWidth="1"
      />
      <g fill="currentColor">
        <circle cx="20" cy="4" r="2" />
        <circle cx="14" cy="8" r="1.3" />
        <circle cx="9" cy="12" r="1.3" />
        <circle cx="7" cy="22" r="1.4" />
        <circle cx="13" cy="19" r="1.2" />
      </g>
    </svg>
  );
}
