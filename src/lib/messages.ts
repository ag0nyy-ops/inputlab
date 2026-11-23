// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab
import type { EndReason } from './types';

export const endMessages: Record<EndReason, { title: string; text: string; problem: boolean }> = {
  'exit-button': { title: 'you left', text: 'the exit button ended the test. capture is released.', problem: false },
  'esc-hold': { title: 'you left', text: 'esc was held long enough. the mouse is free again.', problem: false },
  combo: { title: 'you left', text: 'the exit combo was held long enough. shortcuts work again.', problem: false },
  'focus-lost': { title: 'focus lost', text: 'the window lost focus, so capture was released and the test ended. nothing stays locked.', problem: true },
  watchdog: { title: 'ui stopped answering', text: 'rust ended the test and released everything because the window stopped answering.', problem: true },
  'ui-gone': { title: 'ui went away', text: 'the window was reloaded or closed, so the test ended and capture was released.', problem: true },
  shutdown: { title: 'closing', text: 'the app is quitting.', problem: false },
};
