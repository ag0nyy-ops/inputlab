// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab · source viewable, not reusable: see LICENSE
// the easter egg: an eye that follows a controller stick. a stick that rests off-centre (drift)
// would make the eye stare into a corner, so the stick is checked first: the eye waits until the
// stick holds still, takes that resting spot as centre, and ignores anything inside a deadzone
// sized from how much the stick wobbled while it rested. a stick that never settles is left alone.

/** how long the stick must hold still before its resting spot is trusted */
const STILL_MS = 800;
/** widest wobble (per axis) that still counts as holding still */
const STILL_SPREAD = 0.04;
/** a stick resting further out than this is broken, not drifting: the eye ignores it */
const MAX_OFFSET = 0.45;
/** how long the eye waits for a stick to settle before it says so */
const GIVE_UP_MS = 5000;
const MIN_DEADZONE = 0.07;

export type GazeState = 'learning' | 'ready' | 'broken';

export class StickGaze {
  private win: [number, number, number][] = [];
  private cx = 0;
  private cy = 0;
  private dz = MIN_DEADZONE;
  private since = -1;
  state: GazeState = 'learning';
  /** ms the eye has been waiting for the stick to hold still (0 once it is ready) */
  waitedMs = 0;

  /** one sample per frame, including frames where the stick sent nothing: a perfectly still stick sends no events */
  feed(x: number, y: number, now: number): void {
    if (this.since < 0) this.since = now;
    this.win.push([now, x, y]);
    const keep = this.state === 'ready' ? 1500 : STILL_MS;
    while (this.win.length > 1 && now - this.win[0][0] > keep) this.win.shift();
    const span = now - this.win[0][0];
    const s = spread(this.win);

    if (this.state === 'ready') {
      // drift creeps: while the stick rests near its centre, follow the resting spot
      if (span >= 1400 && s.x < 0.03 && s.y < 0.03 && Math.hypot(s.mx - this.cx, s.my - this.cy) < 0.25) {
        this.cx = s.mx;
        this.cy = s.my;
      }
      return;
    }
    if (span >= STILL_MS - 50 && s.x < STILL_SPREAD && s.y < STILL_SPREAD) {
      if (Math.hypot(s.mx, s.my) > MAX_OFFSET) {
        this.state = 'broken';
        return;
      }
      this.cx = s.mx;
      this.cy = s.my;
      this.dz = MIN_DEADZONE + Math.max(s.x, s.y);
      this.state = 'ready';
      this.waitedMs = 0;
      return;
    }
    this.state = 'learning';
    this.waitedMs = now - this.since;
  }

  /** true once the stick has been waited on long enough that the eye should say it is ignoring it */
  get unsettled(): boolean {
    return this.state !== 'ready' && (this.state === 'broken' || this.waitedMs > GIVE_UP_MS);
  }

  /** where to look, -1..1 on both axes, y up. zero while learning, broken or inside the deadzone */
  look(x: number, y: number): [number, number] {
    if (this.state !== 'ready') return [0, 0];
    const dx = x - this.cx;
    const dy = y - this.cy;
    const r = Math.hypot(dx, dy);
    if (r <= this.dz) return [0, 0];
    const full = Math.max(0.3, 1 - this.dz - Math.hypot(this.cx, this.cy));
    const k = Math.min(1, (r - this.dz) / full) / r;
    return [dx * k, dy * k];
  }

  /** forget everything and check the stick again */
  reset(): void {
    this.win = [];
    this.state = 'learning';
    this.waitedMs = 0;
    this.since = -1;
  }
}

function spread(w: [number, number, number][]): { x: number; y: number; mx: number; my: number } {
  let x0 = Infinity, x1 = -Infinity, y0 = Infinity, y1 = -Infinity, sx = 0, sy = 0;
  for (const [, x, y] of w) {
    if (x < x0) x0 = x;
    if (x > x1) x1 = x;
    if (y < y0) y0 = y;
    if (y > y1) y1 = y;
    sx += x;
    sy += y;
  }
  return { x: x1 - x0, y: y1 - y0, mx: sx / w.length, my: sy / w.length };
}
