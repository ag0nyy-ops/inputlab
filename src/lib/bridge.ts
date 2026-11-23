// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab
// the only file that talks to tauri. everything else imports from here, so the
// dev mock can stand in when the app runs in a plain browser.

import { Channel, invoke } from '@tauri-apps/api/core';
import type { AppInfo, DeviceListMsg, Frame, KeyLabels, SessionCommand, SessionInfo, SessionSettings } from './types';

export const inTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

export interface Bridge {
  appInfo(): Promise<AppInfo>;
  subscribeDevices(cb: (m: DeviceListMsg) => void): () => void;
  rescan(): Promise<void>;
  startSession(deviceId: string, settings: SessionSettings, onFrame: (f: Frame) => void): Promise<SessionInfo>;
  endSession(): Promise<void>;
  heartbeat(): Promise<void>;
  command(cmd: SessionCommand): Promise<void>;
  keyLabels(): Promise<KeyLabels | null>;
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
  startSession(deviceId, settings, onFrame) {
    const ch = new Channel<Frame>();
    ch.onmessage = onFrame;
    return invoke<SessionInfo>('start_session', { deviceId, settings, onFrame: ch });
  },
  endSession: () => invoke('end_session'),
  heartbeat: () => invoke('session_heartbeat'),
  command: (cmd) => invoke('session_command', { cmd }),
  keyLabels: () => invoke<KeyLabels | null>('key_labels'),
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
