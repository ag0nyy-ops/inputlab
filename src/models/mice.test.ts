// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab
import { describe, expect, it } from 'vitest';
import { MICE, matchMouse } from './mice';
import { SHAPES } from './mouseShapes';

const d = (name: string, vid: number | null = null, pid: number | null = null) => ({ name, vid, pid });

describe('mouse models', () => {
  it('every model points at a real shape and has a unique id', () => {
    const ids = new Set<string>();
    for (const m of MICE) {
      expect(SHAPES[m.shape], m.id).toBeDefined();
      expect(ids.has(m.id)).toBe(false);
      ids.add(m.id);
    }
  });

  it('a g305 and a g pro superlight get different silhouettes', () => {
    const a = matchMouse(d('Logitech G305 Gaming Mouse'));
    const b = matchMouse(d('Logitech G PRO X SUPERLIGHT Wireless Gaming Mouse'));
    expect(a.model?.id).toBe('logi-g305');
    expect(b.model?.id).toBe('logi-g-pro-x-superlight');
    expect(a.shape).not.toBe(b.shape);
  });

  it('g pro wireless and superlight are told apart', () => {
    expect(matchMouse(d('G PRO Wireless Gaming Mouse')).model?.id).toBe('logi-g-pro-wireless');
    expect(matchMouse(d('G PRO X Wireless')).model?.id).toBe('logi-g-pro-x-superlight');
  });

  it('matches by usb id when the name says nothing', () => {
    const m = matchMouse(d('USB Optical Mouse', 0x046d, 0xc08b));
    expect(m.model?.id).toBe('logi-g502');
    expect(m.confidence).toBe('usb id');
  });

  it('names win over usb ids', () => {
    expect(matchMouse(d('Razer Viper Mini', 0x046d, 0xc08b)).model?.id).toBe('razer-viper');
  });

  it('a shared wireless receiver is ambiguous, not guessed', () => {
    const m = matchMouse(d('USB Receiver', 0x046d, 0xc539));
    expect(m.confidence).toBe('ambiguous');
    expect(m.model).toBeNull();
    expect(m.receiver).toBe(true);
  });

  it('a user pick beats everything', () => {
    const m = matchMouse(d('USB Receiver', 0x046d, 0xc539), 'logi-g305');
    expect(m.confidence).toBe('picked');
    expect(m.shape).toBe('sym-pear');
  });

  it('an unknown mouse falls back to the generic shape', () => {
    const m = matchMouse(d('Some Office Mouse', 0x1111, 0x2222));
    expect(m.shape).toBe('generic');
    expect(m.confidence).toBe('none');
  });

  it('a stale pick for a model that no longer exists is ignored', () => {
    expect(matchMouse(d('Some Office Mouse'), 'gone').confidence).toBe('none');
  });
});
