// dev check: generic hid, midi, analog keyboard against the dev mock.
const { chromium } = require('playwright');
const assert = require('assert');
(async () => {
  const b = await chromium.launch();
  const errs = [];
  const open = async (url, idx) => {
    const p = await b.newPage({ viewport: { width: 1440, height: 900 } });
    p.on('console', (m) => ['error', 'warning'].includes(m.type()) && errs.push(m.text()));
    p.on('pageerror', (e) => errs.push(String(e)));
    await p.goto(url); await p.waitForSelector('.case');
    await p.locator('.case').nth(idx).click(); await p.getByRole('button', { name: 'start test' }).click(); await p.waitForSelector('.session__body > *');
    return p;
  };
  // generic hid
  let p = await open('http://127.0.0.1:1420/', 3);
  await p.keyboard.down('1'); await p.keyboard.down('3'); await p.keyboard.down('ArrowRight'); await p.waitForTimeout(150);
  await p.screenshot({ path: '/tmp/h1.png' });
  await p.keyboard.up('1'); await p.keyboard.up('3'); await p.keyboard.up('ArrowRight');
  assert.ok(await p.locator('.hv__axes li').count() === 4, 'four axes');
  await p.getByRole('button', { name: 'exit test' }).click(); await p.close();
  // errors + raw only
  p = await open('http://127.0.0.1:1420/?hiderr=1', 3); await p.screenshot({ path: '/tmp/h2.png' }); assert.match(await p.locator('.banner').first().innerText(), /could not be opened/i); await p.close();
  p = await open('http://127.0.0.1:1420/?rawonly=1', 3); assert.match(await p.locator('.session__body').innerText(), /raw only/i); await p.close();
  // midi
  p = await open('http://127.0.0.1:1420/', 4);
  for (const k of ['KeyA', 'KeyD', 'KeyG', 'KeyW']) await p.keyboard.down(k);
  await p.mouse.wheel(0, -100); await p.mouse.wheel(0, -100); await p.waitForTimeout(250);
  await p.screenshot({ path: '/tmp/h3.png' });
  assert.ok(await p.locator('.logrow').count() >= 4, 'midi log rows');
  await p.close();
  // analog keyboard
  p = await open('http://127.0.0.1:1420/?analog=1', 5);
  await p.keyboard.down('KeyW'); await p.waitForTimeout(300); await p.keyboard.down('KeyA'); await p.waitForTimeout(250);
  await p.screenshot({ path: '/tmp/h4.png' });
  assert.match(await p.locator('.banner').first().innerText(), /experimental/i);
  await p.close();
  console.log('console problems:', errs.length ? errs : 'none');
  await b.close();
})().catch((e) => { console.error('FAIL', e.message); process.exit(1); });
