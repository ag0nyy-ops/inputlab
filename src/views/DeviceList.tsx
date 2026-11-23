// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab · source viewable, not reusable: see LICENSE
import { getBridge } from '../lib/bridge';
import { backendLabel, caseNo, kindLabel, vidpid } from '../lib/format';
import { setState, useStore } from '../lib/store';
import { Stamp } from '../components/Stamp';
import { deviceTitle, usePick } from '../models/pick';
import type { DeviceInfo } from '../lib/types';

export function DeviceList() {
  const devices = useStore((s) => s.devices);
  const selectedId = useStore((s) => s.selectedId);
  const loaded = useStore((s) => s.devicesLoaded);
  const showHidden = useStore((s) => s.showHidden);
  const shown = devices.filter((d) => d.usable || showHidden);
  const hiddenCount = devices.filter((d) => !d.usable).length;
  const connected = shown.filter((d) => d.connected).length;

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
        {loaded && shown.length === 0 && (
          <li className="callout">
            <p className="label" style={{ marginBottom: '.4rem' }}>no devices</p>
            <p className="case__sub">nothing is plugged in, or the os is not letting us see it. plug something in; the list updates on its own.</p>
          </li>
        )}
        {shown.map((d, i) => (
          <CaseRow key={d.id} d={d} i={i} selected={selectedId === d.id} />
        ))}
        {hiddenCount > 0 && (
          <li>
            <button className="btn btn--sm" aria-pressed={showHidden} onClick={() => setState({ showHidden: !showHidden })}>
              {showHidden ? 'hide' : 'show'} {hiddenCount} hidden interface{hiddenCount === 1 ? '' : 's'}
            </button>
            <p className="case__sub" style={{ marginTop: '.4rem' }}>windows and the hardware expose plumbing next to real devices: config channels, virtual and remote-desktop inputs.</p>
          </li>
        )}
      </ul>
    </aside>
  );
}

function CaseRow({ d, i, selected }: { d: DeviceInfo; i: number; selected: boolean }) {
  const picked = usePick(d);
  return (
    <li>
      <button className={`case ${d.connected ? '' : 'case--gone'} ${d.usable ? '' : 'case--hidden'}`} aria-pressed={selected} onClick={() => setState({ selectedId: d.id, screen: 'home' })}>
        <span className="label case__no">{caseNo(i)} / {kindLabel[d.kind]}</span>
        <span className="case__name">{deviceTitle(d, picked)}</span>
        <span className="case__sub">
          {vidpid(d.vid, d.pid)} · {backendLabel(d.backend)}
        </span>
        <span className="case__stamp">
          {!d.usable ? <Stamp size="sm" tone="wip">interface</Stamp> : d.connected ? <Stamp size="sm">live</Stamp> : <Stamp size="sm" tone="wip">unplugged</Stamp>}
        </span>
      </button>
    </li>
  );
}
