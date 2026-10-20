// End-to-end smoke test: drives the SPA in headless Chrome against a
// freshly installed server, through the UI as a user would. Started by
// run.sh (see README.md), which sets BASE, OUT, PUSHOVER_DIR and MONICA_BIN.

import puppeteer from 'puppeteer-core';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import zlib from 'node:zlib';
import { execFileSync } from 'node:child_process';

const BASE = process.env.BASE ?? 'http://127.0.0.1:8095';
const OUT = process.env.OUT ?? 'output';
const PUSHOVER_DIR = process.env.PUSHOVER_DIR ?? path.join(OUT, 'pushover');
const SHOTS = path.join(OUT, 'screenshots');
fs.mkdirSync(SHOTS, { recursive: true });

const run = Date.now().toString(36);
const user = {
  firstName: 'Ada',
  lastName: 'Lovelace',
  username: `ada-${run}`,
  password: 'Analytical-Engine-1843!',
};
const state = {};

// --- browser and problem tracking ----------------------------------------

const browser = await puppeteer.launch({
  executablePath: '/usr/bin/google-chrome',
  headless: true,
  args: ['--no-sandbox', '--lang=en-US'],
});
const page = await browser.newPage();
await page.setViewport({ width: 1280, height: 900 });
page.setDefaultTimeout(15000);

let problems = [];
// Responses ≥ 400 the current step expects: [method, path regexp, status].
let expected = [];
const isExpected = (method, url, status) =>
  expected.some(([m, re, s]) => m === method && re.test(new URL(url).pathname) && s === status);

page.on('console', (m) => {
  if (m.type() !== 'error') return;
  // Chrome logs every failed response; those are checked below.
  if (m.text().startsWith('Failed to load resource')) return;
  problems.push(`console error at ${page.url()}: ${m.text()}`);
});
page.on('pageerror', (e) => problems.push(`page error at ${page.url()}: ${e.message}`));
// SMOKE_TRACE=1 prints the API requests that change something.
if (process.env.SMOKE_TRACE) {
  page.on('request', (r) => {
    if (r.url().startsWith(`${BASE}/api/`) && r.method() !== 'GET') {
      console.log('     >', r.method(), r.url().slice(BASE.length), r.postData()?.slice(0, 300) ?? '');
    }
  });
}
page.on('requestfailed', (r) => {
  // Requests cut short by a navigation (e.g. the fire-and-forget contact
  // view counter) are aborted; anything else is a network failure.
  if (r.url().startsWith(BASE) && r.failure()?.errorText !== 'net::ERR_ABORTED') {
    problems.push(`request failed: ${r.method()} ${r.url().slice(BASE.length)} ${r.failure()?.errorText}`);
  }
});
page.on('response', (r) => {
  if (r.status() < 400 || !r.url().startsWith(BASE)) return;
  const method = r.request().method();
  if (isExpected(method, r.url(), r.status())) return;
  problems.push(`HTTP ${r.status()} ${method} ${r.url().slice(BASE.length)} (at ${page.url().slice(BASE.length)})`);
});

// --- helpers ---------------------------------------------------------------

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

/** Waits until no request is in flight for a moment. */
async function settle() {
  await page.waitForNetworkIdle({ idleTime: 300, timeout: 20000 }).catch(() => {});
}

async function goto(pathname) {
  await page.goto(BASE + pathname, { waitUntil: 'load' });
  await settle();
}

async function reload() {
  await page.reload({ waitUntil: 'load' });
  await settle();
}

/** The current path. */
const here = () => new URL(page.url()).pathname;

/** Waits for the SPA to be at a path matching `re`. */
async function waitForPath(re, timeout = 15000) {
  await page.waitForFunction((src) => new RegExp(src).test(location.pathname), { timeout }, re.source);
  await settle();
}

/**
 * The innermost visible element matching `selector` (within `within`)
 * whose text is `text` (or contains it, unless `exact`).
 */
async function find(text, { selector = '*', within = null, exact = true, timeout = 10000 } = {}) {
  try {
    const handle = await page.waitForFunction(
      (text, selector, root, exact) => {
        const norm = (s) => (s ?? '').replace(/\s+/g, ' ').trim();
        const matches = [...(root ?? document).querySelectorAll(selector)].filter((e) => {
          if (e.getClientRects().length === 0) return false;
          // innerText follows CSS (`uppercase` buttons), textContent doesn't.
          return [norm(e.innerText), norm(e.textContent)].some((t) => (exact ? t === text : t.includes(text)));
        });
        return matches.find((e) => !matches.some((o) => o !== e && e.contains(o))) ?? null;
      },
      { timeout },
      text,
      selector,
      within,
      exact,
    );
    return handle.asElement();
  } catch {
    throw new Error(`no visible ${selector} with text ${JSON.stringify(text)}`);
  }
}

/** Whether such an element exists now. */
async function exists(text, opts = {}) {
  return find(text, { ...opts, timeout: 500 })
    .then(() => true)
    .catch(() => false);
}

async function click(text, opts = {}) {
  await clickElement(await find(text, opts));
}

/** Clicks `el` in the middle of the viewport (away from the sticky header). */
async function clickElement(el) {
  await el.evaluate((e) => e.scrollIntoView({ block: 'center' }));
  await el.click();
  await settle();
}

/** Asserts `text` is on the page (or not, with `absent`). */
async function see(text, { within = null, absent = false, timeout = 8000 } = {}) {
  try {
    await page.waitForFunction(
      (text, root, absent) => {
        const t = ((root ?? document.body).innerText ?? '').replace(/\s+/g, ' ');
        return absent ? !t.includes(text) : t.includes(text);
      },
      { timeout },
      text,
      within,
      absent,
    );
  } catch {
    throw new Error(
      absent ? `${JSON.stringify(text)} is still on the page` : `${JSON.stringify(text)} is not on the page`,
    );
  }
}

/** The form control labelled `label` (a `<label for>` whose text starts with it). */
async function control(label, { within = null } = {}) {
  const handle = await page
    .waitForFunction(
      (label, root) => {
        const norm = (s) => (s ?? '').replace(/\s+/g, ' ').trim();
        const l = [...(root ?? document).querySelectorAll('label[for]')].find(
          (e) => e.getClientRects().length > 0 && norm(e.textContent).replace(/ optional$/, '') === label,
        );
        if (!l) return null;
        // Some components share an id (`TextArea`'s default): prefer the label's own field.
        return l.parentElement.querySelector(`#${CSS.escape(l.htmlFor)}`) ?? document.getElementById(l.htmlFor);
      },
      { timeout: 10000 },
      label,
      within,
    )
    .catch(() => {
      throw new Error(`no field labelled ${JSON.stringify(label)}`);
    });
  return handle.asElement();
}

/** Clears an input or textarea and types `value` into it. */
async function typeInto(el, value) {
  await el.evaluate((e) => e.scrollIntoView({ block: 'center' }));
  await el.click();
  await page.keyboard.down('Control');
  await page.keyboard.press('KeyA');
  await page.keyboard.up('Control');
  await page.keyboard.press('Backspace');
  await el.type(String(value));
}

async function fill(label, value, opts = {}) {
  await typeInto(await control(label, opts), value);
}

/** Selects the option whose text is `optionText` in the select labelled `label` (or the select itself). */
async function choose(labelOrSelect, optionText, opts = {}) {
  const el = typeof labelOrSelect === 'string' ? await control(labelOrSelect, opts) : labelOrSelect;
  const found = await page
    .waitForFunction((s, text) => [...s.options].some((o) => o.text.trim() === text), { timeout: 5000 }, el, optionText)
    .catch(() => null);
  if (!found) {
    const options = await el.evaluate((s) => [...s.options].map((o) => o.text.trim()));
    throw new Error(`no option ${JSON.stringify(optionText)} in ${JSON.stringify(options)}`);
  }
  const value = await el.evaluate((s, text) => [...s.options].find((o) => o.text.trim() === text).value, optionText);
  await el.select(value);
  await settle();
}

/**
 * The root of a contact page module: the block holding the title `title`
 * (each module is the first child of a class-less wrapper `div`).
 */
async function moduleRoot(title) {
  const handle = await page
    .waitForFunction(
      (title) => {
        const norm = (s) => (s ?? '').replace(/\s+/g, ' ').trim();
        const heads = [...document.querySelectorAll('span, div, p')].filter(
          (e) => e.getClientRects().length > 0 && norm(e.textContent) === title && e.children.length === 0,
        );
        for (const h of heads) {
          let n = h;
          while (n.parentElement && !(n.parentElement.tagName === 'DIV' && n.parentElement.className === '')) {
            n = n.parentElement;
          }
          if (n.parentElement) return n;
        }
        return null;
      },
      { timeout: 10000 },
      title,
    )
    .catch(() => {
      throw new Error(`no module ${JSON.stringify(title)} on ${here()}`);
    });
  return handle.asElement();
}

