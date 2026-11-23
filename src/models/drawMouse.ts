// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab · source viewable, not reusable: see LICENSE
import { FG, LINE_STRONG, MUTED, legend, shade } from '../lib/draw';
import { SHAPES, SHAPE_ASPECT, type ShapeId } from './mouseShapes';

export interface MouseDrawState {
  /** brightness 0..1 for buttons 1..5 (left, right, middle, back, forward) */
  b: [number, number, number, number, number];
  wheel: { up: number; down: number; left: number; right: number };
}

export const idleMouse = (): MouseDrawState => ({ b: [0, 0, 0, 0, 0], wheel: { up: 0, down: 0, left: 0, right: 0 } });

const bodies = new Map<ShapeId, Path2D>();
const body = (id: ShapeId): Path2D => {
  let p = bodies.get(id);
  if (!p) bodies.set(id, (p = new Path2D(SHAPES[id].body)));
  return p;
};

export function mouseHeight(w: number): number {
  return Math.round(w * SHAPE_ASPECT);
}

/** draw a mouse into the box at (x, y) that is `w` css px wide */
export function drawMouse(ctx: CanvasRenderingContext2D, id: ShapeId, x: number, y: number, w: number, st: MouseDrawState, labels = true): void {
  const sh = SHAPES[id];
  const cx = sh.wheel.x + sh.wheel.w / 2;
  ctx.save();
  ctx.translate(x, y);
  ctx.scale(w / 200, w / 200);

  const p = body(id);
  ctx.fillStyle = '#000';
  ctx.fill(p);

  // click buttons, wheel: clipped to the body so the outline decides their shape
  ctx.save();
  ctx.clip(p);
  ctx.lineWidth = 1.5;
  ctx.strokeStyle = LINE_STRONG;
  const gl = cx - sh.wheel.w / 2 - 2;
  const gr = cx + sh.wheel.w / 2 + 2;
  ctx.fillStyle = shade(st.b[0]);
  ctx.fillRect(0, 0, gl, sh.split);
  ctx.strokeRect(0, 0, gl, sh.split);
  ctx.fillStyle = shade(st.b[1]);
  ctx.fillRect(gr, 0, 200 - gr, sh.split);
  ctx.strokeRect(gr, 0, 200 - gr, sh.split);
  const wb = sh.wheel;
  ctx.fillStyle = shade(st.b[2]);
  ctx.fillRect(wb.x, wb.y, wb.w, wb.h);
  ctx.strokeStyle = FG;
  ctx.strokeRect(wb.x, wb.y, wb.w, wb.h);
  ctx.restore();

  ctx.lineWidth = 2.5;
  ctx.strokeStyle = FG;
  ctx.stroke(p);

  // decoration that is not an input: dashed
  ctx.setLineDash([3, 3]);
  ctx.lineWidth = 1;
  ctx.strokeStyle = MUTED;
  for (const e of sh.extras ?? []) ctx.strokeRect(e.x, e.y, e.w, e.h);
  ctx.setLineDash([]);

  // side buttons
  for (const s of sh.sides) {
    const b = st.b[s.n - 1]!;
    ctx.fillStyle = shade(b);
    ctx.fillRect(s.box.x, s.box.y, s.box.w, s.box.h);
    ctx.lineWidth = 1.5;
    ctx.strokeStyle = FG;
    ctx.strokeRect(s.box.x, s.box.y, s.box.w, s.box.h);
  }

  // wheel scroll / tilt indicators
  const tri = (px: number, py: number, dir: 'u' | 'd' | 'l' | 'r', b: number) => {
    const a = 6;
    ctx.beginPath();
    if (dir === 'u') { ctx.moveTo(px, py - a); ctx.lineTo(px + a, py + a * 0.6); ctx.lineTo(px - a, py + a * 0.6); }
    if (dir === 'd') { ctx.moveTo(px, py + a); ctx.lineTo(px + a, py - a * 0.6); ctx.lineTo(px - a, py - a * 0.6); }
    if (dir === 'l') { ctx.moveTo(px - a, py); ctx.lineTo(px + a * 0.6, py - a); ctx.lineTo(px + a * 0.6, py + a); }
    if (dir === 'r') { ctx.moveTo(px + a, py); ctx.lineTo(px - a * 0.6, py - a); ctx.lineTo(px - a * 0.6, py + a); }
    ctx.closePath();
    ctx.fillStyle = shade(b);
    ctx.fill();
    ctx.lineWidth = 1.2;
    ctx.strokeStyle = FG;
    ctx.stroke();
  };
  const wy = wb.y + wb.h / 2;
  tri(cx, wb.y - 11, 'u', st.wheel.up);
  tri(cx, wb.y + wb.h + 11, 'd', st.wheel.down);
  tri(cx - wb.w / 2 - 13, wy, 'l', st.wheel.left);
  tri(cx + wb.w / 2 + 13, wy, 'r', st.wheel.right);

  if (labels) {
    legend(ctx, '1', cx - 30, sh.split * 0.5, st.b[0], 15);
    legend(ctx, '2', cx + 30, sh.split * 0.5, st.b[1], 15);
    legend(ctx, '3', cx, wy, st.b[2], 12);
    for (const s of sh.sides) legend(ctx, String(s.n), s.box.x + s.box.w / 2, s.box.y + s.box.h / 2, st.b[s.n - 1]!, 11);
  }
  ctx.restore();
}
