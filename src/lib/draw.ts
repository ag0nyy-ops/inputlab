// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab · source viewable, not reusable: see LICENSE
// canvas helpers. the test surface rule everywhere: pure black = no input,
// pure white = active, analog values scale continuously between them.

export const FG = '#ececec';
export const BG = '#060606';

function mix(a: [number, number, number], b: [number, number, number], t: number): string {
  const c = a.map((v, i) => Math.round(v * t + b[i]! * (1 - t)));
  return `rgb(${c[0]} ${c[1]} ${c[2]})`;
}
const FG_RGB: [number, number, number] = [0xec, 0xec, 0xec];
const BG_RGB: [number, number, number] = [0x06, 0x06, 0x06];

export const LINE = mix(FG_RGB, BG_RGB, 0.13);
export const LINE_STRONG = mix(FG_RGB, BG_RGB, 0.32);
export const MUTED = mix(FG_RGB, BG_RGB, 0.74);

export const MONO = '"IBM Plex Mono", ui-monospace, Consolas, monospace';

/** brightness 0..1 -> a grey from pure black to pure white */
export function shade(b: number): string {
  const v = Math.round(Math.max(0, Math.min(1, b)) * 255);
  return `rgb(${v} ${v} ${v})`;
}

export interface Sized {
  ctx: CanvasRenderingContext2D;
  w: number;
  h: number;
  dpr: number;
}

/** size the backing store for the device pixel ratio; returns a drawing context in css px */
export function fitCanvas(canvas: HTMLCanvasElement, w: number, h: number): Sized {
  const dpr = Math.min(window.devicePixelRatio || 1, 3);
  const bw = Math.max(1, Math.round(w * dpr));
  const bh = Math.max(1, Math.round(h * dpr));
  if (canvas.width !== bw || canvas.height !== bh) {
    canvas.width = bw;
    canvas.height = bh;
  }
  canvas.style.width = `${w}px`;
  canvas.style.height = `${h}px`;
  const ctx = canvas.getContext('2d')!;
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  return { ctx, w, h, dpr };
}

/**
 * legend that stays readable on both states: dark text on light fills, light on
 * dark, and a contrasting halo in the middle where neither would reach 4.5:1.
 */
export function legend(ctx: CanvasRenderingContext2D, text: string, x: number, y: number, brightness: number, px: number, weight = 600): void {
  if (!text) return;
  ctx.font = `${weight} ${px}px ${MONO}`;
  ctx.textAlign = 'center';
  ctx.textBaseline = 'middle';
  const light = brightness < 0.5;
  if (brightness > 0.3 && brightness < 0.7) {
    ctx.lineWidth = Math.max(2, px * 0.28);
    ctx.lineJoin = 'round';
    ctx.strokeStyle = light ? '#000' : '#fff';
    ctx.strokeText(text, x, y);
  }
  ctx.fillStyle = light ? FG : '#000';
  ctx.fillText(text, x, y);
}

export function pathFrom(d: string): Path2D {
  return new Path2D(d);
}
