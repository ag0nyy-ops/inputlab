// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab
// mirrors the structs rust sends. keep in sync with src-tauri/src.

export type DeviceKind = 'keyboard' | 'mouse' | 'gamepad' | 'hid' | 'midi';
export type Viewer = 'keyboard' | 'mouse' | 'gamepad' | 'hid' | 'midi';

export interface DeviceInfo {
  id: string;
  name: string;
  kind: DeviceKind;
  viewer: Viewer;
  vid: number | null;
  pid: number | null;
  connected: boolean;
  backend: string;
  interfaces: number;
  analogProfile: string | null;
  note: string | null;
}

export interface PlatformCaps {
  rawInput: boolean;
  captureMouse: boolean;
  captureKeyboard: boolean;
  nativeKeyLabels: boolean;
  notes: string[];
}

export interface AppInfo {
  version: string;
  os: 'windows' | 'macos' | 'linux';
  rawBackend: string;
  caps: PlatformCaps;
  noCapture: boolean;
  hidError: string | null;
}

export interface DeviceListMsg {
  devices: DeviceInfo[];
}
