// node --test scripts/localize.test.mjs
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { natcasecmp, sortKeys, usedKeys } from './localize.mjs';

test('sorts like PHP ksort(SORT_NATURAL | SORT_FLAG_CASE)', () => {
  assert.deepEqual(Object.keys(sortKeys({ 'item 10': 1, 'Item 2': 1, '(Help)': 1, 'a b': 1, Ab: 1, ac: 1 })), [
    '(Help)',
    'a b',
    'Ab',
    'ac',
    'Item 2',
    'item 10',
  ]);
  assert.ok(natcasecmp('Account settings', 'A contact information') < 0);
  assert.ok(natcasecmp('A contact information', 'Action') < 0);
  // Case is folded with toupper: `A` (0x41) sorts before `_` (0x5f).
  assert.ok(natcasecmp('I agree to the :terms and :policy', 'I agree to the :terms_of_service') < 0);
  assert.ok(natcasecmp('a_b', 'aB') > 0);
  // Only ASCII letters are folded: `é` (0xc3 0xa9) stays above `Z`.
  assert.ok(natcasecmp('é', 'Z') > 0);
  // A run starting with 0 is compared left-aligned, digit by digit.
  assert.ok(natcasecmp('x 05', 'x 5') < 0);
  assert.ok(natcasecmp('x 010', 'x 09') < 0);
  // Leading zeros of the whole string are skipped.
  assert.equal(natcasecmp('007', '7'), 0);
  assert.ok(natcasecmp('x2', 'x10') < 0);
});

test('keeps the order of the lang files', () => {
  for (const locale of ['en', 'de', 'fr', 'ja']) {
    const keys = Object.keys(JSON.parse(readFileSync(new URL(`../lang/${locale}.json`, import.meta.url), 'utf8')));
    const inversions = keys
      .slice(1)
      .filter((k, i) => natcasecmp(keys[i], k) > 0 && !keys[i].startsWith('As we are about'));
    assert.deepEqual(inversions, [], locale);
  }
});

test('finds the keys of the SPA and the server', () => {
  const en = JSON.parse(readFileSync(new URL('../lang/en.json', import.meta.url), 'utf8'));
  const keys = usedKeys(new Set(Object.keys(en)));
  assert.ok(keys.has('Sign in to your account'), 'a $t() key of a page');
  assert.ok(keys.has('Reminder for :name'), 'an i18n::t() key of the server');
  assert.ok(keys.has('Male'), 'a seeded name the server stores');
  assert.ok(keys.has('added a contact information'), 'a label of a table the SPA translates later');
  assert.ok(
    keys.has('The provided password does not match your current password.'),
    'an i18n::t(&i18n::current(), …) key',
  );
  assert.ok(!keys.has('🔔 Reminder: :label for :contactName'), 'a key only the Telegram channel used');
  assert.ok(![...keys].some((k) => k.startsWith('format.')), 'PHP group keys are left out');
});
