// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab · source viewable, not reusable: see LICENSE
// the keyboard: an on-screen board that lights on keydown and goes dark on keyup,
// driven by physical keys from rust (never layout characters).
import { useEffect, useMemo, useRef, useState } from 'react';
import { HexDump, type HexDumpHandle } from '../components/HexDump';
import { Stamp } from '../components/Stamp';
import { FG, LINE_STRONG, MONO, fitCanvas, legend, shade } from '../lib/draw';
import { loadNativeLabels } from '../lib/nativeLabels';
import { addFrame } from '../lib/raf';
import { results } from '../lib/results';
import { onFrame, sendCommand } from '../lib/session';
import { useSettings } from '../lib/settings';
import { useStore } from '../lib/store';
import type { SessionInfo } from '../lib/types';
import { SIZES, getLayout, keyRects, type KeyDef, type Size, type Variant } from '../layouts/build';
import { detectFamily, detectSize } from '../layouts/detect';
import { FAMILIES, labelFor, type LabelFamily } from '../layouts/labels';

const GLOW_MS = 110; // a tap shorter than a frame is still visible for a moment
const MAX_LOG = 150;

interface Live {
  down: Set<string>;
  upAt: Map<string, number>;
  depth: Map<string, number>; // analog keyboards: 0..1 per key
  tested: Set<string>;
  chatter: Map<string, number>;
  dirty: boolean;
  held: number;
  max: number;
}

