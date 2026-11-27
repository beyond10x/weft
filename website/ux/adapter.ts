// The UX harness's adapter for the weft documentation site (ui-iteration-loop § 1, docs-site values):
// the pages, the viewports, the themes, the always-visible elements and the screen-only user's
// scenarios. No guidance cue and no protected focus: L0 and gate 6 do not apply; a scenario is done
// when the screen-only user has its truth in view.

import { existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';

export interface Viewport {
  name: string;
  width: number;
  height: number;
}

export interface Theme {
  name: 'light' | 'dark';
}

/** A control found by its accessible role and name only. */
export interface Control {
  role: string;
  name: string;
}

/** One page of the sitemap, by its path below the base URL (`''` is the landing page). */
export interface SitePage {
  name: string;
  path: string;
}

/** One always-visible element. */
export interface Required {
  name: string;
  selector: string;
  /** Only on pages that have it (the docs sidebar is not on the landing page). */
  onlyWhere?: string;
}

export interface Scenario {
  id: string;
  question: string;
  /** The page the user starts on, below the base URL. */
  start: string;
  /** Words the user scans the screen for, best first. */
  words: string[];
  /** The page that answers, below the base URL, and a text of the answer that must be in view. */
  truth: { path: string; text: string };
}

export const UX = __dirname;
export const SITE = join(UX, '..');
export const OUT = process.env.UX_OUT ?? join(UX, '.out');
export const BUILD = process.env.UX_BUILD ?? join(SITE, 'build');
export const PORT = Number(process.env.UX_PORT ?? 4327);
export const ORIGIN = `http://127.0.0.1:${PORT}`;
export const BASE_PATH = '/weft/';
export const BASE = `${ORIGIN}${BASE_PATH}`;

/** Docusaurus shows the desktop navbar and the docs sidebar from 997 px. */
const DESKTOP = 997;

export const GATING: Viewport[] = [
  { name: '1280x800', width: 1280, height: 800 },
  { name: '390x844', width: 390, height: 844 },
];

export const MEASURED: Viewport[] = [
  { name: '1440x900', width: 1440, height: 900 },
  { name: '1024x768', width: 1024, height: 768 },
  { name: '768x1024', width: 768, height: 1024 },
];

export const THEMES: Theme[] = [{ name: 'light' }, { name: 'dark' }];

/** Every route of the built sitemap, named by its path. */
export function sitemapPages(): SitePage[] {
  const file = join(BUILD, 'sitemap.xml');
  if (!existsSync(file)) throw new Error(`no ${file}: build the site first (npm --prefix website run build)`);
  return [...readFileSync(file, 'utf8').matchAll(/<loc>([^<]+)<\/loc>/g)]
    .map((m) => new URL(m[1]!).pathname.slice(BASE_PATH.length).replace(/\/$/, ''))
    .sort()
    .map((path) => ({ name: path === '' ? 'landing' : path.replace(/\//g, '-'), path }));
}

export function alwaysVisible(viewport: Viewport): Required[] {
  const common: Required[] = [
    { name: 'navbar', selector: 'nav.navbar' },
    { name: 'search', selector: 'nav.navbar input.navbar__search-input' },
    { name: 'theme toggle', selector: 'nav.navbar [class*="colorModeToggle"] button' },
  ];
  return viewport.width >= DESKTOP
    ? [...common, { name: 'docs sidebar', selector: 'nav[aria-label="Docs sidebar"]', onlyWhere: '.theme-doc-sidebar-container' }]
    : [...common, { name: 'navigation menu button', selector: 'nav.navbar button.navbar__toggle' }];
}

/** Navigation regions: a control in one is a glance of 1. */
export const NAVIGATION = ['nav.navbar', 'nav[aria-label="Docs sidebar"]', '.table-of-contents', '.navbar-sidebar'];

/** Controls that reveal hidden navigation, tried when nothing on screen matches. */
export const REVEAL: Control[] = [{ role: 'button', name: 'Toggle navigation bar' }];

/** Controls that close revealed navigation that holds no match. */
export const DISMISS: Control[] = [{ role: 'button', name: 'Close navigation bar' }];

/** The search field: the user types the first word there when no control on screen carries a word. */
export const SEARCH: Control = { role: 'searchbox', name: 'Search' };

/** A selector present until the page is interactive (hydrated). */
export const NOT_READY = '[class*="toggleButtonDisabled"]';

export const SCENARIOS: Scenario[] = [
  {
    id: 'validate',
    question: 'Find the command that validates the specification',
    start: '',
    words: ['validate', 'specification'],
    truth: { path: 'docs/specification/validate', text: 'ess specify validate --path ess --strict-requires' },
  },
  {
    id: 'ess-version',
    question: 'Find the ess version the specification requires',
    start: '',
    words: ['requires', 'version', 'get started'],
    truth: { path: 'docs/getting-started', text: 'the version ess/ess-inputs.yaml requires' },
  },
  {
    id: 'replay',
    question: 'Find how to replay a walk',
    start: '',
    words: ['replay', 'weft-sim'],
    truth: { path: 'docs/simulator/run', text: 'weft-sim replay --spec ess' },
  },
  {
    id: 'planned',
    question: 'Find which parts are planned',
    start: '',
    words: ['status', 'planned'],
    truth: { path: 'docs/status', text: 'nothing planned runs today' },
  },
  {
    id: 'border',
    question: 'Find what the border of a formation is',
    start: '',
    words: ['border', 'formation', 'formations', 'concepts'],
    truth: { path: 'docs/concepts/formations', text: 'The components reachable from outside the formation' },
  },
];
