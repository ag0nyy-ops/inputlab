// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab · source viewable, not reusable: see LICENSE
// the one requestAnimationFrame loop. views register a draw function; the loop
// runs only while something is registered.

type Draw = (now: number) => void;
const subs = new Set<Draw>();
let handle = 0;

function tick(now: number): void {
  subs.forEach((d) => d(now));
  handle = subs.size ? requestAnimationFrame(tick) : 0;
}

export function addFrame(d: Draw): () => void {
  subs.add(d);
  if (!handle) handle = requestAnimationFrame(tick);
  return () => {
    subs.delete(d);
  };
}
