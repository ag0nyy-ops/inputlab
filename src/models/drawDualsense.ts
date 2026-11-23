// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab · source viewable, not reusable: see LICENSE
// the dualsense: the real outline (see dualsenseGlyph.ts) drawn as it is, with every input lit on
// top of it. positions below are in the outline's own 128 x 128 units, mapped onto the 400 x 280
// pad canvas by SC / OX / OY.
import { FG, legend, shade } from '../lib/draw';
import { DUALSENSE_GLYPH } from './dualsenseGlyph';
import type { PadDrawState } from './drawPad';

const SC = 2.9;
const OX = 14;
const OY = -12 * SC;
const X = (x: number): number => OX + x * SC;
const Y = (y: number): number => OY + y * SC;
const R = (r: number): number => r * SC;

let outline: Path2D[] | null = null;
const paths = (): Path2D[] => (outline ??= DUALSENSE_GLYPH.map((d) => new Path2D(d)));

const best = (st: PadDrawState, names: string[]): number => Math.max(...names.map((n) => st.v(n)));

/** a lit region: stays out of sight at 0, goes white at 1 */
function lit(ctx: CanvasRenderingContext2D, v: number, draw: () => void): void {
  if (v <= 0.002) return;
  ctx.fillStyle = shade(v);
  draw();
}

function symbol(ctx: CanvasRenderingContext2D, g: 'cross' | 'circle' | 'square' | 'triangle', cx: number, cy: number, on: boolean): void {
  ctx.strokeStyle = on ? '#000' : FG;
  ctx.lineWidth = 1.5;
  const s = R(1.45);
  ctx.beginPath();
  if (g === 'cross') { ctx.moveTo(cx - s, cy - s); ctx.lineTo(cx + s, cy + s); ctx.moveTo(cx + s, cy - s); ctx.lineTo(cx - s, cy + s); }
  else if (g === 'circle') ctx.arc(cx, cy, s * 1.05, 0, Math.PI * 2);
  else if (g === 'square') ctx.rect(cx - s, cy - s, s * 2, s * 2);
  else { ctx.moveTo(cx, cy - s * 1.15); ctx.lineTo(cx + s * 1.1, cy + s * 0.8); ctx.lineTo(cx - s * 1.1, cy + s * 0.8); ctx.closePath(); }
  ctx.stroke();
}

