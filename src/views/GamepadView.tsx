// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab
// the controller: a diagram that lights per button (analog triggers fill by how far
// they are pulled), live stick dots, and the tests: circularity, drift, polling,
// rumble, plus a raw table of every control the pad reports.
import { useEffect, useMemo, useRef, useState } from 'react';
import { Stamp } from '../components/Stamp';
import { FG, LINE, LINE_STRONG, MONO, MUTED, fitCanvas } from '../lib/draw';
import { addFrame } from '../lib/raf';
import { results } from '../lib/results';
import { onFrame, sendCommand } from '../lib/session';
import type { PadItem, PollSnap, RestSnap, SessionInfo, StickSnap } from '../lib/types';
import { drawPad, PAD_H, PAD_W } from '../models/drawPad';
import { matchPad } from '../models/pads';
import { getPick } from '../models/pick';

type Tab = 'sticks' | 'polling' | 'rumble' | 'raw';
const GLOW = 110;

export function GamepadView({ info }: { info: SessionInfo }) {
  const match = useMemo(() => matchPad(info.device, getPick(info.device)), [info.device]);
  const [tab, setTab] = useState<Tab>(match.style === 'generic' ? 'raw' : 'sticks');
  const [poll, setPoll] = useState<PollSnap | null>(null);
  const [sticks, setSticks] = useState<{ left: StickSnap; right: StickSnap; rest: RestSnap } | null>(null);
  const [padInfo, setPadInfo] = useState<{ forceFeedback: boolean; power: string; battery: number | null; pollSource: string } | null>(null);
  const [raw, setRaw] = useState<PadItem[]>([]);
  const [strong, setStrong] = useState(0.7);
  const [weak, setWeak] = useState(0.7);

  const vals = useRef(new Map<string, number>());
  const upAt = useRef(new Map<string, number>());
  const rawMap = useRef(new Map<string, PadItem>());
  const dirty = useRef(true);
  const snaps = useRef<{ poll?: PollSnap; sticks?: { left: StickSnap; right: StickSnap; rest: RestSnap } }>({});
  const diagram = useRef<HTMLCanvasElement>(null);
  const diagramBox = useRef<HTMLDivElement>(null);
  const plotL = useRef<HTMLCanvasElement>(null);
  const plotR = useRef<HTMLCanvasElement>(null);
  const graph = useRef<HTMLCanvasElement>(null);
  const lastSticks = useRef<{ left: StickSnap; right: StickSnap } | null>(null);

  useEffect(() => {
    const off = onFrame((events) => {
      for (const e of events) {
        if (e.t === 'pad') {
          for (const i of e.items) {
            const was = vals.current.get(i.name) ?? 0;
            vals.current.set(i.name, i.value);
            if (!i.axis && was > 0.5 && i.value <= 0.5) upAt.current.set(i.name, performance.now());
            rawMap.current.set(`${i.name}|${i.code}`, i);
          }
          dirty.current = true;
        } else if (e.t === 'poll') snaps.current.poll = e;
        else if (e.t === 'sticks') snaps.current.sticks = e;
        else if (e.t === 'padInfo') setPadInfo(e);
      }
    });
    const t = window.setInterval(() => {
      const s = snaps.current;
      if (s.poll) { setPoll(s.poll); s.poll = undefined; }
      if (s.sticks) { setSticks(s.sticks); lastSticks.current = s.sticks; s.sticks = undefined; dirty.current = true; }
      setRaw([...rawMap.current.values()].sort((a, b) => a.name.localeCompare(b.name)));
    }, 100);
    return () => { off(); clearInterval(t); };
  }, []);

  // drawing
  useEffect(() => {
    const box = diagramBox.current!;
    let w = 0;
    const ro = new ResizeObserver(() => { w = Math.min(520, Math.floor(box.clientWidth)); dirty.current = true; });
    ro.observe(box);
    w = Math.min(520, Math.floor(box.clientWidth));
    const off = addFrame((now) => {
      const fading = [...upAt.current.values()].some((t) => now - t < GLOW);
      if (!dirty.current && !fading) return;
      dirty.current = false;
      const get = (n: string) => {
        const v = vals.current.get(n) ?? 0;
        const u = upAt.current.get(n);
        if (v <= 0.5 && u !== undefined && now - u < GLOW) return Math.max(v, 1 - (now - u) / GLOW);
        return Math.max(0, Math.min(1, v));
      };
      const stick = (side: 'left' | 'right'): [number, number] => [vals.current.get(side === 'left' ? 'LeftStickX' : 'RightStickX') ?? 0, vals.current.get(side === 'left' ? 'LeftStickY' : 'RightStickY') ?? 0];

      const cv = diagram.current;
      if (cv && w > 50) {
        const h = Math.round((w * PAD_H) / PAD_W);
        const { ctx } = fitCanvas(cv, w, h);
        ctx.clearRect(0, 0, w, h);
        drawPad(ctx, match, w, { v: get, stick });
      }
      if (plotL.current) drawPlot(plotL.current, stick('left'), lastSticks.current?.left ?? null);
      if (plotR.current) drawPlot(plotR.current, stick('right'), lastSticks.current?.right ?? null);
      if (fading) dirty.current = true;
    });
    return () => { off(); ro.disconnect(); };
  }, [match]);

  useEffect(() => {
    if (tab === 'polling' && graph.current && poll) drawGraph(graph.current, poll);
  }, [tab, poll]);
  useEffect(() => { dirty.current = true; }, [tab]);

  useEffect(() => {
    results.set('gamepad', { model: match.model?.name ?? null, style: match.style, polling: poll && { avgHz: poll.avgHz, minHz: poll.minHz, maxHz: poll.maxHz, nominal: poll.nominal, source: padInfo?.pollSource }, sticks: sticks && { left: pickStick(sticks.left), right: pickStick(sticks.right), rest: sticks.rest.entries }, battery: padInfo && { power: padInfo.power, level: padInfo.battery }, rumble: padInfo?.forceFeedback ?? null });
  }, [match, poll, sticks, padInfo]);

  const f0 = (n: number | null | undefined) => (n == null ? '-' : Math.round(n).toLocaleString());
  const pct = (n: number | null | undefined) => (n == null ? '-' : `${Math.round(n * 100)}%`);
  const running = sticks?.left.active || sticks?.right.active;
  const TABS: [Tab, string][] = [['sticks', 'sticks'], ['polling', 'polling'], ['rumble', 'rumble + battery'], ['raw', 'raw table']];

  return (
    <div className="gv">
      <div ref={diagramBox} className="gv__pad">
        <canvas ref={diagram} role="img" aria-label={`${match.model?.name ?? 'generic'} controller, lit while buttons are held`} />
        <p className="label label--sm" style={{ textAlign: 'center' }}>{match.model ? match.model.name : 'generic controller'}</p>
        {padInfo?.battery != null && <p className="label label--sm" style={{ textAlign: 'center' }}>battery {padInfo.battery}% · {padInfo.power}</p>}
      </div>

      <section className="gv__panel">
        <div className="seg" role="tablist" aria-label="controller tests">
          {TABS.map(([id, name]) => (
            <button key={id} role="tab" aria-selected={tab === id} aria-pressed={tab === id} onClick={() => setTab(id)}>{name}</button>
          ))}
        </div>

        {tab === 'sticks' && (
          <div className="gv__body">
            <div className="gv__plots">
              <figure><canvas ref={plotL} role="img" aria-label="left stick position and traced gate" /><figcaption className="label label--sm">left stick</figcaption></figure>
              <figure><canvas ref={plotR} role="img" aria-label="right stick position and traced gate" /><figcaption className="label label--sm">right stick</figcaption></figure>
            </div>
            <div className="gv__row">
              {running ? <button className="btn btn--sm" onClick={() => void sendCommand({ type: 'padCircleStop' })}>stop circle test</button> : <button className="btn btn--sm btn--solid" onClick={() => void sendCommand({ type: 'padCircleStart' })}>start circle test</button>}
              <button className="btn btn--sm" onClick={() => void sendCommand({ type: 'padCircleReset' })}>clear</button>
              <button className="btn btn--sm" disabled={sticks?.rest.running} onClick={() => void sendCommand({ type: 'padRestStart', secs: 3 })}>{sticks?.rest.running ? `hands off · ${sticks.rest.secsLeft.toFixed(1)}s` : 'measure drift (3 s)'}</button>
              {running && <Stamp tone="wip">recording</Stamp>}
            </div>
            <table className="mv__table">
              <thead><tr><th></th><th>circularity</th><th>coverage</th><th>max range</th><th>deadzone</th></tr></thead>
              <tbody>
                {(['left', 'right'] as const).map((side) => {
                  const s = sticks?.[side];
                  return (
                    <tr key={side}>
                      <td>{side} stick</td>
                      <td className="num">{s?.circularity != null ? pct(s.circularity) : s ? 'push all the way round' : '-'}</td>
                      <td className="num">{s ? pct(s.coverage) : '-'}</td>
                      <td className="num">{s ? pct(s.maxRange) : '-'}</td>
                      <td className="num">{s?.deadzone != null ? pct(s.deadzone) : '-'}</td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
            {sticks && sticks.rest.entries.length > 0 && (
              <div>
                <p className="label">resting drift {sticks.rest.running ? '· measuring' : ''}</p>
                <table className="mv__table">
                  <thead><tr><th>control</th><th>off-centre</th><th>noise</th><th></th></tr></thead>
                  <tbody>
                    {sticks.rest.entries.map((r) => (
                      <tr key={r.name}>
                        <td>{r.name}</td>
                        <td className="num">{(r.offset * 100).toFixed(1)}%</td>
                        <td className="num">{(r.noise * 100).toFixed(1)}%</td>
                        <td>{sticks.rest.running ? '' : r.drift ? <Stamp tone="problem" size="sm">drift</Stamp> : <Stamp size="sm">centred</Stamp>}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}
            <p className="case__sub">circularity is the smallest over the largest radius your stick reached all the way round: 100% is a round gate, a square gate lands near 70%. deadzone is where the output first leaves zero. drift is the resting offset, flagged above 5% (triggers above 3%). values are the pad's raw output with no filtering.</p>
          </div>
        )}

        {tab === 'polling' && (
          <div className="gv__body">
            <p className="label">polling rate · {padInfo?.pollSource === 'hid reports' ? 'from hid report timestamps' : 'from controller events'}</p>
            <p className="mv__hz num">{poll?.hz ? f0(poll.hz) : poll?.avgHz ? f0(poll.avgHz) : '0'}<span> hz{poll?.hz ? '' : poll?.avgHz ? ' avg' : ''}</span></p>
            {poll?.nominal ? <Stamp>≈ {poll.nominal} hz</Stamp> : <p className="case__sub">wiggle the sticks. an idle pad may send nothing.</p>}
            <dl className="kv mv__stats">
              <dt>average</dt><dd className="num">{f0(poll?.avgHz)} hz</dd>
              <dt>slowest 100 ms</dt><dd className="num">{f0(poll?.minHz)} hz</dd>
              <dt>fastest 100 ms</dt><dd className="num">{f0(poll?.maxHz)} hz</dd>
              <dt>jitter (σ)</dt><dd className="num">{poll?.jitterUs != null ? `${poll.jitterUs.toFixed(0)} µs` : '-'}</dd>
            </dl>
            <canvas ref={graph} className="mv__graph" role="img" aria-label="controller polling stability" />
            {padInfo?.pollSource === 'hid reports' ? (
              <p className="case__sub">real usb/bluetooth report rate: every hid input report is timestamped as it arrives.</p>
            ) : (
              <p className="case__sub">this pad could not be opened for raw hid reports (xinput pads and some drivers refuse), so this counts the events the os hands over. that tracks how often the state changes, not the usb polling rate.</p>
            )}
          </div>
        )}

        {tab === 'rumble' && (
          <div className="gv__body">
            <p className="label">vibration</p>
            {padInfo && !padInfo.forceFeedback && (
              <div className="banner"><Stamp tone="wip" size="sm">not reported</Stamp><p className="banner__p">the driver does not report force feedback for this pad. the buttons below may do nothing.</p><span /></div>
            )}
            <label className="field"><span className="label label--sm">strong motor · {Math.round(strong * 100)}%</span><input type="range" min="0" max="1" step="0.05" value={strong} onChange={(e) => setStrong(Number(e.target.value))} className="range" /></label>
            <label className="field"><span className="label label--sm">weak motor · {Math.round(weak * 100)}%</span><input type="range" min="0" max="1" step="0.05" value={weak} onChange={(e) => setWeak(Number(e.target.value))} className="range" /></label>
            <div className="gv__row">
              <button className="btn btn--sm btn--solid" onClick={() => void sendCommand({ type: 'padRumble', strong, weak, ms: 600 })}>buzz both</button>
              <button className="btn btn--sm" onClick={() => void sendCommand({ type: 'padRumble', strong, weak: 0, ms: 600 })}>strong only</button>
              <button className="btn btn--sm" onClick={() => void sendCommand({ type: 'padRumble', strong: 0, weak, ms: 600 })}>weak only</button>
            </div>
            <p className="label" style={{ marginTop: '.6rem' }}>battery</p>
            <p className="mv__result">
              {padInfo ? <Stamp tone={padInfo.power === 'unknown' ? 'wip' : 'plain'}>{padInfo.power}</Stamp> : <Stamp tone="wip">reading</Stamp>}
              {padInfo?.battery != null && <span className="num">{padInfo.battery}%</span>}
            </p>
            <p className="case__sub">{padInfo?.power === 'unknown' ? 'the driver does not expose battery state for this pad.' : 'as reported by the os. wired pads report no level.'}</p>
          </div>
        )}

        {tab === 'raw' && (
          <div className="gv__body">
            <p className="label">every control the pad has reported · {raw.length}</p>
            {raw.length === 0 && <p className="case__sub">press something. controls appear here the first time they change, with the pad's own code next to gilrs's name for it.</p>}
            <table className="mv__table gv__raw">
              <thead><tr><th>name</th><th>code</th><th>kind</th><th>value</th><th></th></tr></thead>
              <tbody>
                {raw.map((r) => (
                  <tr key={`${r.name}|${r.code}`}>
                    <td>{r.name}</td>
                    <td className="num">{r.code}</td>
                    <td>{r.axis ? 'axis' : 'button'}</td>
                    <td className="num">{r.value.toFixed(3)}</td>
                    <td className="gv__bar"><span style={{ width: `${Math.round(Math.min(1, Math.abs(r.value)) * 100)}%` }} /></td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </section>
    </div>
  );
}

const pickStick = (s: StickSnap) => ({ circularity: s.circularity, coverage: s.coverage, maxRange: s.maxRange, deadzone: s.deadzone });

/** unit-circle plot: gate ring, traced gate shape, deadzone ring, live position */
function drawPlot(cv: HTMLCanvasElement, live: [number, number], snap: StickSnap | null): void {
  const S = 190;
  const { ctx } = fitCanvas(cv, S, S);
  ctx.fillStyle = '#000'; ctx.fillRect(0, 0, S, S);
  const c = S / 2, R = S / 2 - 12;
  ctx.strokeStyle = LINE; ctx.lineWidth = 1;
  ctx.beginPath(); ctx.moveTo(c, 4); ctx.lineTo(c, S - 4); ctx.moveTo(4, c); ctx.lineTo(S - 4, c); ctx.stroke();
  ctx.strokeStyle = LINE_STRONG; ctx.setLineDash([3, 3]); ctx.beginPath(); ctx.arc(c, c, R, 0, Math.PI * 2); ctx.stroke(); ctx.setLineDash([]);
  if (snap) {
    if (snap.deadzone) { ctx.strokeStyle = MUTED; ctx.beginPath(); ctx.arc(c, c, snap.deadzone * R, 0, Math.PI * 2); ctx.stroke(); }
    if (snap.path.length > 1) {
      ctx.strokeStyle = 'rgb(236 236 236 / .35)'; ctx.lineWidth = 1; ctx.beginPath();
      snap.path.forEach((p, i) => { const x = c + p[0] * R, y = c - p[1] * R; if (i) ctx.lineTo(x, y); else ctx.moveTo(x, y); });
      ctx.stroke();
    }
    // traced gate: max radius reached per 5 degree bin
    const n = snap.bins.length;
    if (n && snap.bins.some((b) => b > 0)) {
      ctx.strokeStyle = FG; ctx.lineWidth = 1.8; ctx.beginPath();
      snap.bins.forEach((b, i) => { const a = ((i + 0.5) / n) * Math.PI * 2; const x = c + Math.cos(a) * b * R, y = c - Math.sin(a) * b * R; if (i) ctx.lineTo(x, y); else ctx.moveTo(x, y); });
      ctx.closePath(); ctx.stroke();
    }
  }
  const x = c + live[0] * R, y = c - live[1] * R;
  ctx.fillStyle = '#fff'; ctx.beginPath(); ctx.arc(x, y, 5, 0, Math.PI * 2); ctx.fill();
  ctx.strokeStyle = '#000'; ctx.lineWidth = 1.5; ctx.stroke();
  ctx.font = `400 9px ${MONO}`; ctx.fillStyle = MUTED; ctx.textAlign = 'left'; ctx.textBaseline = 'top';
  ctx.fillText(`${live[0].toFixed(2)}, ${live[1].toFixed(2)}`, 6, 5);
}

function drawGraph(cv: HTMLCanvasElement, p: PollSnap): void {
  const W = Math.min(720, Math.floor(cv.parentElement?.clientWidth ?? 480));
  const h = 90;
  const { ctx } = fitCanvas(cv, W, h);
  ctx.fillStyle = '#000'; ctx.fillRect(0, 0, W, h);
  const g = p.graph;
  if (g.length < 2) { ctx.fillStyle = MUTED; ctx.font = `400 11px ${MONO}`; ctx.textBaseline = 'middle'; ctx.fillText('waiting for movement', 10, h / 2); return; }
  const avg = p.avgHz ?? g.reduce((a, b) => a + b, 0) / g.length;
  const lo = Math.min(...g, avg) * 0.95, hi = Math.max(...g, avg) * 1.05;
  const y = (v: number) => h - 14 - ((v - lo) / Math.max(1e-6, hi - lo)) * (h - 28);
  ctx.strokeStyle = LINE_STRONG; ctx.setLineDash([3, 3]); ctx.beginPath(); ctx.moveTo(0, y(avg)); ctx.lineTo(W, y(avg)); ctx.stroke(); ctx.setLineDash([]);
  ctx.strokeStyle = FG; ctx.lineWidth = 1.5; ctx.beginPath();
  g.forEach((v, i) => { const x = (i / 59) * W; if (i) ctx.lineTo(x, y(v)); else ctx.moveTo(x, y(v)); });
  ctx.stroke();
  ctx.fillStyle = MUTED; ctx.font = `400 10px ${MONO}`; ctx.textBaseline = 'top'; ctx.fillText(`${Math.round(hi)} hz`, 6, 3); ctx.textBaseline = 'bottom'; ctx.fillText(`${Math.round(lo)} hz`, 6, h - 3);
}
