// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab · source viewable, not reusable: see LICENSE
// dev-only fake backend. imported behind import.meta.env.DEV, so it never ships.
// url switches: ?empty=1  ?state=offline (no raw input)  ?os=macos|linux  ?many=1

import type { AppInfo, CaptureMode, DeviceInfo, DeviceListMsg, Ev, Frame, SessionCommand, SessionInfo, SessionSettings } from './types';
import type { Bridge } from './bridge';

const q = new URLSearchParams(location.search);

const dev = (p: Partial<DeviceInfo> & Pick<DeviceInfo, 'id' | 'name' | 'kind'>): DeviceInfo => ({
  usable: true,
  hiddenReason: null,
  hiddenInterfaces: 0,
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
    dev({ id: 'kbd-0006', name: 'sample analog keyboard', kind: 'keyboard', vid: 0x1234, pid: 0x0006, analogProfile: 'wooting', connected: !!q.get('analog') }),
  ];
  if (q.get('laptop')) {
    list.push(dev({ id: 'mouse-sys', name: 'laptop touchpad', kind: 'mouse', backend: 'rawinput' }));
    list.push(dev({ id: 'mouse-046dc539', name: 'Logitech wireless mouse (usb receiver)', kind: 'mouse', vid: 0x046d, pid: 0xc539, backend: 'rawinput' }));
  }
  if (q.get('hidden')) {
    list.push(dev({ id: 'hid-v1', name: 'virtual input device', kind: 'keyboard', usable: false, hiddenReason: 'a virtual input device windows creates (remote desktop, hyper-v, converted hid)', backend: 'rawinput' }));
    list.push(dev({ id: 'hid-v2', name: 'hid device 1532:00aa', kind: 'hid', backend: 'hidapi', usable: false, hiddenReason: 'only vendor-defined or system-control interfaces: a configuration channel, not an input' }));
    list[1] = { ...list[1]!, hiddenInterfaces: 4, note: 'also exposes a keyboard interface (macro keys or a pointer); it is folded in here' };
  }
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
  let sessionNoCapture = false;
  let cmdHandler: ((c: SessionCommand) => void) | null = null;
  const modeOf = (d: DeviceInfo): CaptureMode => (d.viewer === 'mouse' ? 'mouse' : d.viewer === 'keyboard' ? 'keyboard' : 'other');
  return {
    appInfo: async () => info,
    async startSession(deviceId, settings: SessionSettings, onFrame: (f: Frame) => void) {
      const device = devices().find((d) => d.id === deviceId)!;
      const mode = modeOf(device);
      activeMode = mode;
      const noCapture = info.noCapture || settings.disableCapture;
      sessionNoCapture = noCapture;
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
      const sticksMsg = (): Ev => {
        const cov = circBins.filter((b) => b > 0).length / 72;
        const mk = (left: boolean) => ({ x: pv[left ? 'LeftStickX' : 'RightStickX'] ?? 0, y: pv[left ? 'LeftStickY' : 'RightStickY'] ?? 0, active: circ, circularity: left && cov > 0.9 ? Math.min(...circBins.filter((b) => b > 0)) / Math.max(...circBins) : null, coverage: left ? cov : 0, maxRange: left ? Math.max(0, ...circBins) : 0, deadzone: left && circPath.length ? 0.08 : null, bins: left ? [...circBins] : new Array(72).fill(0), path: left ? circPath.slice(-300) : [] });
        const left = mk(true);
        return { t: 'sticks', left, right: mk(false), rest: { running: restUntil > performance.now(), secsLeft: Math.max(0, (restUntil - performance.now()) / 1000), entries: restAcc.length ? [{ name: 'left stick', meanX: 0.09, meanY: 0.01, offset: 0.09, noise: 0.012, drift: true }, { name: 'right stick', meanX: 0.002, meanY: 0, offset: 0.004, noise: 0.008, drift: false }] : [] } };
      };
      if (device.viewer === 'gamepad') {
        const bus = new URLSearchParams(location.search).get('bus') ?? 'usb';
        buf.push({ t: 'padInfo', forceFeedback: true, power: 'discharging', battery: 72, pollSource: 'hid reports', bus, hidPad: 'dualsense' });
        // a pad lying a little off level, turning slowly
        let ph = 0;
        setInterval(() => { ph += 0.1; buf.push({ t: 'motion', gyro: [Math.sin(ph) * 4, Math.cos(ph * 0.7) * 3, Math.sin(ph * 0.4) * 2], accel: [0.05 + Math.sin(ph) * 0.2, Math.cos(ph) * 0.15, 0.98] }); }, 33);
        setInterval(() => { if (circ || restUntil) buf.push(sticksMsg()); }, 100);
      }
      cmdHandler = (c) => {
        if (c.type === 'padCircleStart') { circ = true; circBins.fill(0); circPath.length = 0; buf.push(sticksMsg()); }
        if (c.type === 'padCircleStop') { circ = false; buf.push(sticksMsg()); }
        if (c.type === 'padCircleReset') { circ = false; circBins.fill(0); circPath.length = 0; buf.push(sticksMsg()); }
        if (c.type === 'padRestStart') { restUntil = performance.now() + c.secs * 1000; restAcc.push(1); buf.push(sticksMsg()); }
        if (c.type === 'dpiStart') { dpiState = 'counting'; dpiDist = c.distanceIn; sx = sy = path = 0; }
        if (c.type === 'dpiStop') dpiState = 'done';
        if (c.type === 'dpiReset') { dpiState = 'idle'; sx = sy = path = 0; }
        if (c.type === 'sensorStart') { sensorOn = true; sPath.length = 0; angles.fill(0); px = py = 0; }
        if (c.type === 'sensorStop') sensorOn = false;
        if (c.type === 'sensorReset') { sensorOn = false; sPath.length = 0; angles.fill(0); }
        if (c.type === 'mouseReset') { reports = 0; graph.length = 0; clicks.forEach((x) => Object.assign(x, { clicks: 0, holdMin: null, holdAvg: null, holdMax: null, chatter: 0, minGap: null })); }
      };
      // fake pad: keys stand in for the controller (wasd left stick, arrows right, ijkl face, q/e bumpers, z/c triggers)
      const pv: Record<string, number> = {};
      let circ = false, restUntil = 0;
      const circBins = new Array(72).fill(0);
      const circPath: [number, number][] = [];
      const restAcc: number[] = [];
      const padSet = (name: string, v: number, axis = false) => { pv[name] = v; buf.push({ t: 'pad', items: [{ name, code: `mock-${name}`, value: v, axis }] }); };
      const stickKeys: Record<string, [string, number]> = { KeyA: ['LeftStickX', -1], KeyD: ['LeftStickX', 1], KeyW: ['LeftStickY', 1], KeyS: ['LeftStickY', -1], ArrowLeft: ['RightStickX', -1], ArrowRight: ['RightStickX', 1], ArrowUp: ['RightStickY', 1], ArrowDown: ['RightStickY', -1] };
      const btnKeys: Record<string, string> = { KeyI: 'North', KeyJ: 'West', KeyK: 'South', KeyL: 'East', KeyQ: 'LeftTrigger', KeyE: 'RightTrigger', KeyZ: 'LeftTrigger2', KeyC: 'RightTrigger2', Enter: 'Start', Backspace: 'Select', KeyF: 'LeftThumb', KeyG: 'RightThumb', KeyT: 'DPadUp', KeyV: 'DPadDown', KeyB: 'DPadRight', KeyX: 'DPadLeft' };
      const padKey = (e: KeyboardEvent, isDown: boolean) => {
        if (e.repeat) return;
        const st = stickKeys[e.code];
        if (st) {
          const cur = pv[st[0]] ?? 0;
          padSet(st[0], isDown ? st[1] : cur === st[1] ? 0 : cur, true);
          if (st[0].startsWith('Left')) { const x = pv.LeftStickX ?? 0, y = pv.LeftStickY ?? 0; const r = Math.hypot(x, y); if (circ && r > 0.02) { const a = (Math.atan2(y, x) * 180 / Math.PI + 360) % 360; const b = Math.min(71, Math.floor(a / 5)); circBins[b] = Math.max(circBins[b], r); circPath.push([x, y]); } }
        }
        const bt = btnKeys[e.code];
        if (bt) padSet(bt, isDown ? 1 : 0);
      };
      const kd = (e: KeyboardEvent) => { if (device.viewer === 'gamepad') padKey(e, true); onKey(e, true); };
      const ku = (e: KeyboardEvent) => { if (device.viewer === 'gamepad') padKey(e, false); onKey(e, false); };
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
        if (device.viewer === 'gamepad') buf.push({ t: 'poll', hz: 250, avgHz: 250, minHz: 244, maxHz: 256, intervalMinUs: 3900, intervalAvgUs: 4000, intervalMaxUs: 4100, jitterUs: 60, nominal: null, reports: 500, graph: Array.from({ length: 60 }, (_, i) => 248 + 4 * Math.sin(i / 3)) });
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
        hid: device.viewer === 'hid'
          ? q.get('hiderr') ? { layout: null, error: 'hid_open failed: access denied' } : q.get('rawonly') ? { layout: { parsed: false, buttons: [], axes: [], reportIds: [], descriptorLen: 0 }, error: null } : { layout: { parsed: true, buttons: [...Array.from({ length: 12 }, (_, i) => `button ${i + 1}`), 'hat up', 'hat right', 'hat down', 'hat left'], axes: [{ name: 'x', min: 0, max: 255 }, { name: 'y', min: 0, max: 255 }, { name: 'z', min: 0, max: 255 }, { name: 'rz', min: 0, max: 255 }], reportIds: [], descriptorLen: 76 }, error: null }
          : null,
        analog: device.analogProfile ? { profile: 'wooting', verified: false, error: null } : null,
        midiError: device.viewer === 'midi' && q.get('miderr') ? 'the midi port is busy' : null,
      };
      // fake generic hid + midi + analog, driven by the keyboard / wheel
      if (device.viewer === 'hid' || device.analogProfile) {
        const rep = new Array(8).fill(0);
        let last = rep.slice();
        const bytes = () => { const out = rep.slice(); const ch = out.map((v, i) => (v !== last[i] ? i : -1)).filter((i) => i >= 0); last = out.slice(); return { out, ch }; };
        const hk = (e: KeyboardEvent, d: boolean) => {
          const n = '1234567890'.indexOf(e.key);
          if (n >= 0 && n < 8) { rep[0] = d ? rep[0] | (1 << n) : rep[0] & ~(1 << n); }
          if (e.code === 'ArrowLeft') rep[2] = d ? 0 : 128; if (e.code === 'ArrowRight') rep[2] = d ? 255 : 128;
          if (e.code === 'ArrowUp') rep[3] = d ? 0 : 128; if (e.code === 'ArrowDown') rep[3] = d ? 255 : 128;
          const { out, ch } = bytes();
          buf.push({ t: 'hid', id: null, report: out, changed: ch, n: 1, buttons: [...Array.from({ length: 12 }, (_, i) => i < 8 ? !!(out[0] & (1 << i)) : false), false, false, false, false], axes: [out[2] / 255, out[3] / 255, 0.5, 0.5] });
        };
        rep[2] = 128; rep[3] = 128;
        addEventListener('keydown', (e) => !e.repeat && hk(e, true)); addEventListener('keyup', (e) => hk(e, false));
        if (device.analogProfile) {
          const t = setInterval(() => { const keys: [string, number][] = [...down].filter((k) => k.startsWith('Key')).map((k) => [k, Math.min(1, ((performance.now() - (downAt[k] ?? performance.now())) / 600))]); if (keys.length) buf.push({ t: 'analog', keys }); }, 40);
          const prev = stop; stop = () => { clearInterval(t); prev(); };
        }
      }
      if (device.viewer === 'midi') {
        const notes: Record<string, number> = { KeyA: 60, KeyW: 61, KeyS: 62, KeyE: 63, KeyD: 64, KeyF: 65, KeyT: 66, KeyG: 67, KeyY: 68, KeyH: 69, KeyU: 70, KeyJ: 71, KeyK: 72 };
        const name = (n: number) => ['C', 'C#', 'D', 'D#', 'E', 'F', 'F#', 'G', 'G#', 'A', 'A#', 'B'][n % 12] + (Math.floor(n / 12) - 1);
        const mk = (e: KeyboardEvent, d: boolean) => { const n = notes[e.code]; if (n === undefined || e.repeat) return; buf.push({ t: 'midi', kind: d ? 'noteOn' : 'noteOff', channel: 1, a: n, b: d ? 90 : 0, value: d ? 40 + (n % 5) * 20 : 0, ts: performance.now() - t0, text: d ? `note on ${name(n)} velocity ${40 + (n % 5) * 20}` : `note off ${name(n)}` }); };
        addEventListener('keydown', (e) => mk(e, true)); addEventListener('keyup', (e) => mk(e, false));
        let cc = 0;
        addEventListener('wheel', (e) => { cc = Math.max(0, Math.min(127, cc + (e.deltaY < 0 ? 8 : -8))); buf.push({ t: 'midi', kind: 'cc', channel: 1, a: 1, b: cc, value: cc, ts: performance.now() - t0, text: `cc 1 = ${cc}` }); buf.push({ t: 'midi', kind: 'pitch', channel: 1, a: 0, b: 0, value: (cc - 64) * 128, ts: performance.now() - t0, text: 'pitch bend' }); });
      }
      return info2;
    },
    async endSession() {
      // mirrors the engine: the button never ends a mouse session
      if (activeMode === 'mouse' && !sessionNoCapture) return;
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
