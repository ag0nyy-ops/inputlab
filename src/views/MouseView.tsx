// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab · source viewable, not reusable: see LICENSE
// the mouse: a model that lights up under your clicks, a movement arena with a live
// trail, and four tests. everything is driven from the keyboard because a mouse
// session captures the mouse completely (no clickable controls exist).
import { useEffect, useMemo, useRef, useState } from 'react';
import { Kbd } from '../components/Kbd';
import { Stamp } from '../components/Stamp';
import { FG, LINE, LINE_STRONG, MONO, MUTED, fitCanvas, shade } from '../lib/draw';
import { addFrame } from '../lib/raf';
import { results } from '../lib/results';
import { onFrame, sendCommand } from '../lib/session';
import { useSettings } from '../lib/settings';
import type { ButtonStat, DpiSnap, PollSnap, SensorSnap, SessionInfo } from '../lib/types';
import { drawMouse, idleMouse, mouseHeight, type MouseDrawState } from '../models/drawMouse';
import { matchMouse } from '../models/mice';
import { getPick } from '../models/pick';

type Tab = 'poll' | 'dpi' | 'sensor' | 'clicks';
const TABS: { id: Tab; name: string; key: string }[] = [
  { id: 'poll', name: 'polling rate', key: '1' },
  { id: 'dpi', name: 'dpi', key: '2' },
  { id: 'sensor', name: 'sensor', key: '3' },
  { id: 'clicks', name: 'clicks', key: '4' },
];

const GLOW = 120;
const COMMON_CPI = [200, 400, 500, 800, 1000, 1200, 1600, 2000, 2400, 3200, 4000, 4800, 6400, 8000, 12000, 16000, 20000, 25600, 26000, 30000];

interface Live {
  down: boolean[];
  upAt: number[];
  wheelAt: { up: number; down: number; left: number; right: number };
  pos: { x: number; y: number };
  trail: { x: number; y: number; t: number }[];
  dirty: boolean;
}

