// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab · source viewable, not reusable: see LICENSE
import { getBridge } from '../lib/bridge';
import { backendLabel, caseNo, kindLabel, vidpid } from '../lib/format';
import { setState, useStore } from '../lib/store';
import { Stamp } from '../components/Stamp';

export function DeviceList() {
  const devices = useStore((s) => s.devices);
  const selectedId = useStore((s) => s.selectedId);
  const loaded = useStore((s) => s.devicesLoaded);
  const connected = devices.filter((d) => d.connected).length;

  return (
    <aside className="side" aria-label="detected devices">
      <div className="side__head">
        <span className="label">devices / {connected} live</span>
        <button className="btn btn--sm" onClick={() => void getBridge().then((b) => b.rescan())} aria-label="rescan devices">
          rescan
        </button>
      </div>
      <ul className="side__list scroll">
        {!loaded && <li className="label label--hollow">scanning</li>}
        {loaded && devices.length === 0 && (
          <li className="callout">
            <p className="label" style={{ marginBottom: '.4rem' }}>no devices</p>
            <p className="case__sub">nothing is plugged in, or the os is not letting us see it. plug something in; the list updates on its own.</p>
          </li>
        )}
        {devices.map((d, i) => (
          <li key={d.id}>
            <button className={`case ${d.connected ? '' : 'case--gone'}`} aria-pressed={selectedId === d.id} onClick={() => setState({ selectedId: d.id, screen: 'home' })}>
              <span className="label case__no">{caseNo(i)} / {kindLabel[d.kind]}</span>
              <span className="case__name">{d.name}</span>
              <span className="case__sub">
                {vidpid(d.vid, d.pid)} · {backendLabel(d.backend)}
              </span>
              <span className="case__stamp">
                {d.connected ? <Stamp size="sm">live</Stamp> : <Stamp size="sm" tone="wip">unplugged</Stamp>}
              </span>
            </button>
          </li>
        ))}
      </ul>
    </aside>
  );
}
