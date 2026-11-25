// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab · source viewable, not reusable: see LICENSE
import { useEffect, useRef, useState } from 'react';
import { fitCanvas } from '../lib/draw';
import { resetSettings, updateSettings, useSettings } from '../lib/settings';

function NumberField({ label, value, min, max, unit, onValid, hint }: { label: string; value: number; min: number; max: number; unit: string; hint: string; onValid: (n: number) => void }) {
  const [text, setText] = useState(String(value));
  const n = Number(text);
  const bad = !Number.isFinite(n) || n < min || n > max;
  return (
    <div className="field">
      <label className="label" htmlFor={label}>{label}</label>
      <div style={{ display: 'flex', gap: '.6rem', alignItems: 'center' }}>
        <input
          id={label}
          className="input"
          style={{ maxWidth: '10rem' }}
          inputMode="numeric"
          value={text}
          aria-invalid={bad}
          onChange={(e) => {
            setText(e.target.value);
            const v = Number(e.target.value);
            if (Number.isFinite(v) && v >= min && v <= max) onValid(v);
          }}
        />
        <span className="label label--sm">{unit}</span>
      </div>
      {bad ? (
        <p className="err" role="alert"><span className="err__tag">ERR</span><span>{hint}</span></p>
      ) : (
        <p className="case__sub">{hint}</p>
      )}
    </div>
  );
}

export function SettingsView() {
  const s = useSettings();
  return (
    <article className="panel files">
      <header className="file">
        <p className="label file__no">settings</p>
        <h1 className="title" style={{ marginTop: '.5rem' }}>settings</h1>
        <div className="rule" />
      </header>

      <div className="field">
        <span className="label">safe mode</span>
        <button className="toggle" role="switch" aria-checked={s.safeMode} onClick={() => updateSettings({ safeMode: !s.safeMode })}>
          <span className="toggle__box" />
          <span className="label label--sm">{s.safeMode ? 'on: nothing is locked' : 'off: tests capture the device'}</span>
        </button>
        <p className="case__sub">same as starting with --no-capture. the mouse and keyboard stay free, nothing is hooked, and the exit button works in every test. turn it on the first time you try a new machine.</p>
      </div>

      <NumberField label="exit combo hold (keyboard tests)" value={s.comboHoldMs} min={500} max={6000} unit="ms" hint="how long ctrl+alt+shift+backspace is held. 500 to 6000." onValid={(n) => updateSettings({ comboHoldMs: n })} />

      <NumberField label="chatter threshold" value={s.chatterMs} min={5} max={200} unit="ms" hint="a key or button that comes back faster than this after release counts as chatter. 5 to 200." onValid={(n) => updateSettings({ chatterMs: n })} />

      <div className="field">
        <span className="label">dpi test distance</span>
        <div style={{ display: 'flex', gap: '.8rem', flexWrap: 'wrap', alignItems: 'center' }}>
          <DistanceInput />
          <div className="seg" role="group" aria-label="distance unit">
            {(['in', 'cm'] as const).map((u) => (
              <button key={u} aria-pressed={s.distanceUnit === u} onClick={() => updateSettings({ distanceUnit: u, dpiDistance: u === 'in' ? 2 : 5 })}>{u === 'in' ? 'inches' : 'centimetres'}</button>
            ))}
          </div>
        </div>
        <p className="case__sub">default is 2 in or 5 cm. a longer move averages out your hand.</p>
      </div>

      <RulerCalibration />

      <div className="field">
        <span className="label">sight</span>
        <div className="seg" role="group" aria-label="visual effects">
          {(['full', 'balanced', 'low'] as const).map((f) => (
            <button key={f} aria-pressed={s.fx === f} onClick={() => updateSettings({ fx: f })}>
              {f === 'full' ? 'full' : f === 'balanced' ? 'limited' : 'min'}
            </button>
          ))}
        </div>
        <p className="case__sub">grain and vignette. min turns them off.</p>
      </div>

      <div><button className="btn" onClick={resetSettings}>reset to defaults</button></div>
    </article>
  );
}

function DistanceInput() {
  const s = useSettings();
  const [text, setText] = useState(String(s.dpiDistance));
  const n = Number(text);
  const bad = !Number.isFinite(n) || n < 0.5 || n > 40;
  return (
    <div className="field">
      <label className="sr" htmlFor="dpi-dist">distance</label>
      <input id="dpi-dist" className="input" style={{ maxWidth: '8rem' }} inputMode="decimal" value={text} aria-invalid={bad} onChange={(e) => { setText(e.target.value); const v = Number(e.target.value); if (Number.isFinite(v) && v >= 0.5 && v <= 40) updateSettings({ dpiDistance: v }); }} />
      {bad && <p className="err" role="alert"><span className="err__tag">ERR</span><span>0.5 to 40.</span></p>}
    </div>
  );
}

/** hold a real ruler to the screen and stretch this one until they match: it sets css px per inch */
function RulerCalibration() {
  const s = useSettings();
  const cv = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    const W = 640, h = 56;
    const { ctx } = fitCanvas(cv.current!, W, h);
    ctx.fillStyle = '#000'; ctx.fillRect(0, 0, W, h);
    ctx.strokeStyle = '#ececec'; ctx.fillStyle = '#ececec'; ctx.font = '10px "IBM Plex Mono", monospace'; ctx.textAlign = 'center';
    for (let u = 0; u * (s.rulerPxPerInch / 8) < W - 20; u++) {
      const x = 10 + (u * s.rulerPxPerInch) / 8;
      const major = u % 8 === 0;
      ctx.beginPath(); ctx.moveTo(x + 0.5, 4); ctx.lineTo(x + 0.5, 4 + (major ? 24 : u % 4 === 0 ? 16 : 9)); ctx.stroke();
      if (major) ctx.fillText(String(u / 8), x, 44);
    }
  }, [s.rulerPxPerInch]);
  return (
    <div className="field">
      <label className="label" htmlFor="ruler">on-screen ruler size · {s.rulerPxPerInch} px per inch</label>
      <input id="ruler" className="range" type="range" min="60" max="220" step="1" value={s.rulerPxPerInch} onChange={(e) => updateSettings({ rulerPxPerInch: Number(e.target.value) })} />
      <canvas ref={cv} className="mv__ruler" role="img" aria-label="calibration ruler" />
      <p className="case__sub">hold a real ruler to the screen and slide until the inch marks line up. only the on-screen guide in the dpi test uses this; the dpi result never does.</p>
    </div>
  );
}