export function KeyboardView({ info }: { info: SessionInfo }) {
  const settings = useSettings();
  const mac = useStore((st) => st.info?.os) === 'macos';
  const [sizeSel, setSizeSel] = useState<Size>(mac ? 'macbook' : 'full');
  const [variantSel, setVariantSel] = useState<Variant>('ansi');
  const [variantManual, setVariantManual] = useState(false);
  const [famSel, setFamSel] = useState<LabelFamily | 'auto'>('auto');
  const [checklist, setChecklist] = useState(true);
  const [native, setNative] = useState<Map<string, string> | null>(null);
  const [testedN, setTestedN] = useState(0);
  const [suggest, setSuggest] = useState<Size | null>(null);
  const [chatterN, setChatterN] = useState<Record<string, number>>({});
  const [observed, setObserved] = useState<string[]>([]);

  const live = useRef<Live>({ down: new Set(), upAt: new Map(), depth: new Map(), tested: new Set(), chatter: new Map(), dirty: true, held: 0, max: 0 });
  const wrap = useRef<HTMLDivElement>(null);
  const canvas = useRef<HTMLCanvasElement>(null);
  const heldEl = useRef<HTMLSpanElement>(null);
  const maxEl = useRef<HTMLSpanElement>(null);
  const logEl = useRef<HTMLOListElement>(null);
  const dump = useRef<HexDumpHandle>(null);

  const layout = useMemo(() => getLayout(sizeSel, variantSel), [sizeSel, variantSel]);
  const family: LabelFamily = famSel === 'auto' ? (native ? detectFamily(native) : 'qwerty') : famSel;
  const nativeForLabels = famSel === 'auto' ? native : null; // a manual family means "show me that layout"

  // mutable mirror for the draw loop (no react work per frame)
  const view = useRef({ layout, family, native: nativeForLabels, checklist, mac });
  view.current = { layout, family, native: nativeForLabels, checklist, mac };

  useEffect(() => {
    void loadNativeLabels().then(setNative);
  }, []);

  // frames from rust
  useEffect(() => {
    const l = live.current;
    results.set('keyboard', { tested: [], total: 0, maxHeld: 0, chatter: {}, layout: '' });
    return onFrame((events) => {
      let testedGrew = false;
      let chatterGrew = false;
      for (const e of events) {
        if (e.t === 'reserved') {
          setObserved((o) => (o.includes(e.label) ? o : [...o, e.label]));
          continue;
        }
        if (e.t === 'analog') {
          for (const [code, v] of e.keys) {
            if (v > 0) l.depth.set(code, v);
            else l.depth.delete(code);
          }
          l.dirty = true;
          continue;
        }
        if (e.t === 'hid') {
          dump.current?.push(e.report, e.changed);
          continue;
        }
        if (e.t !== 'key') continue;
        if (e.down) {
          l.down.add(e.code);
          if (!l.tested.has(e.code)) {
            l.tested.add(e.code);
            testedGrew = true;
          }
          if (e.chatter) {
            l.chatter.set(e.code, (l.chatter.get(e.code) ?? 0) + 1);
            chatterGrew = true;
          }
        } else {
          l.down.delete(e.code);
          l.upAt.set(e.code, performance.now());
        }
        l.held = e.held;
        l.max = e.max;
        l.dirty = true;
        pushLog(logEl.current, e);
      }
      if (heldEl.current) heldEl.current.textContent = String(l.held);
      if (maxEl.current) maxEl.current.textContent = String(l.max);
      if (testedGrew) {
        setTestedN(l.tested.size);
        const d = detectSize(l.tested);
        setSuggest(d.size);
        if (d.variant === 'iso') setVariantSel((v) => (variantManualRef.current ? v : 'iso'));
      }
      if (chatterGrew) setChatterN(Object.fromEntries(l.chatter));
      if (testedGrew || chatterGrew) {
        results.set('keyboard', { tested: [...l.tested], total: view.current.layout.keys.length, maxHeld: l.max, chatter: Object.fromEntries(l.chatter), layout: view.current.layout.id });
      }
    });
  }, []);
  const variantManualRef = useRef(false);
  variantManualRef.current = variantManual;

  // drawing
  useEffect(() => {
    const el = canvas.current!;
    const box = wrap.current!;
    let cssW = 0;
    const resize = () => {
      cssW = Math.floor(box.clientWidth);
      live.current.dirty = true;
    };
    const ro = new ResizeObserver(resize);
    ro.observe(box);
    resize();

    const draw = (now: number) => {
      const l = live.current;
      const v = view.current;
      let fading = false;
      for (const t of l.upAt.values()) if (now - t < GLOW_MS) fading = true;
      if (!l.dirty && !fading) return;
      l.dirty = false;
      if (cssW < 50) return;

      const unit = Math.min(cssW / v.layout.w, 64);
      const w = Math.floor(unit * v.layout.w);
      const h = Math.ceil(unit * v.layout.h);
      const { ctx } = fitCanvas(el, w, h);
      ctx.clearRect(0, 0, w, h);
      const gap = Math.max(1.5, unit * 0.06);

      for (const k of v.layout.keys) {
        const depthB = l.depth.get(k.code);
        let b = 0;
        if (l.down.has(k.code)) b = depthB ?? 1;
        else {
          const up = l.upAt.get(k.code);
          if (up !== undefined && now - up < GLOW_MS) b = 1 - (now - up) / GLOW_MS;
          else if (depthB !== undefined) b = depthB;
        }
        drawKey(ctx, k, unit, gap, b, l.tested.has(k.code), l.chatter.has(k.code), v.checklist, labelFor(k.code, v.family, v.native, v.mac), depthB);
      }
      if (fading) l.dirty = true;
    };
    const off = addFrame(draw);
    return () => {
      off();
      ro.disconnect();
    };
  }, []);

  // layout / label changes need a repaint
  useEffect(() => {
    live.current.dirty = true;
  }, [layout, family, nativeForLabels, checklist]);

  const untested = useMemo(() => layout.keys.filter((k) => !live.current.tested.has(k.code)), [layout, testedN]); // eslint-disable-line react-hooks/exhaustive-deps
  const total = layout.keys.length;
  const testedInLayout = total - untested.length;
  const chatterKeys = Object.entries(chatterN).sort((a, b) => b[1] - a[1]);

  const reset = () => {
    const l = live.current;
    l.tested.clear();
    l.chatter.clear();
    l.down.clear();
    l.upAt.clear();
    l.held = 0;
    l.max = 0;
    l.dirty = true;
    setTestedN(0);
    setChatterN({});
    setSuggest(null);
    setObserved([]);
    if (heldEl.current) heldEl.current.textContent = '0';
    if (maxEl.current) maxEl.current.textContent = '0';
    if (logEl.current) logEl.current.replaceChildren();
    void sendCommand({ type: 'resetKeys' });
  };

  return (
    <div className="kb">
      <div className="kb__bar">
        <label className="field">
          <span className="label label--sm">form factor</span>
          <select className="select" value={sizeSel} onChange={(e) => setSizeSel(e.target.value as Size)}>
            {SIZES.map((s) => (
              <option key={s.id} value={s.id}>{s.name}</option>
            ))}
          </select>
        </label>
        <label className="field">
          <span className="label label--sm">variant</span>
          <select className="select" value={variantSel} onChange={(e) => { setVariantManual(true); setVariantSel(e.target.value as Variant); }}>
            <option value="ansi">ansi</option>
            <option value="iso">iso</option>
          </select>
        </label>
        <label className="field">
          <span className="label label--sm">legends</span>
          <select className="select" value={famSel} onChange={(e) => setFamSel(e.target.value as LabelFamily | 'auto')}>
            <option value="auto">auto ({family}{native ? ', from os' : ''})</option>
            {FAMILIES.map((f) => (
              <option key={f} value={f}>{f}</option>
            ))}
          </select>
        </label>
        <button className="toggle" role="switch" aria-checked={checklist} onClick={() => setChecklist((c) => !c)}>
          <span className="toggle__box" />
          <span className="label label--sm">checklist</span>
        </button>
        <button className="btn btn--sm" onClick={reset}>reset</button>
      </div>

      {suggest && testedN >= 12 && SIZES.findIndex((s) => s.id === suggest) > SIZES.findIndex((s) => s.id === sizeSel) && (
        <div className="banner">
          <Stamp tone="wip" size="sm">guess</Stamp>
          <p className="banner__p">pressed {testedN} keys and every one fits a {SIZES.find((s) => s.id === suggest)?.name} board. this might be one.</p>
          <button className="btn btn--sm" onClick={() => setSizeSel(suggest)}>use it</button>
        </div>
      )}

      {info.analog && (
        <div className={`banner ${info.analog.error ? 'banner--problem' : ''}`}>
          <Stamp tone={info.analog.error ? 'problem' : 'wip'} size="sm">{info.analog.error ? 'no analog' : 'analog'}</Stamp>
          <p className="banner__p">
            {info.analog.error
              ? `could not open the analog interface (${info.analog.error}). keys still work as normal switches.`
              : `${info.analog.profile} analog profile: key brightness follows how far each key is pressed. ${info.analog.verified ? '' : 'this profile is experimental and has not been checked against a real board; the raw report panel shows exactly what the keyboard sends.'}`}
          </p>
          <span />
        </div>
      )}

      <div ref={wrap} className="kb__stage">
        <canvas ref={canvas} role="img" aria-label={`on-screen ${layout.name} keyboard, lit while keys are held`} />
      </div>

      <div className="kb__panels">
        <section className="card">
          <p className="label">tested</p>
          <p className="kb__big num">{testedInLayout} <span>/ {total}</span></p>
          <div className="meter" role="progressbar" aria-valuemin={0} aria-valuemax={total} aria-valuenow={testedInLayout} aria-label="keys tested">
            <span style={{ width: `${(testedInLayout / total) * 100}%` }} />
          </div>
          <details className="kb__untested">
            <summary className="label label--sm">untested keys ({untested.length})</summary>
            <p className="kb__chips">
              {untested.map((k) => (
                <span key={k.code} className="chip">{labelFor(k.code, family, nativeForLabels, mac) || k.code}</span>
              ))}
            </p>
          </details>
        </section>

        <section className="card">
          <p className="label">rollover</p>
          <p className="kb__big num"><span ref={heldEl} style={{ color: 'inherit', fontSize: 'inherit' }}>0</span> <span>held · max <b ref={maxEl}>0</b></span></p>
          <p className="case__sub">most keys down at once this session. a board that tops out at 6 is 6kro; the os may also cap it.</p>
          <p className="label" style={{ marginTop: '.8rem' }}>chatter &lt; {settings.chatterMs}ms</p>
          {chatterKeys.length === 0 ? (
            <p className="case__sub">none seen.</p>
          ) : (
            <ul className="kb__chatter">
              {chatterKeys.map(([code, n]) => (
                <li key={code} className="leader"><span>{code}</span><span><Stamp tone="problem" size="sm">{n}x</Stamp></span></li>
              ))}
            </ul>
          )}
        </section>

        <section className="card">
          <p className="label">reserved by os</p>
          <ul className="kb__reserved">
            {info.reserved.map((r) => (
              <li key={r.label}>
                <span className="mono">{r.label}</span>
                <Stamp tone={observed.includes(r.label) ? 'problem' : 'wip'} size="sm">{observed.includes(r.label) ? 'seen: focus taken' : 'reserved by os'}</Stamp>
                <span className="case__sub">{r.why}</span>
              </li>
            ))}
            <li>
              <span className="mono">fn</span>
              <Stamp tone="wip" size="sm">firmware</Stamp>
              <span className="case__sub">handled inside the keyboard, never sent to the os.</span>
            </li>
          </ul>
        </section>

        {info.analog && !info.analog.error && (
          <section className="card kb__logcard">
            <p className="label">raw analog reports · changed bytes flash</p>
            <HexDump ref={dump} ariaLabel="raw analog keyboard report bytes" />
          </section>
        )}

        <section className="card kb__logcard">
          <p className="label">key log</p>
          <ol ref={logEl} className="kb__log" aria-label="key events, newest first" />
        </section>
      </div>
    </div>
  );
}

