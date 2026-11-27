// In-page probes, passed to `page.evaluate`. Each function is self-contained: Playwright sends its
// source to the page, so none may use anything declared outside its own body. Adapted from the
// docs harness of a sibling beyond10x site.

export interface Box {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface Overflow {
  /**
   * `document` (the page is wider than the viewport, scrollable or not), `viewport` (an element cut
   * by the viewport's edge or parked past its right edge), `box` (ink past the block that holds it,
   * across or down) or `cut` (ink cut off by a box that hides its overflow).
   */
  kind: 'document' | 'viewport' | 'box' | 'cut';
  what: string;
  /** How far past, in CSS pixels. */
  by: number;
}

/**
 * Hard gate 1, docs variant: no horizontal page scroll and nothing a reader cannot reach. Vertical
 * page scroll is allowed, and so is content inside a box that scrolls on its own (a code block, a
 * wide table in its scroll container). A page wider than the viewport fails even when html or body
 * hides the overflow: what is past the right edge is then out of reach. An element entirely left
 * of or above the viewport (a skip link parked off-screen) does not count, and neither does text
 * hidden for screen readers only (a box of 2 px or less) nor ink a hiding box shows none of.
 */
export function overflow(): Overflow[] {
  const describe = (el: Element): string => {
    const id = el.id ? `#${el.id}` : '';
    const cls = typeof el.className === 'string' && el.className.trim() ? `.${el.className.trim().split(/\s+/).slice(0, 2).join('.')}` : '';
    const text = (el.textContent ?? '').replace(/\s+/g, ' ').trim().slice(0, 40);
    return `${el.tagName.toLowerCase()}${id}${cls}${text ? ` "${text}"` : ''}`;
  };
  const styles = new Map<Element, CSSStyleDeclaration>();
  const style = (el: Element): CSSStyleDeclaration => {
    let s = styles.get(el);
    if (!s) {
      s = getComputedStyle(el);
      styles.set(el, s);
    }
    return s;
  };
  const hides = (v: string) => v === 'hidden' || v === 'clip';
  const scrolls = (v: string) => v === 'auto' || v === 'scroll';
  const out: Overflow[] = [];
  const root = document.scrollingElement ?? document.documentElement;
  if (root.scrollWidth > window.innerWidth + 1) {
    const hidden = hides(style(document.documentElement).overflowX) || hides(style(document.body).overflowX);
    out.push({ kind: 'document', what: hidden ? 'document (past the right edge, out of reach: overflow-x hidden)' : 'document', by: root.scrollWidth - window.innerWidth });
  }
  const shown = (el: Element) => {
    for (let a: Element | null = el; a; a = a.parentElement) {
      const s = style(a);
      if (s.display === 'none' || s.visibility === 'hidden') return false;
    }
    return true;
  };
  /** An ancestor below body that scrolls (or hides) horizontally. */
  const boxedX = (el: Element) => {
    for (let a = el.parentElement; a && a !== document.body; a = a.parentElement) {
      const x = style(a).overflowX;
      if (scrolls(x) || hides(x)) return true;
    }
    return false;
  };
  const cuts: { el: Element; by: number }[] = [];
  for (const el of Array.from(document.body.querySelectorAll('*'))) {
    if (el instanceof SVGElement && !(el instanceof SVGSVGElement)) continue;
    const r = el.getBoundingClientRect();
    if (r.width === 0 || r.height === 0) continue;
    if (style(el).position === 'fixed' && (r.left >= window.innerWidth || r.right <= 0)) continue;
    const cutByViewport = (r.right > window.innerWidth + 1 && r.left < window.innerWidth) || (r.left < -1 && r.right > 0);
    const pastRight = r.left >= window.innerWidth - 1;
    if (!(cutByViewport || pastRight) || boxedX(el) || !shown(el)) continue;
    cuts.push({ el, by: Math.round(Math.max(r.right - window.innerWidth, -r.left)) });
  }
  // A cut element cuts its descendants too: report the outermost one.
  const cutSet = new Set(cuts.map((c) => c.el));
  for (const { el, by } of cuts) if (!el.parentElement || !cutSet.has(el.parentElement)) out.push({ kind: 'viewport', what: describe(el), by });
  // Ink: a run of text, an image or a drawing. It may not reach past the block that holds it,
  // across or down, unless a box on the way scrolls in that axis; and a box on the way that hides
  // its overflow may not cut it off. A layout idiom that widens an empty box (a negative margin
  // around padded columns) puts no ink past anything and does not count.
  const blockOf = (el: Element): Element => {
    for (let a: Element | null = el; a && a !== document.body; a = a.parentElement) {
      const d = style(a).display;
      if (d !== 'inline' && d !== 'contents' && !d.startsWith('inline') && !(a instanceof SVGElement && !(a instanceof SVGSVGElement))) return a;
    }
    return document.body;
  };
  const past = new Map<Element, number>();
  const cut = new Map<Element, number>();
  const note = (holder: Element, r: DOMRect, glyphs: boolean) => {
    if (r.width === 0 || r.height === 0 || !shown(holder)) return;
    // A glyph box is taller than a tight line (line-height below the font's own): the overhang
    // above and below the line box is the font's, not content past its box.
    const lineHeight = parseFloat(style(holder).lineHeight);
    const slack = !glyphs ? 0 : Number.isFinite(lineHeight) ? Math.max(0, (r.height - lineHeight) / 2) : r.height * 0.25;
    const down = (b: DOMRect) => Math.max(r.bottom - b.bottom, b.top - r.top) - slack;
    let sx = false;
    let sy = false;
    for (let a: Element | null = holder; a && a !== document.body; a = a.parentElement) {
      const s = style(a);
      const hx = hides(s.overflowX);
      const hy = hides(s.overflowY);
      if (hx || hy) {
        const b = a.getBoundingClientRect();
        // Hidden for screen readers only, or ink the box shows none of: not shown, so not cut.
        if (b.width <= 2 || b.height <= 2) return;
        if (r.right <= b.left || r.left >= b.right || r.bottom <= b.top || r.top >= b.bottom) return;
        const by = Math.max(hx && !sx ? Math.max(r.right - b.right, b.left - r.left) : 0, hy && !sy ? down(b) : 0);
        if (by > 1) cut.set(a, Math.max(cut.get(a) ?? 0, Math.round(by)));
        return;
      }
      if (scrolls(s.overflowX)) sx = true;
      if (scrolls(s.overflowY)) sy = true;
    }
    const block = blockOf(holder);
    const b = block.getBoundingClientRect();
    const by = Math.max(sx ? 0 : Math.max(r.right - b.right, b.left - r.left), sy ? 0 : down(b));
    if (by > 1) past.set(block, Math.max(past.get(block) ?? 0, Math.round(by)));
  };
  const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
  const range = document.createRange();
  for (let node = walker.nextNode(); node; node = walker.nextNode()) {
    if (!(node.nodeValue ?? '').trim() || !node.parentElement || /^(SCRIPT|STYLE|NOSCRIPT)$/.test(node.parentElement.tagName)) continue;
    range.selectNodeContents(node);
    for (const r of Array.from(range.getClientRects())) note(node.parentElement, r, true);
  }
  for (const el of Array.from(document.body.querySelectorAll('img, svg, video, canvas, iframe'))) {
    if (el instanceof SVGElement && el.parentElement?.closest('svg')) continue;
    note(el.parentElement ?? el, el.getBoundingClientRect(), false);
  }
  for (const [block, by] of past) out.push({ kind: 'box', what: describe(block), by });
  for (const [box, by] of cut) out.push({ kind: 'cut', what: describe(box), by });
  return out;
}


export interface Visibility {
  name: string;
  selector: string;
  ok: boolean;
  why: string;
}

/** Hard gate 2: each selector's first shown element is inside the viewport, not clipped and not covered. */
export function fullyVisible(required: { name: string; selector: string }[]): Visibility[] {
  const out: Visibility[] = [];
  // An overlay that lets the pointer through (pointer-events: none) still hides what is under it:
  // that is lifted while looking, once for every element of the call (adding the lift and removing
  // it restyle the whole page), and taken away before the call returns.
  let lift: HTMLStyleElement | null = null;
  try {
    for (const { name, selector } of required) {
      const found = Array.from(document.querySelectorAll(selector)).filter((el) => {
        const r = el.getBoundingClientRect();
        const s = getComputedStyle(el);
        return r.width > 0 && r.height > 0 && s.visibility !== 'hidden';
      });
      const el = found[0];
      if (!el) {
        out.push({ name, selector, ok: false, why: 'absent or not shown' });
        continue;
      }
      const r = el.getBoundingClientRect();
      let why = '';
      if (r.left < -0.5 || r.top < -0.5 || r.right > window.innerWidth + 0.5 || r.bottom > window.innerHeight + 0.5) {
        why = `outside the viewport (${Math.round(r.left)},${Math.round(r.top)} ${Math.round(r.width)}x${Math.round(r.height)})`;
      }
      for (let a = el.parentElement; !why && a && a !== document.body; a = a.parentElement) {
        const style = getComputedStyle(a);
        // A clip path or paint containment clips even a fixed descendant to the ancestor's box.
        const clips = style.clipPath !== 'none' || /paint|strict|content/.test(style.contain);
        if (style.overflowX === 'visible' && style.overflowY === 'visible' && !clips) continue;
        const c = a.getBoundingClientRect();
        if (r.left < c.left - 0.5 || r.top < c.top - 0.5 || r.right > c.right + 0.5 || r.bottom > c.bottom + 0.5) {
          why = `clipped by ${a.tagName.toLowerCase()}`;
        }
      }
      for (let a: Element | null = el; !why && a; a = a.parentElement) {
        if (Number(getComputedStyle(a).opacity) === 0) why = a === el ? 'at opacity 0' : `at opacity 0 inside ${a.tagName.toLowerCase()}`;
      }
      if (!why) {
        const inset = 2;
        const points: [number, number][] = [
          [(r.left + r.right) / 2, (r.top + r.bottom) / 2],
          [r.left + inset, r.top + inset],
          [r.right - inset, r.top + inset],
          [r.left + inset, r.bottom - inset],
          [r.right - inset, r.bottom - inset],
        ];
        // With the overlays lifted, walk down the stack at each point. An element above that paints
        // nothing (no background, no text of its own, no image) hides nothing.
        if (!lift) {
          lift = document.createElement('style');
          lift.textContent = '*, *::before, *::after { pointer-events: auto !important; }';
          document.head.appendChild(lift);
        }
        const paints = (e: Element): boolean => {
          const s = getComputedStyle(e);
          if (Number(s.opacity) === 0 || s.visibility === 'hidden') return false;
          if (/^(IMG|SVG|CANVAS|VIDEO|IFRAME|INPUT|BUTTON|SELECT|TEXTAREA)$/i.test(e.tagName)) return true;
          if (s.backgroundImage !== 'none') return true;
          const alpha = s.backgroundColor.match(/rgba?\(([^)]+)\)/)?.[1]?.split(/[ ,/]+/).filter(Boolean)[3];
          if (s.backgroundColor !== 'transparent' && (alpha === undefined || Number(alpha) > 0)) return true;
          return Array.from(e.childNodes).some((n) => n.nodeType === Node.TEXT_NODE && (n.nodeValue ?? '').trim() !== '');
        };
        points.forEach(([x, y], i) => {
          if (why) return;
          for (const hit of document.elementsFromPoint(x, y)) {
            if (hit === el || el.contains(hit)) return;
            if (hit.contains(el)) {
              // The element is not hit where its box is: a clip path or the like cuts it away. A
              // rounded corner does the same at a corner point, so only the centre counts.
              if (i === 0) why = `not shown at its centre ${Math.round(x)},${Math.round(y)}`;
              return;
            }
            // Drawn on the control and inside its box (the search field's magnifier icon): part of it.
            const h = hit.getBoundingClientRect();
            const onIt = h.left >= r.left - 0.5 && h.top >= r.top - 0.5 && h.right <= r.right + 0.5 && h.bottom <= r.bottom + 0.5;
            if (paints(hit) && !onIt) {
              why = `covered at ${Math.round(x)},${Math.round(y)} by ${hit.tagName.toLowerCase()}`;
              return;
            }
          }
          if (i === 0) why = `not shown at its centre ${Math.round(x)},${Math.round(y)}`;
        });
      }
      out.push({ name, selector, ok: !why, why: why || 'in view' });
    }
  } finally {
    lift?.remove();
  }
  return out;
}

