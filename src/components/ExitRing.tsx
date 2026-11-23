// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab · source viewable, not reusable: see LICENSE
import { useEffect, useRef } from 'react';
import { onFrame } from '../lib/session';
import type { ExitKind } from '../lib/types';

const R = 26;
const C = 2 * Math.PI * R;

/** hollow ring that fills while an exit hold is in progress. driven straight from rust frames. */
export function ExitRing({ kind, label }: { kind: ExitKind; label: string }) {
  const arc = useRef<SVGCircleElement>(null);
  const root = useRef<HTMLDivElement>(null);

  useEffect(
    () =>
      onFrame((events) => {
        for (const e of events) {
          if (e.t === 'exit' && e.kind === kind && arc.current && root.current) {
            arc.current.style.strokeDashoffset = String(C * (1 - e.progress));
            root.current.setAttribute('aria-valuenow', String(Math.round(e.progress * 100)));
            root.current.dataset.holding = e.progress > 0 ? '1' : '0';
          }
        }
      }),
    [kind],
  );

  return (
    <div ref={root} className="ring" role="progressbar" aria-label={label} aria-valuemin={0} aria-valuemax={100} aria-valuenow={0} data-holding="0">
      <svg viewBox="0 0 64 64" width="64" height="64" aria-hidden="true">
        <circle cx="32" cy="32" r={R} fill="none" stroke="var(--line-strong)" strokeWidth="2" />
        <circle ref={arc} cx="32" cy="32" r={R} fill="none" stroke="var(--fg)" strokeWidth="5" strokeDasharray={C} strokeDashoffset={C} transform="rotate(-90 32 32)" />
      </svg>
    </div>
  );
}
