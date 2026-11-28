// The component library both consoles share: status chips, buttons, meters, cards, empty and error
// states, skeletons, menus and modal frames (drawer and dialog). Styling lives in styles/app.css and
// reads only the CSS custom properties the document's tokens define (theme.ts).

import { useEffect, useId, useLayoutEffect, useRef, useState } from 'react';
import type { ReactNode, KeyboardEvent as ReactKeyboardEvent } from 'react';
import type { Tone } from '../renderer/types.ts';
import { Icon } from './Icon.tsx';
import { shortId } from '../renderer/format.ts';

export function Badge({ tone = 'neutral', children, title }: { tone?: Tone; children: ReactNode; title?: string }) {
  return (
    <span className={`badge tone-${tone}`} title={title}>
      <span className="badge-dot" aria-hidden="true" />
      {children}
    </span>
  );
}

export type ButtonKind = 'primary' | 'secondary' | 'danger' | 'ghost';

export function Button({
  kind = 'secondary', children, onClick, icon, type = 'button', disabled, size, label, ...rest
}: {
  kind?: ButtonKind; children?: ReactNode; onClick?: () => void; icon?: string; type?: 'button' | 'submit';
  disabled?: boolean; size?: 'sm'; label?: string; 'aria-haspopup'?: 'menu'; 'aria-expanded'?: boolean; 'aria-controls'?: string;
}) {
  return (
    <button
      type={type}
      className={`btn btn-${kind}${size ? ` btn-${size}` : ''}${children ? '' : ' btn-icon'}`}
      onClick={onClick}
      disabled={disabled}
      aria-label={label}
      title={children ? undefined : label}
      {...rest}
    >
      {icon && <Icon name={icon} size={size === 'sm' ? 15 : 17} />}
      {children && <span>{children}</span>}
    </button>
  );
}

/**
 * A value drawn against a limit: role meter. `usage` (default) turns warning and danger as it fills
 * (a quota, a budget); `goal` turns success when full (ready replicas against wanted ones).
 */
export function Meter({ value, max, min = 0, label, text, mark, kind = 'usage' }: { value: number; max: number; min?: number; label: string; text?: string; mark?: number; kind?: 'usage' | 'goal' }) {
  const span = Math.max(max - min, 0);
  const ratio = span > 0 ? Math.min(Math.max((value - min) / span, 0), 1) : 0;
  const tone: Tone = kind === 'goal'
    ? (span === 0 ? 'neutral' : ratio >= 1 ? 'success' : ratio >= 0.5 ? 'warning' : 'danger')
    : ratio >= 0.95 ? 'danger' : ratio >= 0.8 ? 'warning' : 'info';
  return (
    <div className="meter" role="meter" aria-label={label} aria-valuemin={min} aria-valuemax={max} aria-valuenow={value} aria-valuetext={text}>
      <div className="meter-track" aria-hidden="true">
        <div className={`meter-fill tone-${tone}`} style={{ width: `${ratio * 100}%` }} />
        {mark !== undefined && span > 0 && <div className="meter-mark" style={{ left: `${Math.min(Math.max((mark - min) / span, 0), 1) * 100}%` }} />}
      </div>
      <span className="meter-text">{text ?? `${value} / ${max}`}</span>
    </div>
  );
}

export function Skeleton({ rows = 3, kind = 'rows' }: { rows?: number; kind?: 'rows' | 'tile' | 'chart' }) {
  return (
    <div className={`skeleton skeleton-${kind}`} aria-hidden="true">
      {Array.from({ length: kind === 'rows' ? rows : 1 }, (_, i) => (
        <div key={i} className="skeleton-line" style={{ width: kind === 'rows' ? `${92 - ((i * 17) % 35)}%` : undefined }} />
      ))}
    </div>
  );
}

export function EmptyState({ title, message, action }: { title?: string; message: string; action?: ReactNode }) {
  return (
    <div className="empty">
      <div className="empty-art" aria-hidden="true">
        <svg width="44" height="44" viewBox="0 0 44 44" fill="none">
          <rect x="6" y="10" width="32" height="24" rx="5" stroke="currentColor" strokeWidth="1.6" strokeDasharray="3 3" />
          <path d="M14 22h16M18 27h8" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" />
        </svg>
      </div>
      {title && <p className="empty-title">{title}</p>}
      <p className="empty-message">{message}</p>
      {action && <div className="empty-action">{action}</div>}
    </div>
  );
}

export function ErrorState({ message, onRetry }: { message: string; onRetry?: () => void }) {
  return (
    <div className="error-state" role="alert">
      <Icon name="alert" size={20} />
      <div>
        <p className="error-title">This section could not load</p>
        <p className="error-message">{message}</p>
      </div>
      {onRetry && <Button size="sm" onClick={onRetry} icon="sort">Retry</Button>}
    </div>
  );
}

