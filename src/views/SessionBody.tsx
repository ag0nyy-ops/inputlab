// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab
// picks the view for the device. views for the other device types land in later phases.
import { useEffect, useRef } from 'react';
import { onFrame } from '../lib/session';
import type { SessionInfo } from '../lib/types';
import { KeyboardView } from './KeyboardView';
import { MouseView } from './MouseView';

export function SessionBody({ info }: { info: SessionInfo }) {
  if (info.device.viewer === 'keyboard') return <KeyboardView info={info} />;
  if (info.device.viewer === 'mouse') return <MouseView info={info} />;
  return <RawMonitor info={info} />;
}

/** plain event log: proves capture works before the real views exist, and stays useful for diagnosis */
function RawMonitor({ info }: { info: SessionInfo }) {
  const log = useRef<HTMLPreElement>(null);
  const lines = useRef<string[]>([]);

  useEffect(
    () =>
      onFrame((events) => {
        for (const e of events) {
          if (e.t === 'reserved') lines.current.push(`          reserved by os: ${e.label}`);
        }
        lines.current = lines.current.slice(-40);
        if (log.current) log.current.textContent = lines.current.join('\n');
      }),
    [],
  );

  return (
    <div className="monitor">
      <p className="label">raw events / {info.device.backend}</p>
      <pre ref={log} className="monitor__log" aria-live="off" />
    </div>
  );
}
