// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab · source viewable, not reusable: see LICENSE
//! the same physical keys go by different names per os: the exit combo is
//! ctrl + alt + shift + backspace on a pc and control + option + shift + delete on a mac.
import type { AppInfo } from './types';

type Os = AppInfo['os'] | null | undefined;

export function exitComboKeys(os: Os): string[] {
  return os === 'macos' ? ['control', 'option', 'shift', 'delete'] : ['ctrl', 'alt', 'shift', 'backspace'];
}

/** what the keyboard test blocks, and the one combo the os keeps for itself */
export function keyboardLockText(os: Os): { blocked: string; reserved: string } {
  if (os === 'macos') {
    return {
      blocked: 'cmd+tab, cmd+q, cmd+space, the media keys and the rest',
      reserved: 'cmd+option+esc and the lock screen belong to macos and are shown as reserved.',
    };
  }
  return { blocked: 'esc, alt+f4, alt+tab, the win key and the rest', reserved: 'ctrl+alt+del cannot be blocked and is shown as reserved.' };
}