export function MouseView({ info }: { info: SessionInfo }) {
  const settings = useSettings();
  const match = useMemo(() => matchMouse(info.device, getPick(info.device)), [info.device]);
  const [tab, setTab] = useState<Tab>('poll');
  const [poll, setPoll] = useState<PollSnap | null>(null);
  const [dpi, setDpi] = useState<DpiSnap | null>(null);
  const [sensor, setSensor] = useState<SensorSnap | null>(null);
  const [clicks, setClicks] = useState<ButtonStat[]>([]);
  const [wheel, setWheel] = useState({ up: 0, down: 0, left: 0, right: 0 });

  const live = useRef<Live>({ down: [false, false, false, false, false], upAt: [0, 0, 0, 0, 0], wheelAt: { up: 0, down: 0, left: 0, right: 0 }, pos: { x: 0, y: 0 }, trail: [], dirty: true });
  const latest = useRef({ poll, dpi, sensor, tab });
  latest.current = { poll, dpi, sensor, tab };
  const snaps = useRef<{ poll?: PollSnap; dpi?: DpiSnap; sensor?: SensorSnap; clicks?: ButtonStat[] }>({});
  const modelCv = useRef<HTMLCanvasElement>(null);
  const arenaCv = useRef<HTMLCanvasElement>(null);
  const arenaBox = useRef<HTMLDivElement>(null);
  const graphCv = useRef<HTMLCanvasElement>(null);
  const rulerCv = useRef<HTMLCanvasElement>(null);
  const traceCv = useRef<HTMLCanvasElement>(null);

  const inches = settings.distanceUnit === 'in' ? settings.dpiDistance : settings.dpiDistance / 2.54;

  // frames from rust. react state only gets the low-rate snapshots, throttled.
  useEffect(() => {
    const l = live.current;
    const off = onFrame((events) => {
      for (const e of events) {
        switch (e.t) {
          case 'mouse':
            l.pos.x += e.dx * 0.35;
            l.pos.y += e.dy * 0.35;
            l.trail.push({ x: l.pos.x, y: l.pos.y, t: performance.now() });
            if (l.trail.length > 900) l.trail.splice(0, 300);
            l.dirty = true;
            break;
          case 'button': {
            const i = e.n - 1;
            l.down[i] = e.down;
            if (!e.down) l.upAt[i] = performance.now();
            l.dirty = true;
            break;
          }
          case 'wheel': {
            const t = performance.now();
            if (e.dy > 0) l.wheelAt.up = t;
            if (e.dy < 0) l.wheelAt.down = t;
            if (e.dx < 0) l.wheelAt.left = t;
            if (e.dx > 0) l.wheelAt.right = t;
            setWheel((w) => ({ up: w.up + (e.dy > 0 ? 1 : 0), down: w.down + (e.dy < 0 ? 1 : 0), left: w.left + (e.dx < 0 ? 1 : 0), right: w.right + (e.dx > 0 ? 1 : 0) }));
            l.dirty = true;
            break;
          }
          case 'poll': snaps.current.poll = e; break;
          case 'dpi': snaps.current.dpi = e; break;
          case 'sensor': snaps.current.sensor = e; break;
          case 'clicks': snaps.current.clicks = e.buttons; break;
        }
      }
    });
    const tick = window.setInterval(() => {
      const s = snaps.current;
      if (s.poll) { setPoll(s.poll); s.poll = undefined; }
      if (s.dpi) { setDpi(s.dpi); s.dpi = undefined; }
      if (s.sensor) { setSensor(s.sensor); s.sensor = undefined; }
      if (s.clicks) { setClicks(s.clicks); s.clicks = undefined; }
    }, 100);
    return () => {
      off();
      clearInterval(tick);
    };
  }, []);

  // keyboard controls (the mouse is captured)
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.repeat) return;
      const t = TABS.find((x) => x.key === e.key);
      if (t) return setTab(t.id);
      const cur = latest.current;
      if (e.code === 'Space') {
        if (cur.tab === 'dpi') void sendCommand(cur.dpi?.state === 'counting' ? { type: 'dpiStop' } : { type: 'dpiStart', distanceIn: inchesRef.current });
        if (cur.tab === 'sensor') void sendCommand(cur.sensor?.active ? { type: 'sensorStop' } : { type: 'sensorStart' });
      }
      if (e.key.toLowerCase() === 'r') {
        if (cur.tab === 'poll' || cur.tab === 'clicks') void sendCommand({ type: 'mouseReset' });
        if (cur.tab === 'dpi') void sendCommand({ type: 'dpiReset' });
        if (cur.tab === 'sensor') void sendCommand({ type: 'sensorReset' });
        if (cur.tab === 'poll') setWheel({ up: 0, down: 0, left: 0, right: 0 });
      }
    };
    addEventListener('keydown', onKey);
    return () => removeEventListener('keydown', onKey);
  }, []);
  const inchesRef = useRef(inches);
  inchesRef.current = inches;

  // drawing: model + arena in the shared loop
  useEffect(() => {
    const box = arenaBox.current!;
    let w = 0;
    const ro = new ResizeObserver(() => { w = Math.floor(box.clientWidth); live.current.dirty = true; });
    ro.observe(box);
    w = Math.floor(box.clientWidth);

    const off = addFrame((now) => {
      const l = live.current;
      const trailAlive = l.trail.length > 0 && now - l.trail[l.trail.length - 1]!.t < 1600;
      const fading = l.upAt.some((t) => now - t < GLOW) || Object.values(l.wheelAt).some((t) => now - t < GLOW + 60);
      if (!l.dirty && !trailAlive && !fading) return;
      l.dirty = false;

      // model
      const mc = modelCv.current;
      if (mc) {
        const mw = 150;
        const { ctx } = fitCanvas(mc, mw, mouseHeight(mw));
        ctx.clearRect(0, 0, mw, mouseHeight(mw));
        const st: MouseDrawState = idleMouse();
        for (let i = 0; i < 5; i++) st.b[i] = l.down[i] ? 1 : Math.max(0, 1 - (now - l.upAt[i]!) / GLOW);
        const wv = (t: number) => Math.max(0, 1 - (now - t) / (GLOW + 60));
        st.wheel = { up: wv(l.wheelAt.up), down: wv(l.wheelAt.down), left: wv(l.wheelAt.left), right: wv(l.wheelAt.right) };
        drawMouse(ctx, match.shape, 0, 0, mw, st);
      }

      // arena
      const ac = arenaCv.current;
      if (ac && w > 50) {
        const h = Math.max(100, Math.min(170, Math.round(window.innerHeight * 0.17)));
        const { ctx } = fitCanvas(ac, w, h);
        ctx.fillStyle = '#000';
        ctx.fillRect(0, 0, w, h);
        ctx.strokeStyle = LINE;
        ctx.lineWidth = 1;
        for (let x = 0; x < w; x += 40) { ctx.beginPath(); ctx.moveTo(x + 0.5, 0); ctx.lineTo(x + 0.5, h); ctx.stroke(); }
        for (let y = 0; y < h; y += 40) { ctx.beginPath(); ctx.moveTo(0, y + 0.5); ctx.lineTo(w, y + 0.5); ctx.stroke(); }
        // virtual cursor: raw counts, kept inside the arena
        l.pos.x = Math.max(-w / 2, Math.min(w / 2, l.pos.x));
        l.pos.y = Math.max(-h / 2, Math.min(h / 2, l.pos.y));
        const cx = w / 2, cy = h / 2;
        ctx.lineJoin = 'round';
        for (let i = 1; i < l.trail.length; i++) {
          const a = l.trail[i - 1]!, b = l.trail[i]!;
          const age = (now - b.t) / 1600;
          if (age >= 1) continue;
          const ax = Math.max(-w / 2, Math.min(w / 2, a.x)), ay = Math.max(-h / 2, Math.min(h / 2, a.y));
          const bx = Math.max(-w / 2, Math.min(w / 2, b.x)), by = Math.max(-h / 2, Math.min(h / 2, b.y));
          ctx.strokeStyle = shade(1 - age);
          ctx.lineWidth = 1 + (1 - age) * 1.5;
          ctx.beginPath(); ctx.moveTo(cx + ax, cy + ay); ctx.lineTo(cx + bx, cy + by); ctx.stroke();
        }
        const hx = cx + l.pos.x, hy = cy + l.pos.y;
        ctx.strokeStyle = FG; ctx.lineWidth = 1.5;
        ctx.beginPath(); ctx.moveTo(hx - 9, hy); ctx.lineTo(hx + 9, hy); ctx.moveTo(hx, hy - 9); ctx.lineTo(hx, hy + 9); ctx.stroke();
        ctx.fillStyle = FG; ctx.fillRect(hx - 2, hy - 2, 4, 4);
        ctx.font = `400 11px ${MONO}`; ctx.fillStyle = MUTED; ctx.textAlign = 'left'; ctx.textBaseline = 'top';
        ctx.fillText('movement · raw counts, not pointer pixels', 10, 8);
      }
      if (trailAlive || fading) l.dirty = true;
    });
    return () => { off(); ro.disconnect(); };
  }, [match.shape]);

  // panel canvases
  useEffect(() => {
    if (tab === 'poll' && poll && graphCv.current) drawGraph(graphCv.current, poll);
  }, [tab, poll]);
  useEffect(() => {
    if (tab === 'dpi' && rulerCv.current) drawRuler(rulerCv.current, inches, settings.distanceUnit, settings.dpiDistance, settings.rulerPxPerInch);
  }, [tab, inches, settings.distanceUnit, settings.dpiDistance, settings.rulerPxPerInch]);
  useEffect(() => {
    if (tab === 'sensor' && sensor && traceCv.current) drawSensor(traceCv.current, sensor);
  }, [tab, sensor]);

  // keep the summary current
  useEffect(() => {
    results.set('mouse', { model: match.model?.name ?? null, shape: match.shape, polling: poll && { avgHz: poll.avgHz, minHz: poll.minHz, maxHz: poll.maxHz, nominal: poll.nominal, jitterUs: poll.jitterUs }, dpi: dpi && dpi.dpi !== null ? { cpi: dpi.dpi, counts: dpi.counts, distanceIn: dpi.distanceIn, straightness: dpi.straightness } : null, sensor: sensor && { jitterPct: sensor.jitterPct, snapRatio: sensor.snapRatio, snapping: sensor.snapping }, buttons: clicks, wheel });
  }, [match, poll, dpi, sensor, clicks, wheel]);

  const nearestCpi = dpi?.dpi ? COMMON_CPI.reduce((a, b) => (Math.abs(b - dpi.dpi!) < Math.abs(a - dpi.dpi!) ? b : a)) : null;
  const cpiNear = nearestCpi !== null && dpi?.dpi ? Math.abs(nearestCpi - dpi.dpi) / nearestCpi <= 0.05 : false;
  const f0 = (n: number | null | undefined) => (n === null || n === undefined ? '-' : Math.round(n).toLocaleString());
  const f1 = (n: number | null | undefined) => (n === null || n === undefined ? '-' : n.toFixed(1));

  return (
    <div className="mv">
      <aside className="mv__side">
        <canvas ref={modelCv} role="img" aria-label={`${match.model?.name ?? 'mouse'} silhouette, lit while buttons are held`} />
        <p className="label label--sm" style={{ textAlign: 'center' }}>{match.model ? match.model.name : 'generic mouse'}</p>
        <dl className="kv mv__wheel">
          <dt>wheel up</dt><dd className="num">{wheel.up}</dd>
          <dt>wheel down</dt><dd className="num">{wheel.down}</dd>
          <dt>tilt left</dt><dd className="num">{wheel.left}</dd>
          <dt>tilt right</dt><dd className="num">{wheel.right}</dd>
        </dl>
      </aside>

      <section className="mv__panel">
        <div className="seg" role="tablist" aria-label="mouse tests">
          {TABS.map((t) => (
            // not buttons: a mouse session has no clickable controls. keys 1-4 switch tabs;
            // the click handler only matters under --no-capture, where the mouse is free.
            <div key={t.id} className="seg__item" role="tab" aria-selected={tab === t.id} data-on={tab === t.id ? '1' : '0'} onClick={() => setTab(t.id)}>
              {t.key} · {t.name}
            </div>
          ))}
        </div>

        {tab === 'poll' && (
          <div className="mv__body">
            <p className="label">polling rate · measured from raw report timestamps</p>
            <p className="mv__hz num">{poll?.hz ? f0(poll.hz) : poll?.avgHz ? f0(poll.avgHz) : '0'}<span> hz{poll?.hz ? '' : poll?.avgHz ? ' avg' : ''}</span></p>
            {poll?.nominal ? <Stamp>≈ {poll.nominal} hz</Stamp> : <p className="case__sub">move the mouse in fast circles. an idle mouse sends nothing, so there is nothing to measure.</p>}
            <dl className="kv mv__stats">
              <dt>average</dt><dd className="num">{f0(poll?.avgHz)} hz</dd>
              <dt>slowest 100 ms</dt><dd className="num">{f0(poll?.minHz)} hz</dd>
              <dt>fastest 100 ms</dt><dd className="num">{f0(poll?.maxHz)} hz</dd>
              <dt>interval</dt><dd className="num">{f1(poll?.intervalMinUs)} / {f1(poll?.intervalAvgUs)} / {f1(poll?.intervalMaxUs)} µs</dd>
              <dt>jitter (σ)</dt><dd className="num">{f1(poll?.jitterUs)} µs</dd>
              <dt>reports</dt><dd className="num">{poll?.reports.toLocaleString() ?? 0}</dd>
            </dl>
            <canvas ref={graphCv} className="mv__graph" role="img" aria-label="polling rate stability, last 6 seconds of movement" />
            <p className="case__sub">one point per 100 ms of movement, counted from raw report timestamps, so 2k, 4k and 8k mice read correctly when moved fast enough to send every report. the os can still batch very high rates; this reads what it delivers.</p>
          </div>
        )}

        {tab === 'dpi' && (
          <div className="mv__body">
            <p className="label">dpi test · raw counts over a known distance</p>
            <ol className="mv__steps">
              <li>put a real ruler on the desk, mouse at 0. the on-screen ruler is only a guide ({settings.dpiDistance} {settings.distanceUnit}).</li>
              <li>press <Kbd>space</Kbd>, slide the mouse in a straight line the full distance, press <Kbd>space</Kbd> again.</li>
              <li><Kbd>r</Kbd> clears. the distance is in settings.</li>
            </ol>
            <canvas ref={rulerCv} className="mv__ruler" role="img" aria-label="on-screen ruler" />
            <div className="mv__result">
              <p className="mv__hz num">{dpi?.dpi ? f0(dpi.dpi) : '-'}<span> cpi</span></p>
              {dpi?.state === 'counting' && <Stamp tone="wip">counting</Stamp>}
              {dpi?.state === 'done' && dpi.dpi && <Stamp>{cpiNear ? `≈ ${nearestCpi} cpi setting` : 'measured'}</Stamp>}
              {(!dpi || dpi.state === 'idle') && <Stamp tone="wip">press space</Stamp>}
            </div>
            <dl className="kv mv__stats">
              <dt>counts (net)</dt><dd className="num">{f0(dpi?.counts)}</dd>
              <dt>distance</dt><dd className="num">{inches.toFixed(2)} in · {(inches * 2.54).toFixed(1)} cm</dd>
              <dt>straightness</dt><dd className="num">{dpi && dpi.path > 0 ? `${Math.round(dpi.straightness * 100)}%` : '-'}</dd>
            </dl>
            {dpi?.state === 'done' && dpi.straightness < 0.9 && <p className="case__sub">the path wobbled, so only the net distance counted. a straighter, longer move gives a better number.</p>}
            <p className="case__sub">raw input skips os acceleration and pointer speed, so this is the sensor's own cpi. the real-world distance is only as accurate as your measuring.</p>
          </div>
        )}

        {tab === 'sensor' && (
          <div className="mv__body">
            <p className="label">sensor · slow circle</p>
            <p className="case__sub">press <Kbd>space</Kbd> and draw slow, steady circles (about 2 seconds each) for 15 seconds, then <Kbd>space</Kbd> again. <Kbd>r</Kbd> clears.</p>
            <div className="mv__result">
              {sensor?.active ? <Stamp tone="wip">recording</Stamp> : sensor && sensor.steps > 0 ? <Stamp>stopped</Stamp> : <Stamp tone="wip">press space</Stamp>}
              {sensor?.snapping && <Stamp tone="problem">angle snapping likely</Stamp>}
            </div>
            <canvas ref={traceCv} className="mv__trace" role="img" aria-label="circle path and direction histogram" />
            <dl className="kv mv__stats">
              <dt>jitter</dt><dd className="num">{sensor?.jitterPct != null ? `${sensor.jitterPct.toFixed(0)}% of step` : 'need more movement'}</dd>
              <dt>snap ratio</dt><dd className="num">{sensor?.snapRatio != null ? `${sensor.snapRatio.toFixed(2)}x (1.0 = even)` : 'need more movement'}</dd>
              <dt>circle radius</dt><dd className="num">{sensor?.radius != null ? `${Math.round(sensor.radius)} counts` : '-'}</dd>
              <dt>steps</dt><dd className="num">{sensor?.steps ?? 0}</dd>
            </dl>
            <p className="case__sub">jitter has no pass or fail: compare it between mice and surfaces. angle snapping shows as direction spikes at 45° multiples; a hand-drawn circle spreads evenly.</p>
          </div>
        )}

        {tab === 'clicks' && (
          <div className="mv__body">
            <p className="label">buttons · hold time and double-click chatter (&lt; {settings.chatterMs} ms)</p>
            <table className="mv__table">
              <thead><tr><th>button</th><th>clicks</th><th>hold min / avg / max</th><th>shortest gap</th><th>chatter</th></tr></thead>
              <tbody>
                {(clicks.length ? clicks : ['left', 'right', 'middle', 'back', 'forward'].map((_, i) => ({ n: i + 1, clicks: 0, holdMin: null, holdAvg: null, holdMax: null, chatter: 0, minGap: null }))).map((b) => (
                  <tr key={b.n}>
                    <td>{b.n} · {['left', 'right', 'middle', 'back', 'forward'][b.n - 1]}</td>
                    <td className="num">{b.clicks}</td>
                    <td className="num">{f0(b.holdMin)} / {f0(b.holdAvg)} / {f0(b.holdMax)} ms</td>
                    <td className="num">{f1(b.minGap)} ms</td>
                    <td>{b.chatter > 0 ? <Stamp tone="problem" size="sm">{b.chatter}x</Stamp> : <span className="num">0</span>}</td>
                  </tr>
                ))}
              </tbody>
            </table>
            {clicks.length > 0 && clicks.slice(3).every((b) => b.clicks === 0) && (
              <p className="case__sub">buttons 4 and 5 (the side buttons) never registered? remapping software (logitech g hub, razer synapse, steelseries gg) turns a button into a macro or a keystroke before windows sees it, so inputlab cannot see it either. while testing, set those buttons back to their defaults, or put the mouse in onboard-memory mode. the capture line in the footer counts keystrokes sent by software, which rises when a macro button fires.</p>
            )}
            <p className="case__sub">chatter is a release-then-press faster than a hand can click: a worn switch firing twice. true click latency needs a light sensor, so this measures hold time and bounce, which is what shows a failing switch. double-click a button quickly and slowly to see both.</p>
          </div>
        )}
      </section>

      <div ref={arenaBox} className="mv__arena"><canvas ref={arenaCv} role="img" aria-label="live trail of raw mouse movement" /></div>
    </div>
  );
}

