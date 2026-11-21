import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { execFileSync } from 'node:child_process';

const ROOT = process.cwd();
const RS_PATH = path.join(ROOT, 'rust/crates/chord-platform/src/spotify_mpris.rs');
const LIB_PATH = path.join(ROOT, 'rust/crates/chord-platform/src/lib.rs');

test('TASK-1013A: spotify_mpris.rs exists in chord-platform, is exported, forbids unsafe, and compiles', () => {
  assert.ok(fs.existsSync(RS_PATH), 'rust/crates/chord-platform/src/spotify_mpris.rs must exist');
  const libSrc = fs.readFileSync(LIB_PATH, 'utf8');
  assert.match(libSrc, /\bpub\s+mod\s+spotify_mpris\s*;/, 'chord-platform/src/lib.rs must export pub mod spotify_mpris');

  const src = fs.readFileSync(RS_PATH, 'utf8');
  assert.match(src, /#!\[forbid\(unsafe_code\)\]/, 'must forbid unsafe_code');
  assert.ok(src.split('\n').length < 800, 'file must be under 800 lines');

  assert.match(src, /org\.mpris\.MediaPlayer2\.spotify/);
  assert.match(src, /PropertiesChanged/);
  assert.match(src, /NameOwnerChanged/);
  assert.match(src, /mpris:length/);
  assert.match(src, /ACTIVITY_TYPE_LISTENING\s*:\s*u8\s*=\s*2/);
  assert.match(src, /GATEWAY_OPCODE_STATUS_UPDATE\s*:\s*u8\s*=\s*3/);
});

test('TASK-1013A: cargo test -p chord-platform --lib spotify_mpris passes all unit tests', () => {
  const out = execFileSync(
    'cargo',
    ['test', '--manifest-path', 'rust/Cargo.toml', '-p', 'chord-platform', '--lib', 'spotify_mpris'],
    { cwd: ROOT, encoding: 'utf8' }
  );
  assert.match(out, /test result: ok\./);
  const match = out.match(/(\d+)\s+passed;\s+0\s+failed/);
  assert.ok(match && Number(match[1]) >= 6, `expected >= 6 passing Rust tests, got: ${out}`);
});

test('TASK-1013A: 5/5 mutation kill rate on spotify_mpris.rs', () => {
  const original = fs.readFileSync(RS_PATH, 'utf8');
  const mutants = [
    {
      name: 'wrong_activity_type',
      find: 'pub const ACTIVITY_TYPE_LISTENING: u8 = 2;',
      replace: 'pub const ACTIVITY_TYPE_LISTENING: u8 = 0;',
    },
    {
      name: 'wrong_gateway_opcode',
      find: 'pub const GATEWAY_OPCODE_STATUS_UPDATE: u8 = 3;',
      replace: 'pub const GATEWAY_OPCODE_STATUS_UPDATE: u8 = 4;',
    },
    {
      name: 'frozen_presence_on_spotify_close',
      find: 'if new_owner.is_empty() {',
      replace: 'if false && new_owner.is_empty() {',
    },
    {
      name: 'timestamp_desync_end_before_start',
      find: 'let end_ms = start_ms.saturating_add(length_ms);',
      replace: 'let end_ms = start_ms.saturating_sub(length_ms);',
    },
    {
      name: 'ignore_paused_playback_status',
      find: 'PlaybackStatus::Playing => {',
      replace: 'PlaybackStatus::Playing | PlaybackStatus::Paused => {',
    },
  ];

  let killed = 0;
  try {
    for (const m of mutants) {
      assert.ok(original.includes(m.find), `mutant target not found: ${m.name}`);
      fs.writeFileSync(RS_PATH, original.replace(m.find, m.replace));
      let failed = false;
      try {
        execFileSync(
          'cargo',
          ['test', '--manifest-path', 'rust/Cargo.toml', '-p', 'chord-platform', '--lib', 'spotify_mpris'],
          { cwd: ROOT, stdio: 'pipe' }
        );
      } catch {
        failed = true;
      }
      assert.ok(failed, `mutant survived: ${m.name}`);
      killed += 1;
    }
  } finally {
    fs.writeFileSync(RS_PATH, original);
  }
  assert.equal(killed, 5, 'must kill 5/5 mutants');
});
