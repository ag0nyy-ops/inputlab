// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab
// builds the readme screenshots from the real interface running against the dev mock
// (generic sample data, never a real device). 2x, saved as png; convert with webp q90.
const { chromium } = require('playwright');
const out = process.argv[2] || '/tmp/shots';
require('fs').mkdirSync(out, { recursive: true });
(async () => {
  const b = await chromium.launch();
  const page = async (url) => { const p = await b.newPage({ viewport: { width: 1280, height: 800 }, deviceScaleFactor: 2 }); await p.goto(url); await p.waitForSelector('.case'); return p; };
  const start = async (p, idx) => { await p.locator('.case').nth(idx).click(); await p.getByRole('button', { name: 'start test' }).click(); await p.waitForSelector('.session__body > *'); };

  let p = await page('http://127.0.0.1:1420/?analog=1');
  await p.locator('.case').nth(1).click(); await p.waitForTimeout(400);
  await p.screenshot({ path: `${out}/devices.png` });
  await p.close();

  p = await page('http://127.0.0.1:1420/');
  await start(p, 0);
  for (const k of ['KeyQ', 'KeyW', 'KeyE', 'KeyR', 'KeyA', 'KeyS', 'KeyD', 'KeyF', 'KeyZ', 'KeyX', 'Space', 'Digit1', 'Digit2', 'Enter', 'Tab', 'F5', 'ArrowUp', 'ArrowLeft']) { await p.keyboard.down(k); await p.keyboard.up(k); }
  for (const k of ['KeyW', 'KeyA', 'KeyS', 'ShiftLeft', 'Space']) await p.keyboard.down(k);
  await p.waitForTimeout(300); await p.screenshot({ path: `${out}/keyboard.png` });
  await p.close();

  p = await page('http://127.0.0.1:1420/');
  await p.locator('.case').nth(1).click(); await p.selectOption('.picker select', 'logi-g305');
  await p.getByRole('button', { name: 'start test' }).click(); await p.waitForSelector('.mv');
  await p.mouse.move(640, 600); await p.mouse.move(900, 640, { steps: 30 }); await p.mouse.move(760, 700, { steps: 30 });
  await p.mouse.down(); await p.waitForTimeout(170); await p.mouse.up(); await p.mouse.down();
  await p.waitForTimeout(250); await p.screenshot({ path: `${out}/mouse.png` });
  await p.close();

  p = await page('http://127.0.0.1:1420/');
  await p.locator('.case').nth(2).click(); await p.selectOption('.picker select', 'dualsense');
  await p.getByRole('button', { name: 'start test' }).click(); await p.waitForSelector('.gv');
  for (const k of ['KeyK', 'KeyD', 'KeyW', 'KeyZ', 'KeyQ']) await p.keyboard.down(k);
  await p.getByRole('button', { name: 'start circle test' }).click();
  for (const seq of [['KeyD'], ['KeyD', 'KeyW'], ['KeyW'], ['KeyW', 'KeyA'], ['KeyA'], ['KeyA', 'KeyS'], ['KeyS']]) { for (const k of seq) await p.keyboard.down(k); await p.waitForTimeout(50); for (const k of seq) await p.keyboard.up(k); }
  for (const k of ['KeyD', 'KeyW']) await p.keyboard.down(k);
  await p.waitForTimeout(300); await p.screenshot({ path: `${out}/controller.png` });
  await p.close();
  await b.close();
})();
