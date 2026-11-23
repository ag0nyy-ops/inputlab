// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab · source viewable, not reusable: see LICENSE
import { FG, LINE_STRONG, MONO, MUTED, legend, shade } from '../lib/draw';
import { BODIES, padElements, type PadEl } from './padLayouts';
import type { PadMatch } from './pads';

export const PAD_W = 400;
export const PAD_H = 280;

export interface PadDrawState {
  /** gilrs control name -> 0..1 (buttons, analog triggers) */
  v: (name: string) => number;
  /** stick position, -1..1, y up */
  stick: (side: 'left' | 'right') => [number, number];
}

const bodies = new Map<string, Path2D>();
const bodyPath = (b: PadMatch['body']): Path2D => {
  let p = bodies.get(b);
  if (!p) bodies.set(b, (p = new Path2D(BODIES[b])));
  return p;
};

const first = (st: PadDrawState, names: string[]): number => Math.max(...names.map((n) => st.v(n)));

function glyph(ctx: CanvasRenderingContext2D, g: string, x: number, y: number, r: number, b: number): void {
  const ink = b > 0.5 ? '#000' : FG;
  ctx.strokeStyle = ink;
  ctx.fillStyle = ink;
  ctx.lineWidth = 2;
  const s = r * 0.5;
  ctx.beginPath();
  if (g === 'cross') { ctx.moveTo(x - s, y - s); ctx.lineTo(x + s, y + s); ctx.moveTo(x + s, y - s); ctx.lineTo(x - s, y + s); ctx.stroke(); return; }
  if (g === 'circle') { ctx.arc(x, y, s * 1.05, 0, Math.PI * 2); ctx.stroke(); return; }
  if (g === 'square') { ctx.rect(x - s, y - s, s * 2, s * 2); ctx.stroke(); return; }
  if (g === 'triangle') { ctx.moveTo(x, y - s * 1.15); ctx.lineTo(x + s * 1.1, y + s * 0.8); ctx.lineTo(x - s * 1.1, y + s * 0.8); ctx.closePath(); ctx.stroke(); return; }
  legend(ctx, g, x, y, b, r * 1.15);
}

/** draw a controller at (0,0) scaled to `w` css px wide */
export function drawPad(ctx: CanvasRenderingContext2D, match: PadMatch, w: number, st: PadDrawState): void {
  const sc = w / PAD_W;
  ctx.save();
  ctx.scale(sc, sc);

  const els = padElements(match.style, match.model);

  // body first: the triggers sit above it
  const bp = bodyPath(match.body);
  ctx.fillStyle = '#000';
  ctx.fill(bp);
  ctx.lineWidth = 2.5;
  ctx.strokeStyle = FG;
  ctx.stroke(bp);

  for (const e of els) drawEl(ctx, e, st);
  ctx.restore();
}

