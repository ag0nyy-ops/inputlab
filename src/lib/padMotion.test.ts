// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab · source viewable, not reusable: see LICENSE
import { describe, expect, it } from 'vitest';
import { MotionTracker, type V3 } from './padMotion';

/** feed the same reading for `ms`, 100 samples a second */
function hold(m: MotionTracker, gyro: V3, accel: V3, from: number, ms: number): number {
  let t = from;
  for (; t < from + ms; t += 10) m.feed(gyro, accel, t);
  return t;
}

describe('MotionTracker', () => {
  it('waits for the pad to hold still, and restarts if it is moved meanwhile', () => {
    const m = new MotionTracker();
    let t = hold(m, [0, 0, 0], [0, 0, 1], 0, 600);
    expect(m.settling).toBe(true);
    t = hold(m, [0, 300, 0], [0, 0, 1], t, 50); // knocked
    t = hold(m, [0, 0, 0], [0, 0, 1], t, 600);
    expect(m.settling).toBe(true);
    hold(m, [0, 0, 0], [0, 0, 1], t, 500);
    expect(m.settling).toBe(false);
  });

  it("removes the gyro's resting offset, so a pad lying still does not slowly turn", () => {
    const m = new MotionTracker();
    let t = hold(m, [1.5, -2, 0.5], [0, 0, 1], 0, 1200);
    expect(m.settling).toBe(false);
    t = hold(m, [1.5, -2, 0.5], [0, 0, 1], t, 5000);
    expect(Math.abs(m.heading)).toBeLessThan(0.5);
    expect(Math.abs(m.rates[1])).toBeLessThan(0.01);
  });

  it('adds up a real turn into a heading', () => {
    const m = new MotionTracker();
    const t = hold(m, [0, 0, 0], [0, 0, 1], 0, 1200);
    hold(m, [0, 90, 0], [0, 0, 1], t, 1000); // 90 degrees per second for one second
    expect(m.heading).toBeGreaterThan(85);
    expect(m.heading).toBeLessThan(95);
  });

  it('measures how far the pad is tipped from where it lay at zero, whichever way up it lay', () => {
    for (const flat of [[0, 0, 1], [0, 1, 0], [0.02, 0.99, 0.1]] as V3[]) {
      const m = new MotionTracker();
      const t = hold(m, [0, 0, 0], flat, 0, 1200);
      hold(m, [0, 0, 0], flat, t, 20);
      expect(m.tiltDeg).toBeLessThan(0.5);
      expect(Math.hypot(...m.ball)).toBeLessThan(0.05);
    }
    // tipped 30 degrees about the first axis, from lying with axis 3 up
    const m = new MotionTracker();
    const t = hold(m, [0, 0, 0], [0, 0, 1], 0, 1200);
    const a = (30 * Math.PI) / 180;
    hold(m, [0, 0, 0], [Math.sin(a), 0, Math.cos(a)], t, 20);
    expect(m.tiltDeg).toBeGreaterThan(29);
    expect(m.tiltDeg).toBeLessThan(31);
    expect(Math.abs(m.ball[0])).toBeGreaterThan(0.5);
    expect(Math.abs(m.ball[1])).toBeLessThan(0.05);
  });

  it('zero checks the pad again', () => {
    const m = new MotionTracker();
    hold(m, [0, 0, 0], [0, 0, 1], 0, 1200);
    m.zero();
    expect(m.settling).toBe(true);
    expect(m.heading).toBe(0);
  });
});
