// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab · source viewable, not reusable: see LICENSE
import { describe, expect, it } from 'vitest';
import { PADS, matchPad } from './pads';
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
  it('dualsense has a touchpad, a dualshock 3 does not', () => {
    const has = (id: string) => padElements('playstation', PADS.find((p) => p.id === id)!).some((e) => e.k === 'decor' && e.label === 'touchpad');
    expect(has('dualsense')).toBe(true);
    expect(has('dualshock3')).toBe(false);
  });
  it('every body outline exists', () => {
    for (const p of PADS) expect(BODIES[p.body]).toBeTruthy();
  });
});