let shotCount = 0;
async function screenshot(name) {
  shotCount += 1;
  await fullScreenshot(path.join(SHOTS, `${String(shotCount).padStart(2, '0')}-${name}.png`));
}

/** A full-page screenshot, scrolled to the top (the sticky header stays there). */
async function fullScreenshot(file) {
  await page.evaluate(() => window.scrollTo(0, 0)).catch(() => {});
  await page.screenshot({ path: file, fullPage: true }).catch(() => {});
}

/** A screenshot of `el` alone, taken from the top of the page (so the sticky header doesn't cover it). */
async function elementScreenshot(el, file) {
  const clip = await el.evaluate((e) => {
    window.scrollTo(0, 0);
    const r = e.getBoundingClientRect();
    return { x: r.x, y: r.y, width: r.width, height: r.height };
  });
  await page.screenshot({ path: file, clip, captureBeyondViewport: true }).catch(() => {});
}

/** Asserts the page shows real content: not blank, not an error page. */
async function assertPageRenders() {
  const info = await page.evaluate(() => ({
    text: document.body.innerText.trim(),
    title: document.title,
  }));
  if (
    /^(Unauthorized|Forbidden|Not Found|Page Expired|Too Many Requests|Server Error|Service Unavailable)\b/.test(
      info.title,
    )
  ) {
    throw new Error(`error page: ${info.title}`);
  }
  if (info.text.length < 40) throw new Error(`blank page (${info.text.length} characters)`);
  await assertNoGarbage();
}

/** Asserts the page shows no value a broken adapter would render. */
async function assertNoGarbage() {
  const text = await page.evaluate(() => document.body.innerText);
  const bad = /\b(undefined|NaN|null)\b|Invalid Date|\[object Object\]/.exec(text);
  if (bad) {
    const at = text.slice(Math.max(0, bad.index - 60), bad.index + 40).replace(/\s+/g, ' ');
    throw new Error(`the page shows ${JSON.stringify(bad[0])}: …${at}…`);
  }
}

// --- Pushover ----------------------------------------------------------------

/** The messages the fake Pushover got: their form fields (`token`, `user`, `title`, `message`). */
function readPushes() {
  if (!fs.existsSync(PUSHOVER_DIR)) return [];
  return fs
    .readdirSync(PUSHOVER_DIR)
    .filter((f) => f.endsWith('.json'))
    .sort()
    .map((f) => ({ file: f, ...JSON.parse(fs.readFileSync(path.join(PUSHOVER_DIR, f), 'utf8')) }));
}

async function waitForPush(predicate, what, timeout = 20000) {
  const start = Date.now();
  while (Date.now() - start < timeout) {
    const push = readPushes().find(predicate);
    if (push) return push;
    await sleep(250);
  }
  throw new Error(`no ${what} notification arrived at the fake Pushover`);
}

// --- TOTP (RFC 6238, SHA-1, 6 digits, 30 s) ---------------------------------

function base32(secret) {
  const alphabet = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ234567';
  let bits = '';
  for (const c of secret.replace(/[\s=]/g, '').toUpperCase()) bits += alphabet.indexOf(c).toString(2).padStart(5, '0');
  const bytes = [];
  for (let i = 0; i + 8 <= bits.length; i += 8) bytes.push(parseInt(bits.slice(i, i + 8), 2));
  return Buffer.from(bytes);
}

const timeStep = (ms = Date.now()) => Math.floor(ms / 1000 / 30);

function totp(secret, step = timeStep()) {
  const counter = Buffer.alloc(8);
  counter.writeBigUInt64BE(BigInt(step));
  const h = crypto.createHmac('sha1', base32(secret)).update(counter).digest();
  const offset = h[h.length - 1] & 0xf;
  return String((h.readUInt32BE(offset) & 0x7fffffff) % 1_000_000).padStart(6, '0');
}

// --- generated files ---------------------------------------------------------

/** A `size`×`size` RGB PNG with a diagonal gradient. */
function makePng(size = 640) {
  const crcTable = Array.from({ length: 256 }, (_, n) => {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    return c >>> 0;
  });
  const crc = (buf) => {
    let c = 0xffffffff;
    for (const b of buf) c = crcTable[(c ^ b) & 0xff] ^ (c >>> 8);
    return (c ^ 0xffffffff) >>> 0;
  };
  const chunk = (type, data) => {
    const len = Buffer.alloc(4);
    len.writeUInt32BE(data.length);
    const td = Buffer.concat([Buffer.from(type), data]);
    const c = Buffer.alloc(4);
    c.writeUInt32BE(crc(td));
    return Buffer.concat([len, td, c]);
  };
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(size, 0);
  ihdr.writeUInt32BE(size, 4);
  ihdr[8] = 8; // bit depth
  ihdr[9] = 2; // RGB
  const raw = Buffer.alloc(size * (size * 3 + 1));
  for (let y = 0; y < size; y++) {
    const row = y * (size * 3 + 1);
    for (let x = 0; x < size; x++) {
      raw[row + 1 + x * 3] = (x * 255) / size;
      raw[row + 2 + x * 3] = (y * 255) / size;
      raw[row + 3 + x * 3] = 160;
    }
  }
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk('IHDR', ihdr),
    chunk('IDAT', zlib.deflateSync(raw)),
    chunk('IEND', Buffer.alloc(0)),
  ]);
}

/** A one-page PDF saying `text`. */
function makePdf(text) {
  const objects = [
    '<< /Type /Catalog /Pages 2 0 R >>',
    '<< /Type /Pages /Kids [3 0 R] /Count 1 >>',
    '<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 144] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>',
    null,
    '<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>',
  ];
  const stream = `BT /F1 18 Tf 20 70 Td (${text}) Tj ET`;
  objects[3] = `<< /Length ${stream.length} >>\nstream\n${stream}\nendstream`;
  let pdf = '%PDF-1.4\n';
  const offsets = [];
  objects.forEach((o, i) => {
    offsets.push(pdf.length);
    pdf += `${i + 1} 0 obj\n${o}\nendobj\n`;
  });
  const xref = pdf.length;
  pdf += `xref\n0 ${objects.length + 1}\n0000000000 65535 f \n`;
  for (const o of offsets) pdf += `${String(o).padStart(10, '0')} 00000 n \n`;
  pdf += `trailer\n<< /Size ${objects.length + 1} /Root 1 0 R >>\nstartxref\n${xref}\n%%EOF\n`;
  return Buffer.from(pdf, 'latin1');
}

// --- steps -----------------------------------------------------------------

const results = [];

/** Runs a step (or sub-step): records its problems and a screenshot. */
async function step(name, fn) {
  problems = [];
  expected = [];
  const start = Date.now();
  let error = null;
  try {
    await fn();
    await settle();
  } catch (e) {
    error = e;
  }
  await screenshot(error ? `${name}-FAILED` : name);
  const issues = [...(error ? [`${error.message} (at ${here()})`] : []), ...problems];
  results.push({ name, ok: issues.length === 0, issues, ms: Date.now() - start });
  console.log(`${issues.length === 0 ? 'PASS' : 'FAIL'} ${name} (${((Date.now() - start) / 1000).toFixed(1)} s)`);
  for (const i of issues) console.log(`     ${i}`);
  if (error && process.env.SMOKE_DEBUG) console.log(error.stack);
  return issues.length === 0;
}

/** Logs in with the username and password, through the 2FA challenge when `secret` is given. */
async function logIn(secret = null) {
  expected.push(['GET', /^\/api\/v1\/me\/bootstrap$/, 401]);
  await goto('/login');
  await fill('Username', user.username);
  await typeInto(await page.$('#password'), user.password);
  await click('Log in', { selector: 'button' });
  if (secret) {
    await waitForPath(/^\/two-factor-challenge$/);
    // A code is accepted once: use a time step after the last one used.
    while (timeStep() <= state.lastTotpStep) await sleep(1000);
    state.lastTotpStep = timeStep();
    await typeInto(await page.$('#code'), totp(secret, state.lastTotpStep));
    await click('Log in', { selector: 'button' });
  }
  await waitForPath(/^\/vaults/);
}

