// Port of `php artisan monica:localize` (K008): finds the translation keys
// the app uses, adds the missing ones to every lang/<locale>.json (and with
// --remove-missing drops the unused ones), machine-translates the empty
// values with Google Translate, and sorts the files.
//
//   node scripts/localize.mjs [--remove-missing] [--restart] [--no-translate]
//
// Keys are found as amirami/localizator found them (config/localizator.php)
// in the SPA (`$t`, `trans`, `wTrans`, `transChoice`, `$tChoice`), and in the
// Rust server: the strings of `i18n::t(…, "key", …)` calls. In both, any string
// literal that already is a key counts too (the server's seeded names such as
// `trans_key('Male')`, the SPA's tables of feed action labels).
// The arrays of lang/<locale>/*.json (Laravel's `auth`, `validation`,
// `format` and `pagination` groups) are left alone.

import { readFileSync, writeFileSync, readdirSync, statSync, existsSync } from 'node:fs';
import { join, extname } from 'node:path';
import { pathToFileURL } from 'node:url';
import { getPluralIndex } from 'laravel-vue-i18n/utils/get-plural-index';

const ROOT = new URL('..', import.meta.url).pathname;
const LOCALES = [
  'en',
  'ar',
  'bn',
  'ca',
  'da',
  'de',
  'es',
  'el',
  'fr',
  'he',
  'hi',
  'it',
  'ja',
  'ml',
  'nl',
  'nn',
  'pa',
  'pl',
  'pt',
  'pt_BR',
  'ro',
  'ru',
  'sv',
  'te',
  'tr',
  'ur',
  'vi',
  'zh_CN',
  'zh_TW',
];
const args = new Set(process.argv.slice(2));

function* files(dir, exts) {
  for (const name of readdirSync(dir)) {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) {
      // The generated API client holds no translation keys.
      if (name !== 'node_modules' && path !== join(ROOT, 'resources/js/api')) yield* files(path, exts);
    } else if (exts.includes(extname(name))) {
      yield path;
    }
  }
}

const unescape = (s, quote) => s.replaceAll(`\\${quote}`, quote).replaceAll('\\\\', '\\');

