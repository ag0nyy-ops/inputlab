// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab · source viewable, not reusable: see LICENSE
// mouse silhouettes, drawn top-down on a 200 x 320 grid, front of the mouse at the top.
// these are grip families (small symmetrical, slim ambidextrous, ergonomic...), not
// product renders: the point is that a g305 and a superlight do not look the same.
// the buttons are clipped to the body, so adding a shape is one path plus a few numbers.

export type ShapeId = 'sym-pear' | 'ambi-slim' | 'ambi-wide' | 'ergo-right' | 'ergo-large' | 'touchpad' | 'generic';

export interface Box {
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface MouseShape {
  id: ShapeId;
  name: string;
  /** svg path of the outline, 200 x 320 */
  body: string;
  /** y of the line between the click buttons and the palm */
  split: number;
  wheel: Box;
  /** side buttons: 5 = forward, 4 = back */
  sides: { n: 4 | 5; box: Box }[];
  /** where the click buttons are: above the split (a mouse) or below it (a touchpad) */
  zone?: 'top' | 'bottom';
  /** x of the line between left and right click, when it is not the wheel's centre */
  gutter?: number;
  /** non-input decoration: thumb shelf, dpi button... drawn as a faint outline */
  extras?: Box[];
}

export const SHAPES: Record<ShapeId, MouseShape> = {
  'sym-pear': {
    id: 'sym-pear',
    name: 'small symmetrical',
    body: 'M100 30 C142 30 164 66 166 124 C168 190 164 250 140 282 C126 300 74 300 60 282 C36 250 32 190 34 124 C36 66 58 30 100 30 Z',
    split: 138,
    wheel: { x: 92, y: 64, w: 16, h: 42 },
    sides: [{ n: 5, box: { x: 31, y: 154, w: 13, h: 28 } }, { n: 4, box: { x: 31, y: 188, w: 13, h: 28 } }],
    extras: [{ x: 94, y: 112, w: 12, h: 10 }],
  },
  'ambi-slim': {
    id: 'ambi-slim',
    name: 'slim ambidextrous',
    body: 'M100 26 C130 26 146 52 148 104 C150 160 144 206 146 248 C147 276 134 304 100 306 C66 304 53 276 54 248 C56 206 50 160 52 104 C54 52 70 26 100 26 Z',
    split: 122,
    wheel: { x: 93, y: 58, w: 14, h: 40 },
    sides: [{ n: 5, box: { x: 49, y: 138, w: 11, h: 26 } }, { n: 4, box: { x: 49, y: 170, w: 11, h: 26 } }],
  },
  'ambi-wide': {
    id: 'ambi-wide',
    name: 'wide ambidextrous',
    body: 'M100 24 C138 24 160 54 164 106 C168 168 168 222 156 262 C148 290 124 304 100 304 C76 304 52 290 44 262 C32 222 32 168 36 106 C40 54 62 24 100 24 Z',
    split: 128,
    wheel: { x: 92, y: 56, w: 16, h: 42 },
    sides: [{ n: 5, box: { x: 34, y: 148, w: 13, h: 28 } }, { n: 4, box: { x: 34, y: 182, w: 13, h: 28 } }],
  },
  'ergo-right': {
    id: 'ergo-right',
    name: 'ergonomic right hand',
    body: 'M92 26 C136 22 170 54 178 118 C186 186 178 252 148 290 C132 308 84 308 68 290 C52 272 46 232 48 196 C50 164 42 140 46 108 C50 64 60 32 92 26 Z',
    split: 136,
    wheel: { x: 87, y: 62, w: 16, h: 42 },
    sides: [{ n: 5, box: { x: 42, y: 150, w: 13, h: 30 } }, { n: 4, box: { x: 42, y: 186, w: 13, h: 30 } }],
    extras: [{ x: 56, y: 226, w: 26, h: 40 }],
  },
  'ergo-large': {
    id: 'ergo-large',
    name: 'large ergonomic',
    body: 'M96 22 C146 20 182 56 188 122 C194 192 186 260 152 298 C134 316 84 316 66 298 C42 278 28 244 26 206 C24 174 36 160 36 134 C36 80 52 26 96 22 Z',
    split: 142,
    wheel: { x: 88, y: 56, w: 16, h: 46 },
    sides: [{ n: 5, box: { x: 30, y: 160, w: 15, h: 32 } }, { n: 4, box: { x: 30, y: 198, w: 15, h: 32 } }],
    extras: [{ x: 28, y: 236, w: 24, h: 34 }],
  },
  touchpad: {
    id: 'touchpad',
    name: 'laptop touchpad',
    body: 'M24 96 H176 V236 H24 Z',
    split: 196,
    zone: 'bottom',
    gutter: 100,
    // two-finger scroll shows up as wheel events: a strip down the right edge
    wheel: { x: 164, y: 104, w: 8, h: 84 },
    sides: [],
  },
  generic: {
    id: 'generic',
    name: 'generic mouse',
    body: 'M100 30 C140 30 160 52 160 96 L160 250 C160 284 136 302 100 302 C64 302 40 284 40 250 L40 96 C40 52 60 30 100 30 Z',
    split: 132,
    wheel: { x: 92, y: 60, w: 16, h: 42 },
    sides: [{ n: 5, box: { x: 36, y: 150, w: 13, h: 28 } }, { n: 4, box: { x: 36, y: 184, w: 13, h: 28 } }],
  },
};

export const SHAPE_ASPECT = 320 / 200;
