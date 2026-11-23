// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab · source viewable, not reusable: see LICENSE
// the whole js contract of the eye: feed it numbers, animate nothing.

export function initEyes(): () => void {
  if (matchMedia('(prefers-reduced-motion: reduce)').matches) return () => {};
  const root = document.documentElement;
  let raf = 0;
  let px = 0;
  let py = 0;
  let idleTimer = 0;

  const wake = () => {
    if (root.dataset.idle) delete root.dataset.idle;
    clearTimeout(idleTimer);
    idleTimer = window.setTimeout(() => (root.dataset.idle = 'asleep'), 25_000);
  };

  const flush = () => {
    raf = 0;
    root.style.setProperty('--mx', String(px));
    root.style.setProperty('--my', String(py));
    document.querySelectorAll<HTMLElement>('.eye').forEach((e) => {
      const r = e.getBoundingClientRect();
      e.style.setProperty('--jdx-ex', String(Math.round(r.left + r.width / 2)));
      e.style.setProperty('--jdx-ey', String(Math.round(r.top + r.height / 2)));
    });
  };

  const move = (ev: PointerEvent) => {
    px = ev.clientX;
    py = ev.clientY;
    wake();
    if (!raf) raf = requestAnimationFrame(flush);
  };

  addEventListener('pointermove', move, { passive: true });
  addEventListener('keydown', wake, { passive: true });
  wake();
  return () => {
    removeEventListener('pointermove', move);
    removeEventListener('keydown', wake);
    clearTimeout(idleTimer);
  };
}
