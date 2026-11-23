// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab · source viewable, not reusable: see LICENSE
// controller diagrams as data on a 400 x 280 grid. each element names the gilrs
// control(s) that light it, so the same drawing code serves every pad.
import type { PadModel, PadStyle } from './pads';

export type FaceGlyph = 'cross' | 'circle' | 'square' | 'triangle' | string;

export type PadEl =
  | { k: 'face'; names: string[]; x: number; y: number; r: number; glyph: FaceGlyph }
  | { k: 'pill'; names: string[]; x: number; y: number; w: number; h: number; label?: string }
  | { k: 'round'; names: string[]; x: number; y: number; r: number; label?: string }
  | { k: 'trigger'; names: string[]; x: number; y: number; w: number; h: number; label: string }
  | { k: 'stick'; side: 'left' | 'right'; x: number; y: number; r: number; click: string }
  | { k: 'dpad'; x: number; y: number }
  | { k: 'decor'; x: number; y: number; w: number; h: number; label?: string }
  /** a drawn line or outline (panel seams, light bar): not an input */
  | { k: 'path'; d: string; dash?: boolean };

export const BODIES: Record<PadModel['body'], string> = {
  // dualsense: broad shoulders, grips swept outward and down
  ps5: 'M86 70 C86 52 98 42 118 42 L282 42 C302 42 314 52 314 70 C330 108 352 160 358 206 C364 240 346 260 322 254 C302 249 290 226 276 208 C268 197 258 190 244 190 L156 190 C142 190 132 197 124 208 C110 226 98 249 78 254 C54 260 36 240 42 206 C48 160 70 108 86 70 Z',
  ps: 'M70 84 C70 56 96 44 132 44 L268 44 C304 44 330 56 330 84 L352 200 C358 236 336 256 312 246 C286 236 272 204 250 194 L150 194 C128 204 114 236 88 246 C64 256 42 236 48 200 Z',
  xbox: 'M80 86 C80 58 106 48 140 48 L260 48 C294 48 320 58 320 86 L346 206 C352 242 330 256 306 246 C282 237 270 206 250 196 L150 196 C130 206 118 237 94 246 C70 256 48 242 54 206 Z',
  x360: 'M84 90 C84 60 110 48 142 48 L258 48 C290 48 316 60 316 90 L344 206 C350 240 326 254 304 244 C282 236 268 206 248 196 L152 196 C132 206 118 236 96 244 C74 254 50 240 56 206 Z',
  switch: 'M76 88 C76 56 102 46 138 46 L262 46 C298 46 324 56 324 88 L348 200 C354 238 332 258 304 248 C282 240 270 208 252 198 L148 198 C130 208 118 240 96 248 C68 258 46 238 52 200 Z',
  generic: 'M80 86 C80 58 106 48 140 48 L260 48 C294 48 320 58 320 86 L346 206 C352 242 330 256 306 246 C282 237 270 206 250 196 L150 196 C130 206 118 237 94 246 C70 256 48 242 54 206 Z',
};

const triggers = (lt: string, rt: string, lb: string, rb: string): PadEl[] => [
  { k: 'trigger', names: ['LeftTrigger2', 'LeftZ'], x: 84, y: 10, w: 58, h: 28, label: lt },
  { k: 'trigger', names: ['RightTrigger2', 'RightZ'], x: 258, y: 10, w: 58, h: 28, label: rt },
  { k: 'pill', names: ['LeftTrigger'], x: 84, y: 40, w: 66, h: 12, label: lb },
  { k: 'pill', names: ['RightTrigger'], x: 250, y: 40, w: 66, h: 12, label: rb },
];

/** the dualsense: black centre panel, touchpad with the light bar either side, create and options, ps and mute */
function dualsense(m: PadModel | null): PadEl[] {
  const els: PadEl[] = [
    // the white shell and the black centre panel meet along these seams
    { k: 'path', dash: true, d: 'M126 46 L274 46 C292 48 300 58 300 72 L300 132 C300 146 290 154 276 154 L124 154 C110 154 100 146 100 132 L100 72 C100 58 108 48 126 46 Z' },
    { k: 'path', dash: true, d: 'M100 120 C84 150 66 196 62 226 M300 120 C316 150 334 196 338 226' },
    // light bar
    { k: 'path', d: 'M152 62 C142 80 142 100 152 118 M248 62 C258 80 258 100 248 118' },
    ...triggers('L2', 'R2', 'L1', 'R1'),
    { k: 'dpad', x: 112, y: 100 },
    { k: 'face', names: ['North'], x: 288, y: 76, r: 12, glyph: 'triangle' },
    { k: 'face', names: ['East'], x: 312, y: 100, r: 12, glyph: 'circle' },
    { k: 'face', names: ['South'], x: 288, y: 124, r: 12, glyph: 'cross' },
    { k: 'face', names: ['West'], x: 264, y: 100, r: 12, glyph: 'square' },
    { k: 'pill', names: ['Touchpad'], x: 160, y: 62, w: 80, h: 52, label: 'touchpad' },
    { k: 'pill', names: ['Select'], x: 130, y: 72, w: 18, h: 9 },
    { k: 'pill', names: ['Start'], x: 252, y: 72, w: 18, h: 9 },
    { k: 'round', names: ['Mode'], x: 200, y: 140, r: 8, label: 'PS' },
    { k: 'pill', names: ['Mute'], x: 189, y: 158, w: 22, h: 8 },
    { k: 'stick', side: 'left', x: 158, y: 166, r: 27, click: 'LeftThumb' },
    { k: 'stick', side: 'right', x: 242, y: 166, r: 27, click: 'RightThumb' },
  ];
  if (m?.paddles) els.push({ k: 'decor', x: 120, y: 214, w: 14, h: 28 }, { k: 'decor', x: 266, y: 214, w: 14, h: 28 });
  return els;
}

