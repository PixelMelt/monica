// Port of `php artisan monica:localize` (K008): finds the translation keys
// the app uses, adds the missing ones to every lang/<locale>.json (and with
// --remove-missing drops the unused ones), machine-translates the empty
// values with Google Translate, and sorts the files.
//
//   node scripts/localize.mjs [--remove-missing] [--restart] [--no-translate]
//
// Keys are found as amirami/localizator found them (config/localizator.php)
// in the SPA (`$t`, `trans`, `wTrans`, `transChoice`, `$tChoice`), and in the
// Rust server: the strings of `i18n::t(…, "key", …)` calls, and any string
// literal that already is a key (seeded names such as `trans_key('Male')`).
// The framework files (lang/<locale>/*.php, the PHP `lang:add`/`lang:update`
// of laravel-lang) are left alone.

import { readFileSync, writeFileSync, readdirSync, statSync, existsSync } from 'node:fs';
import { join, extname } from 'node:path';
import { pathToFileURL } from 'node:url';
import { getPluralIndex } from 'laravel-vue-i18n/utils/get-plural-index';

const ROOT = new URL('..', import.meta.url).pathname;
const LOCALES = ['en', 'ar', 'bn', 'ca', 'da', 'de', 'es', 'el', 'fr', 'he', 'hi', 'it', 'ja', 'ml', 'nl', 'nn', 'pa', 'pl', 'pt', 'pt_BR', 'ro', 'ru', 'sv', 'te', 'tr', 'ur', 'vi', 'zh_CN', 'zh_TW'];
const args = new Set(process.argv.slice(2));

function* files(dir, exts) {
  for (const name of readdirSync(dir)) {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) {
      if (name !== 'node_modules' && name !== 'api') yield* files(path, exts);
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
  for (const path of files(join(ROOT, 'resources/js'), ['.vue', '.js'])) {
    for (const m of readFileSync(path, 'utf8').matchAll(js)) keys.add(unescape(m[2], m[1]));
  }
  const call = /i18n::t\(\s*&?[\w.()]+\s*,\s*"((?:\\.|[^"\\])*)"/g;
  const literal = /"((?:\\.|[^"\\])*)"/g;
  for (const path of files(join(ROOT, 'rust/src'), ['.rs'])) {
    const text = readFileSync(path, 'utf8');
    for (const m of text.matchAll(call)) keys.add(unescape(m[1], '"'));
    for (const m of text.matchAll(literal)) {
      const s = unescape(m[1], '"');
      if (known.has(s)) keys.add(s);
    }
  }
  // `group.key` of a lang/<locale>/<group>.php file is not a JSON key.
  for (const key of keys) {
    const group = key.match(/^([a-z_]+)\.[\w.]+$/)?.[1];
    if (group && existsSync(join(ROOT, `lang/en/${group}.php`))) keys.delete(key);
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
  return [...result.keys()].sort((a, b) => a - b).map((k) => result.get(k)).join('|');
}

/**
 * PHP's `strnatcasecmp`: bytes compared case-insensitively, whitespace
 * skipped, digit runs as numbers.
 */
function natcasecmp(a, b) {
  const x = Buffer.from(a.toLowerCase().replace(/\s+/g, ''));
  const y = Buffer.from(b.toLowerCase().replace(/\s+/g, ''));
  const digit = (c) => c >= 0x30 && c <= 0x39;
  let i = 0;
  let j = 0;
  while (i < x.length && j < y.length) {
    if (digit(x[i]) && digit(y[j])) {
      let ei = i;
      let ej = j;
      while (ei < x.length && digit(x[ei])) ei++;
      while (ej < y.length && digit(y[ej])) ej++;
      const n = BigInt(x.subarray(i, ei).toString()) - BigInt(y.subarray(j, ej).toString());
      if (n !== 0n) return n < 0n ? -1 : 1;
      i = ei;
      j = ej;
    } else {
      if (x[i] !== y[j]) return x[i] - y[j];
      i++;
      j++;
    }
  }
  return x.length - i - (y.length - j);
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
            strings[key] = key.includes('|') ? await translateCount(locale, key, target) : await translateText(key, target);
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
    const path = join(ROOT, `lang/${locale}/pagination.php`);
    if (!existsSync(path)) continue;
    const text = readFileSync(path, 'utf8').replaceAll('&laquo; ', '❮ ').replaceAll(' &raquo;', ' ❯');
    writeFileSync(path, text);
  }
}

if (import.meta.url === pathToFileURL(process.argv[1]).href) await main();
