// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab · source viewable, not reusable: see LICENSE
import { describe, expect, it } from 'vitest';
import { PADS, matchPad } from './pads';
import { DUALSENSE_GLYPH } from './dualsenseGlyph';
import { BODIES, padElements } from './padLayouts';

const d = (name: string, vid: number | null = null, pid: number | null = null) => ({ name, vid, pid });

describe('controller models', () => {
  it('ids and ids are unique', () => {
    const ids = new Set(PADS.map((p) => p.id));
    expect(ids.size).toBe(PADS.length);
  });
  it('a dualsense is a dualsense, an edge is an edge', () => {
    expect(matchPad(d('DualSense Wireless Controller', 0x054c, 0x0ce6)).model?.id).toBe('dualsense');
    expect(matchPad(d('DualSense Edge Wireless Controller', 0x054c, 0x0df2)).model?.id).toBe('dualsense-edge');
  });
  it('usb ids beat names', () => {
    expect(matchPad(d('Wireless Controller', 0x054c, 0x09cc)).model?.id).toBe('dualshock4');
  });
  it('xinput pads have no vid/pid and are matched by name', () => {
    const m = matchPad(d('Xbox 360 Controller (XInput)'));
    expect(m.style).toBe('xbox');
    expect(m.confidence).toBe('name');
  });
  it('switch pro is switch style', () => expect(matchPad(d('Pro Controller', 0x057e, 0x2009)).style).toBe('switch'));
  it('unknown pads are generic', () => expect(matchPad(d('Arcade Stick 3000', 1, 2)).style).toBe('generic'));
  it('a manual pick wins', () => expect(matchPad(d('Arcade Stick 3000'), 'switch-pro').style).toBe('switch'));
});

describe('controller layouts', () => {
  it('every style has the controls a tester needs', () => {
    for (const style of ['playstation', 'xbox', 'switch'] as const) {
      const els = padElements(style, null);
      const names = els.flatMap((e) => ('names' in e ? e.names : []));
      for (const n of ['North', 'South', 'East', 'West', 'LeftTrigger', 'RightTrigger', 'LeftTrigger2', 'RightTrigger2', 'Select', 'Start']) expect(names, `${style} ${n}`).toContain(n);
      expect(els.filter((e) => e.k === 'stick').length).toBe(2);
      expect(els.some((e) => e.k === 'dpad')).toBe(true);
    }
  });
  it('playstation draws shapes, xbox and switch draw letters', () => {
    const glyphs = (s: 'playstation' | 'xbox' | 'switch') => padElements(s, null).filter((e) => e.k === 'face').map((e) => (e.k === 'face' ? e.glyph : ''));
    expect(glyphs('playstation')).toEqual(expect.arrayContaining(['cross', 'circle', 'square', 'triangle']));
    expect(glyphs('xbox').sort()).toEqual(['A', 'B', 'X', 'Y']);
    expect(glyphs('switch').sort()).toEqual(['A', 'B', 'X', 'Y']);
  });
  it('switch and xbox put the same letters on different buttons', () => {
    const at = (s: 'xbox' | 'switch', letter: string) => padElements(s, null).find((e) => e.k === 'face' && e.glyph === letter) as { names: string[] };
    expect(at('xbox', 'A').names).toEqual(['South']);
    expect(at('switch', 'A').names).toEqual(['East']);
  });
  it('the dualsense uses the real outline, with a touchpad and a mute button the others lack', () => {
    expect(DUALSENSE_GLYPH).toHaveLength(23);
    expect(DUALSENSE_GLYPH.every((d) => d.startsWith('M'))).toBe(true);
    expect(PADS.find((p) => p.id === 'dualsense')).toMatchObject({ touchpad: true, mute: true, body: 'ps5' });
    expect(PADS.find((p) => p.id === 'dualshock3')?.touchpad).toBeFalsy();
  });
  it('the dualsense and the dualshock 4 are drawn differently', () => {
    const a = matchPad({ name: 'x', vid: 0x054c, pid: 0x0ce6 });
    const b = matchPad({ name: 'x', vid: 0x054c, pid: 0x09cc });
    expect(a.body).toBe('ps5');
    expect(b.body).toBe('ps');
  });
  it('every body outline exists', () => {
    // the dualsense is drawn from its own outline, everything else from a body path
    for (const p of PADS.filter((x) => x.body !== 'ps5')) expect(BODIES[p.body]).toBeTruthy();
  });
});
