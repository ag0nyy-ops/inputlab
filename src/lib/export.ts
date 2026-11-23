// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab
// the session summary: one json document and one plain-text version of it.
// only what the test measured and the device's public ids go in: no serials, no paths.
import type { AppInfo, SessionInfo } from './types';
import type { Results } from './results';
import { matchMouse } from '../models/mice';
import { matchPad } from '../models/pads';
import { vidpid } from './format';

export interface ReportInput {
  app: AppInfo | null;
  info: SessionInfo;
  results: Results;
  startedAt: number;
  endedAt: number;
  reason: string;
  /** user-picked model ids, so the report names the model the user saw */
  picks?: { mouse?: string | null; pad?: string | null };
}

const r1 = (n: unknown, d = 1): string => (typeof n === 'number' && Number.isFinite(n) ? n.toFixed(d) : '-');
const r0 = (n: unknown): string => (typeof n === 'number' && Number.isFinite(n) ? Math.round(n).toLocaleString('en-US') : '-');

export function buildJson(i: ReportInput): Record<string, unknown> {
  const d = i.info.device;
  const model =
    d.viewer === 'mouse' ? matchMouse(d, i.picks?.mouse).model?.name ?? null : d.viewer === 'gamepad' ? matchPad(d, i.picks?.pad).model?.name ?? null : null;
  return {
    app: { name: 'inputlab', version: i.app?.version ?? null, os: i.app?.os ?? null, rawBackend: i.app?.rawBackend ?? null },
    generatedAt: new Date(i.endedAt).toISOString(),
    device: { name: d.name, kind: d.kind, model, vid: d.vid === null ? null : d.vid.toString(16).padStart(4, '0'), pid: d.pid === null ? null : d.pid.toString(16).padStart(4, '0'), backend: d.backend, interfaces: d.interfaces, analogProfile: d.analogProfile },
    session: { mode: i.info.mode, durationSeconds: Math.round((i.endedAt - i.startedAt) / 1000), endedBy: i.reason, captureActive: i.info.capture.active },
    results: i.results,
  };
}

export function buildText(i: ReportInput): string {
  const d = i.info.device;
  const j = buildJson(i) as { device: { model: string | null } };
  const L: string[] = [];
  L.push('inputlab session report');
  L.push(`${new Date(i.endedAt).toLocaleString()} · ${i.app?.os ?? ''} · inputlab ${i.app?.version ?? ''}`.trim());
  L.push('');
  L.push(`device    ${d.name}${j.device.model ? ` (${j.device.model})` : ''}`);
  L.push(`type      ${d.kind} · read by ${d.backend} · ${vidpid(d.vid, d.pid)}`);
  L.push(`duration  ${Math.round((i.endedAt - i.startedAt) / 1000)} s · ended by ${i.reason}`);

  const kb = i.results.keyboard;
  if (kb) {
    L.push('', 'keyboard');
    L.push(`  tested keys   ${kb.tested.length} of ${kb.total} (${kb.layout})`);
    L.push(`  max rollover  ${kb.maxHeld} keys at once`);
    const ch = Object.entries(kb.chatter);
    L.push(`  chatter       ${ch.length ? ch.map(([k, n]) => `${k} x${n}`).join(', ') : 'none'}`);
  }
  const m = i.results.mouse as Record<string, any> | undefined;
  if (m) {
    L.push('', 'mouse');
    if (m.model) L.push(`  model         ${m.model}`);
    if (m.polling) L.push(`  polling       avg ${r0(m.polling.avgHz)} hz · slowest ${r0(m.polling.minHz)} · fastest ${r0(m.polling.maxHz)}${m.polling.nominal ? ` · ≈ ${m.polling.nominal} hz` : ''} · jitter ${r1(m.polling.jitterUs)} µs`);
    if (m.dpi) L.push(`  dpi           ${r0(m.dpi.cpi)} cpi (${r0(m.dpi.counts)} counts over ${r1(m.dpi.distanceIn, 2)} in, ${Math.round(m.dpi.straightness * 100)}% straight)`);
    if (m.sensor) L.push(`  sensor        jitter ${r1(m.sensor.jitterPct, 0)}% · snap ratio ${r1(m.sensor.snapRatio, 2)}x${m.sensor.snapping ? ' · ANGLE SNAPPING LIKELY' : ''}`);
    const btn = (m.buttons as { n: number; clicks: number; chatter: number; holdAvg: number | null }[] | undefined)?.filter((b) => b.clicks > 0);
    if (btn?.length) L.push(`  buttons       ${btn.map((b) => `${b.n}: ${b.clicks} clicks${b.chatter ? `, ${b.chatter} chatter` : ''}`).join(' · ')}`);
    const w = m.wheel as { up: number; down: number; left: number; right: number } | undefined;
    if (w && w.up + w.down + w.left + w.right > 0) L.push(`  wheel         up ${w.up} · down ${w.down} · tilt left ${w.left} · tilt right ${w.right}`);
  }
  const g = i.results.gamepad as Record<string, any> | undefined;
  if (g) {
    L.push('', 'controller');
    if (g.model) L.push(`  layout        ${g.model}`);
    if (g.polling) L.push(`  polling       avg ${r0(g.polling.avgHz)} hz${g.polling.source ? ` (${g.polling.source})` : ''}`);
    for (const side of ['left', 'right'] as const) {
      const s = g.sticks?.[side];
      if (s) L.push(`  ${side} stick    circularity ${s.circularity == null ? '-' : `${Math.round(s.circularity * 100)}%`} · max range ${Math.round(s.maxRange * 100)}% · deadzone ${s.deadzone == null ? '-' : `${Math.round(s.deadzone * 100)}%`}`);
    }
    for (const e of (g.sticks?.rest ?? []) as { name: string; offset: number; drift: boolean }[]) L.push(`  rest ${e.name}  ${(e.offset * 100).toFixed(1)}% off-centre${e.drift ? ' · DRIFT' : ''}`);
    if (g.battery) L.push(`  battery       ${g.battery.power}${g.battery.level != null ? ` ${g.battery.level}%` : ''}`);
    if (g.rumble != null) L.push(`  rumble        ${g.rumble ? 'reported by driver' : 'not reported'}`);
  }
  const h = i.results.hid as Record<string, any> | undefined;
  if (h) {
    L.push('', d.kind === 'midi' ? 'midi' : 'hid');
    if (h.midi) L.push(`  messages      ${h.midi.messages} · ${h.midi.controllers} controllers`);
    else {
      L.push(`  descriptor    ${h.descriptorParsed ? `parsed: ${h.buttons} buttons, axes ${(h.axes as string[]).join(', ') || 'none'}` : 'not parsed (raw bytes only)'}`);
      if (h.polling) L.push(`  report rate   avg ${r0(h.polling.avgHz)} hz`);
    }
  }
  L.push('', 'measured from raw input by inputlab. dpi depends on how accurately the distance was measured.');
  return L.join('\n');
}
