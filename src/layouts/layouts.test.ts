// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab · source viewable, not reusable: see LICENSE
import { describe, expect, it } from 'vitest';
import { buildLayout, keyRects, SIZES, type KeyDef, type Variant } from './build';
import { detectFamily, detectSize } from './detect';
import { labelFor, staticLabel } from './labels';

const variants: Variant[] = ['ansi', 'iso'];

function overlaps(ka: KeyDef, kb: KeyDef) {
  const e = 0.001;
  return keyRects(ka).some((a) => keyRects(kb).some((b) => a.x < b.x + b.w - e && b.x < a.x + a.w - e && a.y < b.y + b.h - e && b.y < a.y + a.h - e));
}

describe('layouts', () => {
  for (const s of SIZES) {
    for (const v of variants) {
      it(`${s.id}/${v}: no overlapping keys, no duplicates`, () => {
        const l = buildLayout(s.id, v);
        const seen = new Set<string>();
        for (const k of l.keys) {
          expect(seen.has(k.code), `duplicate ${k.code}`).toBe(false);
          seen.add(k.code);
        }
        for (let i = 0; i < l.keys.length; i++)
          for (let j = i + 1; j < l.keys.length; j++)
            expect(overlaps(l.keys[i]!, l.keys[j]!), `${l.keys[i]!.code} overlaps ${l.keys[j]!.code}`).toBe(false);
      });
    }
  }

  it('ansi full has the expected 104 minus nothing: 104 physical keys', () => {
    expect(buildLayout('full', 'ansi').keys.length).toBe(104);
  });
  it('iso full has 105 keys (extra key by the left shift)', () => {
    const l = buildLayout('full', 'iso');
    expect(l.keys.length).toBe(105);
    expect(l.keys.some((k) => k.code === 'IntlBackslash')).toBe(true);
    expect(l.keys.find((k) => k.code === 'Enter')?.shape).toBe('isoEnter');
  });
  it('full is 22.5u wide, tkl 18.25u, 60% 15u', () => {
    expect(buildLayout('full', 'ansi').w).toBe(22.5);
    expect(buildLayout('tkl', 'ansi').w).toBe(18.25);
    expect(buildLayout('60', 'ansi').w).toBe(15);
  });
  it('main block rows are exactly 15u wide', () => {
    const l = buildLayout('60', 'ansi');
    for (const y of [0, 1, 2, 3, 4]) {
      const row = l.keys.filter((k) => k.y === y);
      expect(Math.max(...row.map((k) => k.x + k.w)), `row ${y}`).toBe(15);
    }
  });
  it('every layout can reach the exit combo keys', () => {
    for (const s of SIZES) {
      const codes = new Set(buildLayout(s.id, 'ansi').keys.map((k) => k.code));
      for (const k of ['ControlLeft', 'AltLeft', 'ShiftLeft', 'Backspace']) expect(codes.has(k), `${s.id} ${k}`).toBe(true);
    }
  });
  it('60% uses escape where the other boards have backquote', () => {
    const codes = buildLayout('60', 'ansi').keys.map((k) => k.code);
    expect(codes).toContain('Escape');
    expect(codes).not.toContain('Backquote');
  });
});

describe('detection', () => {
  it('numpad means full size', () => expect(detectSize(['KeyA', 'Numpad5']).size).toBe('full'));
  it('f-row with a nav cluster is tkl, without is 75', () => {
    expect(detectSize(['F5', 'Pause']).size).toBe('tkl');
    expect(detectSize(['F5', 'Delete']).size).toBe('75');
  });
  it('arrows without an f-row is 65, plain letters is 60', () => {
    expect(detectSize(['ArrowUp']).size).toBe('65');
    expect(detectSize(['KeyA', 'Space']).size).toBe('60');
  });
  it('the iso key switches the variant', () => expect(detectSize(['IntlBackslash']).variant).toBe('iso'));
  it('detects os layout families from a key map', () => {
    const m = (o: Record<string, string>) => new Map(Object.entries(o));
    expect(detectFamily(m({ KeyQ: 'a', KeyA: 'q' }))).toBe('azerty');
    expect(detectFamily(m({ KeyY: 'z', KeyZ: 'y' }))).toBe('qwertz');
    expect(detectFamily(m({ KeyQ: "'", KeyW: ',' }))).toBe('dvorak');
    expect(detectFamily(m({ KeyE: 'f', KeyS: 'r', KeyD: 's' }))).toBe('colemak');
    expect(detectFamily(m({ KeyQ: 'q', KeyA: 'a', KeyY: 'y' }))).toBe('qwerty');
  });
});

describe('labels', () => {
  it('physical key names, not layout chars', () => {
    expect(staticLabel('KeyQ', 'qwerty')).toBe('Q');
    expect(staticLabel('KeyQ', 'azerty')).toBe('A');
    expect(staticLabel('KeyY', 'qwertz')).toBe('Z');
    expect(staticLabel('KeyS', 'colemak')).toBe('R');
    expect(staticLabel('KeyW', 'dvorak')).toBe(',');
  });
  it('modifiers are the same in every family', () => expect(staticLabel('ShiftLeft', 'azerty')).toBe('Shift'));
  it('the os map wins for printable keys but never renames modifiers', () => {
    const native = new Map([['KeyQ', 'a'], ['Enter', 'x']]);
    expect(labelFor('KeyQ', 'qwerty', native)).toBe('A');
    expect(labelFor('Enter', 'qwerty', native)).toBe('Enter');
  });
});

describe('macbook', () => {
  it('is 14.5u wide with fn, a half-height function row and inverted-t arrows', () => {
    const l = buildLayout('macbook', 'ansi');
    expect(l.w).toBe(14.5);
    expect(l.keys.length).toBe(77);
    const k = (c: string) => l.keys.find((x) => x.code === c)!;
    expect(k('Fn').x).toBe(0);
    expect(k('F1').h).toBe(0.5);
    expect(k('ArrowUp').y + k('ArrowUp').h).toBe(k('ArrowDown').y);
    expect(l.keys.some((x) => x.code === 'ContextMenu' || x.code === 'ControlRight')).toBe(false);
  });
  it('iso adds the key by the left shift and a tall return', () => {
    const l = buildLayout('macbook', 'iso');
    expect(l.w).toBe(14.5);
    expect(l.keys.length).toBe(78);
    expect(l.keys.find((x) => x.code === 'Enter')?.shape).toBe('isoEnter');
  });
  it('labels modifiers the mac way only on a mac', () => {
    expect(labelFor('MetaLeft', 'qwerty', null, true)).toBe('⌘ command');
    expect(labelFor('AltLeft', 'qwerty', null, true)).toBe('⌥ option');
    expect(labelFor('ControlLeft', 'qwerty', null, true)).toBe('⌃ control');
    expect(labelFor('Backspace', 'qwerty', null, true)).toBe('delete');
    expect(labelFor('Fn', 'qwerty', null, true)).toBe('fn');
    expect(labelFor('MetaLeft', 'qwerty', null)).toBe('Win');
    expect(staticLabel('KeyA', 'qwerty', true)).toBe('A');
  });
});