/** First run: any page leads to the setup page, which creates the account and its first user; sign-up is closed after that. */
async function setUp() {
  expected.push(['GET', /^\/api\/v1\/me\/bootstrap$/, 401]);
  await goto('/vaults');
  await waitForPath(/^\/register$/);
  await see('Set up Monica');
  await fill('First name', user.firstName);
  await fill('Last name', user.lastName);
  await fill('Username', user.username);
  await fill('Password', user.password);
  await fill('Confirm Password', user.password);
  await click('Create Account', { selector: 'button' });
  await waitForPath(/^\/vaults/);
  await see('Create a vault');

  await click('Logout', { selector: 'button' });
  await waitForPath(/^\/login$/);
  await goto('/register');
  await waitForPath(/^\/login$/);
  await logIn();
  await see('Create a vault');
}

async function enableTwoFactor() {
  await goto('/user/profile');
  await see('Two Factor Authentication');
  await click('Enable', { selector: 'button' });
  await confirmPasswordIfAsked();
  const setup = await find('Setup Key:', { selector: 'p', exact: false });
  const secret = (await setup.evaluate((e) => e.innerText)).replace('Setup Key:', '').trim();
  if (!/^[A-Z2-7]{16,}=*$/.test(secret)) throw new Error(`unexpected setup key ${JSON.stringify(secret)}`);
  state.lastTotpStep = timeStep();
  await typeInto(await page.$('#code'), totp(secret, state.lastTotpStep));
  await click('Confirm', { selector: 'button' });
  await confirmPasswordIfAsked();
  await see('You have enabled two factor authentication.');

  await reload();
  await see('You have enabled two factor authentication.');

  expected.push(['GET', /^\/api\/v1\/me\/bootstrap$/, 401]);
  await click('Logout', { selector: 'button' });
  await waitForPath(/^\/login$/);
  await logIn(secret);
  await goto('/user/profile');
  await see('You have enabled two factor authentication.');
}

async function confirmPasswordIfAsked() {
  await sleep(500);
  const dialog = await page.$('input[placeholder="Password"][type="password"]');
  if (dialog && (await dialog.isVisible())) {
    await typeInto(dialog, user.password);
    await click('Confirm', {
      selector: 'button',
      within: await dialog.evaluateHandle((e) => e.closest('[role="dialog"], .fixed') ?? document.body),
    });
  }
}

async function createVaultAndContacts() {
  await goto('/vaults');
  await click('Create a vault', { exact: false, selector: 'a, button' });
  await waitForPath(/^\/vaults\/create$/);
  await fill('Name', 'Smoke vault');
  await fill('Description', 'Made by the smoke test');
  await click('Create', { selector: 'button' });
  await waitForPath(/^\/vaults$/);
  await click('Smoke vault', { selector: 'a' });
  await waitForPath(/^\/vaults\/[0-9a-f-]{36}$/);
  state.vault = here().split('/')[2];

  state.contacts = [];
  for (const [first, last] of [
    ['Charles', 'Babbage'],
    ['Mary', 'Somerville'],
  ]) {
    await goto(`/vaults/${state.vault}/contacts`);
    await click('Add a contact', { selector: 'a, button' });
    await waitForPath(/\/contacts\/create$/);
    await fill('First name', first);
    await fill('Last name', last);
    await click('Add', { selector: 'button' });
    await waitForPath(/\/contacts\/[0-9a-f-]{36}$/);
    state.contacts.push({ id: here().split('/')[4], name: `${first} ${last}` });
    await see(`${first} ${last}`);
  }
  await goto(`/vaults/${state.vault}/contacts`);
  await see('Charles Babbage');
  await see('Mary Somerville');
}

// --- step 4: contact modules -------------------------------------------------

const vaultPath = (rest = '') => `/vaults/${state.vault}${rest}`;
const contactPath = (c = state.contacts[0], rest = '') => vaultPath(`/contacts/${c.id}${rest}`);

/** Opens contact `c`'s page, on the tab named `tab` (clicking it, as a user would). */
async function openContact(tab = null, c = state.contacts[0]) {
  await goto(contactPath(c));
  if (tab) {
    await click(tab, { selector: 'a' });
    await waitForPath(/\/tabs\/[^/]+$/);
  }
}

/**
 * The innermost element within `within` that holds `text` and a control:
 * a leaf element with the text `control`, or (`css:…`) a selector.
 */
async function itemWith(text, control, within = null) {
  const handle = await page
    .waitForFunction(
      (text, control, root) => {
        const norm = (s) => (s ?? '').replace(/\s+/g, ' ').trim();
        const visible = (e) => e.getClientRects().length > 0;
        const hasControl = (n) =>
          control.startsWith('css:')
            ? n.querySelector(control.slice(4))
            : [...n.querySelectorAll('*')].some(
                (e) => e.children.length === 0 && visible(e) && norm(e.textContent) === control,
              );
        const holders = [...(root ?? document.body).querySelectorAll('*')].filter(
          (e) => visible(e) && norm(e.textContent).includes(text),
        );
        const inner = holders.filter((e) => !holders.some((o) => o !== e && e.contains(o)));
        for (let n of inner) {
          while (n && n !== root && !hasControl(n)) n = n.parentElement;
          if (n && n !== root) return n;
        }
        return null;
      },
      { timeout: 10000 },
      text,
      control,
      within,
    )
    .catch(() => {
      throw new Error(`no item ${JSON.stringify(text)} with ${JSON.stringify(control)}`);
    });
  return handle.asElement();
}

/** Opens the hover menu (`HoverMenu.vue`) of `item` and picks `action`. */
async function menu(item, action) {
  const link = await item.$('.ant-dropdown-link');
  await link.evaluate((e) => e.scrollIntoView({ block: 'center' }));
  for (let attempt = 1; ; attempt++) {
    // The menu opens on mouseenter: come from outside the link.
    await page.mouse.move(0, 0);
    await link.hover();
    const opened = await find(action, { selector: '.ant-dropdown-menu a', timeout: 2000 }).catch(() => null);
    if (opened) {
      await opened.click();
      await settle();
      break;
    }
    if (attempt === 3) throw new Error(`the menu of the item does not open (${action})`);
  }
  await page.mouse.move(0, 0);
}

/** Asserts `text` is (or isn't) in module `title`, now and after a reload. */
async function checkModule(title, text, { absent = false } = {}) {
  await see(text, { within: await moduleRoot(title), absent });
  await assertNoGarbage();
  await reload();
  await see(text, { within: await moduleRoot(title), absent });
  await assertNoGarbage();
  if (!absent) {
    moduleShots += 1;
    const file = path.join(
      SHOTS,
      'modules',
      `${String(moduleShots).padStart(3, '0')}-${title.replace(/\W+/g, '-')}.png`,
    );
    await elementScreenshot(await moduleRoot(title), file);
  }
}
let moduleShots = 0;
fs.mkdirSync(path.join(SHOTS, 'modules'), { recursive: true });

/** Today in the DatePicker input format of the `en` locale. */
const today = new Date();
const pickerDate = (d = today) =>
  `${String(d.getMonth() + 1).padStart(2, '0')}/${String(d.getDate()).padStart(2, '0')}/${d.getFullYear()}`;

/** Types a date into the n-th DatePicker input of `within`. */
async function pickDate(within, value = pickerDate(), index = 0) {
  const inputs = await within.$$('input:not([type]), input[type="text"]');
  const candidates = [];
  for (const i of inputs) {
    if (await i.evaluate((e) => !e.id && e.closest('.vc-popover-content-wrapper') === null)) candidates.push(i);
  }
  const input = candidates[index];
  if (!input) throw new Error('no date input');
  await typeInto(input, value);
  // Blur (not Enter, which submits the form) applies the typed date.
  await page.mouse.click(5, 5);
  await settle();
}

/**
 * Adds `name` with a ContactSelector (`Shared/Form/ContactSelector.vue`) in
 * `block`: its "+ add a contact", a search, then "Add" on the result.
 */
async function pickContact(block, query, name) {
  if (await exists('+ add a contact', { within: block, selector: 'p' })) {
    await click('+ add a contact', { within: block, selector: 'p' });
  }
  const input = await block.$('input[placeholder="Find a contact in this vault"]');
  await typeInto(input, query);
  await click('Add', { within: await itemWith(name, 'Add', block), selector: 'li' });
}

/** The block around the element with text `label` (`up` levels up). */
async function blockOf(label, up = 2, opts = {}) {
  const el = await find(label, opts);
  return (
    await el.evaluateHandle((e, up) => {
      let n = e;
      for (let i = 0; i < up; i++) n = n.parentElement;
      return n;
    }, up)
  ).asElement();
}