export interface Candidate {
  index: number;
  role: string;
  name: string;
  href: string | null;
  /** The heading of the section the control sits in. */
  context: string;
  box: Box;
  inView: boolean;
  covered: boolean;
  /** The control sits in a navigation region. */
  navigation: boolean;
}

/**
 * Controls a screen-only reader can see, with a role and a name computed as a sighted reader reads
 * them: `aria-label`, else the visible text, else the title. Reading order: top to bottom, then
 * left to right.
 */
export function candidates(navigation: string[]): Candidate[] {
  const query = 'a[href], button, input, [role=button], [role=link], [role=tab], [role=option], summary';
  const regions = navigation.flatMap((s) => Array.from(document.querySelectorAll(s)));
  const roleOf = (el: Element): string => {
    const explicit = el.getAttribute('role');
    if (explicit) return explicit;
    if (el.tagName === 'A') return 'link';
    if (el.tagName === 'BUTTON' || el.tagName === 'SUMMARY') return 'button';
    if (el instanceof HTMLInputElement) return el.type === 'search' || el.type === 'text' ? 'searchbox' : el.type;
    return 'generic';
  };
  // A sighted reader reads the visible text; a control with none (an icon) by its label or title.
  const nameOf = (el: Element): string => {
    if (el instanceof HTMLInputElement) return (el.placeholder || el.getAttribute('aria-label') || el.title || '').trim();
    const text = (el instanceof HTMLElement ? el.innerText : (el.textContent ?? '')).replace(/[\s​]+/g, ' ').trim();
    if (/[\p{L}\p{N}]/u.test(text)) return text;
    return (el.getAttribute('aria-label') ?? el.getAttribute('title') ?? text).trim();
  };
  const headingOf = (el: Element): string => {
    let at: Element | null = el;
    while (at && at !== document.body) {
      let sib: Element | null = at.previousElementSibling;
      while (sib) {
        if (/^H[1-4]$/.test(sib.tagName)) return (sib.textContent ?? '').replace(/\s+/g, ' ').replace(/​/g, '').trim();
        sib = sib.previousElementSibling;
      }
      at = at.parentElement;
    }
    return '';
  };
  const out: Candidate[] = [];
  Array.from(document.querySelectorAll(query)).forEach((el, index) => {
    const r = el.getBoundingClientRect();
    if (r.width === 0 || r.height === 0) return;
    for (let a: Element | null = el; a; a = a.parentElement) {
      const s = getComputedStyle(a);
      if (s.visibility === 'hidden' || s.display === 'none' || s.opacity === '0') return;
    }
    const inView = r.left >= -0.5 && r.top >= -0.5 && r.right <= window.innerWidth + 0.5 && r.bottom <= window.innerHeight + 0.5;
    const cx = (r.left + r.right) / 2;
    const cy = (r.top + r.bottom) / 2;
    const hit = inView ? document.elementFromPoint(cx, cy) : null;
    const covered = !hit || (hit !== el && !el.contains(hit) && !(hit instanceof HTMLLabelElement && hit.contains(el)));
    out.push({
      index,
      role: roleOf(el),
      name: nameOf(el),
      href: el instanceof HTMLAnchorElement ? el.href : null,
      context: headingOf(el),
      box: { x: r.left, y: r.top, width: r.width, height: r.height },
      inView,
      covered,
      navigation: regions.some((n) => n.contains(el)),
    });
  });
  return out.sort((a, b) => (Math.abs(a.box.y - b.box.y) > 4 ? a.box.y - b.box.y : a.box.x - b.box.x));
}

