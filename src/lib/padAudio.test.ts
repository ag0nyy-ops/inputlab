// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab · source viewable, not reusable: see LICENSE
import { describe, expect, it } from 'vitest';
import { matchPadAudio, type AudioDev } from './padAudio';

const d = (kind: string, label: string, deviceId: string): AudioDev => ({ kind, label, deviceId });

describe('matchPadAudio', () => {
  it('finds the controller among the other audio devices', () => {
    const r = matchPadAudio([
      d('audioinput', 'Default - Microphone (Realtek)', 'default'),
      d('audioinput', 'Microphone (Realtek)', 'a'),
      d('audioinput', 'Microphone (Wireless Controller) (054c:0ce6)', 'pad-in'),
      d('audiooutput', 'Speakers (Realtek)', 'b'),
      d('audiooutput', 'Speakers (Wireless Controller) (054c:0ce6)', 'pad-out'),
      d('videoinput', 'Wireless Controller camera', 'cam'),
    ]);
    expect(r.input?.deviceId).toBe('pad-in');
    expect(r.output?.deviceId).toBe('pad-out');
  });

  it('knows a dualsense by name or by usb id, and never picks the default alias', () => {
    expect(matchPadAudio([d('audiooutput', 'Headset (DualSense Wireless Controller)', 'x')]).output?.deviceId).toBe('x');
    expect(matchPadAudio([d('audiooutput', 'Speakers (054c:0ce6)', 'y')]).output?.deviceId).toBe('y');
    expect(matchPadAudio([d('audiooutput', 'Default - Speakers (Wireless Controller)', 'default')]).output).toBeNull();
  });

  it('finds nothing when there is no controller audio (bluetooth, or not plugged in)', () => {
    expect(matchPadAudio([d('audioinput', 'Microphone (Realtek)', 'a'), d('audiooutput', 'Speakers (Realtek)', 'b')])).toEqual({ input: null, output: null });
    expect(matchPadAudio([])).toEqual({ input: null, output: null });
  });
});
