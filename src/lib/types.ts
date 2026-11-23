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

export type CaptureMode = 'mouse' | 'keyboard' | 'other';
export type ExitKind = 'esc' | 'combo';

export interface ExitInfo {
  kind: ExitKind | null;
  holdMs: number;
  button: boolean;
  combo: string[];
}

export interface CaptureReport {
  active: boolean;
  cursorClipped: boolean;
  keyboardHooked: boolean;
  mouseSwallowed: boolean;
  warnings: string[];
}

export interface ReservedCombo {
  label: string;
  keys: string[];
  why: string;
}

export interface SessionInfo {
  device: DeviceInfo;
  mode: CaptureMode;
  exit: ExitInfo;
  capture: CaptureReport;
  reserved: ReservedCombo[];
  comboWarning: string | null;
  noCapture: boolean;
}

export type EndReason = 'exit-button' | 'esc-hold' | 'combo' | 'focus-lost' | 'watchdog' | 'ui-gone' | 'shutdown';

export type Ev =
  | { t: 'key'; code: string; down: boolean; ts: number }
  | { t: 'exit'; kind: ExitKind; progress: number }
  | { t: 'reserved'; label: string }
  | { t: 'ended'; reason: EndReason };

export interface Frame {
  seq: number;
  events: Ev[];
}

export interface SessionSettings {
  escHoldMs: number;
  comboHoldMs: number;
}