function drawEl(ctx: CanvasRenderingContext2D, e: PadEl, st: PadDrawState): void {
  ctx.lineWidth = 1.6;
  ctx.strokeStyle = FG;
  switch (e.k) {
    case 'trigger': {
      const v = first(st, e.names);
      ctx.fillStyle = '#000';
      ctx.fillRect(e.x, e.y, e.w, e.h);
      // analog trigger: fills from the bottom up with how far it is pulled
      ctx.fillStyle = '#fff';
      ctx.fillRect(e.x, e.y + e.h * (1 - v), e.w, e.h * v);
      ctx.strokeRect(e.x, e.y, e.w, e.h);
      legend(ctx, e.label, e.x + e.w / 2, e.y + e.h / 2, v > 0.5 ? 1 : 0, 12);
      if (v > 0.001) legend(ctx, `${Math.round(v * 100)}%`, e.x + e.w / 2, e.y - 6, 0, 9, 400);
      break;
    }
    case 'pill': {
      const v = first(st, e.names);
      ctx.fillStyle = shade(v);
      ctx.fillRect(e.x, e.y, e.w, e.h);
      ctx.strokeRect(e.x, e.y, e.w, e.h);
      if (e.label) legend(ctx, e.label, e.x + e.w / 2, e.y + e.h / 2, v, 10);
      break;
    }
    case 'round': {
      const v = first(st, e.names);
      ctx.beginPath(); ctx.arc(e.x, e.y, e.r, 0, Math.PI * 2);
      ctx.fillStyle = shade(v); ctx.fill(); ctx.stroke();
      if (e.label) legend(ctx, e.label, e.x, e.y, v, 8);
      break;
    }
    case 'face': {
      const v = first(st, e.names);
      ctx.beginPath(); ctx.arc(e.x, e.y, e.r, 0, Math.PI * 2);
      ctx.fillStyle = shade(v); ctx.fill(); ctx.stroke();
      glyph(ctx, e.glyph, e.x, e.y, e.r, v);
      break;
    }
    case 'dpad': {
      const arm = 11, len = 15;
      const dirs: [string, number, number, number, number][] = [
        ['DPadUp', -arm / 2, -arm / 2 - len, arm, len], ['DPadDown', -arm / 2, arm / 2, arm, len],
        ['DPadLeft', -arm / 2 - len, -arm / 2, len, arm], ['DPadRight', arm / 2, -arm / 2, len, arm],
      ];
      for (const [n, dx, dy, w, h] of dirs) {
        const v = n === 'DPadUp' || n === 'DPadDown' || n === 'DPadLeft' || n === 'DPadRight' ? Math.max(st.v(n), dpadAxis(st, n)) : 0;
        ctx.fillStyle = shade(v);
        ctx.fillRect(e.x + dx, e.y + dy, w, h);
        ctx.strokeRect(e.x + dx, e.y + dy, w, h);
      }
      ctx.fillStyle = '#000'; ctx.fillRect(e.x - arm / 2 + 1, e.y - arm / 2 + 1, arm - 2, arm - 2);
      break;
    }
    case 'stick': {
      const [x, y] = st.stick(e.side);
      const pressed = st.v(e.click);
      ctx.beginPath(); ctx.arc(e.x, e.y, e.r, 0, Math.PI * 2);
      ctx.fillStyle = shade(pressed); ctx.fill(); ctx.stroke();
      ctx.strokeStyle = LINE_STRONG; ctx.lineWidth = 1;
      ctx.beginPath(); ctx.arc(e.x, e.y, e.r * 0.55, 0, Math.PI * 2); ctx.stroke();
      // live position of the stick
      const dx = e.x + x * e.r * 0.78, dy = e.y - y * e.r * 0.78;
      ctx.beginPath(); ctx.arc(dx, dy, e.r * 0.34, 0, Math.PI * 2);
      ctx.fillStyle = pressed > 0.5 ? '#000' : '#fff'; ctx.fill();
      ctx.lineWidth = 1.5; ctx.strokeStyle = pressed > 0.5 ? '#fff' : '#000'; ctx.stroke();
      break;
    }
    case 'path': {
      ctx.setLineDash(e.dash ? [4, 4] : []);
      ctx.strokeStyle = e.dash ? MUTED : FG;
      ctx.lineWidth = e.dash ? 1 : 2;
      ctx.stroke(new Path2D(e.d));
      ctx.setLineDash([]);
      break;
    }
    case 'decor': {
      ctx.setLineDash([3, 3]); ctx.strokeStyle = MUTED; ctx.lineWidth = 1;
      ctx.strokeRect(e.x, e.y, e.w, e.h); ctx.setLineDash([]);
      if (e.label) { ctx.font = `400 8px ${MONO}`; ctx.fillStyle = MUTED; ctx.textAlign = 'center'; ctx.textBaseline = 'middle'; ctx.fillText(e.label, e.x + e.w / 2, e.y + e.h / 2); }
      break;
    }
  }
}

function dpadAxis(st: PadDrawState, n: string): number {
  // some pads report the hat as DPadX / DPadY axes (-1..1)
  const x = st.v('DPadX'), y = st.v('DPadY');
  if (n === 'DPadRight') return x > 0.5 ? 1 : 0;
  if (n === 'DPadLeft') return x < -0.5 ? 1 : 0;
  if (n === 'DPadUp') return y > 0.5 ? 1 : 0;
  return y < -0.5 ? 1 : 0;
}
