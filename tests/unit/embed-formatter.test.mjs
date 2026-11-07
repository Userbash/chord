// @ts-check
import test from 'node:test';
import assert from 'node:assert/strict';

import {
  EMBED_LIMITS,
  isSafeEmbedUrl,
  normalizeEmbedForRender,
} from '../../rust/gui-spike/ui/features/embed-formatter.js';

test('EMBED_LIMITS matches Discord rich embed limits', () => {
  assert.equal(EMBED_LIMITS.MAX_EMBEDS_PER_MESSAGE, 10);
  assert.equal(EMBED_LIMITS.MAX_TITLE_CHARS, 256);
  assert.equal(EMBED_LIMITS.MAX_DESCRIPTION_CHARS, 4096);
  assert.equal(EMBED_LIMITS.MAX_FIELDS, 25);
  assert.equal(EMBED_LIMITS.MAX_FIELD_NAME_CHARS, 256);
  assert.equal(EMBED_LIMITS.MAX_FIELD_VALUE_CHARS, 1024);
  assert.equal(EMBED_LIMITS.MAX_FOOTER_CHARS, 2048);
  assert.equal(Object.isFrozen(EMBED_LIMITS), true);
});

test('isSafeEmbedUrl accepts only absolute http and https urls', () => {
  assert.equal(isSafeEmbedUrl('https://discord.com/channels/@me'), true);
  assert.equal(isSafeEmbedUrl('http://example.com/a?b=c#d'), true);
  assert.equal(isSafeEmbedUrl('HTTPS://EXAMPLE.COM'), true);
});

test('isSafeEmbedUrl rejects dangerous, relative and empty values', () => {
  for (const value of [
    'javascript:alert(1)',
    'JavaScript:alert(1)',
    'data:text/html,<b>x</b>',
    'file:///etc/passwd',
    '//example.com/x',
    '/relative/path',
    'relative/path',
    'mailto:a@b.c',
    '',
    '   ',
    null,
    undefined,
  ]) {
    assert.equal(isSafeEmbedUrl(value), false, `expected unsafe: ${String(value)}`);
  }
});

test('isSafeEmbedUrl rejects non-string input', () => {
  assert.equal(isSafeEmbedUrl(123), false);
  assert.equal(isSafeEmbedUrl({ href: 'https://example.com' }), false);
});

test('normalizeEmbedForRender truncates long strings to embed limits', () => {
  const normalized = normalizeEmbedForRender({
    title: 't'.repeat(300),
    description: 'd'.repeat(5000),
    footer: { text: 'f'.repeat(3000) },
  });
  assert.equal(normalized.title.length, EMBED_LIMITS.MAX_TITLE_CHARS);
  assert.equal(normalized.description.length, EMBED_LIMITS.MAX_DESCRIPTION_CHARS);
  assert.equal(normalized.footer?.text?.length, EMBED_LIMITS.MAX_FOOTER_CHARS);
  assert.equal(normalized.title, 't'.repeat(256));
});

test('normalizeEmbedForRender keeps only safe urls', () => {
  assert.equal(
    normalizeEmbedForRender({ url: 'https://example.com/x' }).url,
    'https://example.com/x',
  );
  assert.equal(normalizeEmbedForRender({ url: 'javascript:alert(1)' }).url, null);
  assert.equal(normalizeEmbedForRender({ url: '/relative' }).url, null);
  assert.equal(normalizeEmbedForRender({}).url, null);
});

test('normalizeEmbedForRender maps integer colors to hex and falls back', () => {
  assert.equal(normalizeEmbedForRender({ color: 0x5865f2 }).color, '#5865f2');
  assert.equal(normalizeEmbedForRender({ color: 0 }).color, '#000000');
  assert.equal(normalizeEmbedForRender({ color: 0xffffff }).color, '#ffffff');
  assert.equal(normalizeEmbedForRender({ color: 0x1000000 }).color, '#2b2d31');
  assert.equal(normalizeEmbedForRender({ color: -1 }).color, '#2b2d31');
  assert.equal(normalizeEmbedForRender({ color: '5865f2' }).color, '#2b2d31');
  assert.equal(normalizeEmbedForRender({}).color, '#2b2d31');
});

test('normalizeEmbedForRender slices fields to 25 and truncates name/value', () => {
  const fields = Array.from({ length: 30 }, (_, i) => ({
    name: `name${i}-${'n'.repeat(300)}`,
    value: `value${i}-${'v'.repeat(1200)}`,
    inline: i % 2 === 0,
  }));
  const normalized = normalizeEmbedForRender({ fields });
  assert.equal(normalized.fields.length, EMBED_LIMITS.MAX_FIELDS);
  assert.equal(normalized.fields[0]?.name.length, EMBED_LIMITS.MAX_FIELD_NAME_CHARS);
  assert.equal(normalized.fields[0]?.value.length, EMBED_LIMITS.MAX_FIELD_VALUE_CHARS);
  assert.equal(normalized.fields[24]?.name.startsWith('name24-'), true);
  assert.equal(normalized.fields[0]?.inline, true);
  assert.equal(normalized.fields[1]?.inline, false);
});

test('normalizeEmbedForRender tolerates missing and malformed input', () => {
  const empty = normalizeEmbedForRender(null);
  assert.equal(empty.title, '');
  assert.equal(empty.description, '');
  assert.equal(empty.url, null);
  assert.equal(empty.color, '#2b2d31');
  assert.deepEqual(empty.fields, []);
  assert.equal(empty.footer, null);

  const malformed = normalizeEmbedForRender(
    /** @type {any} */ ({ title: 42, description: undefined, fields: 'nope', footer: 'nope' }),
  );
  // Non-string text values are dropped rather than coerced into the DOM.
  assert.equal(malformed.title, '');
  assert.equal(malformed.description, '');
  assert.deepEqual(malformed.fields, []);
  assert.equal(malformed.footer, null);
});

test('normalizeEmbedForRender does not mutate the source embed', () => {
  const raw = Object.freeze({
    title: 'x'.repeat(300),
    url: 'javascript:alert(1)',
    fields: Object.freeze([Object.freeze({ name: 'n', value: 'v' })]),
  });
  const normalized = normalizeEmbedForRender(raw);
  assert.equal(raw.title.length, 300);
  assert.equal(raw.url, 'javascript:alert(1)');
  assert.equal(normalized.title.length, 256);
  assert.equal(normalized.url, null);
});
