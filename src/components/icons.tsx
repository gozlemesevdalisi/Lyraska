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
