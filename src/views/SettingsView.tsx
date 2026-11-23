// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab
import { useState } from 'react';
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

      <NumberField label="esc hold (mouse tests)" value={s.escHoldMs} min={300} max={5000} unit="ms" hint="how long esc is held to leave a mouse test. 300 to 5000." onValid={(n) => updateSettings({ escHoldMs: n })} />
      <NumberField label="exit combo hold (keyboard tests)" value={s.comboHoldMs} min={500} max={6000} unit="ms" hint="how long ctrl+alt+shift+backspace is held. 500 to 6000." onValid={(n) => updateSettings({ comboHoldMs: n })} />

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