function drawGraph(cv: HTMLCanvasElement, p: PollSnap): void {
  const w = Math.floor(cv.parentElement?.clientWidth ?? 480);
  const h = 84;
  const { ctx } = fitCanvas(cv, Math.min(w, 720), h);
  const W = Math.min(w, 720);
  ctx.fillStyle = '#000'; ctx.fillRect(0, 0, W, h);
  const g = p.graph;
  if (g.length < 2) { ctx.fillStyle = MUTED; ctx.font = `400 11px ${MONO}`; ctx.textBaseline = 'middle'; ctx.fillText('waiting for movement', 10, h / 2); return; }
  const avg = p.avgHz ?? g.reduce((a, b) => a + b, 0) / g.length;
  const lo = Math.min(...g, avg) * 0.97, hi = Math.max(...g, avg) * 1.03;
  const y = (v: number) => h - 14 - ((v - lo) / Math.max(1e-6, hi - lo)) * (h - 28);
  ctx.strokeStyle = LINE_STRONG; ctx.setLineDash([3, 3]); ctx.beginPath(); ctx.moveTo(0, y(avg)); ctx.lineTo(W, y(avg)); ctx.stroke(); ctx.setLineDash([]);
  ctx.strokeStyle = FG; ctx.lineWidth = 1.5; ctx.beginPath();
  g.forEach((v, i) => { const x = (i / 59) * W; if (i) ctx.lineTo(x, y(v)); else ctx.moveTo(x, y(v)); });
  ctx.stroke();
  ctx.fillStyle = MUTED; ctx.font = `400 10px ${MONO}`; ctx.textBaseline = 'top'; ctx.fillText(`${Math.round(hi)} hz`, 6, 3); ctx.textBaseline = 'bottom'; ctx.fillText(`${Math.round(lo)} hz`, 6, h - 3);
}

