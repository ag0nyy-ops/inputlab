// builds the app icon: the dossier eye inside a thin ring on near-black.
// usage: node scripts/make-icon.cjs  ->  src-tauri/icons/source.png (1024x1024)
const { chromium } = require('playwright');
const fs = require('fs');
const path = require('path');

function rng(seed) {
  let s = seed >>> 0;
  return () => ((s = (s * 1664525 + 1013904223) >>> 0) / 4294967296);
}

function eye({ w = 600 }) {
  const cx = 300, cy = 150, r = rng(7);
  const lens = 'M 20 150 Q 300 -50 580 150 Q 300 350 20 150 Z';
  let fibres = '';
  for (let i = 0; i < 96; i++) {
    const a = (i / 96) * Math.PI * 2 + r() * 0.03;
    const r0 = 40 + r() * 6, r1 = 70 + r() * 14;
    fibres += `<line x1="${(cx + Math.cos(a) * r0).toFixed(1)}" y1="${(cy + Math.sin(a) * r0).toFixed(1)}" x2="${(cx + Math.cos(a) * r1).toFixed(1)}" y2="${(cy + Math.sin(a) * r1).toFixed(1)}" stroke="#ececec" stroke-opacity="${(0.25 + r() * 0.5).toFixed(2)}" stroke-width="1.4"/>`;
  }
  let lashes = '';
  for (let i = 0; i < 7; i++) {
    const t = 0.14 + (i / 6) * 0.72;
    const x = (1 - t) * (1 - t) * 20 + 2 * (1 - t) * t * 300 + t * t * 580;
    const y = (1 - t) * (1 - t) * 150 + 2 * (1 - t) * t * -50 + t * t * 150;
    const dx = (x - 300) / 300;
    lashes += `<line x1="${x.toFixed(1)}" y1="${y.toFixed(1)}" x2="${(x + dx * 26).toFixed(1)}" y2="${(y - 30 + Math.abs(dx) * 14).toFixed(1)}" stroke="url(#g)" stroke-width="5" stroke-linecap="round"/>`;
  }
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 -20 600 340" width="${w}">
    <defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#fff"/><stop offset="1" stop-color="#8a8a8a"/></linearGradient>
    <clipPath id="c"><path d="${lens}"/></clipPath></defs>
    <path d="${lens}" fill="#000" stroke="url(#g)" stroke-width="7" stroke-linejoin="round"/>
    <g clip-path="url(#c)">${fibres}
      <circle cx="${cx}" cy="${cy}" r="86" fill="none" stroke="#ececec" stroke-width="5"/>
      <circle cx="${cx}" cy="${cy}" r="40" fill="none" stroke="#ececec" stroke-opacity=".8" stroke-width="2.5"/>
      <circle cx="${cx}" cy="${cy}" r="28" fill="#ececec"/>
      <circle cx="${cx + 9}" cy="${cy - 9}" r="7" fill="#060606"/></g>
    ${lashes}</svg>`;
}

(async () => {
  const out = path.join(__dirname, '..', 'src-tauri', 'icons');
  fs.mkdirSync(out, { recursive: true });
  const html = `<body style="margin:0;background:#060606;width:1024px;height:1024px;display:grid;place-items:center">
    <div style="position:relative;width:1024px;height:1024px;display:grid;place-items:center">
      <svg style="position:absolute;inset:0" viewBox="0 0 1024 1024"><circle cx="512" cy="512" r="470" fill="none" stroke="#ececec" stroke-opacity=".55" stroke-width="10"/></svg>
      ${eye({ w: 760 })}</div></body>`;
  const b = await chromium.launch();
  const p = await b.newPage({ viewport: { width: 1024, height: 1024 } });
  await p.setContent(html);
  await p.screenshot({ path: path.join(out, 'source.png') });
  await b.close();
  fs.writeFileSync(path.join(__dirname, 'eye.svg'), eye({ w: 600 }));
  console.log('ok');
})();
