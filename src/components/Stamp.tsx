// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab · source viewable, not reusable: see LICENSE
import type { ReactNode } from 'react';

interface Props {
  children: ReactNode;
  /** solid = fine/on (default outline), wip = waiting, problem = inverted */
  tone?: 'plain' | 'wip' | 'problem';
  size?: 'sm' | 'md' | 'big';
  className?: string;
}

export function Stamp({ children, tone = 'plain', size = 'md', className = '' }: Props) {
  const cls = ['stamp', tone === 'wip' && 'stamp--wip', tone === 'problem' && 'stamp--solid', size === 'sm' && 'stamp--sm', size === 'big' && 'stamp--big', className]
    .filter(Boolean)
    .join(' ');
  return <span className={cls}>{children}</span>;
}