async function moduleNotes() {
  await openContact('Information');
  let m = await moduleRoot('Notes');
  await click('Add a note', { within: m, selector: 'button' });
  await fill('Body', 'Talked about the difference engine.', { within: m });
  await click('+ add title', { within: m });
  await fill('Title', 'First meeting', { within: m });
  await click('Save', { within: m, selector: 'button' });
  await checkModule('Notes', 'Talked about the difference engine.');

  m = await moduleRoot('Notes');
  await menu(await itemWith('Talked about the difference engine.', 'css:.ant-dropdown-link', m), 'Edit');
  await fill('Body', 'Talked about the analytical engine.', { within: m });
  await click('Update', { within: m, selector: 'button' });
  await checkModule('Notes', 'Talked about the analytical engine.');

  m = await moduleRoot('Notes');
  await menu(await itemWith('Talked about the analytical engine.', 'css:.ant-dropdown-link', m), 'Delete');
  await checkModule('Notes', 'Talked about the analytical engine.', { absent: true });
}

async function moduleImportantDates() {
  await openContact();
  let m = await moduleRoot('Important dates');
  await clickElement(await m.$('a'));
  await waitForPath(/\/dates$/);
  await click('Add a date', { selector: 'button' });
  await fill('Name', 'First meeting');
  await click('I only know the day and month, not the year', { selector: 'label' });
  await choose('Month', 'June');
  await choose('Day', '5');
  await click('Save', { selector: 'button' });
  await see('First meeting');
  await reload();
  await see('First meeting');
  await openContact();
  await see('First meeting', { within: await moduleRoot('Important dates') });

  await goto(contactPath(state.contacts[0], '/dates'));
  await click('Edit', { within: await itemWith('First meeting', 'Edit') });
  await fill('Name', 'Royal Society meeting');
  await click('Save', { selector: 'button' });
  await see('Royal Society meeting');
  await openContact();
  await see('Royal Society meeting', { within: await moduleRoot('Important dates') });

  await goto(contactPath(state.contacts[0], '/dates'));
  await click('Delete', { within: await itemWith('Royal Society meeting', 'Delete') });
  await see('Royal Society meeting', { absent: true });
  await openContact();
  await see('Royal Society meeting', { within: await moduleRoot('Important dates'), absent: true });
}

async function moduleLabels() {
  await openContact();
  const open = async () => clickElement(await (await moduleRoot('Labels')).$('span.cursor-pointer'));
  const filter = async (text) => typeInto(await page.$('input[placeholder="Filter list or create a new label"]'), text);

  await open();
  await filter('Inventor');
  await click('Create new label "Inventor"', { selector: 'li' });
  await see('Inventor', { within: await moduleRoot('Labels') });
  await checkModule('Labels', 'Inventor');

  // Edit: swap the label for another one.
  await open();
  await filter('Mathematician');
  await click('Create new label "Mathematician"', { selector: 'li' });
  await filter('Inventor');
  await click('Inventor', { selector: 'li' });
  await click('Close', { within: await moduleRoot('Labels') });
  await checkModule('Labels', 'Mathematician');
  await see('Inventor', { within: await moduleRoot('Labels'), absent: true });

  await open();
  await filter('Mathematician');
  await click('Mathematician', { selector: 'li' });
  await click('Close', { within: await moduleRoot('Labels') });
  await checkModule('Labels', 'Not set');
}

async function moduleGenderPronoun() {
  await openContact();
  await clickElement(await (await moduleRoot('Gender')).$('a'));
  await waitForPath(/\/edit$/);
  await choose('Gender', 'Male');
  await choose('Pronoun', 'he/him');
  await click('Update', { selector: 'button' });
  await waitForPath(/\/contacts\/[0-9a-f-]{36}$/);
  await see('he/him');
  await reload();
  await see('Male');

  await goto(contactPath(state.contacts[0], '/edit'));
  await choose('Gender', 'Other');
  await choose('Pronoun', 'they/them');
  await click('Update', { selector: 'button' });
  await waitForPath(/\/contacts\/[0-9a-f-]{36}$/);
  await reload();
  await see('they/them');

  await goto(contactPath(state.contacts[0], '/edit'));
  // The gender and pronoun lists have no empty choice: they can't be unset.
}

async function moduleJobInformation() {
  await openContact();
  const open = async () => clickElement(await (await moduleRoot('Job information')).$('span.cursor-pointer'));
  await open();
  if (await exists('Or create a new one', { selector: 'p' })) await click('Or create a new one', { selector: 'p' });
  await fill('Company name', 'Analytical Society');
  await fill('Job position', 'Mathematician');
  await click('Save', { within: await moduleRoot('Job information'), selector: 'button' });
  await checkModule('Job information', 'Mathematician');
  await see('Analytical Society', { within: await moduleRoot('Job information') });

  await open();
  await fill('Job position', 'Lucasian Professor');
  await click('Save', { within: await moduleRoot('Job information'), selector: 'button' });
  await checkModule('Job information', 'Lucasian Professor');

  await open();
  await click('Or reset the fields', { selector: 'p' });
  await reload();
  await see('Not set', { within: await moduleRoot('Job information') });
}

async function moduleReligion() {
  await openContact();
  const open = async () => clickElement(await (await moduleRoot('Religion')).$('span.cursor-pointer'));
  const select = async () => (await moduleRoot('Religion')).$('select');
  await open();
  await choose(await select(), 'Christian');
  await click('Save', { within: await moduleRoot('Religion'), selector: 'button' });
  await checkModule('Religion', 'Christian');

  await open();
  await choose(await select(), 'Buddhist');
  await click('Save', { within: await moduleRoot('Religion'), selector: 'button' });
  await checkModule('Religion', 'Buddhist');
  // The religion list has no empty choice: it can't be unset.
}

async function moduleAddresses() {
  await openContact('Ways to connect');
  let m = await moduleRoot('Addresses');
  await click('Add an address', { within: m, selector: 'button' });
  if (await exists('Create a new address', { selector: 'label' }))
    await click('Create a new address', { selector: 'label' });
  await fill('Address', '1 Dorset Street', { within: m });
  await fill('City', 'London', { within: m });
  await fill('Postal code', 'W1U 4EG', { within: m });
  await fill('Country', 'United Kingdom', { within: m });
  await click('Save', { within: m, selector: 'button' });
  await checkModule('Addresses', '1 Dorset Street');

  m = await moduleRoot('Addresses');
  await click('Edit', { within: await itemWith('1 Dorset Street', 'Edit', m) });
  await fill('Address', '5 Devonshire Street', { within: m });
  await click('Save', { within: m, selector: 'button' });
  await checkModule('Addresses', '5 Devonshire Street');

  m = await moduleRoot('Addresses');
  await click('Delete', { within: await itemWith('5 Devonshire Street', 'Delete', m) });
  await click('Delete', { selector: 'button' });
  await checkModule('Addresses', '5 Devonshire Street', { absent: true });
}

async function moduleContactInformation() {
  await openContact('Ways to connect');
  let m = await moduleRoot('Contact information');
  await click('Add a contact information', { within: m, selector: 'button' });
  await choose('Type', 'Email address', { within: m });
  await fill('Data', 'charles@example.com', { within: m });
  await click('Save', { within: m, selector: 'button' });
  await checkModule('Contact information', 'charles@example.com');

  m = await moduleRoot('Contact information');
  await click('Edit', { within: await itemWith('charles@example.com', 'Edit', m) });
  await fill('Data', 'babbage@example.com', { within: m });
  await click('Save', { within: m, selector: 'button' });
  await checkModule('Contact information', 'babbage@example.com');

  m = await moduleRoot('Contact information');
  await click('Delete', { within: await itemWith('babbage@example.com', 'Delete', m) });
  await click('Delete', { selector: 'button' });
  await checkModule('Contact information', 'babbage@example.com', { absent: true });
}

async function moduleRelationships() {
  await openContact('Social');
  await click('Add a relationship', { within: await moduleRoot('Relationships'), selector: 'a' });
  await waitForPath(/\/relationships\/create$/);
  await choose('Select a relationship type', 'parent ↔ child');
  await click('Choose an existing contact', { selector: 'label' });
  await pickContact(await blockOf('Choose an existing contact', 3, { selector: 'label' }), 'Mary', 'Mary Somerville');
  await click('Add', { selector: 'button' });
  await waitForPath(/\/contacts\/[0-9a-f-]{36}/);
  await openContact('Social');
  await checkModule('Relationships', 'Mary Somerville');
  // A family relationship: in the family summary too.
  await see('Mary Somerville', { within: await moduleRoot('Family summary') });
  // The other side of the relationship.
  await openContact('Social', state.contacts[1]);
  await see('Charles Babbage', { within: await moduleRoot('Relationships') });

  // There is no edit in the relationships module: remove it.
  await openContact('Social');
  const m = await moduleRoot('Relationships');
  await click('Remove', { within: await itemWith('Mary Somerville', 'Remove', m) });
  await checkModule('Relationships', 'Mary Somerville', { absent: true });
  await see('Family summary', { absent: true });
}

