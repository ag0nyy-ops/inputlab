// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab · source viewable, not reusable: see LICENSE
// what a playstation pad has beyond buttons and sticks: gyro + accelerometer, adaptive triggers,
// and its own speaker, microphone and haptic motors. the audio parts need a usb cable, because the
// controller only shows up as an audio device over usb.
import { useEffect, useRef, useState } from 'react';
import { Stamp } from '../components/Stamp';
import { FG, LINE, LINE_STRONG, MONO, MUTED, fitCanvas } from '../lib/draw';
import { MicMeter, PadSpeaker, findPadAudio, recordClip, type PadAudioDevices } from '../lib/padAudio';
import { MotionTracker, type V3 } from '../lib/padMotion';
import { addFrame } from '../lib/raf';
import { onFrame, sendCommand } from '../lib/session';

interface Props {
  bus: string;
  hidPad: string;
  /** current value of a control, 0..1 */
  get: (name: string) => number;
}

const USB_ONLY = 'connect via usb-c';

export function PadExtras({ bus, hidPad, get }: Props) {
  const dualsense = hidPad === 'dualsense';
  const bluetooth = bus === 'bluetooth';
  return (
    <div className="gv__body">
      <Motion />
      {dualsense ? <Triggers get={get} /> : <p className="case__sub">adaptive triggers and the controller's own audio are dualsense features.</p>}
      {dualsense && <Audio off={bluetooth} />}
    </div>
  );
}

// ------------------------------------------------------------------ motion

function Motion() {
  const tracker = useRef(new MotionTracker());
  const cv = useRef<HTMLCanvasElement>(null);
  const [view, setView] = useState({ rates: [0, 0, 0] as V3, accel: [0, 0, 0] as V3, heading: 0, tilt: 0, settling: true, seen: false });
  const seen = useRef(false);

  useEffect(() => {
    const off = onFrame((events) => {
      for (const e of events) {
        if (e.t === 'motion') {
          seen.current = true;
          tracker.current.feed(e.gyro, e.accel, performance.now());
        }
      }
    });
    const t = window.setInterval(() => {
      const m = tracker.current;
      setView({ rates: [...m.rates] as V3, accel: [...m.accel] as V3, heading: m.heading, tilt: m.tiltDeg, settling: m.settling, seen: seen.current });
    }, 100);
    const stopDraw = addFrame(() => {
      if (cv.current) drawLevel(cv.current, tracker.current);
    });
    return () => { off(); clearInterval(t); stopDraw(); };
  }, []);

  const f1 = (n: number) => n.toFixed(1).padStart(6, ' ');
  return (
    <section>
      <p className="label">gyro + accelerometer</p>
      <div className="gv__row">
        <button className="btn btn--sm" onClick={() => tracker.current.zero()}>zero (hold still 1 s)</button>
        {view.settling ? <Stamp tone="wip">{view.seen ? 'hold it still' : 'no motion data yet'}</Stamp> : <Stamp>zeroed</Stamp>}
      </div>
      <div className="pe__motion">
        <canvas ref={cv} role="img" aria-label="a ball that rolls to the low side of the tilted controller" />
        <table className="mv__table">
          <thead><tr><th></th><th>rate (°/s)</th><th>accel (g)</th></tr></thead>
          <tbody>
            {['pitch', 'yaw', 'roll'].map((n, i) => (
              <tr key={n}><td>{n}</td><td className="num">{f1(view.rates[i]!)}</td><td className="num">{view.accel[i]!.toFixed(2)}</td></tr>
            ))}
            <tr><td>turned</td><td className="num" colSpan={2}>{view.heading.toFixed(0)}° since zero</td></tr>
            <tr><td>tipped</td><td className="num" colSpan={2}>{view.tilt.toFixed(0)}° from level at zero</td></tr>
          </tbody>
        </table>
      </div>
      <p className="case__sub">rates are the gyro's raw output scaled to about ±2000°/s with its resting offset removed, and are not factory-calibrated. the turn angle slowly drifts, like any gyro without a compass. if the ball rolls the wrong way, or the axes look swapped, tell me which pad it is.</p>
    </section>
  );
}

function drawLevel(cv: HTMLCanvasElement, m: MotionTracker): void {
  const S = 190;
  const { ctx } = fitCanvas(cv, S, S);
  ctx.fillStyle = '#000'; ctx.fillRect(0, 0, S, S);
  const c = S / 2, R = S / 2 - 12;
  ctx.strokeStyle = LINE; ctx.lineWidth = 1;
  ctx.beginPath(); ctx.moveTo(c, 4); ctx.lineTo(c, S - 4); ctx.moveTo(4, c); ctx.lineTo(S - 4, c); ctx.stroke();
  ctx.strokeStyle = LINE_STRONG; ctx.setLineDash([3, 3]);
  ctx.beginPath(); ctx.arc(c, c, R, 0, Math.PI * 2); ctx.stroke();
  ctx.beginPath(); ctx.arc(c, c, R / 2, 0, Math.PI * 2); ctx.stroke();
  ctx.setLineDash([]);
  // heading needle: turning the pad flat on the table spins it
  const a = (-m.heading * Math.PI) / 180 - Math.PI / 2;
  ctx.strokeStyle = MUTED; ctx.lineWidth = 1.5;
  ctx.beginPath(); ctx.moveTo(c, c); ctx.lineTo(c + Math.cos(a) * (R - 4), c + Math.sin(a) * (R - 4)); ctx.stroke();
  // the ball
  const bx = c + m.ball[0] * R * 0.9, by = c - m.ball[1] * R * 0.9;
  ctx.fillStyle = m.settling ? MUTED : '#fff';
  ctx.beginPath(); ctx.arc(bx, by, 7, 0, Math.PI * 2); ctx.fill();
  ctx.strokeStyle = '#000'; ctx.lineWidth = 1.5; ctx.stroke();
  ctx.font = `400 9px ${MONO}`; ctx.fillStyle = FG; ctx.textAlign = 'left'; ctx.textBaseline = 'top';
  ctx.fillText(m.settling ? 'settling' : `${m.tiltDeg.toFixed(0)}°`, 6, 5);
}

// --------------------------------------------------------------- triggers

function Triggers({ get }: { get: (n: string) => number }) {
  const [start, setStart] = useState(0.3);
  const [force, setForce] = useState(0.8);
  const l = useRef<HTMLSpanElement>(null);
  const r = useRef<HTMLSpanElement>(null);
  useEffect(() => addFrame(() => {
    if (l.current) l.current.style.width = `${Math.round(get('LeftTrigger2') * 100)}%`;
    if (r.current) r.current.style.width = `${Math.round(get('RightTrigger2') * 100)}%`;
  }), [get]);
  const send = (right: boolean, on: boolean) => void sendCommand({ type: 'padTrigger', right, start, force: on ? force : 0 });
  return (
    <section>
      <p className="label">adaptive triggers</p>
      <label className="field"><span className="label label--sm">starts pushing back after {Math.round(start * 100)}% of the pull</span><input type="range" min="0" max="1" step="0.05" value={start} onChange={(e) => setStart(Number(e.target.value))} className="range" /></label>
      <label className="field"><span className="label label--sm">resistance · {Math.round(force * 100)}%</span><input type="range" min="0.05" max="1" step="0.05" value={force} onChange={(e) => setForce(Number(e.target.value))} className="range" /></label>
      <div className="gv__row">
        <button className="btn btn--sm btn--solid" onClick={() => { send(false, true); send(true, true); }}>stiffen both</button>
        <button className="btn btn--sm" onClick={() => send(false, true)}>left (L2) only</button>
        <button className="btn btn--sm" onClick={() => send(true, true)}>right (R2) only</button>
        <button className="btn btn--sm" onClick={() => { send(false, false); send(true, false); }}>back to normal</button>
      </div>
      <div className="pe__bars">
        <span className="label label--sm">L2</span><span className="pe__bar"><span ref={l} /></span>
        <span className="label label--sm">R2</span><span className="pe__bar"><span ref={r} /></span>
      </div>
      <p className="case__sub">pull a trigger after stiffening it: it should push back. the triggers go back to normal when the test ends. this is unverified on real hardware, so say if nothing happens or the start point feels wrong.</p>
    </section>
  );
}

// ------------------------------------------------------------------ audio

type Step = 'idle' | 'looking' | 'ready';

function Audio({ off }: { off: boolean }) {
  const [step, setStep] = useState<Step>('idle');
  const [devs, setDevs] = useState<PadAudioDevices>({ input: null, output: null });
  const [msg, setMsg] = useState('');
  const [speaker, setSpeaker] = useState(0.6);
  const [mic, setMic] = useState(0.6);
  const [meter, setMeter] = useState<{ level: number; db: number } | null>(null);
  const [recording, setRecording] = useState(false);
  const [clip, setClip] = useState<Blob | null>(null);
  const [hz, setHz] = useState(120);
  const [hap, setHap] = useState(0.5);
  const spk = useRef<PadSpeaker | null>(null);
  const mm = useRef(new MicMeter());

  useEffect(() => () => { mm.current.stop(); spk.current?.close(); }, []);

  const say = (e: unknown) => setMsg(e instanceof Error ? e.message : String(e));
  const find = async () => {
    setStep('looking'); setMsg('');
    try {
      const { devices, note } = await findPadAudio();
      setDevs(devices);
      if (note) setMsg(note);
      else if (!devices.input && !devices.output) setMsg("no controller audio device found. over usb, windows lists it as 'wireless controller': unplug and replug the cable, then try again.");
      setStep('ready');
    } catch (e) { say(e); setStep('idle'); }
  };
  const speaker_ = async (): Promise<PadSpeaker | null> => {
    if (!devs.output) { setMsg('the controller has no speaker device here.'); return null; }
    if (!spk.current) {
      const s = new PadSpeaker();
      try { await s.open(devs.output.deviceId); } catch (e) { s.close(); say(e); return null; }
      spk.current = s;
    }
    // the speaker stays silent until the pad is told its volume
    void sendCommand({ type: 'padAudio', speaker, mic });
    return spk.current;
  };
  const tone = async (side: 'both' | 'left' | 'right') => { const s = await speaker_(); s?.tone(440, 1200, side); };
  const haptic = async () => {
    const s = await speaker_();
    if (s && !s.haptic(hz, 900, hap)) setMsg(`windows gives this controller ${s.channels} audio channels here, and the haptic motors are on channels 3 and 4, so they cannot be driven from this window.`);
  };
  const startMeter = async () => {
    if (!devs.input) return;
    void sendCommand({ type: 'padAudio', speaker, mic });
    try { await mm.current.start(devs.input.deviceId, (level, db) => setMeter({ level, db })); } catch (e) { say(e); }
  };
  const stopMeter = () => { mm.current.stop(); setMeter(null); };
  const record = async () => {
    if (!devs.input) return;
    void sendCommand({ type: 'padAudio', speaker, mic });
    setRecording(true); setClip(null);
    try { setClip(await recordClip(devs.input.deviceId, 3000)); } catch (e) { say(e); }
    setRecording(false);
  };
  const playClip = async () => { const s = await speaker_(); if (s && clip) try { await s.play(clip); } catch (e) { say(e); } };

  const dis = off || step !== 'ready';
  return (
    <section className={off ? 'pe--off' : ''}>
      <p className="label">speaker, microphone and haptics {off && <Stamp tone="wip" size="sm">{USB_ONLY}</Stamp>}</p>
      {off ? (
        <p className="case__sub">the controller only shows up as an audio device over a usb cable. {USB_ONLY} to test its speaker, microphone and haptic motors.</p>
      ) : (
        <div className="gv__row">
          <button className="btn btn--sm btn--solid" disabled={step === 'looking'} onClick={() => void find()}>{step === 'ready' ? 'look again' : step === 'looking' ? 'looking' : 'find the controller audio'}</button>
          {step === 'ready' && <span className="case__sub">speaker: {devs.output ? 'found' : 'not found'} · microphone: {devs.input ? 'found' : 'not found'}</span>}
        </div>
      )}
      {msg && <div className="banner"><Stamp tone="wip" size="sm">note</Stamp><p className="banner__p">{msg}</p><span /></div>}

      <fieldset className="pe__group" disabled={dis || !devs.output}>
        <label className="field"><span className="label label--sm">speaker volume · {Math.round(speaker * 100)}%</span><input type="range" min="0.05" max="1" step="0.05" value={speaker} onChange={(e) => setSpeaker(Number(e.target.value))} className="range" /></label>
        <div className="gv__row">
          <button className="btn btn--sm btn--solid" onClick={() => void tone('both')}>play a tone</button>
          <button className="btn btn--sm" onClick={() => void tone('left')}>left only</button>
          <button className="btn btn--sm" onClick={() => void tone('right')}>right only</button>
        </div>
      </fieldset>

      <fieldset className="pe__group" disabled={dis || !devs.input}>
        <label className="field"><span className="label label--sm">microphone gain · {Math.round(mic * 100)}%</span><input type="range" min="0.1" max="1" step="0.05" value={mic} onChange={(e) => setMic(Number(e.target.value))} className="range" /></label>
        <div className="gv__row">
          {meter ? <button className="btn btn--sm" onClick={stopMeter}>stop the meter</button> : <button className="btn btn--sm btn--solid" onClick={() => void startMeter()}>listen (level meter)</button>}
          <button className="btn btn--sm" disabled={recording} onClick={() => void record()}>{recording ? 'recording 3 s' : 'record 3 s'}</button>
          <button className="btn btn--sm" disabled={!clip} onClick={() => void playClip()}>play it back</button>
        </div>
        <div className="pe__bars"><span className="label label--sm">level</span><span className="pe__bar"><span style={{ width: `${Math.round((meter?.level ?? 0) * 100)}%` }} /></span><span className="num">{meter ? `${meter.db.toFixed(0)} dB` : '-'}</span></div>
      </fieldset>

      <fieldset className="pe__group" disabled={dis || !devs.output}>
        <label className="field"><span className="label label--sm">haptic pitch · {hz} Hz</span><input type="range" min="40" max="300" step="10" value={hz} onChange={(e) => setHz(Number(e.target.value))} className="range" /></label>
        <label className="field"><span className="label label--sm">haptic strength · {Math.round(hap * 100)}%</span><input type="range" min="0.1" max="1" step="0.05" value={hap} onChange={(e) => setHap(Number(e.target.value))} className="range" /></label>
        <div className="gv__row"><button className="btn btn--sm btn--solid" onClick={() => void haptic()}>buzz the haptics</button></div>
      </fieldset>
      <p className="case__sub">the microphone is only read for the level meter and a 3 s clip held in memory until you play it back: nothing is saved or sent anywhere. holding the controller's speaker next to its microphone will squeal, so use the meter and the clip one at a time. unverified on real hardware.</p>
    </section>
  );
}
