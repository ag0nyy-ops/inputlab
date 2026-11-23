// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab
import { Eye } from '../components/Eye';
import { Stamp } from '../components/Stamp';
import { backendLabel, kindLabel, vidpid } from '../lib/format';
import { selectedDevice, useStore } from '../lib/store';

export function DevicePanel() {
  const dev = useStore(selectedDevice);
  const info = useStore((s) => s.info);

  if (!dev) {
    return (
      <div className="empty">
        <Eye className="eye--hero" blink={7.7} delay={-1.2} />
        <p className="label">pick a device</p>
        <p className="panel__lead">every keyboard, mouse, controller, midi and hid gadget the os can see is on the left. choose one to open its file.</p>
      </div>
    );
  }

  return (
    <article className="panel files">
      <header className="file">
        <p className="label file__no">{kindLabel[dev.kind]}</p>
        <h1 className="title" style={{ marginTop: '.5rem' }}>{dev.name}</h1>
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
        {dev.note && (
          <>
            <dt>note</dt>
            <dd>{dev.note}</dd>
          </>
        )}
      </dl>

      {info && !info.caps.rawInput && (dev.kind === 'keyboard' || dev.kind === 'mouse') && (
        <div className="banner banner--problem">
          <Stamp tone="problem" size="sm">no signal</Stamp>
          <div>
            <p className="banner__t">raw input is not available</p>
            <p className="banner__p">{info.caps.notes[0] ?? 'this platform has no raw input layer yet, so this device can be listed but not tested.'}</p>
          </div>
        </div>
      )}
    </article>
  );
}
