#!/usr/bin/env node
// Smoke-test the self-contained HTML report in a real DOM (jsdom): click
// every tab and require it to render content with zero JS errors.
//
// This catches what the Rust renderer tests (string assertions on the HTML)
// structurally cannot: template JS that throws at runtime. Note the report's
// own `safeRender` swallows tab exceptions into a `.tab-error` placeholder +
// console.error — so this harness asserts on BOTH of those channels, not
// just uncaught window errors.
//
// Usage: node scripts/report-smoke.mjs <report.html>
// jsdom is resolved from dashboard/node_modules (dev) or a plain install (CI).

import { createRequire } from 'node:module';
import fs from 'node:fs';
import path from 'node:path';
import url from 'node:url';

const here = path.dirname(url.fileURLToPath(import.meta.url));

function loadJsdom() {
  const require = createRequire(import.meta.url);
  const candidates = [path.join(here, '..', 'dashboard', 'node_modules', 'jsdom'), 'jsdom'];
  for (const c of candidates) {
    try {
      return require(c);
    } catch {
      /* try next */
    }
  }
  console.error('jsdom not found — run `pnpm -C dashboard install` (dev) or `npm i --no-save jsdom` (CI)');
  process.exit(2);
}

const { JSDOM, VirtualConsole } = loadJsdom();

const reportPath = process.argv[2];
if (!reportPath || !fs.existsSync(reportPath)) {
  console.error('usage: node scripts/report-smoke.mjs <report.html>');
  process.exit(2);
}

const html = fs.readFileSync(reportPath, 'utf8');

const consoleErrors = [];
const notImplemented = [];
const vc = new VirtualConsole();
vc.on('error', (...args) => consoleErrors.push(args.map(String).join(' ')));
vc.on('jsdomError', (err) => {
  // jsdom lacks layout/canvas APIs; those gaps are environment noise, not
  // template bugs. Everything else is a real page error.
  if (/Not implemented/i.test(String(err.message))) {
    notImplemented.push(String(err.message));
  } else {
    consoleErrors.push(String(err.stack || err.message));
  }
});

const dom = new JSDOM(html, {
  runScripts: 'dangerously',
  pretendToBeVisual: true,
  url: 'http://localhost/',
  virtualConsole: vc,
});
const { window } = dom;
window.addEventListener('error', (e) => consoleErrors.push('window error: ' + e.message));

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const failures = [];

await sleep(500); // boot + eager overview render

const doc = window.document;
const tabs = [...doc.querySelectorAll('button.tab')];
if (tabs.length < 5) {
  failures.push(`expected the report's tab bar, found only ${tabs.length} .tab buttons`);
}

for (const tab of tabs) {
  const name = tab.textContent.trim();
  const errorsBefore = consoleErrors.length;
  tab.dispatchEvent(new window.MouseEvent('click', { bubbles: true }));
  await sleep(300); // lazy render + setTimeout-based transitions settle

  const active = doc.querySelector('.tab-content.active');
  if (!active) {
    failures.push(`[${name}] no active tab content after click`);
    continue;
  }
  if (active.children.length === 0) {
    failures.push(`[${name}] tab rendered empty`);
  }
  const caught = active.querySelector('.tab-error');
  if (caught) {
    failures.push(`[${name}] safeRender caught an exception: ${caught.textContent}`);
  }
  const newErrors = consoleErrors.slice(errorsBefore);
  console.log(
    `  ${newErrors.length || caught ? '✗' : '✓'} ${name} (${active.children.length} top-level nodes)`
  );
}

await sleep(300); // trailing async work
consoleErrors.forEach((msg) => failures.push(`console error: ${msg.slice(0, 300)}`));

if (notImplemented.length) {
  console.log(`  (ignored ${notImplemented.length} jsdom "not implemented" environment gaps)`);
}

if (failures.length) {
  console.error(`\nreport-smoke FAILED — ${failures.length} problem(s):`);
  failures.forEach((f) => console.error('  ✗ ' + f));
  process.exit(1);
}
console.log(`\nreport-smoke OK — ${tabs.length} tabs rendered clean`);
process.exit(0);
