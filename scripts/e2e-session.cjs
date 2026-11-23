// dev check: drives the ui through every exit rule against the dev mock.
const { chromium } = require('playwright');
const assert = require('assert');
(async () => {
  const b = await chromium.launch();
  const p = await b.newPage({ viewport: { width: 1280, height: 800 } });
  const errs = [];
  p.on('console', (m) => ['error', 'warning'].includes(m.type()) && errs.push(m.text()));
  p.on('pageerror', (e) => errs.push(String(e)));
  const open = async (n) => { await p.goto('http://127.0.0.1:1420/'); await p.waitForSelector('.case'); await p.locator('.case').nth(n).click(); await p.getByRole('button', { name: 'start test' }).click(); await p.waitForSelector('.session'); };
  const onSession = () => p.locator('.session').count().then((n) => n > 0);

  // 1. mouse: no exit button anywhere, esc held ~1s leaves
  await open(1);
  assert.equal(await p.getByRole('button', { name: /exit/i }).count(), 0, 'mouse session must have no exit control');
  assert.equal(await p.locator('.session button').count(), 0, 'mouse session must have no buttons at all');
  await p.screenshot({ path: '/tmp/e2e-mouse.png' });
  await p.keyboard.down('Escape'); await p.waitForTimeout(500);
  assert.ok(await onSession(), 'still in session at 0.5s');
  await p.keyboard.up('Escape'); await p.waitForTimeout(100);
  assert.ok(await onSession(), 'released early: still in session');
  await p.keyboard.down('Escape'); await p.waitForTimeout(1250);
  await p.keyboard.up('Escape');
  await p.waitForSelector('.banner');
  assert.ok(!(await onSession()), 'esc held 1s leaves the mouse session');
  console.log('mouse: ok');

  // 2. keyboard: button works; esc / alt+f4 style keys do nothing; combo leaves; release resets
  await open(0);
  assert.equal(await p.getByRole('button', { name: 'exit test' }).count(), 1);
  assert.ok(await p.getByText(/ctrl.*alt.*shift.*backspace/i).count() > 0, 'combo hint is always visible');
  await p.keyboard.down('Escape'); await p.waitForTimeout(1300); await p.keyboard.up('Escape');
  assert.ok(await onSession(), 'esc must not exit a keyboard session');
  await p.keyboard.down('Control'); await p.keyboard.down('Alt'); await p.keyboard.down('Shift'); await p.keyboard.down('Backspace');
  await p.waitForTimeout(700);
  await p.screenshot({ path: '/tmp/e2e-keyboard-holding.png' });
  await p.keyboard.up('Backspace'); await p.waitForTimeout(100);
  await p.keyboard.up('Shift'); await p.keyboard.up('Alt'); await p.keyboard.up('Control'); await p.waitForTimeout(800);
  assert.ok(await onSession(), 'releasing early resets');
  await p.keyboard.down('Control'); await p.keyboard.down('Alt'); await p.keyboard.down('Shift'); await p.keyboard.down('Backspace');
  await p.waitForTimeout(1800);
  assert.ok(!(await onSession()), 'combo held 1.5s leaves');
  await p.keyboard.up('Backspace'); await p.keyboard.up('Shift'); await p.keyboard.up('Alt'); await p.keyboard.up('Control');
  console.log('keyboard combo: ok');
  await open(0);
  await p.getByRole('button', { name: 'exit test' }).click(); await p.waitForSelector('.banner');
  assert.ok(!(await onSession()), 'exit button leaves');
  console.log('keyboard button: ok');

  // 3. controller: keys never exit; button does
  await open(2);
  await p.keyboard.down('Escape'); await p.keyboard.down('Control'); await p.keyboard.down('Alt'); await p.keyboard.down('Shift'); await p.keyboard.down('Backspace');
  await p.waitForTimeout(2500);
  assert.ok(await onSession(), 'no key may exit a controller session');
  await p.keyboard.up('Backspace'); await p.keyboard.up('Shift'); await p.keyboard.up('Alt'); await p.keyboard.up('Control'); await p.keyboard.up('Escape');
  await p.screenshot({ path: '/tmp/e2e-pad.png' });
  await p.getByRole('button', { name: 'exit test' }).click(); await p.waitForSelector('.banner');
  console.log('controller: ok');
  console.log('console problems:', errs.length ? errs : 'none');
  await b.close();
})().catch((e) => { console.error('FAIL', e.message); process.exit(1); });