function playstation(m: PadModel | null): PadEl[] {
  if (m?.id.startsWith('dualsense')) return dualsense(m);
  const els: PadEl[] = [
    ...triggers('L2', 'R2', 'L1', 'R1'),
    { k: 'dpad', x: 112, y: 96 },
    { k: 'face', names: ['North'], x: 288, y: 74, r: 12, glyph: 'triangle' },
    { k: 'face', names: ['East'], x: 312, y: 98, r: 12, glyph: 'circle' },
    { k: 'face', names: ['South'], x: 288, y: 122, r: 12, glyph: 'cross' },
    { k: 'face', names: ['West'], x: 264, y: 98, r: 12, glyph: 'square' },
    { k: 'stick', side: 'left', x: 158, y: 166, r: 26, click: 'LeftThumb' },
    { k: 'stick', side: 'right', x: 242, y: 166, r: 26, click: 'RightThumb' },
    { k: 'pill', names: ['Select'], x: 138, y: 70, w: 16, h: 10 },
    { k: 'pill', names: ['Start'], x: 246, y: 70, w: 16, h: 10 },
    { k: 'round', names: ['Mode'], x: 200, y: 150, r: 8, label: 'PS' },
  ];
  if (m?.touchpad) els.push({ k: 'decor', x: 164, y: 62, w: 72, h: 44, label: 'touchpad' });
  if (m?.mute) els.push({ k: 'decor', x: 194, y: 120, w: 12, h: 8 });
  if (m?.paddles) els.push({ k: 'decor', x: 150, y: 210, w: 14, h: 28 }, { k: 'decor', x: 236, y: 210, w: 14, h: 28 });
  return els;
}

function xbox(m: PadModel | null): PadEl[] {
  const els: PadEl[] = [
    ...triggers('LT', 'RT', 'LB', 'RB'),
    { k: 'stick', side: 'left', x: 122, y: 100, r: 26, click: 'LeftThumb' },
    { k: 'dpad', x: 158, y: 168 },
    { k: 'stick', side: 'right', x: 244, y: 168, r: 26, click: 'RightThumb' },
    { k: 'face', names: ['North'], x: 290, y: 76, r: 12, glyph: 'Y' },
    { k: 'face', names: ['South'], x: 290, y: 124, r: 12, glyph: 'A' },
    { k: 'face', names: ['West'], x: 266, y: 100, r: 12, glyph: 'X' },
    { k: 'face', names: ['East'], x: 314, y: 100, r: 12, glyph: 'B' },
    { k: 'pill', names: ['Select'], x: 172, y: 96, w: 20, h: 10 },
    { k: 'pill', names: ['Start'], x: 208, y: 96, w: 20, h: 10 },
    { k: 'round', names: ['Mode'], x: 200, y: 72, r: 9 },
  ];
  if (m?.share) els.push({ k: 'decor', x: 190, y: 118, w: 20, h: 8, label: 'share' });
  if (m?.paddles) els.push({ k: 'decor', x: 148, y: 214, w: 14, h: 26 }, { k: 'decor', x: 238, y: 214, w: 14, h: 26 });
  return els;
}

function sw(): PadEl[] {
  return [
    ...triggers('ZL', 'ZR', 'L', 'R'),
    { k: 'stick', side: 'left', x: 116, y: 96, r: 26, click: 'LeftThumb' },
    { k: 'dpad', x: 158, y: 168 },
    { k: 'stick', side: 'right', x: 244, y: 168, r: 26, click: 'RightThumb' },
    { k: 'face', names: ['North'], x: 292, y: 70, r: 12, glyph: 'X' },
    { k: 'face', names: ['South'], x: 292, y: 118, r: 12, glyph: 'B' },
    { k: 'face', names: ['West'], x: 268, y: 94, r: 12, glyph: 'Y' },
    { k: 'face', names: ['East'], x: 316, y: 94, r: 12, glyph: 'A' },
    { k: 'pill', names: ['Select'], x: 174, y: 78, w: 16, h: 10, label: '-' },
    { k: 'pill', names: ['Start'], x: 210, y: 78, w: 16, h: 10, label: '+' },
    { k: 'round', names: ['Mode'], x: 222, y: 110, r: 8 },
    { k: 'decor', x: 170, y: 104, w: 16, h: 12, label: 'capture' },
  ];
}

export function padElements(style: PadStyle, model: PadModel | null): PadEl[] {
  if (style === 'playstation') return playstation(model);
  if (style === 'switch') return sw();
  return xbox(model);
}