function drawKey(ctx: CanvasRenderingContext2D, k: KeyDef, u: number, gap: number, b: number, tested: boolean, chatter: boolean, checklist: boolean, label: string, depth?: number): void {
  ctx.beginPath();
  if (k.shape === 'isoEnter') {
    const x = k.x * u, y = k.y * u, w = k.w * u;
    const g = gap / 2;
    ctx.moveTo(x + g, y + g);
    ctx.lineTo(x + w - g, y + g);
    ctx.lineTo(x + w - g, y + 2 * u - g);
    ctx.lineTo(x + 0.25 * u + g, y + 2 * u - g);
    ctx.lineTo(x + 0.25 * u + g, y + u - g);
    ctx.lineTo(x + g, y + u - g);
    ctx.closePath();
  } else {
    ctx.rect(k.x * u + gap / 2, k.y * u + gap / 2, k.w * u - gap, k.h * u - gap);
  }
  ctx.fillStyle = shade(b);
  ctx.fill();

  // borders: never-pressed keys get a subtle dashed outline; tested keys a solid one.
  // checklist mode pushes the untested ones forward.
  ctx.setLineDash(tested ? [] : [3, 3]);
  ctx.lineWidth = !tested && checklist ? 1.5 : 1;
  ctx.strokeStyle = tested ? FG : checklist ? FG : LINE_STRONG;
  if (tested && !checklist) ctx.strokeStyle = LINE_STRONG;
  if (chatter) {
    ctx.lineWidth = 2.5;
    ctx.setLineDash([2, 2]);
    ctx.strokeStyle = FG;
  }
  ctx.stroke();
  ctx.setLineDash([]);

  const r = keyRects(k)[0]!;
  const cx = (r.x + r.w / 2) * u + (k.shape === 'isoEnter' ? 0.125 * u : 0);
  const cy = (r.y + r.h / 2) * u + (k.shape === 'isoEnter' ? 0.5 * u : 0);
  const px = Math.max(8, Math.min(u * 0.3, (k.w * u * 0.8) / Math.max(1, label.length * 0.62)));
  legend(ctx, label, cx, cy - (depth !== undefined && depth > 0.02 && depth < 0.995 ? px * 0.35 : 0), b, px);
  if (depth !== undefined && depth > 0.02 && depth < 0.995) legend(ctx, `${Math.round(depth * 100)}%`, cx, cy + px * 0.75, b, Math.max(7, px * 0.62), 400);

  // tested marker: a small solid square in the corner (word-free, so it works on both states)
  if (tested && checklist) {
    const m = Math.max(3, u * 0.1);
    ctx.fillStyle = b > 0.5 ? '#000' : FG;
    ctx.fillRect((r.x + r.w) * u - gap / 2 - m - 2, r.y * u + gap / 2 + 2, m, m);
  }
}

function pushLog(ol: HTMLOListElement | null, e: Extract<import('../lib/types').Ev, { t: 'key' }>): void {
  if (!ol) return;
  const li = document.createElement('li');
  li.className = e.chatter ? 'logrow logrow--bad' : 'logrow';
  const t = document.createElement('span');
  t.textContent = `${e.ts.toFixed(1)}ms`;
  const k = document.createElement('b');
  k.textContent = e.code;
  const d = document.createElement('span');
  d.textContent = e.down ? 'down' : `up · held ${e.hold !== null ? e.hold.toFixed(1) : '?'}ms`;
  li.append(t, k, d);
  if (e.chatter) {
    const c = document.createElement('em');
    c.textContent = `CHATTER ${e.gap?.toFixed(1)}ms`;
    li.append(c);
  }
  ol.prepend(li);
  while (ol.childElementCount > MAX_LOG) ol.lastElementChild?.remove();
}

void MONO;
