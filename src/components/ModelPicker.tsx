// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab
import { Stamp } from './Stamp';
import { MouseModelCanvas } from './MouseModelCanvas';
import type { DeviceInfo } from '../lib/types';
import { MICE, matchMouse } from '../models/mice';
import { setPick, usePick } from '../models/pick';

const confidenceText = {
  picked: 'picked by you',
  name: 'matched by name',
  'usb id': 'matched by usb id',
  ambiguous: 'several mice share this receiver: pick yours',
  none: 'unknown mouse: generic shape',
} as const;

/** which mouse is this? shown on the device file so it is settled before the test locks the mouse */
export function MouseModelPicker({ dev }: { dev: DeviceInfo }) {
  const picked = usePick(dev);
  const m = matchMouse(dev, picked);
  const vendors = [...new Set(MICE.map((x) => x.vendor))];

  return (
    <div className="picker">
      <MouseModelCanvas shape={m.shape} width={96} label={`${m.model?.name ?? 'generic'} mouse silhouette`} />
      <div className="picker__body">
        <p className="label">mouse model</p>
        <label className="field">
          <span className="sr">choose the mouse model</span>
          <select className="select" value={picked ?? ''} onChange={(e) => setPick(dev, e.target.value || null)}>
            <option value="">auto{m.model && picked === null ? ` · ${m.model.name}` : ' · generic'}</option>
            {vendors.map((v) => (
              <optgroup key={v} label={v}>
                {MICE.filter((x) => x.vendor === v).map((x) => (
                  <option key={x.id} value={x.id}>{x.name}</option>
                ))}
              </optgroup>
            ))}
          </select>
        </label>
        <p>
          <Stamp size="sm" tone={m.confidence === 'ambiguous' || m.confidence === 'none' ? 'wip' : 'plain'}>{confidenceText[m.confidence]}</Stamp>
        </p>
        <p className="case__sub">silhouettes are grip families, not product renders. the choice is remembered on this machine.</p>
      </div>
    </div>
  );
}
