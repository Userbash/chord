import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { execSync } from 'node:child_process';

const ROOT = path.resolve(import.meta.dirname, '..', '..');
const RS_PATH = path.join(ROOT, 'rust', 'crates', 'chord-core', 'src', 'sessions_manager.rs');

describe('TASK-1009B: Session Manager and Remote Revocation', () => {
  it('sessions_manager.rs exists and is exported in chord-core/src/lib.rs', () => {
    assert.ok(fs.existsSync(RS_PATH), 'sessions_manager.rs must exist');
    const libRs = fs.readFileSync(path.join(ROOT, 'rust', 'crates', 'chord-core', 'src', 'lib.rs'), 'utf8');
    assert.match(libRs, /pub mod sessions_manager;/);
  });

  it('defines SessionInfo fields (id_hash, os, browser, ip, location, approx_last_used_time, is_current)', () => {
    const src = fs.readFileSync(RS_PATH, 'utf8');
    for (const field of ['id_hash', 'os', 'browser', 'ip', 'location', 'approx_last_used_time', 'is_current']) {
      assert.ok(src.includes(field), `Missing field: ${field}`);
    }
  });

  it('highlights current session with green "Current Device" badge', () => {
    const src = fs.readFileSync(RS_PATH, 'utf8');
    assert.ok(src.includes('Current Device'), 'Must include "Current Device" badge label');
  });

  it('passes Rust unit tests in chord-core::sessions_manager', () => {
    const out = execSync(
      'cargo test --manifest-path rust/Cargo.toml -p chord-core --lib sessions_manager',
      { cwd: ROOT, encoding: 'utf8' }
    );
    assert.match(out, /test result: ok\./);
  });
});
