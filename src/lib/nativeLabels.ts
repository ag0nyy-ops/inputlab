// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab
// what is printed on the keys under the os's current layout. native (rust) first,
// then the browser's keyboard map, else nothing (static tables take over).
import { getBridge } from './bridge';

export async function loadNativeLabels(): Promise<Map<string, string> | null> {
  try {
    const b = await getBridge();
    const n = await b.keyLabels();
    if (n && n.labels.length > 20) return new Map(n.labels);
  } catch {
    /* fall through */
  }
  try {
    const kb = (navigator as unknown as { keyboard?: { getLayoutMap(): Promise<Map<string, string>> } }).keyboard;
    const m = await kb?.getLayoutMap();
    if (m && m.size > 20) return new Map(m);
  } catch {
    /* fall through */
  }
  return null;
}