/** A long identifier shown short, in mono; the full value in its title, with a copy button. */
export function IdText({ value, onCopy }: { value: string; onCopy?: (v: string) => void }) {
  const short = shortId(value);
  return (
    <span className="id-text" title={value}>
      <code>{short}</code>
      {onCopy && short !== value && (
        <button type="button" className="id-copy" aria-label={`Copy ${value}`} onClick={(e) => { e.stopPropagation(); onCopy(value); }}>
          <Icon name="copy" size={13} />
        </button>
      )}
    </span>
  );
}

export function Card({ title, count, actions, children, className, id, busy, headingLevel = 2 }: {
  title?: string; count?: number; actions?: ReactNode; children: ReactNode; className?: string; id?: string; busy?: boolean; headingLevel?: 2 | 3;
}) {
  const H = headingLevel === 2 ? 'h2' : 'h3';
  const hid = useId();
  return (
    <section className={`card${className ? ` ${className}` : ''}`} id={id} aria-labelledby={title ? hid : undefined} aria-busy={busy || undefined}>
      {(title || actions) && (
        <header className="card-head">
          {title && (
            <H className="card-title" id={hid}>
              {title}
              {count !== undefined && <span className="count" aria-label={`${count} rows`}>{count}</span>}
            </H>
          )}
          {actions && <div className="card-actions">{actions}</div>}
        </header>
      )}
      <div className="card-body">{children}</div>
    </section>
  );
}

/**
 * A box that scrolls sideways when its content is wider (a table): a keyboard-reachable region with
 * a label. While content is hidden on a side, the box carries `data-overflow` (`start`, `end` or
 * both) and its frame draws an edge shadow on that side; the attribute is kept on scroll and resize
 * and is absent when nothing is hidden.
 */
export function ScrollRegion({ label, className, children }: { label: string; className: string; children: ReactNode }) {
  const ref = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const update = () => {
      const max = el.scrollWidth - el.clientWidth;
      const at = Math.abs(el.scrollLeft);
      const sides = max > 1 ? [at > 1 ? 'start' : '', at < max - 1 ? 'end' : ''].filter(Boolean).join(' ') : '';
      if (sides) el.dataset.overflow = sides;
      else delete el.dataset.overflow;
    };
    update();
    el.addEventListener('scroll', update, { passive: true });
    const ro = typeof ResizeObserver !== 'undefined' ? new ResizeObserver(update) : undefined;
    ro?.observe(el);
    if (el.firstElementChild) ro?.observe(el.firstElementChild);
    return () => { el.removeEventListener('scroll', update); ro?.disconnect(); };
  }, []);
  return (
    <div className="scroll-frame">
      <div ref={ref} className={className} role="region" aria-label={label} tabIndex={0}>{children}</div>
    </div>
  );
}

export interface MenuItem {
  key: string;
  label: string;
  onSelect: () => void;
  icon?: string;
  checked?: boolean;
  danger?: boolean;
}

