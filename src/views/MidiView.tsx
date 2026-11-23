// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab
// midi: a keyboard lit by velocity, controller meters, pitch bend, and the message log.
import { useEffect, useRef, useState } from 'react';
import { Stamp } from '../components/Stamp';
import { FG, LINE_STRONG, fitCanvas, legend, shade } from '../lib/draw';
import { addFrame } from '../lib/raf';
import { results } from '../lib/results';
import { onFrame } from '../lib/session';
import type { SessionInfo } from '../lib/types';

const BLACK = new Set([1, 3, 6, 8, 10]);
const GLOW = 140;
const MAX_LOG = 120;

export function MidiView({ info }: { info: SessionInfo }) {
  const [ccs, setCcs] = useState<Record<number, number>>({});
  const [bend, setBend] = useState(0);
  const [pressure, setPressure] = useState(0);
  const [count, setCount] = useState(0);
  const cv = useRef<HTMLCanvasElement>(null);
  const box = useRef<HTMLDivElement>(null);
  const log = useRef<HTMLOListElement>(null);
  const live = useRef({ vel: new Array<number>(128).fill(0), upAt: new Array<number>(128).fill(0), seen: new Set<number>(), lo: 48, hi: 84, dirty: true });
  const ccRef = useRef<Record<number, number>>({});
  const dirtyCc = useRef(false);

  useEffect(() => {
    const l = live.current;
    let n = 0;
    const off = onFrame((events) => {
      for (const e of events) {
        if (e.t !== 'midi') continue;
        n++;
        if (e.kind === 'noteOn') {
          l.vel[e.a] = e.value / 127;
          l.seen.add(e.a);
          l.lo = Math.min(l.lo, e.a - (e.a % 12));
          l.hi = Math.max(l.hi, e.a - (e.a % 12) + 11);
          l.dirty = true;
        } else if (e.kind === 'noteOff') {
          l.vel[e.a] = 0;
          l.upAt[e.a] = performance.now();
          l.dirty = true;
        } else if (e.kind === 'cc') {
          ccRef.current = { ...ccRef.current, [e.a]: e.value };
          dirtyCc.current = true;
        } else if (e.kind === 'pitch') setBend(e.value);
        else if (e.kind === 'pressure') setPressure(e.value);
        pushLog(log.current, `${e.ts.toFixed(1)}ms`, `ch ${e.channel}`, e.text);
      }
      setCount(n);
    });
    const t = window.setInterval(() => { if (dirtyCc.current) { dirtyCc.current = false; setCcs(ccRef.current); } }, 100);
    return () => { off(); clearInterval(t); };
  }, []);

  useEffect(() => {
    const el = box.current!;
    let w = 0;
    const ro = new ResizeObserver(() => { w = Math.floor(el.clientWidth); live.current.dirty = true; });
    ro.observe(el);
    w = Math.floor(el.clientWidth);
    const off = addFrame((now) => {
      const l = live.current;
      const fading = l.upAt.some((t) => now - t < GLOW);
      if (!l.dirty && !fading) return;
      l.dirty = false;
      if (w < 100 || !cv.current) return;
      const notes: number[] = [];
      for (let n = l.lo; n <= l.hi; n++) notes.push(n);
      const whites = notes.filter((n) => !BLACK.has(n % 12));
      const ww = Math.min(34, w / whites.length);
      const W = ww * whites.length, H = 130;
      const { ctx } = fitCanvas(cv.current, W, H);
      ctx.fillStyle = '#000'; ctx.fillRect(0, 0, W, H);
      const bright = (n: number) => (l.vel[n]! > 0 ? Math.max(0.35, l.vel[n]!) : Math.max(0, 1 - (now - l.upAt[n]!) / GLOW));
      whites.forEach((n, i) => {
        const b = bright(n);
        ctx.fillStyle = shade(b); ctx.fillRect(i * ww + 1, 1, ww - 2, H - 2);
        ctx.strokeStyle = l.seen.has(n) ? FG : LINE_STRONG; ctx.lineWidth = 1; ctx.strokeRect(i * ww + 1.5, 1.5, ww - 3, H - 3);
        if (n % 12 === 0) legend(ctx, `C${Math.floor(n / 12) - 1}`, i * ww + ww / 2, H - 12, b, 9);
      });
      notes.filter((n) => BLACK.has(n % 12)).forEach((n) => {
        const left = whites.indexOf(n - 1);
        if (left < 0) return;
        const b = bright(n);
        ctx.fillStyle = shade(b); ctx.fillRect(left * ww + ww * 0.68, 1, ww * 0.64, H * 0.58);
        ctx.strokeStyle = FG; ctx.lineWidth = 1.5; ctx.strokeRect(left * ww + ww * 0.68, 1, ww * 0.64, H * 0.58);
      });
      if (fading) l.dirty = true;
    });
    return () => { off(); ro.disconnect(); };
  }, []);

  useEffect(() => { results.set('hid', { device: info.device.name, midi: { messages: count, controllers: Object.keys(ccs).length } }); }, [info.device.name, count, ccs]);

  const ccList = Object.entries(ccs).map(([k, v]) => [Number(k), v] as const).sort((a, b) => a[0] - b[0]);
  const ccName = (n: number) => (n === 1 ? 'mod wheel' : n === 7 ? 'volume' : n === 10 ? 'pan' : n === 11 ? 'expression' : n === 64 ? 'sustain' : n === 74 ? 'brightness' : '');

  return (
    <div className="hv">
      {info.midiError && (
        <div className="banner banner--problem">
          <Stamp tone="problem" size="sm">no signal</Stamp>
          <div><p className="banner__t">could not open this midi port</p><p className="banner__p">{info.midiError}. another app may have it open: most midi drivers allow one reader at a time.</p></div>
          <span />
        </div>
      )}
      <section className="card">
        <p className="label">keys · velocity scales brightness · {count} messages</p>
        <div ref={box}><canvas ref={cv} role="img" aria-label="midi keyboard, lit by note velocity" /></div>
      </section>
      <div className="kb__panels">
        <section className="card">
          <p className="label">pitch bend</p>
          <div className="hv__axis" role="meter" aria-label="pitch bend" aria-valuemin={-8192} aria-valuemax={8191}><span style={{ left: `${((bend + 8192) / 16383) * 100}%`, width: '4px', translate: '-2px 0', position: 'absolute' }} /></div>
          <p className="num case__sub">{bend}</p>
          <p className="label" style={{ marginTop: '.8rem' }}>channel pressure</p>
          <div className="hv__axis" role="meter" aria-label="channel pressure" aria-valuemin={0} aria-valuemax={127}><span style={{ width: `${(pressure / 127) * 100}%` }} /></div>
        </section>
        <section className="card">
          <p className="label">controllers seen</p>
          {ccList.length === 0 ? <p className="case__sub">turn a knob, move a fader or the mod wheel.</p> : (
            <ul className="hv__axes">
              {ccList.map(([n, v]) => (
                <li key={n}><span className="mono">cc {n}{ccName(n) ? ` · ${ccName(n)}` : ''}</span><span className="hv__axis"><span style={{ width: `${(v / 127) * 100}%` }} /></span><span className="num">{v}</span></li>
              ))}
            </ul>
          )}
        </section>
        <section className="card kb__logcard">
          <p className="label">message log</p>
          <ol ref={log} className="kb__log" aria-label="midi messages, newest first" />
        </section>
      </div>
    </div>
  );
}

function pushLog(ol: HTMLOListElement | null, a: string, b: string, c: string): void {
  if (!ol) return;
  const li = document.createElement('li');
  li.className = 'logrow';
  const x = document.createElement('span'); x.textContent = a;
  const y = document.createElement('b'); y.textContent = b;
  const z = document.createElement('span'); z.textContent = c;
  li.append(x, y, z);
  ol.prepend(li);
  while (ol.childElementCount > MAX_LOG) ol.lastElementChild?.remove();
}

