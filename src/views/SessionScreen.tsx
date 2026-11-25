// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab · source viewable, not reusable: see LICENSE
// the shell around every test. what it shows for exit depends on the rules:
//   mouse    -> no controls at all, just the esc-hold ring and hint
//   keyboard -> big exit button + the persistent combo hint + ring
//   other    -> big exit button only
import { useEffect, useState } from 'react';
import { onFrame } from '../lib/session';
import { ExitRing } from '../components/ExitRing';
import { Kbd } from '../components/Kbd';
import { Stamp } from '../components/Stamp';
import { endSession } from '../lib/session';
import { useStore } from '../lib/store';
import { exitComboKeys } from '../lib/osKeys';
import { SessionBody } from './SessionBody';
import { deviceTitle, usePick } from '../models/pick';

export function SessionScreen() {
  const s = useStore((st) => st.session);
  const os = useStore((st) => st.info?.os);
  const pick = usePick(useStore((st) => st.session?.device) ?? { id: '', vid: null, pid: null });
  const [notices, setNotices] = useState<string[]>([]);
  const [capText, setCapText] = useState('');
  useEffect(() => onFrame((evs) => { for (const e of evs) { if (e.t === 'notice') setNotices((n) => (n.includes(e.text) ? n : [...n, e.text])); if (e.t === 'capture') setCapText(e.text); } }), []);
  const gone = useStore((st) => st.session !== null && st.devices.some((d) => d.id === st.session!.device.id && !d.connected));

  useEffect(() => {
    // in a session no key may do its normal thing. rust/the os layer blocks it at the lowest
    // level; this is the second layer for platforms where the hook is not available.
    const stop = (e: Event) => e.preventDefault();
    addEventListener('keydown', stop, true);
    addEventListener('contextmenu', stop, true);
    return () => {
      removeEventListener('keydown', stop, true);
      removeEventListener('contextmenu', stop, true);
    };
  }, []);

  if (!s) return null;
  const c = s.capture;
  const secs = (ms: number) => `${(ms / 1000).toFixed(ms % 1000 === 0 ? 0 : 1)}s`;

  return (
    <div className="session">
      <header className="session__top">
        <div className="session__id">
          <p className="label">test in progress</p>
          <h1 className="title title--sm">{deviceTitle(s.device, pick)}</h1>
        </div>
        <div className="session__stamps">
          {gone ? <Stamp tone="problem">unplugged</Stamp> : <Stamp>live</Stamp>}
          {c.cursorClipped && <Stamp size="sm">cursor locked</Stamp>}
          {c.mouseSwallowed && <Stamp size="sm">clicks swallowed</Stamp>}
          {c.keyboardHooked && <Stamp size="sm">shortcuts blocked</Stamp>}
          {!c.active && <Stamp tone="wip" size="sm">not captured</Stamp>}
        </div>
      </header>

      <div className="session__notes">
        {c.warnings.map((w) => (
          <div key={w} className="banner">
            <Stamp tone="wip" size="sm">note</Stamp>
            <p className="banner__p">{w}</p>
            <span />
          </div>
        ))}
        {notices.map((t) => (
          <div key={t} className="banner banner--problem">
            <Stamp tone="problem" size="sm">raw input</Stamp>
            <p className="banner__p">{t}</p>
            <span />
          </div>
        ))}
        {s.comboWarning && (
          <div className="banner banner--problem">
            <Stamp tone="problem" size="sm">warning</Stamp>
            <p className="banner__p">{s.comboWarning}</p>
            <span />
          </div>
        )}
      </div>

      <section className="session__body" aria-label="live view">
        <SessionBody info={s} />
      </section>

      <footer className="session__exit">
        {s.exit.kind === 'combo' && <ExitRing kind="combo" label="exit combo progress" />}
        <div className="session__hint">
          {s.mode === 'mouse' && (
            <p className="label">
              press <Kbd>esc</Kbd> to leave{s.exit.button ? ' / or use the exit button' : ''}
            </p>
          )}
          {s.mode === 'keyboard' && (
            <p className="label">
              or hold{' '}
              {exitComboKeys(os).map((k, i) => (
                <span key={k}>
                  {i > 0 && ' + '}
                  <Kbd>{k}</Kbd>
                </span>
              ))}{' '}
              for {secs(s.exit.holdMs)}
            </p>
          )}
          {s.mode === 'other' && <p className="label">controller and device buttons never exit. use the button.</p>}
          {capText && <p className="case__sub num" aria-label="capture diagnostics">{capText}</p>}
        </div>
        {s.exit.button && (
          <button className="btn btn--solid btn--lg" onClick={() => void endSession()}>
            exit test
          </button>
        )}
      </footer>
    </div>
  );
}
