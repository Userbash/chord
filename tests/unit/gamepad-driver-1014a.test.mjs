import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { execFileSync } from 'node:child_process';

const ROOT = process.cwd();
const RS_PATH = path.join(ROOT, 'rust/crates/chord-platform/src/gamepad_driver.rs');
const LIB_PATH = path.join(ROOT, 'rust/crates/chord-platform/src/lib.rs');

test('TASK-1014A: gamepad_driver.rs exists in chord-platform, is exported, forbids unsafe, and has gilrs contract', () => {
  assert.ok(fs.existsSync(RS_PATH), 'rust/crates/chord-platform/src/gamepad_driver.rs must exist');
  const libSrc = fs.readFileSync(LIB_PATH, 'utf8');
  assert.match(libSrc, /\bpub\s+mod\s+gamepad_driver\s*;/, 'chord-platform/src/lib.rs must export pub mod gamepad_driver');

  const src = fs.readFileSync(RS_PATH, 'utf8');
  assert.match(src, /#!\[forbid\(unsafe_code\)\]/, 'must forbid unsafe_code');
  assert.ok(src.split('\n').length < 800, 'file must be under 800 lines');

  assert.match(src, /DEFAULT_DEADZONE\s*:\s*f32\s*=\s*0\.15/);
  assert.match(src, /NavUp/);
  assert.match(src, /NavDown/);
  assert.match(src, /NavLeft/);
  assert.match(src, /NavRight/);
  assert.match(src, /PrevServer/);
  assert.match(src, /NextServer/);
  assert.match(src, /PrevChannel/);
  assert.match(src, /NextChannel/);
});

test('TASK-1014A: cargo test -p chord-platform --lib gamepad_driver passes all unit tests', () => {
  const out = execFileSync(
    'cargo',
    ['test', '--manifest-path', 'rust/Cargo.toml', '-p', 'chord-platform', '--lib', 'gamepad_driver'],
    { cwd: ROOT, encoding: 'utf8' }
  );
  assert.match(out, /test result: ok\./);
  const match = out.match(/(\d+)\s+passed;\s+0\s+failed/);
  assert.ok(match && Number(match[1]) >= 6, `expected >= 6 passing Rust tests, got: ${out}`);
});

test('TASK-1014A: 5/5 mutation kill rate on gamepad_driver.rs', () => {
  const original = fs.readFileSync(RS_PATH, 'utf8');
  const mutants = [
    {
      name: 'analog_stick_drift_without_deadzone',
      find: 'if value.abs() < self.deadzone {',
      replace: 'if false && value.abs() < self.deadzone {',
    },
    {
      name: 'button_chatter_no_debounce',
      find: 'if now_ms.saturating_sub(last_ms) < self.debounce_ms {',
      replace: 'if false && now_ms.saturating_sub(last_ms) < self.debounce_ms {',
    },
    {
      name: 'disconnect_does_not_clear_active_gamepad',
      find: 'self.connected_ids.remove(&id);',
      replace: '// mutant: self.connected_ids.remove(&id);',
    },
    {
      name: 'wrong_l1_r1_server_mapping',
      find: 'GilrsButton::LeftTrigger => Some(NavAction::PrevServer),',
      replace: 'GilrsButton::LeftTrigger => Some(NavAction::PrevChannel),',
    },
    {
      name: 'wrong_l2_r2_channel_mapping',
      find: 'GilrsButton::LeftTrigger2 => Some(NavAction::PrevChannel),',
      replace: 'GilrsButton::LeftTrigger2 => Some(NavAction::PrevServer),',
    },
  ];

  let killed = 0;
  try {
    for (const mutant of mutants) {
      assert.ok(original.includes(mutant.find), `mutant target not found: ${mutant.name}`);
      const mutated = original.replace(mutant.find, mutant.replace);
      fs.writeFileSync(RS_PATH, mutated, 'utf8');
      try {
        execFileSync(
          'cargo',
          ['test', '--manifest-path', 'rust/Cargo.toml', '-p', 'chord-platform', '--lib', 'gamepad_driver'],
          { cwd: ROOT, stdio: 'pipe' }
        );
        assert.fail(`Mutant survived: ${mutant.name}`);
      } catch {
        killed += 1;
      }
    }
  } finally {
    fs.writeFileSync(RS_PATH, original, 'utf8');
  }
  assert.equal(killed, 5, 'all 5 mutants must be killed');
});
