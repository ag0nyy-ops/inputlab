// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab · source viewable, not reusable: see LICENSE
// live hex dump of the newest input report. a byte that changed flashes white and
// fades; the canvas redraws only while something is flashing.
import { forwardRef, useEffect, useImperativeHandle, useRef } from 'react';
import { MONO, MUTED, fitCanvas, legend, shade } from '../lib/draw';
import { addFrame } from '../lib/raf';

export interface HexDumpHandle {
  push(report: number[], changed: number[]): void;
  clear(): void;
}

const FLASH_MS = 700;
const COLS = 16;
const MAX = 256;

export const HexDump = forwardRef<HexDumpHandle, { ariaLabel?: string }>(function HexDump({ ariaLabel = 'raw input report bytes' }, ref) {
  const cv = useRef<HTMLCanvasElement>(null);
  const box = useRef<HTMLDivElement>(null);
  const st = useRef({ bytes: [] as number[], at: [] as number[], dirty: true });

  useImperativeHandle(ref, () => ({
    push(report, changed) {
      const s = st.current;
      const now = performance.now();
      s.bytes = report.slice(0, MAX);
      for (const i of changed) if (i < MAX) s.at[i] = now;
      s.dirty = true;
    },
    clear() {
      st.current = { bytes: [], at: [], dirty: true };
    },
  }));

  useEffect(() => {
    let w = 0;
    const ro = new ResizeObserver(() => { w = Math.floor(box.current!.clientWidth); st.current.dirty = true; });
    ro.observe(box.current!);
    w = Math.floor(box.current!.clientWidth);
    const off = addFrame((now) => {
      const s = st.current;
      const flashing = s.at.some((t) => t !== undefined && now - t < FLASH_MS);
      if (!s.dirty && !flashing) return;
      s.dirty = false;
      if (w < 100) return;
      const cell = Math.max(28, Math.min(40, Math.floor((w - 8) / COLS)));
      const rows = Math.max(1, Math.ceil(Math.max(s.bytes.length, 8) / COLS));
      const { ctx } = fitCanvas(cv.current!, cell * COLS + 8, rows * (cell * 0.8) + 8);
      ctx.fillStyle = '#000';
      ctx.fillRect(0, 0, cell * COLS + 8, rows * cell * 0.8 + 8);
      const ch = cell * 0.8;
      if (!s.bytes.length) {
        ctx.fillStyle = MUTED; ctx.font = `400 11px ${MONO}`; ctx.textBaseline = 'middle'; ctx.textAlign = 'left';
        ctx.fillText('no report yet. press or move something on the device.', 10, 18);
        return;
      }
      s.bytes.forEach((b, i) => {
        const x = 4 + (i % COLS) * cell, y = 4 + Math.floor(i / COLS) * ch;
        const t = s.at[i];
        const f = t === undefined ? 0 : Math.max(0, 1 - (now - t) / FLASH_MS);
        ctx.fillStyle = shade(f);
        ctx.fillRect(x + 1, y + 1, cell - 2, ch - 2);
        ctx.strokeStyle = f > 0.02 ? '#fff' : 'rgb(236 236 236 / .25)';
        ctx.lineWidth = 1;
        ctx.strokeRect(x + 1.5, y + 1.5, cell - 3, ch - 3);
        legend(ctx, b.toString(16).padStart(2, '0'), x + cell / 2, y + ch / 2, f, Math.min(13, cell * 0.4));
      });
      if (flashing) s.dirty = true;
    });
    return () => { off(); ro.disconnect(); };
  }, []);

  return <div ref={box} className="hexdump"><canvas ref={cv} role="img" aria-label={ariaLabel} /></div>;
});
