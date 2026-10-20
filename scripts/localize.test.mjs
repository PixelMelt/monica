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
});

test('finds the keys of the SPA and the server', () => {
  const en = JSON.parse(readFileSync(new URL('../lang/en.json', import.meta.url), 'utf8'));
  const keys = usedKeys(new Set(Object.keys(en)));
  assert.ok(keys.has('Sign in to your account'), 'a $t() key of a page');
  assert.ok(keys.has('Reminder for :name'), 'an i18n::t() key of the server');
  assert.ok(keys.has('Male'), 'a seeded name the server stores');
  assert.ok(![...keys].some((k) => k.startsWith('format.')), 'PHP group keys are left out');
});