export function drawDualsense(ctx: CanvasRenderingContext2D, st: PadDrawState): void {
  // lit regions first, so the outline stays visible on top of a white fill
  const rect = (x: number, y: number, w: number, h: number, r = 0.8) => { ctx.beginPath(); ctx.roundRect(X(x), Y(y), R(w), R(h), R(r)); ctx.fill(); };
  const disc = (x: number, y: number, r: number) => { ctx.beginPath(); ctx.arc(X(x), Y(y), R(r), 0, Math.PI * 2); ctx.fill(); };
  const bar = (x1: number, y1: number, x2: number, y2: number, w: number) => {
    ctx.strokeStyle = ctx.fillStyle;
    ctx.lineWidth = R(w);
    ctx.lineCap = 'round';
    ctx.beginPath(); ctx.moveTo(X(x1), Y(y1)); ctx.lineTo(X(x2), Y(y2)); ctx.stroke();
    ctx.lineCap = 'butt';
  };

  // touchpad (it clicks like a button)
  lit(ctx, st.v('Touchpad'), () => { ctx.beginPath(); ctx.moveTo(X(45), Y(30)); ctx.quadraticCurveTo(X(64), Y(28), X(83), Y(30)); ctx.lineTo(X(84.6), Y(46)); ctx.quadraticCurveTo(X(84), Y(52.6), X(78), Y(52.8)); ctx.lineTo(X(50), Y(52.8)); ctx.quadraticCurveTo(X(44), Y(52.6), X(43.4), Y(46)); ctx.closePath(); ctx.fill(); });
  // shoulder buttons: the tabs above the grips
  lit(ctx, st.v('LeftTrigger'), () => { ctx.beginPath(); ctx.moveTo(X(21.4), Y(31.8)); ctx.lineTo(X(21.4), Y(29.4)); ctx.quadraticCurveTo(X(22), Y(26.9), X(25), Y(26.6)); ctx.lineTo(X(33), Y(26.6)); ctx.quadraticCurveTo(X(35.6), Y(27), X(36.1), Y(29)); ctx.lineTo(X(36.1), Y(31.8)); ctx.closePath(); ctx.fill(); });
  lit(ctx, st.v('RightTrigger'), () => { ctx.beginPath(); ctx.moveTo(X(106.6), Y(31.8)); ctx.lineTo(X(106.6), Y(29.4)); ctx.quadraticCurveTo(X(106), Y(26.9), X(103), Y(26.6)); ctx.lineTo(X(95), Y(26.6)); ctx.quadraticCurveTo(X(92.4), Y(27), X(91.9), Y(29)); ctx.lineTo(X(91.9), Y(31.8)); ctx.closePath(); ctx.fill(); });
  // d-pad
  const dp = (n: string) => Math.max(st.v(n), dpadAxis(st, n));
  lit(ctx, dp('DPadUp'), () => rect(25.9, 38.6, 6.5, 7.8));
  lit(ctx, dp('DPadDown'), () => rect(25.9, 51.1, 6.5, 7.9));
  lit(ctx, dp('DPadLeft'), () => rect(18.8, 45.7, 7.7, 6.5));
  lit(ctx, dp('DPadRight'), () => rect(31.3, 45.7, 8, 6.5));
  // create / options
  lit(ctx, st.v('Select'), () => bar(37, 33.9, 38.3, 38, 1.7));
  lit(ctx, st.v('Start'), () => bar(91.2, 33.9, 89.9, 38, 1.7));
  // ps button and mute
  lit(ctx, st.v('Mode'), () => rect(57.4, 54.6, 13.2, 4.6, 1.2));
  lit(ctx, st.v('Mute'), () => rect(60.2, 68.8, 7.6, 2.6, 1.1));
  // face buttons
  const face: [string, number, number, 'triangle' | 'circle' | 'cross' | 'square'][] = [['North', 99, 40, 'triangle'], ['East', 107, 48, 'circle'], ['South', 99, 56, 'cross'], ['West', 91, 48, 'square']];
  for (const [n, x, y] of face) lit(ctx, st.v(n), () => disc(x, y, 3.4));
  // stick caps light when clicked
  lit(ctx, st.v('LeftThumb'), () => disc(45.5, 64.3, 7.6));
  lit(ctx, st.v('RightThumb'), () => disc(82.5, 64.3, 7.6));

  // the outline itself, as drawn
  ctx.save();
  ctx.translate(OX, OY);
  ctx.scale(SC, SC);
  ctx.fillStyle = FG;
  for (const p of paths()) ctx.fill(p, 'evenodd');
  ctx.restore();

  for (const [n, x, y, g] of face) symbol(ctx, g, X(x), Y(y), st.v(n) > 0.5);

  // sticks: where each one is pointing
  const sticks: ['left' | 'right', number, string][] = [['left', 45.5, 'LeftThumb'], ['right', 82.5, 'RightThumb']];
  for (const [side, cx, click] of sticks) {
    const [sx, sy] = st.stick(side);
    const on = st.v(click) > 0.5;
    ctx.beginPath(); ctx.arc(X(cx + sx * 4.2), Y(64.3 - sy * 4.2), R(2.5), 0, Math.PI * 2);
    ctx.fillStyle = on ? '#000' : '#fff'; ctx.fill();
    ctx.lineWidth = 1.4; ctx.strokeStyle = on ? '#fff' : '#000'; ctx.stroke();
  }

  // triggers sit above the shoulders, and fill from the bottom the further they are pulled
  const trig: [string, string[], number][] = [['L2', ['LeftTrigger2', 'LeftZ'], 22], ['R2', ['RightTrigger2', 'RightZ'], 92]];
  for (const [label, names, x] of trig) {
    const v = best(st, names);
    const bx = X(x), by = Y(12.2), bw = R(14), bh = R(11);
    ctx.fillStyle = '#000'; ctx.fillRect(bx, by, bw, bh);
    ctx.fillStyle = '#fff'; ctx.fillRect(bx, by + bh * (1 - v), bw, bh * v);
    ctx.lineWidth = 1.6; ctx.strokeStyle = FG; ctx.strokeRect(bx, by, bw, bh);
    legend(ctx, label, bx + bw / 2, by + bh / 2, v > 0.5 ? 1 : 0, 12);
    if (v > 0.001) legend(ctx, `${Math.round(v * 100)}%`, bx + bw / 2, by - 6, 0, 9, 400);
  }
}

function dpadAxis(st: PadDrawState, n: string): number {
  const x = st.v('DPadX'), y = st.v('DPadY');
  if (n === 'DPadRight') return x > 0.5 ? 1 : 0;
  if (n === 'DPadLeft') return x < -0.5 ? 1 : 0;
  if (n === 'DPadUp') return y > 0.5 ? 1 : 0;
  return y < -0.5 ? 1 : 0;
}