/** The keys the SPA and the server use. */
function usedKeys(known) {
  const keys = new Set();
  const js = /(?:\$t|\btrans|\bwTrans|\btransChoice|\$tChoice)\(\s*(['"`])((?:\\.|(?!\1)[^\\])*)\1/gs;
  let spa = '';
  for (const path of files(join(ROOT, 'resources/js'), ['.vue', '.js'])) {
    const text = readFileSync(path, 'utf8');
    for (const m of text.matchAll(js)) keys.add(unescape(m[2], m[1]));
    spa += text;
  }
  const call = /i18n::t\(\s*&?[\w.():]+\s*,\s*"((?:\\.|[^"\\])*)"/g;
  let rust = '';
  for (const path of files(join(ROOT, 'rust/src'), ['.rs'])) {
    const text = readFileSync(path, 'utf8');
    for (const m of text.matchAll(call)) keys.add(unescape(m[1], '"'));
    rust += text;
  }
  // Any string literal that is a key: the SPA's tables of labels it
  // translates later (`trans(labels[action])`), the server's seeded names.
  const quoted = (key, quote) => quote + key.replaceAll('\\', '\\\\').replaceAll(quote, `\\${quote}`) + quote;
  for (const key of known) {
    if (["'", '"', '`'].some((q) => spa.includes(quoted(key, q))) || rust.includes(quoted(key, '"'))) keys.add(key);
  }
  // `group.key` of a lang/<locale>/<group>.json array is not an app string.
  for (const key of keys) {
    const group = key.match(/^([a-z_]+)\.[\w.]+$/)?.[1];
    if (group && existsSync(join(ROOT, `lang/en/${group}.json`))) keys.delete(key);
  }
  return keys;
}

/** Google Translate's free endpoint, as stichoza/google-translate-php uses it. */
async function google(text, target) {
  const url = new URL('https://translate.googleapis.com/translate_a/single');
  url.search = new URLSearchParams({ client: 'gtx', sl: 'en', tl: target, dt: 't', q: text });
  const res = await fetch(url);
  if (!res.ok) throw new Error(`Google Translate: HTTP ${res.status}`);
  const json = await res.json();
  return json[0].map((part) => part[0]).join('');
}

/** `translateText`: placeholders (`:name`) survive the translation. */
async function translateText(text, target) {
  const placeholders = text.match(/:\w+/g) ?? [];
  let str = text;
  placeholders.forEach((p, i) => (str = str.replace(p, `{{#${i}}}`)));
  let translated = await google(str, target);
  placeholders.forEach((p, i) => (translated = translated.replace(`{{#${i}}}`, p)));
  return translated.replaceAll("'", '’');
}

/** `translateCount`: one translation per plural form of the locale. */
async function translateCount(locale, text, target) {
  const strings = text.split('|');
  const result = new Map();
  for (let i = 0; i < 100; i++) {
    const j = getPluralIndex(locale, i);
    if (result.has(j)) continue;
    const message = j >= strings.length ? strings[strings.length - 1] : strings[j];
    let translated = await translateText(message.replaceAll(':count', String(i)), target);
    translated = translated.includes(String(i))
      ? translated.replaceAll(String(i), ':count')
      : await translateText(message, target);
    result.set(j, translated);
  }
  return [...result.keys()]
    .sort((a, b) => a - b)
    .map((k) => result.get(k))
    .join('|');
}

const isdigit = (c) => c >= 0x30 && c <= 0x39;
const isspace = (c) => c === 0x20 || (c >= 0x09 && c <= 0x0d);
const toupper = (c) => (c >= 0x61 && c <= 0x7a ? c - 0x20 : c);

/** `compare_right` of PHP's strnatcmp.c: the longer run of digits wins, then the greater value. */
function compareRight(a, p, b, q) {
  let bias = 0;
  for (; ; p.i++, q.i++) {
    const da = p.i < a.length && isdigit(a[p.i]);
    const db = q.i < b.length && isdigit(b[q.i]);
    if (!da && !db) return bias;
    if (!da) return -1;
    if (!db) return +1;
    if (a[p.i] < b[q.i]) {
      if (!bias) bias = -1;
    } else if (a[p.i] > b[q.i]) {
      if (!bias) bias = +1;
    }
  }
}

/** `compare_left` of PHP's strnatcmp.c: left-aligned digits, the first different one wins. */
function compareLeft(a, p, b, q) {
  for (; ; p.i++, q.i++) {
    const da = p.i < a.length && isdigit(a[p.i]);
    const db = q.i < b.length && isdigit(b[q.i]);
    if (!da && !db) return 0;
    if (!da) return -1;
    if (!db) return +1;
    if (a[p.i] < b[q.i]) return -1;
    if (a[p.i] > b[q.i]) return +1;
  }
}

/**
 * PHP's `strnatcasecmp` (`strnatcmp_ex` with `is_case_insensitive`), on the
 * UTF-8 bytes: leading zeros skipped, whitespace skipped, digit runs
 * compared as numbers, ASCII letters folded to upper case.
 */
function natcasecmp(sa, sb) {
  const a = Buffer.from(sa);
  const b = Buffer.from(sb);
  // Past the end reads the terminating NUL, as in C.
  const at = (s, i) => (i < s.length ? s[i] : 0);
  if (a.length === 0 || b.length === 0) return a.length === b.length ? 0 : a.length > b.length ? 1 : -1;
  const p = { i: 0 };
  const q = { i: 0 };
  let leading = true;
  for (;;) {
    let ca = at(a, p.i);
    let cb = at(b, q.i);
    while (leading && ca === 0x30 && p.i + 1 < a.length && isdigit(a[p.i + 1])) ca = at(a, ++p.i);
    while (leading && cb === 0x30 && q.i + 1 < b.length && isdigit(b[q.i + 1])) cb = at(b, ++q.i);
    leading = false;
    while (isspace(ca)) ca = at(a, ++p.i);
    while (isspace(cb)) cb = at(b, ++q.i);
    if (isdigit(ca) && isdigit(cb)) {
      const fractional = ca === 0x30 || cb === 0x30;
      const result = fractional ? compareLeft(a, p, b, q) : compareRight(a, p, b, q);
      if (result !== 0) return result;
      if (p.i === a.length && q.i === b.length) return 0;
      if (p.i === a.length) return -1;
      if (q.i === b.length) return 1;
      ca = a[p.i];
      cb = b[q.i];
    }
    ca = toupper(ca);
    cb = toupper(cb);
    if (ca < cb) return -1;
    if (ca > cb) return +1;
    p.i++;
    q.i++;
    if (p.i >= a.length && q.i >= b.length) return 0;
    if (p.i >= a.length) return -1;
    if (q.i >= b.length) return 1;
  }
}

/** `ksort($strings, SORT_NATURAL | SORT_FLAG_CASE)`. */
const sortKeys = (strings) => Object.fromEntries(Object.entries(strings).sort(([a], [b]) => natcasecmp(a, b)));

export { natcasecmp, sortKeys, usedKeys, translateText };

async function main() {
  const en = JSON.parse(readFileSync(join(ROOT, 'lang/en.json'), 'utf8'));
  const used = usedKeys(new Set(Object.keys(en)));
  for (const locale of LOCALES) {
    const path = join(ROOT, `lang/${locale}.json`);
    const strings = existsSync(path) ? JSON.parse(readFileSync(path, 'utf8')) : {};
    for (const key of used) if (!(key in strings)) strings[key] = locale === 'en' ? key : '';
    if (args.has('--remove-missing')) for (const key of Object.keys(strings)) if (!used.has(key)) delete strings[key];
    try {
      if (locale !== 'en' && !args.has('--no-translate')) {
        const target = locale === 'nn' ? 'no' : locale.replace('_', '-');
        for (const [key, value] of Object.entries(strings)) {
          if (value === '' || args.has('--restart')) {
            strings[key] = key.includes('|')
              ? await translateCount(locale, key, target)
              : await translateText(key, target);
            console.log(`translating: \`${key}\` to \`${strings[key]}\``);
          }
        }
      }
    } finally {
      for (const key of Object.keys(strings)) strings[key] = strings[key].replaceAll("'", '’');
      writeFileSync(path, `${JSON.stringify(sortKeys(strings), null, 2)}\n`);
    }
  }
  // `fixPagination`
  for (const locale of LOCALES) {
    const path = join(ROOT, `lang/${locale}/pagination.json`);
    if (!existsSync(path)) continue;
    const text = readFileSync(path, 'utf8').replaceAll('&laquo; ', '❮ ').replaceAll(' &raquo;', ' ❯');
    writeFileSync(path, text);
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) await main();
