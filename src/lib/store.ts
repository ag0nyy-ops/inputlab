// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab · source viewable, not reusable: see LICENSE
// tiny external store. react only re-renders for low-rate state (device list,
// selection, screen). high-rate device data never goes through here.

import { useSyncExternalStore } from 'react';
import type { AppInfo, DeviceInfo, EndReason, SessionInfo } from './types';

export type Screen = 'home' | 'settings' | 'session' | 'summary';

export interface AppState {
  info: AppInfo | null;
  devices: DeviceInfo[];
  devicesLoaded: boolean;
  selectedId: string | null;
  screen: Screen;
  session: SessionInfo | null;
  lastEnd: { reason: EndReason; at: number } | null;
  /** the session that just finished, for the summary screen */
  lastSession: { info: SessionInfo; startedAt: number; endedAt: number; reason: EndReason } | null;
}

let state: AppState = {
  info: null,
  devices: [],
  devicesLoaded: false,
  selectedId: null,
  screen: 'home',
  session: null,
  lastEnd: null,
  lastSession: null,
};

const listeners = new Set<() => void>();

export function setState(patch: Partial<AppState> | ((s: AppState) => Partial<AppState>)): void {
  const p = typeof patch === 'function' ? patch(state) : patch;
  state = { ...state, ...p };
  listeners.forEach((l) => l());
}

export function getState(): AppState {
  return state;
}

export function useStore<T>(select: (s: AppState) => T): T {
  return useSyncExternalStore(
    (l) => {
      listeners.add(l);
      return () => listeners.delete(l);
    },
    () => select(state),
  );
}

export function selectedDevice(s: AppState): DeviceInfo | null {
  return s.devices.find((d) => d.id === s.selectedId) ?? null;
}
