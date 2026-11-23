// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab
// dev-only fake backend. imported behind import.meta.env.DEV, so it never ships.
// url switches: ?empty=1  ?state=offline (no raw input)  ?os=macos|linux  ?many=1

import type { AppInfo, DeviceInfo, DeviceListMsg } from './types';
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
  return {
    appInfo: async () => info,
    subscribeDevices(cb) {
      const msg: DeviceListMsg = { devices: devices() };
      const t = setTimeout(() => cb(msg), 60);
      return () => clearTimeout(t);
    },
    rescan: async () => {},
  };
}
