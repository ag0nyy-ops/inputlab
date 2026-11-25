// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab · source viewable, not reusable: see LICENSE
import { describe, expect, it } from 'vitest';
import { exitComboKeys, keyboardLockText } from './osKeys';

describe('os key names', () => {
  it('names the exit combo the way each keyboard prints it', () => {
    expect(exitComboKeys('windows')).toEqual(['ctrl', 'alt', 'shift', 'backspace']);
    expect(exitComboKeys('linux')).toEqual(['ctrl', 'alt', 'shift', 'backspace']);
    expect(exitComboKeys('macos')).toEqual(['control', 'option', 'shift', 'delete']);
    expect(exitComboKeys(null)).toHaveLength(4);
  });

  it('never tells a mac user about ctrl+alt+del', () => {
    expect(keyboardLockText('macos').reserved).not.toContain('ctrl+alt+del');
    expect(keyboardLockText('windows').reserved).toContain('ctrl+alt+del');
  });
});
