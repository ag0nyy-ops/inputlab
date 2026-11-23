// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab · source viewable, not reusable: see LICENSE
// remembered model choices, per device (vid:pid when it has one). local only.
import { useSyncExternalStore } from 'react';
import type { DeviceInfo } from '../lib/types';

const KEY = 'inputlab.models';
let map: Record<string, string> = {};
try {
  map = JSON.parse(localStorage.getItem(KEY) ?? '{}') as Record<string, string>;
} catch {
  map = {};
}
const listeners = new Set<() => void>();

export const modelKey = (d: Pick<DeviceInfo, 'id' | 'vid' | 'pid'>): string =>
  d.vid !== null && d.pid !== null ? `${d.vid.toString(16)}:${d.pid.toString(16)}` : d.id;

export function getPick(d: Pick<DeviceInfo, 'id' | 'vid' | 'pid'>): string | null {
  return map[modelKey(d)] ?? null;
}

export function setPick(d: Pick<DeviceInfo, 'id' | 'vid' | 'pid'>, model: string | null): void {
  const k = modelKey(d);
  if (model) map = { ...map, [k]: model };
  else {
    const { [k]: _gone, ...rest } = map;
    void _gone;
    map = rest;
  }
  try {
    localStorage.setItem(KEY, JSON.stringify(map));
  } catch {
    /* ignore */
  }
  listeners.forEach((l) => l());
}

export function usePick(d: Pick<DeviceInfo, 'id' | 'vid' | 'pid'>): string | null {
  return useSyncExternalStore(
    (l) => {
      listeners.add(l);
      return () => listeners.delete(l);
    },
    () => getPick(d),
  );
}
