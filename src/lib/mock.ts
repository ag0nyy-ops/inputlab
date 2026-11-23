// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab
// dev-only fake backend. imported behind import.meta.env.DEV, so it never ships.
// url switches: ?empty=1  ?state=offline (no raw input)  ?os=macos|linux  ?many=1

import type { AppInfo, CaptureMode, DeviceInfo, DeviceListMsg, Ev, Frame, SessionCommand, SessionInfo, SessionSettings } from './types';
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
  let cmdHandler: ((c: SessionCommand) => void) | null = null;
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
      let maxHeld = 0;
      const downAt: Record<string, number> = {};
      const lastUpAt: Record<string, number> = {};
      let last = 0;
      let seq = 0;
      let buf: Ev[] = [];
      const t0 = performance.now();
      const holdMs = mode === 'mouse' ? settings.escHoldMs : settings.comboHoldMs;
      const cond = () => (mode === 'mouse' ? down.has('Escape') : mode === 'keyboard' ? ['Control', 'Alt', 'Shift'].every((m) => [...down].some((k) => k.startsWith(m))) && down.has('Backspace') : false);
      const onKey = (e: KeyboardEvent, isDown: boolean) => {
        e.preventDefault();
        const wasDown = down.has(e.code);
        if (isDown) down.add(e.code); else down.delete(e.code);
        if (mode === 'keyboard' && !(isDown && wasDown)) {
          const now = performance.now();
          let hold: number | null = null, gap: number | null = null;
          if (isDown) { gap = lastUpAt[e.code] != null ? now - lastUpAt[e.code]! : null; downAt[e.code] = now; }
          else { hold = downAt[e.code] != null ? now - downAt[e.code]! : null; lastUpAt[e.code] = now; }
          buf.push({ t: 'key', code: e.code, down: isDown, ts: now - t0, hold, gap, chatter: gap !== null && gap < settings.chatterMs, held: down.size, max: Math.max(maxHeld, down.size) });
          maxHeld = Math.max(maxHeld, down.size);
        }
        if (cond()) since ||= performance.now(); else since = 0;
      };
      // fake mouse: the browser mouse stands in for raw input
      let mdx = 0, mdy = 0, mn = 0, reports = 0, lastMove = 0;
      let dpiState: 'idle' | 'counting' | 'done' = 'idle', dpiDist = 2, sx = 0, sy = 0, path = 0;
      let sensorOn = false;
      const sPath: [number, number][] = [];
      let px = 0, py = 0;
      const angles = new Array(36).fill(0);
      const graph: number[] = [];
      const clicks = [1, 2, 3, 4, 5].map((n) => ({ n, clicks: 0, holdMin: null as number | null, holdAvg: null as number | null, holdMax: null as number | null, chatter: 0, minGap: null as number | null }));
      const bDown: Record<number, number> = {};
      const mm = (e: MouseEvent) => {
        if (mode !== 'mouse') return;
        mdx += e.movementX; mdy += e.movementY; mn++; reports++; lastMove = performance.now();
        if (dpiState === 'counting') { sx += e.movementX; sy += e.movementY; path += Math.hypot(e.movementX, e.movementY); }
        if (sensorOn) { px += e.movementX; py += e.movementY; sPath.push([px, py]); const a = (Math.atan2(e.movementY, e.movementX) * 180 / Math.PI + 360) % 360; angles[Math.min(35, Math.floor(a / 10))]++; }
      };
      const mb = (e: MouseEvent, isDown: boolean) => {
        if (mode !== 'mouse') return;
        const n = [1, 3, 2, 4, 5][e.button] ?? 1; const now = performance.now(); const c = clicks[n - 1]!;
        if (isDown) c.clicks++;
        else if (bDown[n] != null) { const h = now - bDown[n]!; c.holdMin = Math.min(c.holdMin ?? h, h); c.holdMax = Math.max(c.holdMax ?? h, h); c.holdAvg = ((c.holdAvg ?? h) * (c.clicks - 1) + h) / c.clicks; }
        if (isDown) bDown[n] = now;
        buf.push({ t: 'button', n, down: isDown, ts: now - t0, hold: null, gap: null, chatter: false });
        if (!isDown) buf.push({ t: 'clicks', buttons: clicks.map((x) => ({ ...x })) });
      };
      const md = (e: MouseEvent) => mb(e, true), mu = (e: MouseEvent) => mb(e, false);
      const wh = (e: WheelEvent) => { if (mode === 'mouse') buf.push({ t: 'wheel', dx: -Math.sign(e.deltaX) * 120, dy: -Math.sign(e.deltaY) * 120 }); };
      addEventListener('mousemove', mm); addEventListener('mousedown', md); addEventListener('mouseup', mu); addEventListener('wheel', wh);
      cmdHandler = (c) => {
        if (c.type === 'dpiStart') { dpiState = 'counting'; dpiDist = c.distanceIn; sx = sy = path = 0; }
        if (c.type === 'dpiStop') dpiState = 'done';
        if (c.type === 'dpiReset') { dpiState = 'idle'; sx = sy = path = 0; }
        if (c.type === 'sensorStart') { sensorOn = true; sPath.length = 0; angles.fill(0); px = py = 0; }
        if (c.type === 'sensorStop') sensorOn = false;
        if (c.type === 'sensorReset') { sensorOn = false; sPath.length = 0; angles.fill(0); }
        if (c.type === 'mouseReset') { reports = 0; graph.length = 0; clicks.forEach((x) => Object.assign(x, { clicks: 0, holdMin: null, holdAvg: null, holdMax: null, chatter: 0, minGap: null })); }
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
        removeEventListener('mousemove', mm); removeEventListener('mousedown', md); removeEventListener('mouseup', mu); removeEventListener('wheel', wh);
        cmdHandler = null;
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
        if (mode === 'mouse') {
          if (mn) { buf.push({ t: 'mouse', dx: mdx, dy: mdy, n: mn }); mdx = mdy = mn = 0; }
          const moving = performance.now() - lastMove < 150;
          if (moving) graph.push(990 + Math.random() * 20), graph.length > 60 && graph.shift();
          const counts = Math.hypot(sx, sy);
          buf.push({ t: 'poll', hz: moving ? 1000 : null, avgHz: reports ? 1000 : null, minHz: reports ? 987 : null, maxHz: reports ? 1012 : null, intervalMinUs: reports ? 950 : null, intervalAvgUs: reports ? 1000 : null, intervalMaxUs: reports ? 1090 : null, jitterUs: reports ? 21 : null, nominal: reports ? 1000 : null, reports, graph: [...graph] });
          buf.push({ t: 'dpi', state: dpiState, distanceIn: dpiDist, counts, path, straightness: path ? counts / path : 1, dpi: dpiState !== 'idle' && counts > 0 ? counts / dpiDist : null });
          if (sensorOn || sPath.length) buf.push({ t: 'sensor', active: sensorOn, steps: sPath.length, radius: sPath.length > 40 ? 300 : null, jitterPct: sPath.length > 40 ? 12 : null, snapRatio: sPath.length > 40 ? 1.1 : null, snapping: false, angles: [...angles], path: sPath.slice(-400) });
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
    command: async (c) => cmdHandler?.(c),
    keyLabels: async () => null,
    subscribeDevices(cb) {
      const msg: DeviceListMsg = { devices: devices() };
      const t = setTimeout(() => cb(msg), 60);
      return () => clearTimeout(t);
    },
    rescan: async () => {},
  };
}