/**
 * Where a text is: 1 inside a navigation region and in view, 2 elsewhere in view, 3 on the page but
 * not in view, 0 absent. A text at opacity 0, or under an element that paints over it (a sticky
 * header), is not in view.
 */
export function textGlance(args: { text: string; navigation: string[] }): number {
  const want = args.text.replace(/\s+/g, ' ');
  const regions = args.navigation.flatMap((s) => Array.from(document.querySelectorAll(s)));
  const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
  const range = document.createRange();
  const paints = (e: Element): boolean => {
    const s = getComputedStyle(e);
    if (Number(s.opacity) === 0 || s.visibility === 'hidden') return false;
    if (/^(IMG|SVG|CANVAS|VIDEO|IFRAME|INPUT|BUTTON|SELECT|TEXTAREA)$/i.test(e.tagName)) return true;
    if (s.backgroundImage !== 'none') return true;
    const alpha = s.backgroundColor.match(/rgba?\(([^)]+)\)/)?.[1]?.split(/[ ,/]+/).filter(Boolean)[3];
    if (s.backgroundColor !== 'transparent' && (alpha === undefined || Number(alpha) > 0)) return true;
    return Array.from(e.childNodes).some((n) => n.nodeType === Node.TEXT_NODE && (n.nodeValue ?? '').trim() !== '');
  };
  // Looking through pointer-events: none takes a lift of it, made once for the call (adding it and
  // removing it restyle the whole page) and taken away before the call returns.
  let lift: HTMLStyleElement | null = null;
  /** An element painted over the text's holder at a point, looking through pointer-events: none. */
  const covered = (holder: Element, x: number, y: number): boolean => {
    if (!lift) {
      lift = document.createElement('style');
      lift.textContent = '*, *::before, *::after { pointer-events: auto !important; }';
      document.head.appendChild(lift);
    }
    for (const hit of document.elementsFromPoint(x, y)) {
      if (hit === holder || holder.contains(hit) || hit.contains(holder)) return false;
      if (paints(hit)) return true;
    }
    return false;
  };
  let best = 0;
  try {
    for (let node = walker.nextNode(); node; node = walker.nextNode()) {
      const holder = node.parentElement?.closest('tr, p, li, figcaption, h1, h2, h3, h4, label, pre, td, th') ?? node.parentElement;
      if (!holder || !node.parentElement) continue;
      const text = ((holder instanceof HTMLElement ? holder.innerText : holder.textContent) ?? '').replace(/\s+/g, ' ').replace(/​/g, '');
      if (!text.includes(want)) continue;
      let hidden = false;
      for (let a: Element | null = node.parentElement; a; a = a.parentElement) {
        const s = getComputedStyle(a);
        if (s.visibility === 'hidden' || s.display === 'none' || Number(s.opacity) === 0) hidden = true;
      }
      if (hidden) continue;
      range.selectNodeContents(node);
      const r = range.getBoundingClientRect();
      if (r.width === 0 && r.height === 0) continue;
      const inView =
        r.left >= -0.5 &&
        r.top >= -0.5 &&
        r.right <= window.innerWidth + 0.5 &&
        r.bottom <= window.innerHeight + 0.5 &&
        !covered(node.parentElement, (r.left + r.right) / 2, (r.top + r.bottom) / 2);
      const level = inView ? (regions.some((n) => n.contains(node)) ? 1 : 2) : 3;
      if (best === 0 || level < best) best = level;
    }
  } finally {
    (lift as HTMLStyleElement | null)?.remove();
  }
  return best;
}

/**
 * A signature of what is rendered and where the page stands, to wait until it has settled. It
 * counts the Tab stops too: a box that becomes focusable once it measures its own overflow (a wide
 * table) changes the keyboard path after the text has settled.
 */
export function signature(): string {
  const stops = document.querySelectorAll('[tabindex], a[href], button, input').length;
  return `${location.href}|${Math.round(window.scrollY)}|${document.body.innerText.length}|${stops}`;
}
