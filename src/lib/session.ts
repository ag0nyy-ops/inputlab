// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab
// the live session. frames from rust are fanned out imperatively: views
// subscribe and draw straight to canvas / refs, so react never re-renders
// for device data.

import { getBridge } from './bridge';
import { getSettings } from './settings';
import { setState } from './store';
import type { EndReason, Ev, Frame, SessionInfo } from './types';

type Listener = (events: Ev[]) => void;

const listeners = new Set<Listener>();
let heartbeat = 0;

export function onFrame(l: Listener): () => void {
  listeners.add(l);
  return () => listeners.delete(l);
}

function dispatch(f: Frame): void {
  listeners.forEach((l) => l(f.events));
  const ended = f.events.find((e): e is Extract<Ev, { t: 'ended' }> => e.t === 'ended');
  if (ended) teardown(ended.reason);
}

function teardown(reason: EndReason): void {
  clearInterval(heartbeat);
  document.body.classList.remove('capturing-mouse', 'capturing');
  setState({ session: null, lastEnd: { reason, at: Date.now() }, screen: 'home' });
}

export async function startSession(deviceId: string): Promise<SessionInfo> {
  const b = await getBridge();
  const s = getSettings();
  const info = await b.startSession(deviceId, { escHoldMs: s.escHoldMs, comboHoldMs: s.comboHoldMs }, dispatch);
  document.body.classList.add('capturing');
  if (info.mode === 'mouse') document.body.classList.add('capturing-mouse');
  // rust ends the session itself if these stop (a frozen ui can never trap the user)
  heartbeat = window.setInterval(() => void b.heartbeat(), 1000);
  setState({ session: info, lastEnd: null, screen: 'session' });
  return info;
}

/// the exit button. rust refuses this in mouse sessions.
export async function endSession(): Promise<void> {
  const b = await getBridge();
  await b.endSession();
}
