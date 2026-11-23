// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab · source viewable, not reusable: see LICENSE
// dev check: windows plumbing is hidden by default and a toggle brings it back.
const { chromium } = require('playwright');
const assert = require('assert');
(async () => {
  const b = await chromium.launch();
  const p = await b.newPage({ viewport: { width: 1280, height: 800 } });
  await p.goto('http://127.0.0.1:1420/?hidden=1'); await p.waitForSelector('.case');
  const before = await p.locator('.case').count();
  assert.equal(await p.locator('.case--hidden').count(), 0, 'hidden interfaces are not listed by default');
  await p.getByRole('button', { name: /show 2 hidden interfaces/ }).click();
  assert.equal(await p.locator('.case--hidden').count(), 2);
  assert.equal(await p.locator('.case').count(), before + 2);
  await p.locator('.case').nth(1).click();
  assert.match(await p.locator('.panel').innerText(), /folded in and hidden/i);
  await p.screenshot({ path: '/tmp/hidden.png' });
  await p.getByRole('button', { name: /hide 2 hidden interfaces/ }).click();
  assert.equal(await p.locator('.case--hidden').count(), 0);
  console.log('hidden toggle: ok');
  await b.close();
})().catch((e) => { console.error('FAIL', e.message); process.exit(1); });