async function modulePets() {
  await openContact('Social');
  let m = await moduleRoot('Pets');
  await click('Add a pet', { within: m, selector: 'button' });
  await choose('Pet category', 'Dog', { within: m });
  await fill('Name of the pet', 'Rex', { within: m });
  await click('Save', { within: m, selector: 'button' });
  await checkModule('Pets', 'Rex');

  m = await moduleRoot('Pets');
  await click('Edit', { within: await itemWith('Rex', 'Edit', m) });
  await fill('Name of the pet', 'Rover', { within: m });
  await click('Save', { within: m, selector: 'button' });
  await checkModule('Pets', 'Rover');

  m = await moduleRoot('Pets');
  await click('Delete', { within: await itemWith('Rover', 'Delete', m) });
  await checkModule('Pets', 'Rover', { absent: true });
}

async function moduleGroups() {
  await openContact('Social');
  let m = await moduleRoot('Groups');
  await click('Add to group', { within: m, selector: 'button' });
  await choose('Select a group or create a new one', '+ create a group', { within: m });
  await fill('Name', 'Analytical Society', { within: m });
  await click('Save', { within: m, selector: 'button' });
  await checkModule('Groups', 'Analytical Society');

  // Edit: rename the group from its page.
  m = await moduleRoot('Groups');
  await click('Show', { within: await itemWith('Analytical Society', 'Show', m) });
  await waitForPath(/\/groups\/\d+$/);
  state.group = here().split('/').pop();
  await click('Edit', { selector: 'a', exact: false });
  await waitForPath(/\/groups\/\d+\/edit$/);
  await fill('Name', 'Royal Astronomical Society');
  await click('Save', { selector: 'button' });
  await waitForPath(/\/groups\/\d+$/);
  await see('Royal Astronomical Society');
  await openContact('Social');
  await checkModule('Groups', 'Royal Astronomical Society');

  m = await moduleRoot('Groups');
  await click('Leave', { within: await itemWith('Royal Astronomical Society', 'Leave', m) });
  await checkModule('Groups', 'Royal Astronomical Society', { absent: true });
}

async function moduleLifeEvents() {
  await openContact('Life & goals');
  let m = await moduleRoot('Life events');
  await click('Add a life event', { within: m, selector: 'button' });
  await click('Transportation', { within: m });
  await click('Choose', { within: await itemWith('Walked', 'Choose', m) });
  await click('+ add summary', { within: m });
  await fill('Summary', 'Walked to the Analytical Society', { within: m });
  await click('Save', { within: m, selector: 'button' });
  // Timeline events start collapsed: open it (the state is saved).
  m = await moduleRoot('Life events');
  await clickElement(await m.$('div.mb-4 > div.cursor-pointer'));
  await checkModule('Life events', 'Walked to the Analytical Society');

  m = await moduleRoot('Life events');
  await menu(await itemWith('Walked to the Analytical Society', 'css:.ant-dropdown-link', m), 'Edit');
  await fill('Summary', 'Walked to the Royal Society', { within: m });
  await click('Save', { within: m, selector: 'button' });
  await checkModule('Life events', 'Walked to the Royal Society');

  m = await moduleRoot('Life events');
  await menu(await itemWith('Walked to the Royal Society', 'css:.ant-dropdown-link', m), 'Delete');
  await checkModule('Life events', 'Walked to the Royal Society', { absent: true });
}

async function moduleGoals() {
  await openContact('Life & goals');
  let m = await moduleRoot('Goals');
  await click('Add a goal', { within: m, selector: 'button' });
  await fill('Name', 'Finish the difference engine', { within: m });
  await click('Save', { within: m, selector: 'button' });
  await checkModule('Goals', 'Finish the difference engine');

  // Streak: mark today as done.
  m = await moduleRoot('Goals');
  const days = await m.$$('span.cursor-pointer.text-2xl');
  await clickElement(days[days.length - 1]);
  await checkModule('Goals', '👍');

  await click('View details', { within: await moduleRoot('Goals'), selector: 'a' });
  await waitForPath(/\/goals\/\d+$/);
  await click('Rename', { selector: 'div' });
  await fill('Name', 'Build the analytical engine');
  await click('Update', { selector: 'button' });
  await see('Build the analytical engine');
  await openContact('Life & goals');
  await checkModule('Goals', 'Build the analytical engine');

  await click('View details', { within: await moduleRoot('Goals'), selector: 'a' });
  await waitForPath(/\/goals\/\d+$/);
  await click('Delete', { selector: 'div' });
  await waitForPath(/\/contacts\/[0-9a-f-]{36}/);
  await openContact('Life & goals');
  await checkModule('Goals', 'Build the analytical engine', { absent: true });
}

async function moduleReminders() {
  await openContact('Information');
  let m = await moduleRoot('Reminders');
  await click('Add a reminder', { within: m, selector: 'button' });
  await fill('Name of the reminder', 'Birthday of the engine', { within: m });
  await pickDate(m);
  await click('Add date', { within: m, selector: 'button' });
  await checkModule('Reminders', 'Birthday of the engine');

  m = await moduleRoot('Reminders');
  await click('Edit', { within: await itemWith('Birthday of the engine', 'Edit', m) });
  await fill('Name of the reminder', 'Anniversary of the engine', { within: m });
  await click('Save', { within: m, selector: 'button' });
  await checkModule('Reminders', 'Anniversary of the engine');

  m = await moduleRoot('Reminders');
  await click('Delete', { within: await itemWith('Anniversary of the engine', 'Delete', m) });
  await checkModule('Reminders', 'Anniversary of the engine', { absent: true });
}

async function moduleLoans() {
  await openContact('Information');
  let m = await moduleRoot('Loans');
  await click('Record a loan', { within: m, selector: 'button' });
  await click('The loan is an object', { within: m, selector: 'label' });
  await fill('What is the loan?', 'A slide rule', { within: m });
  await pickContact(await blockOf('Who makes the loan?', 2, { selector: 'label' }), 'Ada', 'Ada Lovelace');
  await pickContact(await blockOf('Who the loan is for?', 2, { selector: 'label' }), 'Charles', 'Charles Babbage');
  await click('Add loan', { within: m, selector: 'button' });
  await checkModule('Loans', 'A slide rule');

  m = await moduleRoot('Loans');
  await click('Edit', { within: await itemWith('A slide rule', 'Edit', m) });
  await fill('What is the loan?', 'A brass gear', { within: m });
  await click('Save', { within: m, selector: 'button' });
  await checkModule('Loans', 'A brass gear');

  // Settle it and revert (settled loans are not listed after a reload).
  m = await moduleRoot('Loans');
  await click('Settle', { within: await itemWith('A brass gear', 'Settle', m) });
  await click('Revert', { within: await itemWith('A brass gear', 'Revert', m) });
  await checkModule('Loans', 'A brass gear');

  m = await moduleRoot('Loans');
  await click('Delete', { within: await itemWith('A brass gear', 'Delete', m) });
  await checkModule('Loans', 'A brass gear', { absent: true });
}

async function moduleTasks() {
  await openContact('Information');
  let m = await moduleRoot('Tasks');
  await click('Add a task', { within: m, selector: 'button' });
  await fill('Title', 'Order more brass', { within: m });
  await click('Save', { within: m, selector: 'button' });
  await checkModule('Tasks', 'Order more brass');

  m = await moduleRoot('Tasks');
  await menu(await itemWith('Order more brass', 'css:.ant-dropdown-link', m), 'Edit');
  await fill('Title', 'Order more steel', { within: m });
  await click('Save', { within: m, selector: 'button' });
  await checkModule('Tasks', 'Order more steel');

  // Complete it: it moves to the completed tasks.
  m = await moduleRoot('Tasks');
  await clickElement(
    await (await itemWith('Order more steel', 'css:input[type="checkbox"]', m)).$('input[type="checkbox"]'),
  );
  await reload();
  m = await moduleRoot('Tasks');
  await see('Order more steel', { within: m, absent: true });
  await click('Show completed tasks (1)', { within: m });
  const done = await itemWith('Order more steel', 'css:input[type="checkbox"]', m);
  if (!(await done.$eval('input[type="checkbox"]', (c) => c.checked)))
    throw new Error('the completed task is not checked');

  await menu(await itemWith('Order more steel', 'css:.ant-dropdown-link', m), 'Delete');
  await settle();
  await reload();
  m = await moduleRoot('Tasks');
  await see('Order more steel', { within: m, absent: true });
  await see('Show completed tasks', { within: m, absent: true });
}