function drawRuler(cv: HTMLCanvasElement, inches: number, unit: 'in' | 'cm', dist: number, pxPerIn: number): void {
  const avail = Math.floor(cv.parentElement?.clientWidth ?? 480);
  const lenPx = inches * pxPerIn;
  const W = Math.max(200, Math.min(avail, Math.ceil(lenPx + 40)));
  const h = 64;
  const { ctx } = fitCanvas(cv, W, h);
  ctx.fillStyle = '#000'; ctx.fillRect(0, 0, W, h);
  const x0 = 20;
  ctx.strokeStyle = FG; ctx.fillStyle = FG; ctx.lineWidth = 1; ctx.font = `400 10px ${MONO}`; ctx.textAlign = 'center'; ctx.textBaseline = 'top';
  const pxPerUnit = unit === 'in' ? pxPerIn : pxPerIn / 2.54;
  const sub = unit === 'in' ? 8 : 10;
  const total = Math.ceil(dist);
  const end = Math.min(W - 10, x0 + Math.min(lenPx, W - 30));
  ctx.beginPath(); ctx.moveTo(x0, 18.5); ctx.lineTo(end, 18.5); ctx.stroke();
  for (let u = 0; u <= total * sub; u++) {
    const x = x0 + (u / sub) * pxPerUnit;
    if (x > end + 0.5 || u / sub > dist + 1e-6) break;
    const major = u % sub === 0, mid = unit === 'in' && u % (sub / 2) === 0;
    ctx.beginPath(); ctx.moveTo(x + 0.5, 18); ctx.lineTo(x + 0.5, 18 + (major ? 18 : mid ? 12 : 7)); ctx.stroke();
    if (major) ctx.fillText(String(u / sub), x, 40);
  }
  ctx.textAlign = 'left'; ctx.fillStyle = MUTED; ctx.fillText('0 = start', x0, 52);
  ctx.textAlign = 'right'; ctx.fillText(`end = ${dist} ${unit}`, end, 52);
  ctx.fillStyle = FG; ctx.fillRect(x0 - 1, 10, 3, 18); ctx.fillRect(end - 1, 10, 3, 18);
}