/** A button that opens a menu: arrow keys move, Escape closes and returns focus. */
export function Menu({ label, items, icon = 'more', text, align = 'end' }: { label: string; items: MenuItem[]; icon?: string; text?: ReactNode; align?: 'start' | 'end' }) {
  const [open, setOpen] = useState(false);
  const id = useId();
  const btn = useRef<HTMLButtonElement>(null);
  const list = useRef<HTMLDivElement>(null);
  const [up, setUp] = useState(false);
  useLayoutEffect(() => {
    if (!open || !btn.current) return;
    const r = btn.current.getBoundingClientRect();
    setUp(window.innerHeight - r.bottom < 40 + items.length * 36 && r.top > window.innerHeight - r.bottom);
    list.current?.querySelector<HTMLElement>('[role=menuitem], [role=menuitemradio]')?.focus();
  }, [open, items.length]);
  useEffect(() => {
    if (!open) return;
    const close = (e: MouseEvent) => {
      if (!list.current?.contains(e.target as Node) && !btn.current?.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener('mousedown', close);
    return () => document.removeEventListener('mousedown', close);
  }, [open]);
  const onKey = (e: ReactKeyboardEvent) => {
    const els = Array.from(list.current?.querySelectorAll<HTMLElement>('[role^=menuitem]') ?? []);
    const at = els.indexOf(document.activeElement as HTMLElement);
    if (e.key === 'Escape') { e.preventDefault(); setOpen(false); btn.current?.focus(); }
    else if (e.key === 'ArrowDown') { e.preventDefault(); els[(at + 1) % els.length]?.focus(); }
    else if (e.key === 'ArrowUp') { e.preventDefault(); els[(at - 1 + els.length) % els.length]?.focus(); }
    else if (e.key === 'Home') { e.preventDefault(); els[0]?.focus(); }
    else if (e.key === 'End') { e.preventDefault(); els[els.length - 1]?.focus(); }
    else if (e.key === 'Tab') setOpen(false);
  };
  const hasChecks = items.some((i) => i.checked !== undefined);
  return (
    <div className="menu">
      <button
        ref={btn}
        type="button"
        className={text ? 'btn btn-ghost menu-trigger' : 'btn btn-ghost btn-icon menu-trigger'}
        aria-haspopup="menu"
        aria-expanded={open}
        aria-controls={open ? id : undefined}
        aria-label={text ? undefined : label}
        title={text ? undefined : label}
        onClick={() => setOpen((o) => !o)}
      >
        {text ?? <Icon name={icon} />}
      </button>
      {open && (
        <div ref={list} id={id} role="menu" aria-label={label} className={`menu-list menu-${align}${up ? ' menu-up' : ''}`} onKeyDown={onKey}>
          {items.map((i) => (
            <button
              key={i.key}
              type="button"
              role={hasChecks ? 'menuitemradio' : 'menuitem'}
              aria-checked={hasChecks ? !!i.checked : undefined}
              tabIndex={-1}
              className={`menu-item${i.danger ? ' menu-danger' : ''}`}
              onClick={() => { setOpen(false); btn.current?.focus(); i.onSelect(); }}
            >
              {hasChecks ? <Icon name={i.checked ? 'check' : 'dot'} size={15} /> : i.icon ? <Icon name={i.icon} size={15} /> : null}
              <span>{i.label}</span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

/** A modal frame: a drawer from the side or a dialog in the middle. Traps focus, closes on Escape. */
export function Modal({ kind, title, onClose, children, footer }: { kind: 'drawer' | 'dialog' | 'fullscreen' | 'popover'; title: string; onClose: () => void; children: ReactNode; footer?: ReactNode }) {
  const ref = useRef<HTMLDivElement>(null);
  const hid = useId();
  useEffect(() => {
    const before = document.activeElement as HTMLElement | null;
    const el = ref.current;
    const first = el?.querySelector<HTMLElement>('input, select, textarea, button:not(.modal-close), [tabindex="0"]');
    (first ?? el)?.focus();
    return () => before?.focus?.();
  }, []);
  const onKey = (e: ReactKeyboardEvent) => {
    if (e.key === 'Escape') { e.stopPropagation(); onClose(); return; }
    if (e.key !== 'Tab' || !ref.current) return;
    const f = Array.from(ref.current.querySelectorAll<HTMLElement>('a[href], button:not([disabled]), input, select, textarea, [tabindex="0"]'));
    if (!f.length) return;
    const first = f[0];
    const last = f[f.length - 1];
    if (e.shiftKey && document.activeElement === first) { e.preventDefault(); last.focus(); }
    else if (!e.shiftKey && document.activeElement === last) { e.preventDefault(); first.focus(); }
  };
  const k = kind === 'drawer' ? 'drawer' : kind === 'fullscreen' ? 'fullscreen' : 'dialog';
  return (
    <div className="modal-root" onKeyDown={onKey}>
      <div className="modal-backdrop" onClick={onClose} aria-hidden="true" />
      <div ref={ref} className={`modal modal-${k}`} role="dialog" aria-modal="true" aria-labelledby={hid} tabIndex={-1}>
        <header className="modal-head">
          <h2 id={hid}>{title}</h2>
          <button type="button" className="btn btn-ghost btn-icon modal-close" aria-label="Close" onClick={onClose}>
            <Icon name="close" />
          </button>
        </header>
        <div className="modal-body">{children}</div>
        {footer && <footer className="modal-foot">{footer}</footer>}
      </div>
    </div>
  );
}

export interface Toast {
  id: number;
  tone: Tone;
  title: string;
  body?: string;
}

export function Toasts({ toasts, onDismiss }: { toasts: Toast[]; onDismiss: (id: number) => void }) {
  return (
    <div className="toasts" role="status" aria-live="polite">
      {toasts.map((t) => (
        <div key={t.id} className={`toast tone-${t.tone}`}>
          <Icon name={t.tone === 'danger' ? 'alert' : t.tone === 'success' ? 'check' : 'info'} size={18} />
          <div className="toast-text">
            <p className="toast-title">{t.title}</p>
            {t.body && <p className="toast-body">{t.body}</p>}
          </div>
          <button type="button" className="btn btn-ghost btn-icon" aria-label="Dismiss" onClick={() => onDismiss(t.id)}>
            <Icon name="close" size={15} />
          </button>
        </div>
      ))}
    </div>
  );
}
