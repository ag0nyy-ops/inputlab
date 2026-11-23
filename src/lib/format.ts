// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab · source viewable, not reusable: see LICENSE

export const hex4 = (n: number | null): string => (n === null ? '----' : n.toString(16).padStart(4, '0'));

export const vidpid = (vid: number | null, pid: number | null): string =>
  vid === null || pid === null ? 'no vid/pid' : `${hex4(vid)}:${hex4(pid)}`;

export const caseNo = (i: number): string => `CASE ${String(i + 1).padStart(3, '0')}`;

export const kindLabel: Record<string, string> = {
  keyboard: 'keyboard',
  mouse: 'mouse',
  gamepad: 'controller',
  hid: 'generic hid',
  midi: 'midi',
};

const backendNames: Record<string, string> = {
  rawinput: 'windows raw input',
  evdev: 'linux evdev',
  iokit: 'macos iokit hid',
  gilrs: 'gilrs',
  hidapi: 'hidapi',
  midir: 'midir',
};
export const backendLabel = (b: string): string => backendNames[b] ?? b;
