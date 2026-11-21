import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { execFileSync } from 'node:child_process';

const ROOT = process.cwd();
const SRC_PATH = path.join(ROOT, 'rust/crates/chord-store/src/keyring_crypto.rs');

test('TASK-1021A: keyring_crypto.rs exists and implements KeyringBackend, Argon2id, and Zeroize', () => {
  assert.equal(fs.existsSync(SRC_PATH), true, 'keyring_crypto.rs must exist');
  const src = fs.readFileSync(SRC_PATH, 'utf8');

  assert.match(src, /pub trait SecretServiceProvider/, 'must define SecretServiceProvider trait');
  assert.match(src, /org\.freedesktop\.secrets/, 'must reference org.freedesktop.secrets oracle');
  assert.match(src, /argon2id/i, 'must implement Argon2id key derivation');
  assert.match(src, /impl Drop for/, 'must zeroize key material on Drop');
  assert.match(src, /\[REDACTED\]/, 'must redact key material in Debug/Display');
});

test('TASK-1021A: cargo test -p chord-store keyring_crypto passes', () => {
  const output = execFileSync(
    'cargo',
    ['test', '--manifest-path', 'rust/Cargo.toml', '-p', 'chord-store', 'keyring_crypto'],
    { cwd: ROOT, encoding: 'utf8' },
  );
  assert.match(output, /test result: ok\./);
});
