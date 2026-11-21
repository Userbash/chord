import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';

const ROOT = process.cwd();
const VIEW_PATH = path.join(ROOT, 'rust/crates/chord-core/src/totp_view.rs');
const MOD_PATH = path.join(ROOT, 'rust/crates/chord-core/src/lib.rs');

test('TASK-1009A: totp_view.rs exists and is exported from chord-core/src/lib.rs', () => {
  assert.ok(fs.existsSync(VIEW_PATH), 'rust/crates/chord-core/src/totp_view.rs must exist');
  const modSrc = fs.readFileSync(MOD_PATH, 'utf8');
  assert.match(modSrc, /pub\s+mod\s+totp_view\s*;/, 'chord-core/src/lib.rs must export pub mod totp_view');
});

test('TASK-1009A: TotpDialog has 6 digit cells styled with #1e1f22', () => {
  const src = fs.readFileSync(VIEW_PATH, 'utf8');
  assert.match(src, /struct\s+TotpDialog\b/, 'TotpDialog struct must be defined');
  assert.match(src, /#1e1f22/, 'Digit input cells must be styled with #1e1f22');
  assert.match(src, /TOTP_DIGIT_COUNT|6/, 'Must define 6 digit input cells');
  assert.match(src, /#!\[forbid\(unsafe_code\)\]/, 'Must forbid unsafe code');
});

test('TASK-1009A: auto-advance focus, Backspace return, and Ctrl+V paste', () => {
  const src = fs.readFileSync(VIEW_PATH, 'utf8');
  assert.match(src, /fn\s+on_digit_input\b/, 'Must handle digit input and auto-advance focus');
  assert.match(src, /fn\s+on_backspace\b/, 'Must handle Backspace and return focus to previous cell');
  assert.match(src, /fn\s+on_paste\b/, 'Must handle Ctrl+V clipboard paste of 6-digit code');
  assert.match(src, /fn\s+submit\b/, 'Must handle submit');
});

test('TASK-1009A: rejects non-numeric characters and incomplete (<6 digits) submission', () => {
  const src = fs.readFileSync(VIEW_PATH, 'utf8');
  assert.match(src, /is_ascii_digit/, 'Must reject non-numeric characters');
  assert.match(src, /len\(\)\s*!=\s*6|TOTP_DIGIT_COUNT/, 'Must reject incomplete code < 6 digits');
});
