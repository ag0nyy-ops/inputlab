// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab · source viewable, not reusable: see LICENSE
// the file after a test: what was measured, plus json export and copy-as-text.
import { useMemo, useState } from 'react';
import { Stamp } from '../components/Stamp';
import { buildJson, buildText } from '../lib/export';
import { endMessages } from '../lib/messages';
import { results } from '../lib/results';
import { setState, useStore } from '../lib/store';
import { deviceTitle, getPick } from '../models/pick';

export function SummaryView() {
  const last = useStore((s) => s.lastSession);
  const app = useStore((s) => s.info);
  const [note, setNote] = useState<string | null>(null);

  const input = useMemo(() => {
    if (!last) return null;
    const d = last.info.device;
    const pick = getPick(d);
    return { app, info: last.info, results: results.get(), startedAt: last.startedAt, endedAt: last.endedAt, reason: last.reason, picks: { mouse: pick, pad: pick } };
  }, [last, app]);

  if (!last || !input) return null;
  const text = buildText(input);
  const reason = endMessages[last.reason];

  const copy = async (what: string, value: string) => {
    try {
      await navigator.clipboard.writeText(value);
      setNote(`${what} copied.`);
    } catch {
      setNote('the clipboard is blocked here. select the text below and copy it by hand.');
    }
  };
  const download = () => {
    const blob = new Blob([JSON.stringify(buildJson(input), null, 2)], { type: 'application/json' });
    const a = document.createElement('a');
    a.href = URL.createObjectURL(blob);
    a.download = `inputlab-${last.info.device.kind}-${new Date(last.endedAt).toISOString().slice(0, 19).replace(/[:T]/g, '-')}.json`;
    document.body.append(a);
    a.click();
    a.remove();
    setTimeout(() => URL.revokeObjectURL(a.href), 4000);
    setNote('json saved to your downloads. if nothing happened, use copy json.');
  };

  return (
    <article className="panel files">
      <header className="file">
        <p className="label file__no">session summary</p>
        <h1 className="title" style={{ marginTop: '.5rem' }}>{deviceTitle(last.info.device, getPick(last.info.device))}</h1>
        <div className="rule" />
      </header>

      <div className={`banner ${reason.problem ? 'banner--problem' : ''}`}>
        <Stamp tone={reason.problem ? 'problem' : 'plain'} size="sm">ended</Stamp>
        <div><p className="banner__t">{reason.title}</p><p className="banner__p">{reason.text}</p></div>
        <span />
      </div>

      <pre className="summary__text" tabIndex={0} aria-label="session report as text">{text}</pre>

      <div className="gv__row">
        <button className="btn btn--solid" onClick={() => void copy('report', text)}>copy as text</button>
        <button className="btn" onClick={download}>export json</button>
        <button className="btn" onClick={() => void copy('json', JSON.stringify(buildJson(input), null, 2))}>copy json</button>
        <button className="btn" onClick={() => setState({ screen: 'home' })}>back to devices</button>
      </div>
      {note && <p className="case__sub" role="status">{note}</p>}
      <p className="case__sub">the report holds what the test measured and the device's name and usb ids. nothing leaves this machine unless you copy or save it.</p>
    </article>
  );
}
