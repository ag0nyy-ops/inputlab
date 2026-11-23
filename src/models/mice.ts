// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab · source viewable, not reusable: see LICENSE
// which mouse is this? names first (usb mice report a product string), then
// vid/pid. wireless receivers are shared by several mice, so those are flagged
// ambiguous and the user picks once; the choice is remembered on this machine.
// to add a mouse: add a row to MICE (see readme).

import type { DeviceInfo } from '../lib/types';
import type { ShapeId } from './mouseShapes';

export interface MouseModel {
  id: string;
  vendor: string;
  name: string;
  shape: ShapeId;
  /** matched against the device's product string */
  match: RegExp;
}

export const MICE: MouseModel[] = [
  // built in
  { id: 'laptop-touchpad', vendor: 'laptop', name: 'laptop touchpad', shape: 'touchpad', match: /touchpad|trackpad|precision/i },
  // logitech
  { id: 'logi-g305', vendor: 'logitech', name: 'G305 lightspeed', shape: 'sym-pear', match: /\bg\s?305\b/i },
  { id: 'logi-g-pro-wireless', vendor: 'logitech', name: 'G Pro wireless', shape: 'ambi-slim', match: /\bg\s?pro\b(?!.*\b(x|superlight)\b)/i },
  { id: 'logi-g-pro-x-superlight', vendor: 'logitech', name: 'G Pro X superlight', shape: 'ambi-slim', match: /\bsuperlight\b|\bg\s?pro\s?x\b/i },
  { id: 'logi-g502', vendor: 'logitech', name: 'G502', shape: 'ergo-right', match: /\bg\s?502\b/i },
  { id: 'logi-g403', vendor: 'logitech', name: 'G403 / G703', shape: 'ergo-right', match: /\bg\s?(403|703)\b/i },
  { id: 'logi-g903', vendor: 'logitech', name: 'G900 / G903', shape: 'ambi-wide', match: /\bg\s?(900|903)\b/i },
  { id: 'logi-g203', vendor: 'logitech', name: 'G203 / G102', shape: 'sym-pear', match: /\bg\s?(203|102)\b/i },
  { id: 'logi-mx-master', vendor: 'logitech', name: 'MX Master', shape: 'ergo-large', match: /\bmx\s?master\b/i },
  // razer
  { id: 'razer-deathadder', vendor: 'razer', name: 'DeathAdder', shape: 'ergo-right', match: /deathadder/i },
  { id: 'razer-basilisk', vendor: 'razer', name: 'Basilisk', shape: 'ergo-right', match: /basilisk/i },
  { id: 'razer-viper', vendor: 'razer', name: 'Viper', shape: 'ambi-slim', match: /viper/i },
  // others
  { id: 'zowie-ec', vendor: 'zowie', name: 'EC series', shape: 'ergo-right', match: /zowie.*\bec\d?/i },
  { id: 'zowie-fk', vendor: 'zowie', name: 'FK series', shape: 'ambi-wide', match: /zowie.*\bfk\d?/i },
  { id: 'glorious-o', vendor: 'glorious', name: 'Model O', shape: 'ambi-slim', match: /model\s?o\b|model\s?o-/i },
  { id: 'glorious-d', vendor: 'glorious', name: 'Model D', shape: 'ergo-right', match: /model\s?d\b/i },
  { id: 'steelseries-aerox', vendor: 'steelseries', name: 'Aerox', shape: 'ambi-slim', match: /aerox/i },
  { id: 'steelseries-rival', vendor: 'steelseries', name: 'Rival', shape: 'ergo-right', match: /rival/i },
  { id: 'pulsar', vendor: 'pulsar', name: 'Pulsar', shape: 'ambi-slim', match: /pulsar/i },
  { id: 'finalmouse', vendor: 'finalmouse', name: 'Finalmouse', shape: 'ambi-slim', match: /finalmouse|starlight|ultralight/i },
];

/** usb ids that name one specific mouse (wired / charging mode). kept to ids i am sure of. */
const BY_ID: { vid: number; pid: number; model: string }[] = [
  { vid: 0x046d, pid: 0xc08b, model: 'logi-g502' },
  { vid: 0x046d, pid: 0xc332, model: 'logi-g502' },
  { vid: 0x046d, pid: 0xc084, model: 'logi-g203' },
  { vid: 0x046d, pid: 0xc092, model: 'logi-g203' },
  { vid: 0x046d, pid: 0xc083, model: 'logi-g403' },
  { vid: 0x046d, pid: 0xc085, model: 'logi-g-pro-wireless' },
  { vid: 0x046d, pid: 0xc088, model: 'logi-g-pro-wireless' },
  { vid: 0x1532, pid: 0x0084, model: 'razer-deathadder' },
  { vid: 0x1532, pid: 0x007a, model: 'razer-viper' },
  { vid: 0x1532, pid: 0x008a, model: 'razer-viper' },
];

export type Confidence = 'picked' | 'name' | 'usb id' | 'ambiguous' | 'none';

export interface MouseMatch {
  model: MouseModel | null;
  shape: ShapeId;
  confidence: Confidence;
  /** a wireless receiver: several different mice share this usb id */
  receiver: boolean;
}

const isReceiver = (d: Pick<DeviceInfo, 'name' | 'vid'>) => /receiver|dongle|unifying|lightspeed|bolt/i.test(d.name) || (d.vid === 0x046d && /usb receiver/i.test(d.name));

export function findModel(id: string): MouseModel | null {
  return MICE.find((m) => m.id === id) ?? null;
}

export function matchMouse(d: Pick<DeviceInfo, 'name' | 'vid' | 'pid'>, picked?: string | null): MouseMatch {
  const receiver = isReceiver(d);
  if (picked) {
    const m = findModel(picked);
    if (m) return { model: m, shape: m.shape, confidence: 'picked', receiver };
  }
  const byName = MICE.find((m) => m.match.test(d.name));
  if (byName) return { model: byName, shape: byName.shape, confidence: 'name', receiver };
  const byId = BY_ID.find((r) => r.vid === d.vid && r.pid === d.pid);
  const m = byId ? findModel(byId.model) : null;
  if (m) return { model: m, shape: m.shape, confidence: 'usb id', receiver };
  return { model: null, shape: 'generic', confidence: receiver ? 'ambiguous' : 'none', receiver };
}
