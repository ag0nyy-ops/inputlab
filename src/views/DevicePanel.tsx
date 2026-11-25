// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab · source viewable, not reusable: see LICENSE
import { useState } from 'react';
import { Eye } from '../components/Eye';
import { MouseModelPicker, PadModelPicker } from '../components/ModelPicker';
import { Kbd } from '../components/Kbd';
import { Stamp } from '../components/Stamp';
import { backendLabel, kindLabel, vidpid } from '../lib/format';
import { endMessages } from '../lib/messages';
import { exitComboKeys, keyboardLockText } from '../lib/osKeys';
import { startSession } from '../lib/session';
import { useSettings } from '../lib/settings';
import { selectedDevice, useStore } from '../lib/store';
import { deviceTitle, usePick } from '../models/pick';
import type { DeviceInfo } from '../lib/types';

function Combo({ keys }: { keys: string[] }) {
  return (
    <>
      {keys.map((k, i) => (
        <span key={k}>
          {i > 0 && ' + '}
          <Kbd>{k}</Kbd>
        </span>
      ))}
    </>
  );
}

function Rules({ dev }: { dev: DeviceInfo }) {
  const s = useSettings();
  const flag = useStore((st) => st.info?.noCapture);
  const os = useStore((st) => st.info?.os);
  const lock = keyboardLockText(os);
  const noCapture = flag || s.safeMode;
  const combo = (s.comboHoldMs / 1000).toString();
  return (
    <div className="callout">
      <p className="label" style={{ marginBottom: '.6rem' }}>how leaving this test works</p>
      {noCapture ? (
        <p>capture is off (--no-capture). nothing is locked, and the exit button works in every test.</p>
      ) : dev.viewer === 'mouse' ? (
        <p>
          the mouse is fully captured: the cursor is locked to this window and hidden, clicks and wheel never reach other apps, and there is no exit button.
          press <Kbd>esc</Kbd> on a keyboard to leave.
        </p>
      ) : dev.viewer === 'keyboard' ? (
        <p>
          shortcuts are blocked so they register as test keys: {lock.blocked}. leave with the exit button, or hold{' '}
          <Combo keys={exitComboKeys(os)} /> for {combo}s from any keyboard. {lock.reserved}
        </p>
      ) : (
        <p>keyboard shortcuts are blocked during this test, and controller buttons never exit. the only way out is the exit button, with the mouse.</p>
      )}
    </div>
  );
}

export function DevicePanel() {
  const dev = useStore(selectedDevice);
  const picked = usePick(dev ?? { id: '', vid: null, pid: null });
  const info = useStore((s) => s.info);
  const lastEnd = useStore((s) => s.lastEnd);
  const [error, setError] = useState<string | null>(null);
  const [starting, setStarting] = useState(false);

  const notice = lastEnd ? endMessages[lastEnd.reason] : null;
  const noticeEl = notice && (
    <div className={`banner ${notice.problem ? 'banner--problem' : ''}`}>
      <Stamp tone={notice.problem ? 'problem' : 'plain'} size="sm">ended</Stamp>
      <div>
        <p className="banner__t">{notice.title}</p>
        <p className="banner__p">{notice.text}</p>
      </div>
      <span />
    </div>
  );

  if (!dev) {
    return (
      <div className="empty">
        {noticeEl}
        <Eye className="eye--hero" blink={7.7} delay={-1.2} />
        <p className="label">pick a device</p>
        <p className="panel__lead">every keyboard, mouse, controller, midi and hid gadget the os can see is on the left. choose one to open its file.</p>
      </div>
    );
  }

  const needsRaw = dev.viewer === 'keyboard' || dev.viewer === 'mouse';
  const blocked = needsRaw && info && !info.caps.rawInput;

  const start = async () => {
    setError(null);
    setStarting(true);
    try {
      await startSession(dev.id);
    } catch (e) {
      setError(String(e));
    } finally {
      setStarting(false);
    }
  };

  return (
    <article className="panel files">
      {noticeEl}
      <header className="file">
        <p className="label file__no">{kindLabel[dev.kind]}</p>
        <h1 className="title" style={{ marginTop: '.5rem' }}>{deviceTitle(dev, picked)}</h1>
        <div className="rule" />
      </header>

      <dl className="kv">
        <dt>vid / pid</dt>
        <dd>{vidpid(dev.vid, dev.pid)}</dd>
        <dt>read by</dt>
        <dd>{backendLabel(dev.backend)}</dd>
        <dt>interfaces</dt>
        <dd>{dev.interfaces}</dd>
        <dt>state</dt>
        <dd>{dev.connected ? <Stamp size="sm">connected</Stamp> : <Stamp size="sm" tone="wip">unplugged</Stamp>}</dd>
        {dev.analogProfile && (
          <>
            <dt>analog profile</dt>
            <dd>{dev.analogProfile}</dd>
          </>
        )}
        {dev.hiddenInterfaces > 0 && (
          <>
            <dt>other interfaces</dt>
            <dd>{dev.hiddenInterfaces} folded in and hidden (config channels, system control)</dd>
          </>
        )}
        {dev.hiddenReason && (
          <>
            <dt>why hidden</dt>
            <dd>{dev.hiddenReason}</dd>
          </>
        )}
        {dev.note && (
          <>
            <dt>note</dt>
            <dd>{dev.note}</dd>
          </>
        )}
      </dl>

      {dev.viewer === 'mouse' && <MouseModelPicker dev={dev} />}
      {dev.viewer === 'gamepad' && <PadModelPicker dev={dev} />}

      {blocked ? (
        <div className="banner banner--problem">
          <Stamp tone="problem" size="sm">no signal</Stamp>
          <div>
            <p className="banner__t">raw input is not available</p>
            <p className="banner__p">{info.caps.notes[0] ?? 'this platform has no raw input layer yet, so this device can be listed but not tested.'}</p>
          </div>
          <span />
        </div>
      ) : (
        <>
          <Rules dev={dev} />
          <div>
            <button className="btn btn--solid btn--lg" disabled={!dev.connected || starting} onClick={() => void start()}>
              {starting ? 'arming' : 'start test'}
            </button>
            {error && (
              <p className="err" role="alert">
                <span className="err__tag">ERR</span>
                <span>{error}</span>
              </p>
            )}
          </div>
        </>
      )}
    </article>
  );
}
