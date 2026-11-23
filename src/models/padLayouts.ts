// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab
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
  | { k: 'decor'; x: number; y: number; w: number; h: number; label?: string };

export const BODIES: Record<PadModel['body'], string> = {
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

function playstation(m: PadModel | null): PadEl[] {
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
