// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab · source viewable, not reusable: see LICENSE
// keyboard layouts as data. a layout is rows of keys in key units (1u = a standard key).
// to add one: add a size to `Size`, then describe it in `buildLayout` (see readme).

export type Size = 'full' | 'tkl' | '75' | '65' | '60';
export type Variant = 'ansi' | 'iso';

export interface KeyDef {
  /** dom-style physical key name, what rust sends */
  code: string;
  x: number;
  y: number;
  w: number;
  h: number;
  /** non-rectangular keys */
  shape?: 'isoEnter';
}

export interface Layout {
  id: string;
  size: Size;
  variant: Variant;
  name: string;
  w: number;
  h: number;
  keys: KeyDef[];
}

export const SIZES: { id: Size; name: string }[] = [
  { id: 'full', name: 'full size' },
  { id: 'tkl', name: 'tkl' },
  { id: '75', name: '75%' },
  { id: '65', name: '65%' },
  { id: '60', name: '60%' },
];

export interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

/** the rectangles a key is made of: one, or two for the l-shaped iso enter */
export function keyRects(k: KeyDef): Rect[] {
  if (k.shape === 'isoEnter') {
    return [
      { x: k.x, y: k.y, w: k.w, h: 1 },
      { x: k.x + 0.25, y: k.y + 1, w: k.w - 0.25, h: k.h - 1 },
    ];
  }
  return [{ x: k.x, y: k.y, w: k.w, h: k.h }];
}

class Row {
  x = 0;
  constructor(
    private readonly y: number,
    private readonly out: KeyDef[],
    x0 = 0,
  ) {
    this.x = x0;
  }
  k(code: string, w = 1, h = 1, shape?: KeyDef['shape']): this {
    this.out.push({ code, x: this.x, y: this.y, w, h, ...(shape ? { shape } : {}) });
    this.x += w;
    return this;
  }
  many(codes: string[], w = 1): this {
    codes.forEach((c) => this.k(c, w));
    return this;
  }
  gap(w: number): this {
    this.x += w;
    return this;
  }
}

const DIGITS = ['Digit1', 'Digit2', 'Digit3', 'Digit4', 'Digit5', 'Digit6', 'Digit7', 'Digit8', 'Digit9', 'Digit0'];
const TOP = ['KeyQ', 'KeyW', 'KeyE', 'KeyR', 'KeyT', 'KeyY', 'KeyU', 'KeyI', 'KeyO', 'KeyP'];
const HOME = ['KeyA', 'KeyS', 'KeyD', 'KeyF', 'KeyG', 'KeyH', 'KeyJ', 'KeyK', 'KeyL'];
const BOTTOM = ['KeyZ', 'KeyX', 'KeyC', 'KeyV', 'KeyB', 'KeyN', 'KeyM', 'Comma', 'Period', 'Slash'];
const F = (a: number, b: number) => Array.from({ length: b - a + 1 }, (_, i) => `F${a + i}`);

export function buildLayout(size: Size, variant: Variant): Layout {
  const keys: KeyDef[] = [];
  const iso = variant === 'iso';
  const hasF = size === 'full' || size === 'tkl' || size === '75';
  const compact = size === '75' || size === '65'; // 16u wide with a right-hand column
  const y0 = hasF ? 1.5 : 0;

  // function row
  if (hasF) {
    const r = new Row(0, keys);
    r.k('Escape');
    if (size === '75') {
      r.many(F(1, 12)).gap(2).k('Delete');
    } else {
      r.gap(1).many(F(1, 4)).gap(0.5).many(F(5, 8)).gap(0.5).many(F(9, 12));
      r.gap(0.25).many(['PrintScreen', 'ScrollLock', 'Pause']);
    }
  }

  // number row
  {
    const r = new Row(y0, keys);
    r.k(hasF ? 'Backquote' : 'Escape').many(DIGITS).many(['Minus', 'Equal']).k('Backspace', 2);
    if (size === 'full' || size === 'tkl') r.gap(0.25).many(['Insert', 'Home', 'PageUp']);
    if (size === '75') r.k('Home');
    if (size === '65') r.k('Delete');
  }
  // tab row (ansi: backslash 1.5, iso: tall enter)
  {
    const r = new Row(y0 + 1, keys);
    r.k('Tab', 1.5).many(TOP).many(['BracketLeft', 'BracketRight']);
    if (iso) r.k('Enter', 1.5, 2, 'isoEnter');
    else r.k('Backslash', 1.5);
    if (size === 'full' || size === 'tkl') r.gap(0.25).many(['Delete', 'End', 'PageDown']);
    if (compact) r.k('PageUp');
  }
  // home row
  {
    const r = new Row(y0 + 2, keys);
    r.k('CapsLock', 1.75).many(HOME).many(['Semicolon', 'Quote']);
    if (iso) r.k('Backslash');
    else r.k('Enter', 2.25);
    if (compact) r.gap(iso ? 1.25 : 0).k('PageDown');
  }
  // shift row
  {
    const r = new Row(y0 + 3, keys);
    if (iso) r.k('ShiftLeft', 1.25).k('IntlBackslash');
    else r.k('ShiftLeft', 2.25);
    r.many(BOTTOM);
    if (compact) {
      r.k('ShiftRight', 1.75).k('ArrowUp').k('End');
    } else {
      r.k('ShiftRight', size === '60' ? 2.75 : 2.75);
      if (size === 'full' || size === 'tkl') r.gap(1.25).k('ArrowUp');
    }
  }
  // bottom row
  {
    const r = new Row(y0 + 4, keys);
    if (compact) {
      r.many(['ControlLeft', 'MetaLeft', 'AltLeft'], 1.25).k('Space', 6.25).k('AltRight').k('ControlRight').gap(1).many(['ArrowLeft', 'ArrowDown', 'ArrowRight']);
    } else {
      r.many(['ControlLeft', 'MetaLeft', 'AltLeft'], 1.25).k('Space', 6.25).many(['AltRight', 'MetaRight', 'ContextMenu', 'ControlRight'], 1.25);
      if (size === 'full' || size === 'tkl') r.gap(0.25).many(['ArrowLeft', 'ArrowDown', 'ArrowRight']);
    }
  }

  // numpad
  if (size === 'full') {
    const x = 18.5;
    const row = (i: number) => new Row(y0 + i, keys, x);
    row(0).many(['NumLock', 'NumpadDivide', 'NumpadMultiply', 'NumpadSubtract']);
    row(1).many(['Numpad7', 'Numpad8', 'Numpad9']).k('NumpadAdd', 1, 2);
    row(2).many(['Numpad4', 'Numpad5', 'Numpad6']);
    row(3).many(['Numpad1', 'Numpad2', 'Numpad3']).k('NumpadEnter', 1, 2);
    row(4).k('Numpad0', 2).k('NumpadDecimal');
  }

  const w = Math.max(...keys.map((k) => k.x + k.w));
  const h = Math.max(...keys.map((k) => k.y + k.h));
  const sizeName = SIZES.find((s) => s.id === size)!.name;
  return { id: `${size}-${variant}`, size, variant, name: `${sizeName} ${variant}`, w, h, keys };
}

const cache = new Map<string, Layout>();
export function getLayout(size: Size, variant: Variant): Layout {
  const id = `${size}-${variant}`;
  let l = cache.get(id);
  if (!l) {
    l = buildLayout(size, variant);
    cache.set(id, l);
  }
  return l;
}
