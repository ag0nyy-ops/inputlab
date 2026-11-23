// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab · source viewable, not reusable: see LICENSE
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
  /** false for interfaces nobody can hold (windows plumbing, config channels): hidden by default */
  usable: boolean;
  hiddenReason: string | null;
  hiddenInterfaces: number;
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
  hid: { layout: LayoutInfo | null; error: string | null } | null;
  analog: { profile: string; verified: boolean; error: string | null } | null;
  midiError: string | null;
}

export interface LayoutInfo {
  parsed: boolean;
  buttons: string[];
  axes: { name: string; min: number; max: number }[];
  reportIds: number[];
  descriptorLen: number;
}

export type EndReason = 'exit-button' | 'esc-hold' | 'combo' | 'focus-lost' | 'watchdog' | 'ui-gone' | 'shutdown';

export type Ev =
  | { t: 'key'; code: string; down: boolean; ts: number; hold: number | null; gap: number | null; chatter: boolean; held: number; max: number }
  | { t: 'mouse'; dx: number; dy: number; n: number }
  | { t: 'button'; n: number; down: boolean; ts: number; hold: number | null; gap: number | null; chatter: boolean }
  | { t: 'wheel'; dx: number; dy: number }
  | ({ t: 'poll' } & PollSnap)
  | ({ t: 'dpi' } & DpiSnap)
  | ({ t: 'sensor' } & SensorSnap)
  | { t: 'clicks'; buttons: ButtonStat[] }
  | { t: 'hid'; id: number | null; report: number[]; changed: number[]; n: number; buttons: boolean[]; axes: number[] }
  | { t: 'analog'; keys: [string, number][] }
  | { t: 'midi'; kind: string; channel: number; a: number; b: number; value: number; ts: number; text: string }
  | { t: 'pad'; items: PadItem[] }
  | { t: 'padInfo'; forceFeedback: boolean; power: string; battery: number | null; pollSource: string }
  | { t: 'sticks'; left: StickSnap; right: StickSnap; rest: RestSnap }
  | { t: 'notice'; text: string }
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
  chatterMs: number;
  disableCapture: boolean;
}

export interface PollSnap {
  hz: number | null;
  avgHz: number | null;
  minHz: number | null;
  maxHz: number | null;
  intervalMinUs: number | null;
  intervalAvgUs: number | null;
  intervalMaxUs: number | null;
  jitterUs: number | null;
  nominal: number | null;
  reports: number;
  graph: number[];
}

export interface DpiSnap {
  state: 'idle' | 'counting' | 'done';
  distanceIn: number;
  counts: number;
  path: number;
  straightness: number;
  dpi: number | null;
}

export interface SensorSnap {
  active: boolean;
  steps: number;
  radius: number | null;
  jitterPct: number | null;
  snapRatio: number | null;
  snapping: boolean;
  angles: number[];
  path: [number, number][];
}

export interface ButtonStat {
  n: number;
  clicks: number;
  holdMin: number | null;
  holdAvg: number | null;
  holdMax: number | null;
  chatter: number;
  minGap: number | null;
}

export interface PadItem {
  name: string;
  code: string;
  value: number;
  axis: boolean;
}

export interface StickSnap {
  x: number;
  y: number;
  active: boolean;
  circularity: number | null;
  coverage: number;
  maxRange: number;
  deadzone: number | null;
  bins: number[];
  path: [number, number][];
}

export interface RestEntry {
  name: string;
  meanX: number;
  meanY: number;
  offset: number;
  noise: number;
  drift: boolean;
}

export interface RestSnap {
  running: boolean;
  secsLeft: number;
  entries: RestEntry[];
}

export type SessionCommand =
  | { type: 'resetKeys' }
  | { type: 'dpiStart'; distanceIn: number }
  | { type: 'dpiStop' }
  | { type: 'dpiReset' }
  | { type: 'sensorStart' }
  | { type: 'sensorStop' }
  | { type: 'sensorReset' }
  | { type: 'mouseReset' }
  | { type: 'padCircleStart' }
  | { type: 'padCircleStop' }
  | { type: 'padCircleReset' }
  | { type: 'padRestStart'; secs: number }
  | { type: 'padRumble'; strong: number; weak: number; ms: number };

export interface KeyLabels {
  layout_name: string;
  labels: [string, string][];
}
