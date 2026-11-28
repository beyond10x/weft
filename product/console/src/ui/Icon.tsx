// The icon set: semantic names (the `icon` of a nav section, a primitive or a button) mapped to
// 24×24 stroke paths. An icon with a label is announced; one without is decoration.

const PATHS: Record<string, string> = {
  gauge: 'M12 14l3.5-3.5M4.9 19a9 9 0 1 1 14.2 0M12 14a1 1 0 1 0 0 .01',
  server: 'M4 5h16v5H4zM4 14h16v5H4zM8 7.5h.01M8 16.5h.01',
  calendar: 'M5 6h14v14H5zM5 10h14M9 3v4M15 3v4',
  users: 'M9 11a3.5 3.5 0 1 0 0-7 3.5 3.5 0 0 0 0 7zM2.5 20a6.5 6.5 0 0 1 13 0M16 4.3a3.5 3.5 0 0 1 0 6.4M18 14a6.5 6.5 0 0 1 3.5 6',
  layers: 'M12 3l9 5-9 5-9-5 9-5zM3 13l9 5 9-5M3 17.5l9 5 9-5',
  home: 'M4 11l8-7 8 7v9h-5v-6H9v6H4z',
  rocket: 'M14 4c3 0 6 3 6 6l-7 7-6-6 7-7zM7 11l-3 1-1 3 4-1M13 17l-1 3-3 1 1-4M15.5 8.5h.01',
  database: 'M4 6c0-1.7 3.6-3 8-3s8 1.3 8 3-3.6 3-8 3-8-1.3-8-3zM4 6v12c0 1.7 3.6 3 8 3s8-1.3 8-3V6M4 12c0 1.7 3.6 3 8 3s8-1.3 8-3',
  chart: 'M4 20V4M4 20h16M8 16v-5M12 16V8M16 16v-8',
  menu: 'M4 6h16M4 12h16M4 18h16',
  close: 'M6 6l12 12M18 6L6 18',
  more: 'M5 12h.01M12 12h.01M19 12h.01',
  chevron_down: 'M6 9l6 6 6-6',
  chevron_right: 'M9 6l6 6-6 6',
  chevron_left: 'M15 6l-6 6 6 6',
  search: 'M11 18a7 7 0 1 0 0-14 7 7 0 0 0 0 14zM20 20l-4-4',
  check: 'M5 12.5l4.5 4.5L19 7.5',
  info: 'M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18zM12 11v6M12 7.5h.01',
  alert: 'M12 4l9 16H3l9-16zM12 10v4.5M12 17.5h.01',
  sun: 'M12 17a5 5 0 1 0 0-10 5 5 0 0 0 0 10zM12 1.5v2M12 20.5v2M4.6 4.6l1.4 1.4M18 18l1.4 1.4M1.5 12h2M20.5 12h2M4.6 19.4L6 18M18 6l1.4-1.4',
  moon: 'M20 14.5A8.5 8.5 0 0 1 9.5 4 8.5 8.5 0 1 0 20 14.5z',
  monitor: 'M3 4h18v12H3zM8 20h8M12 16v4',
  copy: 'M9 9h11v11H9zM5 15H4V4h11v1',
  sort: 'M8 9l4-4 4 4M8 15l4 4 4-4',
  sort_up: 'M8 14l4-4 4 4',
  sort_down: 'M8 10l4 4 4-4',
  terminal: 'M4 5h16v14H4zM8 10l3 2-3 2M13 15h3',
  plug: 'M9 3v5M15 3v5M7 8h10v4a5 5 0 0 1-10 0V8zM12 17v4',
  dot: 'M12 12h.01',
  logout: 'M15 4h4v16h-4M10 8l-4 4 4 4M6 12h10',
};

export function Icon({ name, label, size = 18, className }: { name: string; label?: string; size?: number; className?: string }) {
  const d = PATHS[name] ?? PATHS.dot;
  return (
    <svg
      className={className ? `icon ${className}` : 'icon'}
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.8}
      strokeLinecap="round"
      strokeLinejoin="round"
      role={label ? 'img' : undefined}
      aria-label={label}
      aria-hidden={label ? undefined : true}
      focusable="false"
    >
      <path d={d} />
    </svg>
  );
}

export function hasIcon(name: string): boolean {
  return name in PATHS;
}
