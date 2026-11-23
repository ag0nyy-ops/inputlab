// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab · source viewable, not reusable: see LICENSE
import type { CSSProperties, Ref } from 'react';

interface Props {
  className?: string;
  /** the root element, so a caller can drive the eye directly (the controller easter egg does) */
  ref?: Ref<HTMLSpanElement>;
  /** blink period in seconds. give every eye its own so they never blink together */
  blink?: number;
  delay?: number;
}

export function Eye({ className = '', blink = 6.4, delay = -3.1, ref }: Props) {
  const style = { '--jdx-blink': `${blink}s`, '--jdx-blink-delay': `${delay}s` } as CSSProperties;
  return (
    <span ref={ref} className={`eye ${className}`} style={style} aria-hidden="true">
      <span className="eye__ball">
        <span className="eye__iris">
          <span className="eye__fibers" />
          <span className="eye__pupil" />
        </span>
        <span className="eye__lid eye__lid--top" />
        <span className="eye__lid eye__lid--bottom" />
      </span>
      <span className="eye__lashes" />
    </span>
  );
}