async function moduleCalls() {
  await openContact('Information');
  let m = await moduleRoot('Calls');
  await click('Log a call', { within: m, selector: 'button' });
  await click('+ add reason', { within: m });
  await choose(await m.$('select#types'), 'Just to say hello');
  await click('Save', { within: m, selector: 'button' });
  await checkModule('Calls', 'Just to say hello');
  await see('I called', { within: await moduleRoot('Calls') });

  m = await moduleRoot('Calls');
  await menu(await itemWith('Just to say hello', 'css:.ant-dropdown-link', m), 'Edit');
  await click('Charles Babbage called', { within: m, selector: 'label' });
  await choose(await m.$('select#types'), 'For advice');
  await click('Update', { within: m, selector: 'button' });
  await checkModule('Calls', 'For advice');
  await see('Charles Babbage called', { within: await moduleRoot('Calls') });

  m = await moduleRoot('Calls');
  await menu(await itemWith('For advice', 'css:.ant-dropdown-link', m), 'Delete');
  await checkModule('Calls', 'There are no calls logged yet.');
}

async function quickFacts() {
  await openContact();
  let m = await moduleRoot('Quick facts');
  if (!(await exists('Add one now', { within: m }))) await click('Quick facts', { within: m });
  await click('Add one now', { within: m });
  await fill('Content', 'Hates street musicians', { within: m });
  await click('Save', { within: m, selector: 'button' });
  await see('Hates street musicians', { within: m });
  await reload();
  m = await moduleRoot('Quick facts');
  if (!(await exists('Hates street musicians', { within: m }))) await click('Quick facts', { within: m });
  await see('Hates street musicians', { within: m });

  await menu(await itemWith('Hates street musicians', 'css:.ant-dropdown-link', m), 'Edit');
  await fill('Content', 'Hates organ grinders', { within: m });
  await click('Save', { within: m, selector: 'button' });
  await see('Hates organ grinders', { within: m });
  await reload();
  m = await moduleRoot('Quick facts');
  if (!(await exists('Hates organ grinders', { within: m }))) await click('Quick facts', { within: m });
  await see('Hates organ grinders', { within: m });

  await menu(await itemWith('Hates organ grinders', 'css:.ant-dropdown-link', m), 'Delete');
  await see('Hates organ grinders', { within: m, absent: true });
  await reload();
  await see('Hates organ grinders', { absent: true });
}

async function mood() {
  await goto(vaultPath());
  await click('Record your mood', { selector: 'button' });
  const radio = await page.$('input[type="radio"][id^="input"]');
  await clickElement(radio);
  await click('+ note', {});
  await fill('Add a note', 'Feeling inventive');
  await click('Save', { selector: 'button' });
  await see('Your mood has been recorded!');
  await click('View history', { selector: 'a' });
  await waitForPath(/\/reports\/moodTrackingEvents$/);
  await assertPageRenders();
  // The event is in the user's contact feed (there is no edit or delete in the UI).
}

/** Creates a journal through the UI (once); returns its id. */
async function ensureJournal() {
  if (state.journal) return state.journal;
  await goto(vaultPath('/journals'));
  await click('Create a journal', { selector: 'a, button' });
  await waitForPath(/\/journals\/create$/);
  await fill('Name', 'Engine diary');
  await fill('Description', 'Notes on the engines');
  await click('Add', { selector: 'button' });
  await waitForPath(/\/journals\/\d+$/);
  state.journal = here().split('/').pop();
  return state.journal;
}

/** Writes a post about `contact` in the journal; returns its edit path. */
async function writePost(title, contact) {
  const journal = await ensureJournal();
  await goto(vaultPath(`/journals/${journal}`));
  await click('Create a post', { selector: 'a, button' });
  await waitForPath(/\/posts\/create$/);
  await click('Choose', { selector: 'a, button' });
  await waitForPath(/\/posts\/\d+\/edit$/);
  const edit = here();
  await fill('Title', title);
  await typeInto(await page.$('textarea'), `${title}: a visit to the workshop.`);
  await pickContact(
    await blockOf('Contacts in this post', 2, { selector: 'p' }),
    contact.name.split(' ')[0],
    contact.name,
  );
  await waitForAutosave();
  return edit;
}

/** Waits for the post editor's auto save to finish. */
async function waitForAutosave() {
  await sleep(1500);
  await settle();
  await see('Auto saved a few seconds ago');
}

async function modulePosts() {
  const charles = state.contacts[0];
  const edit = await writePost('Visit from Charles', charles);
  await click('Close', { selector: 'a' });
  await waitForPath(/\/posts\/\d+$/);
  await see('Visit from Charles');
  await openContact('Information');
  await checkModule('Posts', 'Visit from Charles');

  await goto(edit);
  await fill('Title', 'Second visit from Charles');
  await waitForAutosave();
  await openContact('Information');
  await checkModule('Posts', 'Second visit from Charles');

  await goto(edit);
  await click('Delete', { selector: 'div' });
  await waitForPath(/\/journals\/\d+$/);
  await see('Second visit from Charles', { absent: true });
  await openContact('Information');
  await checkModule('Posts', 'There are no posts yet.');
}

// --- step 5: files -----------------------------------------------------------

const FILES = path.join(OUT, 'files');
const DOWNLOADS = path.join(OUT, 'downloads');

/** Clicks `text` (an upload button) and gives the file chooser `file`. */
async function upload(text, file, opts = {}) {
  const [chooser] = await Promise.all([page.waitForFileChooser(), click(text, opts)]);
  await chooser.accept([file]);
  await sleep(500);
  await settle();
}

/** Asserts every `<img>` matching `selector` loaded, and returns their URLs. */
async function imagesLoaded(selector) {
  await page.waitForFunction((sel) => [...document.querySelectorAll(sel)].every((i) => i.complete), {}, selector);
  const imgs = await page.$$eval(selector, (all) => all.map((i) => ({ src: i.src, w: i.naturalWidth })));
  if (imgs.length === 0) throw new Error(`no image ${selector}`);
  for (const i of imgs) if (i.w === 0) throw new Error(`image did not load: ${i.src}`);
  return imgs.map((i) => i.src);
}

/** Fetches `url` in the page's session; returns the status and type. */
async function fetchStatus(url) {
  return page.evaluate(async (u) => {
    const r = await fetch(u, { credentials: 'same-origin' });
    return { status: r.status, type: r.headers.get('content-type'), size: (await r.arrayBuffer()).byteLength };
  }, url);
}