function drawSensor(cv: HTMLCanvasElement, s: SensorSnap): void {
  const W = Math.min(720, Math.floor(cv.parentElement?.clientWidth ?? 480));
  const h = 150;
  const { ctx } = fitCanvas(cv, W, h);
  ctx.fillStyle = '#000'; ctx.fillRect(0, 0, W, h);
  // path trace (left half)
  const half = W / 2;
  if (s.path.length > 2) {
    const xs = s.path.map((p) => p[0]), ys = s.path.map((p) => p[1]);
    const minX = Math.min(...xs), maxX = Math.max(...xs), minY = Math.min(...ys), maxY = Math.max(...ys);
    const sc = Math.min((half - 20) / Math.max(1, maxX - minX), (h - 20) / Math.max(1, maxY - minY));
    ctx.strokeStyle = FG; ctx.lineWidth = 1; ctx.beginPath();
    s.path.forEach((p, i) => { const x = 10 + (p[0] - minX) * sc, y = 10 + (p[1] - minY) * sc; if (i) ctx.lineTo(x, y); else ctx.moveTo(x, y); });
    ctx.stroke();
  }
  // direction histogram (right half): radial bars, spokes at 45° multiples
  const cx = half + half / 2, cy = h / 2, r0 = 22, rmax = Math.min(half / 2, h / 2) - 8;
  ctx.strokeStyle = LINE_STRONG; ctx.lineWidth = 1;
  for (let k = 0; k < 8; k++) { const a = (k * Math.PI) / 4; ctx.beginPath(); ctx.moveTo(cx + Math.cos(a) * r0, cy + Math.sin(a) * r0); ctx.lineTo(cx + Math.cos(a) * (rmax + 4), cy + Math.sin(a) * (rmax + 4)); ctx.stroke(); }
  const max = Math.max(1, ...s.angles);
  ctx.strokeStyle = FG; ctx.lineWidth = 3;
  s.angles.forEach((c, i) => { const a = ((i + 0.5) * 10 * Math.PI) / 180; const len = (c / max) * (rmax - r0); ctx.beginPath(); ctx.moveTo(cx + Math.cos(a) * r0, cy + Math.sin(a) * r0); ctx.lineTo(cx + Math.cos(a) * (r0 + len), cy + Math.sin(a) * (r0 + len)); ctx.stroke(); });
}
