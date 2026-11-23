// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab · source viewable, not reusable: see LICENSE
import { describe, expect, it } from 'vitest';
import { buildJson, buildText, type ReportInput } from './export';
import type { DeviceInfo, SessionInfo } from './types';

const dev = (o: Partial<DeviceInfo>): DeviceInfo => ({ id: 'x', name: 'sample device', kind: 'mouse', viewer: 'mouse', vid: 0x046d, pid: 0xc08b, connected: true, backend: 'rawinput', interfaces: 1, analogProfile: null, note: null, ...o });
const info = (d: DeviceInfo): SessionInfo => ({ device: d, mode: 'mouse', exit: { kind: 'esc', holdMs: 1000, button: false, combo: [] }, capture: { active: true, cursorClipped: true, keyboardHooked: false, mouseSwallowed: true, warnings: [] }, reserved: [], comboWarning: null, noCapture: false, hid: null, analog: null, midiError: null });
const base = (d: DeviceInfo, results: ReportInput['results']): ReportInput => ({ app: { version: '0.1.0', os: 'windows', rawBackend: 'rawinput', caps: { rawInput: true, captureMouse: true, captureKeyboard: true, nativeKeyLabels: true, notes: [] }, noCapture: false, hidError: null }, info: info(d), results, startedAt: 0, endedAt: 95_000, reason: 'esc-hold' });

describe('session report', () => {
  it('json carries ids as hex and no paths or serials', () => {
    const j = buildJson(base(dev({}), {})) as { device: Record<string, unknown>; session: Record<string, unknown> };
    expect(j.device.vid).toBe('046d');
    expect(j.device.pid).toBe('c08b');
    expect(j.device.model).toBe('G502');
    expect(j.session.durationSeconds).toBe(95);
    expect(JSON.stringify(j)).not.toMatch(/\\\\\?\\|serial|\/dev\//i);
  });

  it('mouse text lists polling, dpi and chatter', () => {
    const t = buildText(base(dev({}), { mouse: { model: 'G502', polling: { avgHz: 998.7, minHz: 950, maxHz: 1010, nominal: 1000, jitterUs: 18.2 }, dpi: { cpi: 1587, counts: 3174, distanceIn: 2, straightness: 0.97 }, sensor: { jitterPct: 11, snapRatio: 1.2, snapping: false }, buttons: [{ n: 1, clicks: 40, chatter: 2, holdAvg: 80 }], wheel: { up: 3, down: 2, left: 0, right: 0 } } }));
    expect(t).toMatch(/avg 999 hz/);
    expect(t).toMatch(/1,587 cpi/);
    expect(t).toMatch(/1: 40 clicks, 2 chatter/);
    expect(t).toMatch(/dpi depends on how accurately/);
  });

  it('keyboard text lists tested keys and chatter', () => {
    const k = dev({ kind: 'keyboard', viewer: 'keyboard', name: 'sample keyboard' });
    const t = buildText(base(k, { keyboard: { tested: ['KeyA', 'KeyB'], total: 104, maxHeld: 6, chatter: { KeyJ: 3 }, layout: 'full-ansi' } }));
    expect(t).toMatch(/tested keys\s+2 of 104/);
    expect(t).toMatch(/KeyJ x3/);
    expect(t).toMatch(/6 keys at once/);
  });

  it('missing results do not break the report', () => {
    expect(buildText(base(dev({ kind: 'gamepad', viewer: 'gamepad' }), {}))).toMatch(/inputlab session report/);
    expect(buildText(base(dev({ kind: 'hid', viewer: 'hid' }), { hid: { device: 'x', descriptorParsed: false } }))).toMatch(/not parsed/);
  });

  it('controller drift and snapping are called out in capitals', () => {
    const t = buildText(base(dev({ kind: 'gamepad', viewer: 'gamepad', name: 'pad' }), { gamepad: { model: 'DualSense', sticks: { left: { circularity: 0.93, maxRange: 1.0, deadzone: 0.07 }, right: { circularity: null, maxRange: 0.5, deadzone: null }, rest: [{ name: 'left stick', offset: 0.09, drift: true }] } } }));
    expect(t).toMatch(/DRIFT/);
    expect(t).toMatch(/circularity 93%/);
  });
});
