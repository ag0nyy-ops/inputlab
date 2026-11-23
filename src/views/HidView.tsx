// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab
// anything hidapi can open: remotes, flight sticks, wheels, pedals, arcade sticks, tablets, odd gadgets.
// buttons are black/white squares, axes are bars, and below it all is the raw hex of every report.
import { useEffect, useRef, useState } from 'react';
import { HexDump, type HexDumpHandle } from '../components/HexDump';
import { Stamp } from '../components/Stamp';
import { FG, LINE_STRONG, fitCanvas, legend, shade } from '../lib/draw';
import { addFrame } from '../lib/raf';
import { results } from '../lib/results';
import { onFrame } from '../lib/session';
import type { PollSnap, SessionInfo } from '../lib/types';

const GLOW = 120;

export function HidView({ info }: { info: SessionInfo }) {
  const layout = info.hid?.layout ?? null;
  const err = info.hid?.error ?? null;
  const dump = useRef<HexDumpHandle>(null);
  const [poll, setPoll] = useState<PollSnap | null>(null);
  const [meta, setMeta] = useState<{ id: number | null; len: number }>({ id: null, len: 0 });
  const snap = useRef<PollSnap | undefined>(undefined);
  const btnCv = useRef<HTMLCanvasElement>(null);
  const btnBox = useRef<HTMLDivElement>(null);
  const axisEls = useRef<(HTMLSpanElement | null)[]>([]);
  const axisTxt = useRef<(HTMLSpanElement | null)[]>([]);
  const live = useRef({ buttons: [] as boolean[], upAt: [] as number[], everDown: new Set<number>(), dirty: true });

  useEffect(() => {
    const l = live.current;
    const off = onFrame((events) => {
      for (const e of events) {
        if (e.t === 'poll') snap.current = e;
        if (e.t !== 'hid') continue;
        dump.current?.push(e.report, e.changed);
        e.buttons.forEach((b, i) => {
          if (l.buttons[i] && !b) l.upAt[i] = performance.now();
          if (b) l.everDown.add(i);
          l.buttons[i] = b;
        });
        e.axes.forEach((v, i) => {
          const el = axisEls.current[i];
          if (el) el.style.width = `${Math.round(v * 100)}%`;
          const t = axisTxt.current[i];
          if (t) t.textContent = v.toFixed(3);
        });
        l.dirty = true;
        setMeta((m) => (m.len === e.report.length && m.id === e.id ? m : { id: e.id, len: e.report.length }));
      }
    });
    const t = window.setInterval(() => { if (snap.current) { setPoll(snap.current); snap.current = undefined; } }, 150);
    return () => { off(); clearInterval(t); };
  }, []);

  // button squares
  useEffect(() => {
    const box = btnBox.current;
    if (!box || !layout) return;
    let w = 0;
    const ro = new ResizeObserver(() => { w = Math.floor(box.clientWidth); live.current.dirty = true; });
    ro.observe(box);
    w = Math.floor(box.clientWidth);
    const off = addFrame((now) => {
      const l = live.current;
      const fading = l.upAt.some((t) => t !== undefined && now - t < GLOW);
      if (!l.dirty && !fading) return;
      l.dirty = false;
      if (w < 60 || !btnCv.current) return;
      const n = layout.buttons.length;
      const cell = 44;
      const cols = Math.max(1, Math.floor(w / (cell + 6)));
      const rows = Math.ceil(n / cols);
      const { ctx } = fitCanvas(btnCv.current, w, Math.max(1, rows * (cell + 6)));
      ctx.clearRect(0, 0, w, rows * (cell + 6));
      layout.buttons.forEach((name, i) => {
        const x = (i % cols) * (cell + 6), y = Math.floor(i / cols) * (cell + 6);
        const down = l.buttons[i];
        const b = down ? 1 : l.upAt[i] !== undefined ? Math.max(0, 1 - (now - l.upAt[i]!) / GLOW) : 0;
        ctx.fillStyle = shade(b); ctx.fillRect(x, y, cell, cell);
        ctx.setLineDash(l.everDown.has(i) ? [] : [3, 3]);
        ctx.strokeStyle = l.everDown.has(i) ? FG : LINE_STRONG; ctx.lineWidth = 1; ctx.strokeRect(x + 0.5, y + 0.5, cell - 1, cell - 1);
        ctx.setLineDash([]);
        const short = name.startsWith('button ') ? name.slice(7) : name.replace('hat ', '');
        legend(ctx, short, x + cell / 2, y + cell / 2, b, short.length > 3 ? 9 : 13);
      });
      if (fading) l.dirty = true;
    });
    return () => { off(); ro.disconnect(); };
  }, [layout]);

  useEffect(() => {
    results.set('hid', { device: info.device.name, descriptorParsed: layout?.parsed ?? false, buttons: layout?.buttons.length ?? 0, axes: layout?.axes.map((a) => a.name) ?? [], polling: poll && { avgHz: poll.avgHz, minHz: poll.minHz, maxHz: poll.maxHz, nominal: poll.nominal } });
  }, [info.device.name, layout, poll]);

  const f0 = (n: number | null | undefined) => (n == null ? '-' : Math.round(n).toLocaleString());

  return (
    <div className="hv">
      {err && (
        <div className="banner banner--problem">
          <Stamp tone="problem" size="sm">no signal</Stamp>
          <div>
            <p className="banner__t">this device could not be opened</p>
            <p className="banner__p">{err}. keyboards, mice and some pads are owned by the os and refuse a second reader; on linux, hidraw needs permission (a udev rule or the input group).</p>
          </div>
          <span />
        </div>
      )}

      {layout?.parsed ? (
        <section className="card">
          <p className="label">buttons · {layout.buttons.length}</p>
          <div ref={btnBox} className="hv__buttons"><canvas ref={btnCv} role="img" aria-label="device buttons, white while pressed" /></div>
          {layout.axes.length > 0 && (
            <>
              <p className="label" style={{ marginTop: '1rem' }}>axes · {layout.axes.length}</p>
              <ul className="hv__axes">
                {layout.axes.map((a, i) => (
                  <li key={`${a.name}-${i}`}>
                    <span className="mono">{a.name}</span>
                    <span className="hv__axis" role="meter" aria-label={a.name} aria-valuemin={0} aria-valuemax={1}><span ref={(el) => { axisEls.current[i] = el; }} /></span>
                    <span className="num" ref={(el) => { axisTxt.current[i] = el; }}>0.000</span>
                  </li>
                ))}
              </ul>
            </>
          )}
        </section>
      ) : (
        !err && (
          <div className="banner">
            <Stamp tone="wip" size="sm">raw only</Stamp>
            <p className="banner__p">{layout ? 'the report descriptor had no buttons or axes inputlab understands' : 'the os did not give up a report descriptor for this device'}, so there are no labelled controls. the raw bytes below are still live.</p>
            <span />
          </div>
        )
      )}

      <section className="card">
        <p className="label">raw input report · {meta.len ? `${meta.len} bytes${meta.id !== null ? ` · report id ${meta.id}` : ''}` : 'waiting'} · changed bytes flash</p>
        <HexDump ref={dump} />
        <dl className="kv mv__stats" style={{ marginTop: '.8rem' }}>
          <dt>reports</dt><dd className="num">{poll?.reports.toLocaleString() ?? 0}</dd>
          <dt>rate</dt><dd className="num">{f0(poll?.hz ?? poll?.avgHz)} hz{poll?.nominal ? ` · ≈ ${poll.nominal}` : ''}</dd>
          <dt>interval</dt><dd className="num">{poll?.intervalAvgUs != null ? `${poll.intervalAvgUs.toFixed(0)} µs avg` : '-'}</dd>
          <dt>jitter (σ)</dt><dd className="num">{poll?.jitterUs != null ? `${poll.jitterUs.toFixed(0)} µs` : '-'}</dd>
        </dl>
        <p className="case__sub">the rate counts how often the device sends reports, stamped with a high-resolution clock the moment each arrives. a device that only reports on change reads low until you move it.</p>
      </section>
      {layout && <p className="case__sub">report descriptor: {layout.descriptorLen} bytes · {layout.reportIds.length ? `report ids ${layout.reportIds.join(', ')}` : 'no report ids'}</p>}
    </div>
  );
}
