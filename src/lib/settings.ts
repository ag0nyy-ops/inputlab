// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab · source viewable, not reusable: see LICENSE
// local-only preferences. storage can be missing or blocked, so every access is guarded.

import { useSyncExternalStore } from 'react';

export interface Settings {
  comboHoldMs: number;
  /** release->press gap under this many ms counts as switch chatter */
  chatterMs: number;
  /** safe mode: capture nothing, same as --no-capture */
  safeMode: boolean;
  distanceUnit: 'in' | 'cm';
  /** physical distance of the dpi test, in `distanceUnit` */
  dpiDistance: number;
  /** css px per physical inch for the on-screen ruler (96 is the css nominal) */
  rulerPxPerInch: number;
  fx: 'full' | 'balanced' | 'low';
}

export const defaults: Settings = {
  comboHoldMs: 1500,
  chatterMs: 30,
  safeMode: false,
  distanceUnit: 'in',
  dpiDistance: 2,
  rulerPxPerInch: 96,
  fx: 'full',
};

const KEY = 'inputlab.settings';

function load(): Settings {
  try {
    const raw = localStorage.getItem(KEY);
    if (raw) return { ...defaults, ...(JSON.parse(raw) as Partial<Settings>) };
  } catch {
    /* storage unavailable: defaults */
  }
  return { ...defaults };
}

let current = load();
const listeners = new Set<() => void>();

export function getSettings(): Settings {
  return current;
}

export function updateSettings(patch: Partial<Settings>): void {
  current = { ...current, ...patch };
  try {
    localStorage.setItem(KEY, JSON.stringify(current));
  } catch {
    /* ignore */
  }
  if (patch.fx) {
    document.documentElement.dataset.fx = patch.fx;
    try {
      sessionStorage.setItem('inputlab.fx', patch.fx);
    } catch {
      /* ignore */
    }
  }
  listeners.forEach((l) => l());
}

export function resetSettings(): void {
  updateSettings({ ...defaults });
}

export function useSettings(): Settings {
  return useSyncExternalStore(
    (l) => {
      listeners.add(l);
      return () => listeners.delete(l);
    },
    () => current,
  );
}
