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
