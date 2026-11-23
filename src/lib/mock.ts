// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab
// dev-only fake backend. imported behind import.meta.env.DEV, so it never ships.
// url switches: ?empty=1  ?state=offline (no raw input)  ?os=macos|linux  ?many=1

import type { AppInfo, CaptureMode, DeviceInfo, DeviceListMsg, Ev, Frame, SessionInfo, SessionSettings } from './types';
import type { Bridge } from './bridge';

const q = new URLSearchParams(location.search);

const dev = (p: Partial<DeviceInfo> & Pick<DeviceInfo, 'id' | 'name' | 'kind'>): DeviceInfo => ({
  viewer: p.kind,
  vid: null,
  pid: null,
  connected: true,
  backend: 'rawinput',
  interfaces: 1,
  analogProfile: null,
  note: null,
  ...p,
});

function devices(): DeviceInfo[] {
  if (q.get('empty')) return [];
  const list = [
    dev({ id: 'kbd-0001', name: 'sample keyboard', kind: 'keyboard', vid: 0x1234, pid: 0x0001, interfaces: 2 }),
    dev({ id: 'mouse-0002', name: 'sample mouse 8k', kind: 'mouse', vid: 0x1234, pid: 0x0002 }),
    dev({ id: 'pad-1', name: 'sample controller', kind: 'gamepad', vid: 0x1234, pid: 0x0003, backend: 'gilrs' }),
    dev({ id: 'hid-4', name: 'sample flight stick', kind: 'hid', vid: 0x1234, pid: 0x0004, backend: 'hidapi', note: 'usage 0001:0004' }),
    dev({ id: 'midi-5', name: 'sample midi keys', kind: 'midi', backend: 'midir' }),
    dev({ id: 'kbd-0006', name: 'sample analog keyboard', kind: 'keyboard', vid: 0x1234, pid: 0x0006, analogProfile: 'wooting', connected: false }),
  ];
  if (q.get('many')) {
    for (let i = 0; i < 14; i++) list.push(dev({ id: `hid-m${i}`, name: `sample hid device with a long name number ${i}`, kind: 'hid', backend: 'hidapi', vid: 0x4321, pid: i }));
  }
  return list;
}

export function mockBridge(): Bridge {
  const os = (q.get('os') as AppInfo['os']) ?? 'windows';
  const offline = q.get('state') === 'offline';
  const info: AppInfo = {
    version: '0.1.0',
    os,
    rawBackend: os === 'windows' ? 'rawinput' : os === 'macos' ? 'iokit' : 'evdev',
    caps: {
      rawInput: !offline,
      captureMouse: !offline,
      captureKeyboard: !offline,
      nativeKeyLabels: false,
      notes: offline ? ['no readable /dev/input/event* nodes. add your user to the `input` group so evdev can read keyboards and mice.'] : [],
    },
    noCapture: q.get('nocapture') === '1',
    hidError: null,
  };
  // fake session: the browser keyboard stands in for raw input, with the same exit rules as rust
  let timer = 0;
  let stop = () => {};
  let endNow: ((e: Ev & { t: 'ended' }) => void) | null = null;
  let activeMode: CaptureMode = 'other';
  const modeOf = (d: DeviceInfo): CaptureMode => (d.viewer === 'mouse' ? 'mouse' : d.viewer === 'keyboard' ? 'keyboard' : 'other');
  return {
    appInfo: async () => info,
    async startSession(deviceId, settings: SessionSettings, onFrame: (f: Frame) => void) {
      const device = devices().find((d) => d.id === deviceId)!;
      const mode = modeOf(device);
      activeMode = mode;
      const noCapture = info.noCapture;
      const down = new Set<string>();
      let since = 0;
      let last = 0;
      let seq = 0;
      let buf: Ev[] = [];
      const t0 = performance.now();
      const holdMs = mode === 'mouse' ? settings.escHoldMs : settings.comboHoldMs;
      const cond = () => (mode === 'mouse' ? down.has('Escape') : mode === 'keyboard' ? ['Control', 'Alt', 'Shift'].every((m) => [...down].some((k) => k.startsWith(m))) && down.has('Backspace') : false);
      const onKey = (e: KeyboardEvent, isDown: boolean) => {
        e.preventDefault();
        if (isDown) down.add(e.code); else down.delete(e.code);
        if (mode === 'keyboard') buf.push({ t: 'key', code: e.code, down: isDown, ts: performance.now() - t0 });
        if (cond()) since ||= performance.now(); else since = 0;
      };
      const kd = (e: KeyboardEvent) => onKey(e, true);
      const ku = (e: KeyboardEvent) => onKey(e, false);
      addEventListener('keydown', kd);
      addEventListener('keyup', ku);
      const end = (reason: Ev & { t: 'ended' }) => {
        buf.push(reason);
        flush();
        stop();
      };
      const flush = () => {
        if (buf.length) onFrame({ seq: ++seq, events: buf.splice(0) });
      };
      endNow = end;
      stop = () => {
        endNow = null;
        clearInterval(timer);
        removeEventListener('keydown', kd);
        removeEventListener('keyup', ku);
        stop = () => {};
      };
      timer = window.setInterval(() => {
        if (mode !== 'other') {
          const p = since ? Math.min(1, (performance.now() - since) / holdMs) : 0;
          if (p !== last) {
            last = p;
            buf.push({ t: 'exit', kind: mode === 'mouse' ? 'esc' : 'combo', progress: p });
          }
          if (p >= 1) return end({ t: 'ended', reason: mode === 'mouse' ? 'esc-hold' : 'combo' });
        }
        flush();
      }, 16);
      const info2: SessionInfo = {
        device,
        mode,
        exit: { kind: mode === 'mouse' ? 'esc' : mode === 'keyboard' ? 'combo' : null, holdMs: mode === 'mouse' ? settings.escHoldMs : settings.comboHoldMs, button: mode !== 'mouse' || noCapture, combo: mode === 'keyboard' ? ['ControlLeft', 'AltLeft', 'ShiftLeft', 'Backspace'] : [] },
        capture: noCapture
          ? { active: false, cursorClipped: false, keyboardHooked: false, mouseSwallowed: false, warnings: ['--no-capture is on: nothing is clipped, hooked or swallowed.'] }
          : { active: true, cursorClipped: mode === 'mouse', keyboardHooked: mode !== 'mouse', mouseSwallowed: mode === 'mouse', warnings: [] },
        reserved: [
          { label: 'ctrl + alt + del', keys: ['ControlLeft', 'AltLeft', 'Delete'], why: 'handled by windows before any app can see it' },
          { label: 'win + l', keys: ['MetaLeft', 'KeyL'], why: 'locks the pc, no hook can stop it' },
        ],
        comboWarning: q.get('combowarn') ? 'your desktop treats ctrl+alt+backspace as a session kill. only the on-screen exit button is guaranteed.' : null,
        noCapture,
      };
      return info2;
    },
    async endSession() {
      // mirrors the engine: the button never ends a mouse session
      if (activeMode === 'mouse' && !info.noCapture) return;
      endNow?.({ t: 'ended', reason: 'exit-button' });
    },
    heartbeat: async () => {},
    subscribeDevices(cb) {
      const msg: DeviceListMsg = { devices: devices() };
      const t = setTimeout(() => cb(msg), 60);
      return () => clearTimeout(t);
    },
    rescan: async () => {},
  };
}
