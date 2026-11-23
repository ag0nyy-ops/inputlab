// dev helper: screenshots the ui in chromium. usage: node scripts/shot.cjs <url> <out.png> [w] [h] [clickSelector]
const { chromium } = require('playwright');
(async () => {
  const [url, out, w = '1440', h = '900', click, reduced] = process.argv.slice(2);
  const b = await chromium.launch();
  const p = await b.newPage({ viewport: { width: +w, height: +h }, deviceScaleFactor: 1, reducedMotion: reduced ? 'reduce' : 'no-preference' });
  const reqs = new Set(); const errs = [];
  p.on('request', (r) => reqs.add(new URL(r.url()).host));
  p.on('console', (m) => { if (['error', 'warning'].includes(m.type())) errs.push(m.text()); });
  p.on('pageerror', (e) => errs.push(String(e)));
  await p.goto(url); await p.waitForTimeout(700);
  if (click) { await p.click(click); await p.waitForTimeout(400); }
  const fonts = await p.evaluate(async () => { await document.fonts.ready; return [...document.fonts].map((f) => `${f.family}:${f.weight}:${f.status}`); });
  await p.screenshot({ path: out });
  console.log('hosts', [...reqs].join(','), '\nfonts', fonts.join(' | '), '\nerrors', errs.length ? errs : 'none');
  await b.close();
})();
