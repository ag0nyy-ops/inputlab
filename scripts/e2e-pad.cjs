// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab · source viewable, not reusable: see LICENSE
// dev check: controller view against the dev mock.
const { chromium } = require('playwright');
const assert = require('assert');
(async () => {
  const b = await chromium.launch();
  const p = await b.newPage({ viewport: { width: 1440, height: 900 } });
  const errs = [];
  p.on('console', (m) => ['error', 'warning'].includes(m.type()) && errs.push(m.text()));
  p.on('pageerror', (e) => errs.push(String(e)));
  await p.goto('http://127.0.0.1:1420/'); await p.waitForSelector('.case');
  await p.locator('.case').nth(2).click();
  for (const id of ['dualsense', 'xbox-series', 'switch-pro']) {
    await p.selectOption('.picker select', id); await p.waitForTimeout(150);
    await p.locator('.picker canvas').screenshot({ path: `/tmp/pad-${id}.png` });
  }
  await p.selectOption('.picker select', 'dualsense');
  await p.getByRole('button', { name: 'start test' }).click(); await p.waitForSelector('.gv');
  // face button, stick, trigger held
  for (const k of ['KeyK', 'KeyD', 'KeyW', 'KeyZ', 'KeyQ', 'KeyT']) await p.keyboard.down(k);
  await p.waitForTimeout(250);
  await p.screenshot({ path: '/tmp/pad1.png' });
  for (const k of ['KeyK', 'KeyD', 'KeyW', 'KeyZ', 'KeyQ', 'KeyT']) await p.keyboard.up(k);
  // circle test: rotate the left stick through 8 directions
  await p.getByRole('button', { name: 'start circle test' }).click();
  for (const seq of [['KeyD'], ['KeyD', 'KeyW'], ['KeyW'], ['KeyW', 'KeyA'], ['KeyA'], ['KeyA', 'KeyS'], ['KeyS'], ['KeyS', 'KeyD']]) {
    for (const k of seq) await p.keyboard.down(k); await p.waitForTimeout(60); for (const k of seq) await p.keyboard.up(k);
  }
  await p.getByRole('button', { name: 'measure drift (3 s)' }).click(); await p.waitForTimeout(400);
  await p.screenshot({ path: '/tmp/pad2.png' });
  for (const name of ['polling', 'rumble + battery', 'raw table']) { await p.getByRole('tab', { name }).click(); await p.waitForTimeout(250); await p.screenshot({ path: `/tmp/pad-${name.split(' ')[0]}.png` }); }
  // buttons exit the session; esc does not
  await p.keyboard.press('Escape'); assert.ok(await p.locator('.session').count());
  await p.getByRole('button', { name: 'exit test' }).click(); await p.waitForSelector('.banner');
  console.log('console problems:', errs.length ? errs : 'none');
  await b.close();
})().catch((e) => { console.error('FAIL', e.message); process.exit(1); });
