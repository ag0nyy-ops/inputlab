// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab
// the only file that talks to tauri. everything else imports from here, so the
// dev mock can stand in when the app runs in a plain browser.

import { Channel, invoke } from '@tauri-apps/api/core';
import type { AppInfo, DeviceListMsg } from './types';

export const inTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

export interface Bridge {
  appInfo(): Promise<AppInfo>;
  subscribeDevices(cb: (m: DeviceListMsg) => void): () => void;
  rescan(): Promise<void>;
}

const real: Bridge = {
  appInfo: () => invoke<AppInfo>('app_info'),
  subscribeDevices(cb) {
    const ch = new Channel<DeviceListMsg>();
    ch.onmessage = cb;
    void invoke('subscribe_devices', { onEvent: ch });
    return () => {
      ch.onmessage = () => {};
    };
  },
  rescan: () => invoke('rescan_devices'),
};

let cached: Bridge | null = null;

export async function getBridge(): Promise<Bridge> {
  if (cached) return cached;
  if (import.meta.env.DEV && !inTauri) {
    const { mockBridge } = await import('./mock');
    cached = mockBridge();
  } else {
    cached = real;
  }
  return cached;
}
