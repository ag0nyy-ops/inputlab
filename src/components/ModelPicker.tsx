// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab
import { useEffect, useRef } from 'react';
import { fitCanvas } from '../lib/draw';
import type { DeviceInfo } from '../lib/types';
import { drawPad, PAD_H, PAD_W } from '../models/drawPad';
import { MICE, matchMouse } from '../models/mice';
import { PADS, matchPad } from '../models/pads';
import { setPick, usePick } from '../models/pick';
import { MouseModelCanvas } from './MouseModelCanvas';
import { Stamp } from './Stamp';

const mouseConfidence = {
  picked: 'picked by you',
  name: 'matched by name',
  'usb id': 'matched by usb id',
  ambiguous: 'several mice share this receiver: pick yours',
  none: 'unknown mouse: generic shape',
} as const;

const padConfidence = {
  picked: 'picked by you',
  'usb id': 'matched by usb id',
  name: 'matched by name',
  none: 'unknown controller: generic layout',
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
          <Stamp size="sm" tone={m.confidence === 'ambiguous' || m.confidence === 'none' ? 'wip' : 'plain'}>{mouseConfidence[m.confidence]}</Stamp>
        </p>
        <p className="case__sub">silhouettes are grip families, not product renders. the choice is remembered on this machine.</p>
      </div>
    </div>
  );
}

/** a still controller drawing for the picker */
function PadCanvas({ dev, picked, width }: { dev: DeviceInfo; picked: string | null; width: number }) {
  const ref = useRef<HTMLCanvasElement>(null);
  const m = matchPad(dev, picked);
  useEffect(() => {
    const h = Math.round((width * PAD_H) / PAD_W);
    const { ctx } = fitCanvas(ref.current!, width, h);
    ctx.clearRect(0, 0, width, h);
    drawPad(ctx, m, width, { v: () => 0, stick: () => [0, 0] });
  }, [m.style, m.body, m.model, width]);
  return <canvas ref={ref} role="img" aria-label={`${m.model?.name ?? 'generic'} controller layout`} />;
}

export function PadModelPicker({ dev }: { dev: DeviceInfo }) {
  const picked = usePick(dev);
  const m = matchPad(dev, picked);
  return (
    <div className="picker">
      <PadCanvas dev={dev} picked={picked} width={220} />
      <div className="picker__body">
        <p className="label">controller layout</p>
        <label className="field">
          <span className="sr">choose the controller layout</span>
          <select className="select" value={picked ?? ''} onChange={(e) => setPick(dev, e.target.value || null)}>
            <option value="">auto{m.model && picked === null ? ` · ${m.model.name}` : ' · generic'}</option>
            {PADS.map((x) => (
              <option key={x.id} value={x.id}>{x.name}</option>
            ))}
          </select>
        </label>
        <p>
          <Stamp size="sm" tone={m.confidence === 'none' ? 'wip' : 'plain'}>{padConfidence[m.confidence]}</Stamp>
        </p>
        <p className="case__sub">face buttons follow the pad: playstation shapes, xbox and switch letters (and switch swaps A with B). anything unmapped still shows in the raw table.</p>
      </div>
    </div>
  );
}

