// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab · source viewable, not reusable: see LICENSE
// turns a controller's raw gyro and accelerometer readings into something to look at. like the
// stick eye, it checks the sensor first: "zero" asks for one second of holding still, takes the
// gyro's resting offset (it is never exactly zero) and the resting direction of gravity as the
// reference, and only then reports turning and tilting relative to that.

export type V3 = [number, number, number];

const SETTLE_MS = 1000;
/** the pad counts as moving, and the settling restarts, above this many degrees per second */
const STILL_DPS = 20;

const mag = (v: V3): number => Math.hypot(v[0], v[1], v[2]);

export class MotionTracker {
  /** degrees turned about the yaw axis since zero (drifts slowly: the gyro has no compass) */
  heading = 0;
  /** gyro rates with the resting offset removed, degrees per second: pitch, yaw, roll */
  rates: V3 = [0, 0, 0];
  /** degrees the pad is tipped away from where it lay at zero */
  tiltDeg = 0;
  /** which way a ball would roll on the tipped pad, -1..1 on the two axes that lay flat at zero */
  ball: [number, number] = [0, 0];
  /** true while it is waiting for the pad to hold still */
  settling = true;
  /** the accelerometer in g, as read */
  accel: V3 = [0, 0, 0];

  private bias: V3 = [0, 0, 0];
  private ref: V3 = [0, 0, 1];
  private up = 2;
  private n = 0;
  private sumG: V3 = [0, 0, 0];
  private sumA: V3 = [0, 0, 0];
  private since = -1;
  private last = -1;

  /** check the pad again: hold it still for a second */
  zero(): void {
    this.settling = true;
    this.n = 0;
    this.sumG = [0, 0, 0];
    this.sumA = [0, 0, 0];
    this.since = -1;
    this.heading = 0;
  }

  feed(gyro: V3, accel: V3, now: number): void {
    this.accel = accel;
    const dt = this.last < 0 ? 0 : Math.min(0.1, (now - this.last) / 1000);
    this.last = now;

    if (this.settling) {
      if (mag(gyro) > STILL_DPS) {
        this.n = 0; // it moved: start the second again
        this.sumG = [0, 0, 0];
        this.sumA = [0, 0, 0];
        this.since = -1;
        this.rates = [0, 0, 0];
        return;
      }
      if (this.since < 0) this.since = now;
      this.n++;
      for (let i = 0; i < 3; i++) {
        this.sumG[i]! += gyro[i]!;
        this.sumA[i]! += accel[i]!;
      }
      if (now - this.since >= SETTLE_MS && this.n >= 5) {
        this.bias = this.sumG.map((v) => v / this.n) as V3;
        const a = this.sumA.map((v) => v / this.n) as V3;
        const m = mag(a) || 1;
        this.ref = a.map((v) => v / m) as V3;
        this.up = this.ref.map(Math.abs).indexOf(Math.max(...this.ref.map(Math.abs)));
        this.settling = false;
        this.heading = 0;
      }
      this.rates = [0, 0, 0];
      return;
    }

    this.rates = [gyro[0] - this.bias[0], gyro[1] - this.bias[1], gyro[2] - this.bias[2]];
    this.heading += this.rates[1] * dt;

    const m = mag(accel) || 1;
    const g = accel.map((v) => v / m) as V3;
    const dot = Math.max(-1, Math.min(1, g[0] * this.ref[0] + g[1] * this.ref[1] + g[2] * this.ref[2]));
    this.tiltDeg = (Math.acos(dot) * 180) / Math.PI;
    const [a, b] = [0, 1, 2].filter((i) => i !== this.up) as [number, number];
    // a ball rolls downhill, away from where the accelerometer's reading grew
    this.ball = [clamp(-(g[a]! - this.ref[a]!) / 0.5), clamp(-(g[b]! - this.ref[b]!) / 0.5)];
  }
}

const clamp = (v: number): number => Math.max(-1, Math.min(1, v));