async function files() {
  fs.mkdirSync(FILES, { recursive: true });
  fs.mkdirSync(DOWNLOADS, { recursive: true });
  const png = path.join(FILES, 'engine.png');
  const pdf = path.join(FILES, 'engine-plans.pdf');
  fs.writeFileSync(png, makePng());
  fs.writeFileSync(pdf, makePdf('Plans of the analytical engine'));
  const cdp = await page.createCDPSession();
  await cdp.send('Browser.setDownloadBehavior', { behavior: 'allow', downloadPath: DOWNLOADS });

  // Photo: upload, thumbnail in the module, full image on its page, delete.
  await openContact('Information');
  await upload('Add a photo', png, { within: await moduleRoot('Photos'), selector: 'button' });
  const thumbs = await imagesLoaded('a[href*="/photos/"] img');
  await reload();
  for (const src of await imagesLoaded('a[href*="/photos/"] img')) {
    const r = await fetchStatus(src);
    if (r.status !== 200 || !/^image\//.test(r.type)) throw new Error(`thumbnail ${src}: ${r.status} ${r.type}`);
    if (!src.includes('thumbnail')) throw new Error(`the module does not show the thumbnail: ${src}`);
  }
  // Server-side thumbnails: a 300×300 crop.
  const size = await page.$eval('a[href*="/photos/"] img', (i) => `${i.naturalWidth}×${i.naturalHeight}`);
  if (size !== '300×300') throw new Error(`the thumbnail is ${size}, not 300×300`);
  await clickElement(await page.$('a[href*="/photos/"] img'));
  await waitForPath(/\/photos\/\d+$/);
  const [full] = await imagesLoaded('main img[alt="data.name"]');
  const r = await fetchStatus(full);
  if (r.status !== 200 || !/^image\/png/.test(r.type) || r.size !== fs.statSync(png).size) {
    throw new Error(`full image ${full}: ${r.status} ${r.type} ${r.size} bytes`);
  }
  if (thumbs[0] === full) throw new Error('the thumbnail is the full image');
  await click('Delete', { selector: 'span' });
  await waitForPath(/\/photos$/);
  await openContact('Information');
  await checkModule('Photos', 'There are no photos yet.');

  // Document: upload, download (same bytes), delete.
  await upload('Add a document', pdf, { within: await moduleRoot('Documents'), selector: 'button' });
  await checkModule('Documents', 'engine-plans.pdf');
  const m = await moduleRoot('Documents');
  await click('Download', { within: await itemWith('engine-plans.pdf', 'Download', m), selector: 'a' });
  const downloaded = path.join(DOWNLOADS, 'engine-plans.pdf');
  for (let i = 0; i < 40 && !fs.existsSync(downloaded); i++) await sleep(250);
  if (!fs.existsSync(downloaded)) throw new Error(`the download did not arrive (${fs.readdirSync(DOWNLOADS)})`);
  if (!fs.readFileSync(downloaded).equals(fs.readFileSync(pdf))) throw new Error('the downloaded document differs');
  await click('Delete', { within: await itemWith('engine-plans.pdf', 'Delete', await moduleRoot('Documents')) });
  await checkModule('Documents', 'There are no documents yet.');

  // Avatar: upload, shown (300×300 thumbnail), removed.
  await openContact();
  const before = await page.$eval('img[src*="/avatar"], div.mb-8 img', (i) => i.src).catch(() => null);
  await upload('Upload photo as avatar', png, { selector: 'span' });
  await reload();
  await see('Remove avatar');
  const avatarSrc = await page.$$eval('img', (all) =>
    all.map((i) => i.src).find((s) => /\/files\/\d+\/content/.test(s)),
  );
  if (!avatarSrc) throw new Error(`the avatar is not the uploaded file (was ${before})`);
  const a = await fetchStatus(avatarSrc);
  if (a.status !== 200 || !/^image\//.test(a.type)) throw new Error(`avatar ${avatarSrc}: ${a.status} ${a.type}`);
  await click('Remove avatar', { selector: 'span' });
  await reload();
  await see('Upload photo as avatar');
}

// --- step 6: reminder through Pushover --------------------------------------

async function reminderPushover() {
  // A Pushover channel; its keys are checked with the (fake) Pushover.
  await goto('/settings/notifications');
  await see('You can be notified through Pushover.');
  await click('Add a Pushover channel', { selector: 'button' });
  await fill('Give this channel a name', 'Phone');
  await fill('Your Pushover user key', 'uSmokeUserKey');
  await fill('The token of your Pushover application', 'aSmokeAppToken');
  await click('Add', { selector: 'button' });
  await see('uSmokeUserKey');
  await click('Send test', { selector: 'li' });
  await see('Test notification sent!');
  const test = await waitForPush((m) => /^This is a test notification for /.test(m.message), 'test');
  if (test.user !== 'uSmokeUserKey' || test.token !== 'aSmokeAppToken')
    throw new Error(`test sent with ${JSON.stringify(test)}`);

  const label = `Call Charles about the engine ${run}`;
  await openContact('Information');
  const m = await moduleRoot('Reminders');
  await click('Add a reminder', { within: m, selector: 'button' });
  await fill('Name of the reminder', label, { within: m });
  await pickDate(m);
  await click('Only once, when the next occurence of the date occurs.', { within: m, selector: 'label' });
  await click('Add date', { within: m, selector: 'button' });
  await checkModule('Reminders', label);

  // `monica test-reminders` (test:send-reminders) sends every scheduled
  // reminder now, against the server's database.
  const out = execFileSync(process.env.MONICA_BIN, ['test-reminders'], {
    env: { ...process.env, APP_ENV: 'local', RUST_LOG: 'warn' },
    encoding: 'utf8',
  });
  if (!/[1-9]\d* reminders sent/.test(out)) throw new Error(`test-reminders: ${out.trim()}`);
  const push = await waitForPush((p) => p.message === label, 'reminder');
  console.log(`     reminder: "${push.title}" / "${push.message}" (${push.file})`);
  if (push.title !== `Reminder for ${state.contacts[0].name}`) throw new Error(`unexpected title ${push.title}`);
}

// --- step 7: every page ---------------------------------------------------------

/** The SPA routes of routes.js: `{ path, name }`. */
function appRoutes() {
  const src = fs.readFileSync(new URL('../../resources/js/router/routes.js', import.meta.url), 'utf8');
  const routes = [
    ...[...src.matchAll(/\{ path: '([^']+)', name: '([^']+)'/g)],
    ...[...src.matchAll(/\br\(\s*'([^']+)',\s*'([^']+)'/g)],
  ].map((m) => ({ path: m[1], name: m[2] }));
  for (const r of routes) {
    r.re = new RegExp(`^${r.path.replace(/:[a-zA-Z]+/g, '[^/]+')}$`);
  }
  return routes;
}

// Routes a logged-in user doesn't reach without special data, or that act.
const SKIPPED_ROUTES = {
  login: 'guest only',
  register: 'guest only',
  'two-factor.login': 'guest only',
  'post.store': 'creates a post on visit (step 4 uses it)',
  'not-found': 'the error page',
};

/** Data the pages need beyond step 3, kept (created through the UI). */
async function pageData() {
  const mary = state.contacts[1];
  // A label, a goal, an address and a group on Mary.
  await openContact(null, mary);
  await clickElement(await (await moduleRoot('Labels')).$('span.cursor-pointer'));
  await typeInto(await page.$('input[placeholder="Filter list or create a new label"]'), 'Astronomer');
  await click('Create new label "Astronomer"', { selector: 'li' });
  await see('Astronomer', { within: await moduleRoot('Labels') });

  await openContact('Life & goals', mary);
  let m = await moduleRoot('Goals');
  await click('Add a goal', { within: m, selector: 'button' });
  await fill('Name', 'Translate Laplace', { within: m });
  await click('Save', { within: m, selector: 'button' });
  await see('Translate Laplace', { within: m });
  state.goal = (await m.$eval('a[href*="/goals/"]', (a) => a.getAttribute('href'))).split('/').pop();

  await openContact('Ways to connect', mary);
  m = await moduleRoot('Addresses');
  await click('Add an address', { within: m, selector: 'button' });
  if (await exists('Create a new address', { selector: 'label' }))
    await click('Create a new address', { selector: 'label' });
  await fill('City', 'Edinburgh', { within: m });
  await fill('Country', 'United Kingdom', { within: m });
  await click('Save', { within: m, selector: 'button' });
  await see('Edinburgh', { within: m });

  await openContact('Social', mary);
  m = await moduleRoot('Groups');
  await click('Add to group', { within: m, selector: 'button' });
  await choose('Select a group or create a new one', '+ create a group', { within: m });
  await fill('Name', 'Astronomers', { within: m });
  await click('Save', { within: m, selector: 'button' });
  await see('Astronomers', { within: m });

  await openContact('Information', mary);
  await upload('Add a photo', path.join(FILES, 'engine.png'), {
    within: await moduleRoot('Photos'),
    selector: 'button',
  });
  await imagesLoaded('a[href*="/photos/"] img');
  state.photo = (await page.$eval('a[href*="/photos/"]', (a) => a.getAttribute('href'))).split('/').pop();

  // A slice of life and a post in the journal.
  const journal = await ensureJournal();
  await goto(vaultPath(`/journals/${journal}/slices`));
  await click('Create a slice of life', { selector: 'button' });
  await fill('Name', 'Edinburgh years');
  await click('Save', { selector: 'button' });
  await see('Edinburgh years');
  await writePost('Lecture by Mary', mary);
}

async function everyPage() {
  await pageData();

  const routes = appRoutes();
  const visited = new Map(); // route name → paths
  const routeOf = (p) => routes.find((r) => r.re.test(p) && r.name !== 'not-found');
  const queue = [
    '/',
    '/vaults',
    '/settings',
    '/user/profile',
    '/user/api-tokens',
    vaultPath(),
    contactPath(state.contacts[1]),
  ];
  const seen = new Set();
  const failures = [];
  const shots = path.join(SHOTS, 'pages');
  fs.mkdirSync(shots, { recursive: true });
  let n = 0;
  const visit = async (p) => {
    const route = routeOf(p.split('?')[0]);
    if (!route || SKIPPED_ROUTES[route.name]) return;
    const done = visited.get(route.name) ?? [];
    if (done.length >= 2) return;
    done.push(p);
    visited.set(route.name, done);
    const before = problems.length;
    try {
      await goto(p);
      await assertPageRenders();
    } catch (e) {
      failures.push(`${p}: ${e.message}`);
    }
    n += 1;
    await fullScreenshot(path.join(shots, `${String(n).padStart(3, '0')}-${route.name}.png`));
    if (problems.length > before) failures.push(`${p}: ${problems.length - before} problem(s) below`);
    const links = await page.$$eval('a[href]', (all) => all.map((a) => a.getAttribute('href')));
    for (const href of links) {
      if (!href?.startsWith('/') || href.startsWith('//') || /^\/(api|dav)\//.test(href)) continue;
      if (!seen.has(href)) {
        seen.add(href);
        queue.push(href);
      }
    }
  };
  while (queue.length) await visit(queue.shift());

  // Routes no link led to: build them from the ids created above.
  const year = String(today.getFullYear());
  const params = {
    vault: state.vault,
    contact: state.contacts[1].id,
    slug: 'information',
    year,
    month: String(today.getMonth() + 1),
    journal: state.journal,
    group: state.group,
    goal: state.goal,
    photo: state.photo,
  };
  for (const r of routes) {
    if (SKIPPED_ROUTES[r.name] || visited.has(r.name)) continue;
    const p = r.path.replace(/:([a-zA-Z]+)/g, (_, k) => params[k] ?? `:${k}`);
    if (!p.includes(':')) await visit(p);
  }
  console.log(`     visited ${n} pages for ${visited.size} routes; skipped: ${Object.keys(SKIPPED_ROUTES).join(', ')}`);
  for (const r of routes) {
    if (!SKIPPED_ROUTES[r.name] && !visited.has(r.name)) failures.push(`route ${r.name} (${r.path}) not visited`);
  }
  if (failures.length) throw new Error(failures.join('\n     '));
}

const modules = [
  ['notes', moduleNotes],
  ['important-dates', moduleImportantDates],
  ['labels', moduleLabels],
  ['gender-pronoun', moduleGenderPronoun],
  ['job-information', moduleJobInformation],
  ['religion', moduleReligion],
  ['addresses', moduleAddresses],
  ['contact-information', moduleContactInformation],
  ['relationships', moduleRelationships],
  ['pets', modulePets],
  ['groups', moduleGroups],
  ['life-events', moduleLifeEvents],
  ['goals', moduleGoals],
  ['reminders', moduleReminders],
  ['loans', moduleLoans],
  ['tasks', moduleTasks],
  ['calls', moduleCalls],
  ['quick-facts', quickFacts],
  ['mood', mood],
  ['posts', modulePosts],
];

/**
 * 8. CardDAV/CalDAV with a real client: an API token made on the API
 * tokens page (with `write`), then vdirsyncer pulls the vault's address
 * book and calendars, and pushes a new card, an edit and a deletion that
 * the UI then shows.
 */
async function cardDav() {
  await goto('/user/api-tokens');
  await typeInto(await page.$('#name'), 'vdirsyncer');
  const write = await page.evaluateHandle(() =>
    [...document.querySelectorAll('label')].find((l) => l.textContent.trim() === 'write')?.querySelector('input'),
  );
  if (!write.asElement()) throw new Error('no write permission checkbox');
  await clickElement(write.asElement());
  await click('Create', { selector: 'button' });
  await see('Please copy your new API token');
  const token = await page.evaluate(() => /\d+\|[A-Za-z0-9]+/.exec(document.body.innerText)?.[0]);
  if (!token) throw new Error('the new token is not shown');

  const dir = path.join(OUT, 'vdirsyncer');
  fs.rmSync(dir, { recursive: true, force: true });
  fs.mkdirSync(path.join(dir, 'contacts'), { recursive: true });
  fs.mkdirSync(path.join(dir, 'calendars'), { recursive: true });
  const storage = (kind, type) => `[storage ${kind}_remote]
type = "${type}"
url = "${BASE}/dav/"
username = "${user.username}"
password = "${token}"
`;
  fs.writeFileSync(
    path.join(dir, 'config'),
    `[general]
status_path = "${dir}/status/"

[pair contacts]
a = "contacts_local"
b = "contacts_remote"
collections = ["from b"]
conflict_resolution = "b wins"

[storage contacts_local]
type = "filesystem"
path = "${dir}/contacts/"
fileext = ".vcf"

${storage('contacts', 'carddav')}
[pair calendars]
a = "calendars_local"
b = "calendars_remote"
collections = ["from b"]
conflict_resolution = "b wins"

[storage calendars_local]
type = "filesystem"
path = "${dir}/calendars/"
fileext = ".ics"

${storage('calendars', 'caldav')}`,
  );
  const vdirsyncer = (...args) =>
    execFileSync(process.env.VDIRSYNCER, args, {
      env: { ...process.env, VDIRSYNCER_CONFIG: path.join(dir, 'config') },
      input: 'y\n'.repeat(50),
      encoding: 'utf8',
      stdio: ['pipe', 'pipe', 'pipe'],
    });
  vdirsyncer('discover');
  vdirsyncer('sync');

  // Pull: the vault's address book has a card per contact (with the user's own).
  const books = fs.readdirSync(path.join(dir, 'contacts'));
  if (books.length !== 1) throw new Error(`expected one address book, got ${JSON.stringify(books)}`);
  const book = path.join(dir, 'contacts', books[0]);
  const cards = () => fs.readdirSync(book).filter((f) => f.endsWith('.vcf'));
  const pulled = cards().map((f) => fs.readFileSync(path.join(book, f), 'utf8'));
  for (const c of state.contacts.slice(0, 2)) {
    if (!pulled.some((card) => card.includes(c.name.split(' ')[0]))) throw new Error(`no card for ${c.name}`);
  }
  const calendars = fs.readdirSync(path.join(dir, 'calendars'));
  if (!calendars.some((c) => c.startsWith('dates-')) || !calendars.some((c) => c.startsWith('tasks-'))) {
    throw new Error(`expected the dates and tasks calendars, got ${JSON.stringify(calendars)}`);
  }

  // Push: a new card shows in the contact list.
  const uid = crypto.randomUUID();
  fs.writeFileSync(
    path.join(book, `${uid}.vcf`),
    `BEGIN:VCARD\r\nVERSION:3.0\r\nUID:${uid}\r\nFN:Grace Hopper\r\nN:Hopper;Grace;;;\r\nEMAIL;TYPE=WORK:grace@example.com\r\nEND:VCARD\r\n`,
  );
  vdirsyncer('sync', 'contacts');
  await goto(vaultPath(`/contacts/${uid}`));
  await see('Grace Hopper');
  await see('grace@example.com');

  // An edit made in the client shows in Monica.
  const cardPath = path.join(book, `${uid}.vcf`);
  fs.writeFileSync(
    cardPath,
    fs
      .readFileSync(cardPath, 'utf8')
      .replace(/^FN:.*$/m, 'FN:Grace Murray Hopper')
      .replace(/^N:.*$/m, 'N:Murray Hopper;Grace;;;'),
  );
  vdirsyncer('sync', 'contacts');
  await goto(vaultPath(`/contacts/${uid}`));
  await see('Grace Murray Hopper');
  // The server stores its own rendering of an imported card: the next sync
  // brings it back to the client, as a periodic sync does.
  vdirsyncer('sync', 'contacts');

  // A deletion in the client removes the contact.
  fs.rmSync(cardPath);
  vdirsyncer('sync', 'contacts');
  await goto(vaultPath('/contacts'));
  await see('Grace Murray Hopper', { absent: true });
}

// --- main ----------------------------------------------------------------------

page.on('dialog', (d) => d.accept());

const only = process.env.SMOKE_STEPS?.split(',');
const onlyModules = process.env.SMOKE_MODULES?.split(',');
const steps = [
  ['1-first-run-setup', setUp],
  ['2-two-factor', enableTwoFactor],
  ['3-vault-and-contacts', createVaultAndContacts],
  ...modules
    .filter(([name]) => !onlyModules || onlyModules.includes(name))
    .map(([name, fn]) => [`4-module-${name}`, fn]),
  ['5-photo-and-document', files],
  ['6-reminder-pushover', reminderPushover],
  ['7-every-page', everyPage],
  ['8-carddav-sync', cardDav],
];

let stopped = false;
for (const [name, fn] of steps) {
  const n = name.split('-')[0];
  // SMOKE_STEPS=4,5 runs those steps (after 1 and 3, which they need).
  if (only && !only.includes(n) && n !== '1' && n !== '3') continue;
  if (stopped) {
    results.push({ name, ok: false, issues: ['not run: an earlier step failed'] });
    console.log(`SKIP ${name}`);
    continue;
  }
  if (!(await step(name, fn)) && /^[123]-/.test(name)) stopped = true;
}

await browser.close();

const failed = results.filter((r) => !r.ok);
console.log(`\nSmoke test: ${results.length - failed.length}/${results.length} steps passed`);
for (const r of failed) console.log(`  FAIL ${r.name}: ${r.issues[0]}`);
fs.writeFileSync(path.join(OUT, 'results.json'), JSON.stringify(results, null, 2));
process.exit(failed.length ? 1 : 0);
