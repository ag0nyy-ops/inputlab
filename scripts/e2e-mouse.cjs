// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab
// dev check: mouse view against the dev mock.
const { chromium } = require('playwright');
const assert = require('assert');
(async () => {
  const b = await chromium.launch();
  const p = await b.newPage({ viewport: { width: 1440, height: 900 } });
  const errs = [];
  p.on('console', (m) => ['error', 'warning'].includes(m.type()) && errs.push(m.text()));
  p.on('pageerror', (e) => errs.push(String(e)));
  await p.goto('http://127.0.0.1:1420/'); await p.waitForSelector('.case');
  await p.locator('.case').nth(1).click();
  await p.screenshot({ path: '/tmp/m0.png' });
  // model picker: pick a g305 and a superlight, compare the previews
  await p.selectOption('.picker select', 'logi-g305'); await p.waitForTimeout(150);
  await p.locator('.picker canvas').screenshot({ path: '/tmp/m-g305.png' });
  await p.selectOption('.picker select', 'logi-g-pro-x-superlight'); await p.waitForTimeout(150);
  await p.locator('.picker canvas').screenshot({ path: '/tmp/m-superlight.png' });
  await p.selectOption('.picker select', 'razer-deathadder'); await p.waitForTimeout(150);
  await p.locator('.picker canvas').screenshot({ path: '/tmp/m-da.png' });
  await p.selectOption('.picker select', 'logi-g305');
  await p.getByRole('button', { name: 'start test' }).click(); await p.waitForSelector('.mv');
  await p.mouse.move(700, 600);
  await p.mouse.move(900, 650, { steps: 25 }); await p.mouse.move(800, 700, { steps: 25 });
  await p.mouse.down(); await p.waitForTimeout(160);
  await p.mouse.wheel(0, -120);
  await p.screenshot({ path: '/tmp/m1.png' });
  await p.mouse.up();
  await p.waitForTimeout(300);
  // dpi: 2 -> start, move, stop
  await p.keyboard.press('2'); await p.keyboard.press('Space');
  await p.mouse.move(600, 700); await p.mouse.move(1000, 700, { steps: 40 });
  await p.keyboard.press('Space'); await p.waitForTimeout(300);
  await p.screenshot({ path: '/tmp/m2.png' });
  const txt = await p.locator('.mv__panel').innerText();
  assert.match(txt, /cpi/i);
  await p.keyboard.press('3'); await p.keyboard.press('Space');
  for (let i = 0; i < 60; i++) await p.mouse.move(800 + 120 * Math.cos(i / 9.5), 650 + 120 * Math.sin(i / 9.5));
  await p.waitForTimeout(300);
  await p.screenshot({ path: '/tmp/m3.png' });
  await p.keyboard.press('4'); await p.waitForTimeout(250);
  await p.screenshot({ path: '/tmp/m4.png' });
  await p.keyboard.press('1'); await p.waitForTimeout(250);
  await p.screenshot({ path: '/tmp/m5.png' });
  // esc hold leaves
  await p.keyboard.down('Escape'); await p.waitForTimeout(1250); await p.keyboard.up('Escape');
  await p.waitForSelector('.banner');
  console.log('console problems:', errs.length ? errs : 'none');
  await b.close();
})().catch((e) => { console.error('FAIL', e.message); process.exit(1); });
