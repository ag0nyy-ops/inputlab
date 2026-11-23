// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab
// what the current test has found, kept outside react so views can write at any rate.
// the summary screen (and json export) read this.

export interface Results {
  keyboard?: { tested: string[]; total: number; maxHeld: number; chatter: Record<string, number>; layout: string };
  mouse?: Record<string, unknown>;
  gamepad?: Record<string, unknown>;
  hid?: Record<string, unknown>;
}

let current: Results = {};
export const results = {
  get: (): Results => current,
  set<K extends keyof Results>(k: K, v: Results[K]): void {
    current = { ...current, [k]: v };
  },
  clear(): void {
    current = {};
  },
};
