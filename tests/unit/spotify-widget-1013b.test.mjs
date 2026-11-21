import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { execFileSync } from 'node:child_process';

const ROOT = process.cwd();
const RS_PATH = path.join(ROOT, 'rust/chord-gpui/src/profile/spotify_widget.rs');
const MOD_PATH = path.join(ROOT, 'rust/chord-gpui/src/profile/mod.rs');
const MAIN_PATH = path.join(ROOT, 'rust/chord-gpui/src/main.rs');

test('TASK-1013B: spotify_widget.rs exists in chord-gpui/src/profile, is exported, forbids unsafe, and is < 800 lines', () => {
  assert.ok(fs.existsSync(RS_PATH), 'rust/chord-gpui/src/profile/spotify_widget.rs must exist');
  assert.ok(fs.existsSync(MOD_PATH), 'rust/chord-gpui/src/profile/mod.rs must exist');

  const modSrc = fs.readFileSync(MOD_PATH, 'utf8');
  assert.match(modSrc, /\bpub\s+mod\s+spotify_widget\s*;/, 'profile/mod.rs must export pub mod spotify_widget');

  const mainSrc = fs.readFileSync(MAIN_PATH, 'utf8');
  assert.match(mainSrc, /\bpub\s+mod\s+profile\s*;/, 'main.rs must export pub mod profile');

  const src = fs.readFileSync(RS_PATH, 'utf8');
  assert.match(src, /#!\[forbid\(unsafe_code\)\]/, 'must forbid unsafe_code');
  assert.ok(src.split('\n').length < 800, 'file must be under 800 lines');

  assert.match(src, /#1db954/i, 'must define Spotify brand green #1db954');
  assert.match(src, /SPOTIFY_DESYNC_THRESHOLD_MS\s*:\s*u64\s*=\s*2_?000/);
  assert.match(src, /ListenAlongCommand/);
  assert.match(src, /SpotifyProfileWidget/);
});

test('TASK-1013B: cargo test -p chord-gpui --bin chord-gpui profile::spotify_widget passes all unit tests', () => {
  const out = execFileSync(
    'cargo',
    ['test', '--manifest-path', 'rust/chord-gpui/Cargo.toml', '--bin', 'chord-gpui', 'profile::spotify_widget'],
    { cwd: ROOT, encoding: 'utf8' }
  );
  assert.match(out, /test result: ok\./);
  const match = out.match(/(\d+)\s+passed;\s+0\s+failed/);
  assert.ok(match && Number(match[1]) >= 6, `expected >= 6 passing Rust tests, got: ${out}`);
});

test('TASK-1013B: 5/5 mutation kill rate on spotify_widget.rs', () => {
  const original = fs.readFileSync(RS_PATH, 'utf8');
  const mutants = [
    {
      name: 'wrong spotify green color',
      find: 'pub const SPOTIFY_GREEN_HEX: &str = "#1db954";',
      replace: 'pub const SPOTIFY_GREEN_HEX: &str = "#000000";',
    },
    {
      name: 'progress bar continues when paused',
      find: 'if !self.is_playing {',
      replace: 'if false {',
    },
    {
      name: 'desync threshold raised above 2s',
      find: 'pub const SPOTIFY_DESYNC_THRESHOLD_MS: u64 = 2_000;',
      replace: 'pub const SPOTIFY_DESYNC_THRESHOLD_MS: u64 = 10_000;',
    },
    {
      name: 'progress ratio not clamped to 1.0',
      find: '(self.elapsed_ms as f64 / self.duration_ms as f64).clamp(0.0, 1.0)',
      replace: '(self.elapsed_ms as f64 / self.duration_ms as f64) + 0.5',
    },
    {
      name: 'listen along returns None when active',
      find: 'Some(ListenAlongCommand {',
      replace: 'None; let _unused = Some(ListenAlongCommand {',
    },
  ];

  try {
    for (const m of mutants) {
      assert.ok(original.includes(m.find), `Mutant anchor not found: ${m.name}`);
      fs.writeFileSync(RS_PATH, original.replace(m.find, m.replace));
      let killed = false;
      try {
        execFileSync(
          'cargo',
          ['test', '--manifest-path', 'rust/chord-gpui/Cargo.toml', '--bin', 'chord-gpui', 'profile::spotify_widget'],
          { cwd: ROOT, stdio: 'pipe' }
        );
      } catch {
        killed = true;
      }
      assert.ok(killed, `Mutant survived: ${m.name}`);
    }
  } finally {
    fs.writeFileSync(RS_PATH, original);
  }
});
