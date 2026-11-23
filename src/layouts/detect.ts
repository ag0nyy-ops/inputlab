// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab · source viewable, not reusable: see LICENSE
import type { LabelFamily } from './labels';
import { getLayout, type Size, type Variant } from './build';

const ORDER: Size[] = ['60', '65', '75', 'tkl', 'full'];

/**
 * the smallest form factor that has every key pressed so far. it can only grow as
 * more keys are pressed (a key that exists proves the board has it), and it can never
 * prove a key is missing, so the ui only ever offers it as a suggestion.
 */
export function detectSize(tested: Iterable<string>): { size: Size; variant: Variant } {
  const set = [...new Set(tested)];
  const variant: Variant = set.includes('IntlBackslash') ? 'iso' : 'ansi';
  for (const size of ORDER) {
    const codes = new Set(getLayout(size, variant).keys.map((k) => k.code));
    if (set.every((c) => codes.has(c))) return { size, variant };
  }
  return { size: 'full', variant };
}

export function maxSize(a: Size, b: Size): Size {
  return ORDER.indexOf(a) >= ORDER.indexOf(b) ? a : b;
}

/** which label family an os key map belongs to, from where a few telling keys landed */
export function detectFamily(map: Map<string, string>): LabelFamily {
  const g = (c: string) => (map.get(c) ?? '').toLowerCase();
  if (g('KeyQ') === 'a' && g('KeyA') === 'q') return 'azerty';
  if (g('KeyY') === 'z' && g('KeyZ') === 'y') return 'qwertz';
  if (g('KeyQ') === "'" && g('KeyW') === ',') return 'dvorak';
  if (g('KeyE') === 'f' && g('KeyS') === 'r' && g('KeyD') === 's') return 'colemak';
  return 'qwerty';
}
