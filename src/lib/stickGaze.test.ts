// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab · source viewable, not reusable: see LICENSE
import { describe, expect, it } from 'vitest';
import { StickGaze } from './stickGaze';

/** feed a constant stick position for `ms`, 60 samples a second */
function hold(g: StickGaze, x: number, y: number, from: number, ms: number): number {
  let t = from;
  for (; t < from + ms; t += 16) g.feed(x, y, t);
  return t;
}

describe('StickGaze', () => {
  it('waits for the stick to settle before it looks anywhere', () => {
    const g = new StickGaze();
    g.feed(0.9, 0, 0);
    expect(g.state).toBe('learning');
    expect(g.look(0.9, 0)).toEqual([0, 0]);
  });

  it('a centred stick becomes ready and looks at nothing while resting', () => {
    const g = new StickGaze();
    const t = hold(g, 0, 0, 0, 1000);
    expect(g.state).toBe('ready');
    expect(g.look(0, 0)).toEqual([0, 0]);
    hold(g, 1, 0, t, 16);
    const [lx, ly] = g.look(1, 0);
    expect(lx).toBeCloseTo(1, 1);
    expect(ly).toBeCloseTo(0, 5);
  });

  it('a drifting stick (rests off-centre) is re-centred, not obeyed', () => {
    const g = new StickGaze();
    hold(g, 0.12, -0.08, 0, 1000);
    expect(g.state).toBe('ready');
    // its resting spot is the new centre: the eye looks straight ahead
    expect(g.look(0.12, -0.08)).toEqual([0, 0]);
    // a real push still moves the eye
    const [lx] = g.look(1, -0.08);
    expect(lx).toBeGreaterThan(0.9);
  });

  it('a wobble inside the deadzone is ignored', () => {
    const g = new StickGaze();
    hold(g, 0.1, 0.1, 0, 1000);
    expect(g.look(0.14, 0.1)).toEqual([0, 0]);
  });

  it('a stick resting far from centre is marked broken and ignored', () => {
    const g = new StickGaze();
    hold(g, 0.8, 0.3, 0, 1000);
    expect(g.state).toBe('broken');
    expect(g.unsettled).toBe(true);
    expect(g.look(0.8, 0.3)).toEqual([0, 0]);
  });

  it('a stick that never holds still stays unready and reports it after a while', () => {
    const g = new StickGaze();
    for (let t = 0; t < 7000; t += 16) g.feed(Math.sin(t / 200) * 0.3, 0, t);
    expect(g.state).toBe('learning');
    expect(g.unsettled).toBe(true);
    expect(g.look(0.3, 0)).toEqual([0, 0]);
  });

  it('a stick that settles later becomes ready', () => {
    const g = new StickGaze();
    let t = 0;
    for (; t < 3000; t += 16) g.feed(Math.sin(t / 200) * 0.5, 0, t);
    expect(g.state).toBe('learning');
    hold(g, 0.05, 0, t, 1000);
    expect(g.state).toBe('ready');
    expect(g.unsettled).toBe(false);
  });

  it('follows a drift that creeps while the stick rests', () => {
    const g = new StickGaze();
    let t = hold(g, 0, 0, 0, 1000);
    t = hold(g, 0.1, 0, t, 2000);
    expect(g.look(0.1, 0)).toEqual([0, 0]);
  });

  it('output stays inside the unit circle', () => {
    const g = new StickGaze();
    hold(g, 0, 0, 0, 1000);
    const [x, y] = g.look(1, 1);
    expect(Math.hypot(x, y)).toBeLessThanOrEqual(1.0001);
  });

  it('reset checks the stick again', () => {
    const g = new StickGaze();
    hold(g, 0, 0, 0, 1000);
    g.reset();
    expect(g.state).toBe('learning');
  });
});
