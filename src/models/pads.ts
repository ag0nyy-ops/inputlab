// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab · source viewable, not reusable: see LICENSE
// which controller is this? usb vid/pid is reliable for pads, so ids come first,
// names second. xinput pads on windows have no vid/pid, so the name decides.
// to add a pad: add a row to PADS (see readme).

import type { DeviceInfo } from '../lib/types';

export type PadStyle = 'playstation' | 'xbox' | 'switch' | 'generic';

export interface PadModel {
  id: string;
  name: string;
  style: PadStyle;
  /** outline variant */
  body: 'ps' | 'xbox' | 'x360' | 'switch' | 'generic';
  ids: [number, number][];
  match: RegExp;
  /** decoration only: these have no standard button mapping */
  touchpad?: boolean;
  mute?: boolean;
  share?: boolean;
  paddles?: boolean;
}

export const PADS: PadModel[] = [
  { id: 'dualsense-edge', name: 'DualSense Edge', style: 'playstation', body: 'ps', ids: [[0x054c, 0x0df2]], match: /dualsense\s*edge/i, touchpad: true, mute: true, paddles: true },
  { id: 'dualsense', name: 'DualSense', style: 'playstation', body: 'ps', ids: [[0x054c, 0x0ce6]], match: /dualsense/i, touchpad: true, mute: true },
  { id: 'dualshock4', name: 'DualShock 4', style: 'playstation', body: 'ps', ids: [[0x054c, 0x05c4], [0x054c, 0x09cc]], match: /dualshock\s*4|ps4 controller|^wireless controller$/i, touchpad: true },
  { id: 'dualshock3', name: 'DualShock 3', style: 'playstation', body: 'ps', ids: [[0x054c, 0x0268]], match: /dualshock\s*3|playstation\(r\)3/i },
  { id: 'xbox-series', name: 'Xbox Series', style: 'xbox', body: 'xbox', ids: [[0x045e, 0x0b12], [0x045e, 0x0b13]], match: /xbox\s*(series|wireless controller)|series\s*[xs]/i, share: true },
  { id: 'xbox-elite', name: 'Xbox Elite', style: 'xbox', body: 'xbox', ids: [[0x045e, 0x0b00], [0x045e, 0x02e3]], match: /elite/i, paddles: true },
  { id: 'xbox-one', name: 'Xbox One', style: 'xbox', body: 'xbox', ids: [[0x045e, 0x02ea], [0x045e, 0x02fd], [0x045e, 0x02e0]], match: /xbox\s*one/i },
  { id: 'xbox-360', name: 'Xbox 360', style: 'xbox', body: 'x360', ids: [[0x045e, 0x028e], [0x045e, 0x028f], [0x045e, 0x0719]], match: /xbox\s*360|x360|xinput/i },
  { id: 'switch-pro', name: 'Switch Pro', style: 'switch', body: 'switch', ids: [[0x057e, 0x2009]], match: /pro controller|switch/i },
];

export type PadConfidence = 'picked' | 'usb id' | 'name' | 'none';

export interface PadMatch {
  model: PadModel | null;
  style: PadStyle;
  body: PadModel['body'];
  confidence: PadConfidence;
}

export function findPad(id: string): PadModel | null {
  return PADS.find((p) => p.id === id) ?? null;
}

export function matchPad(d: Pick<DeviceInfo, 'name' | 'vid' | 'pid'>, picked?: string | null): PadMatch {
  const pick = picked ? findPad(picked) : null;
  if (pick) return { model: pick, style: pick.style, body: pick.body, confidence: 'picked' };
  const byId = PADS.find((p) => p.ids.some(([v, pid]) => v === d.vid && pid === d.pid));
  if (byId) return { model: byId, style: byId.style, body: byId.body, confidence: 'usb id' };
  const byName = PADS.find((p) => p.match.test(d.name));
  if (byName) return { model: byName, style: byName.style, body: byName.body, confidence: 'name' };
  return { model: null, style: 'generic', body: 'generic', confidence: 'none' };
}
