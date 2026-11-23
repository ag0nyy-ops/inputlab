// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab · source viewable, not reusable: see LICENSE
// dev check: keyboard view against the dev mock. screenshots + behaviour asserts.
const { chromium } = require('playwright');
const assert = require('assert');
(async () => {
  const b = await chromium.launch();
  const p = await b.newPage({ viewport: { width: 1440, height: 900 } });
  const errs = [];
  p.on('console', (m) => ['error', 'warning'].includes(m.type()) && errs.push(m.text()));
  p.on('pageerror', (e) => errs.push(String(e)));
  await p.goto('http://127.0.0.1:1420/'); await p.waitForSelector('.case');
  await p.locator('.case').nth(0).click(); await p.getByRole('button', { name: 'start test' }).click();
  await p.waitForSelector('canvas');
  await p.waitForTimeout(300);
  await p.screenshot({ path: '/tmp/kb0.png' });
  // hold a chord, tap some keys, make chatter (tap twice < 30ms)
  for (const k of ['KeyW', 'KeyA', 'KeyS', 'KeyD', 'Space', 'ShiftLeft']) await p.keyboard.down(k);
  await p.waitForTimeout(150);
  await p.screenshot({ path: '/tmp/kb1.png' });
  for (const k of ['KeyW', 'KeyA', 'KeyS', 'KeyD', 'Space', 'ShiftLeft']) await p.keyboard.up(k);
  await p.keyboard.down('KeyJ'); await p.keyboard.up('KeyJ'); await p.keyboard.down('KeyJ'); await p.keyboard.up('KeyJ');
  await p.waitForTimeout(250);
  const txt = await p.locator('.kb__panels').innerText();
  assert.match(txt, /max\s*6/i, 'rollover max 6 shown');
  assert.match(txt, /KeyJ/, 'chatter on KeyJ listed');
  const tested = await p.locator('.kb__big').first().innerText();
  console.log('tested:', tested.replace(/\s+/g, ' '));
  assert.ok(await p.locator('.logrow').count() >= 10, 'log has rows');
  await p.screenshot({ path: '/tmp/kb2.png' });
  await p.getByRole('button', { name: 'reset' }).click(); await p.waitForTimeout(150);
  assert.match(await p.locator('.kb__big').first().innerText(), /^0/, 'reset clears tested');
  await p.selectOption('select >> nth=1', 'iso'); await p.selectOption('select >> nth=0', 'tkl'); await p.selectOption('select >> nth=2', 'azerty'); await p.waitForTimeout(200);
  await p.screenshot({ path: '/tmp/kb3.png' });
  console.log('console problems:', errs.length ? errs : 'none');
  await b.close();
})().catch((e) => { console.error('FAIL', e.message); process.exit(1); });
