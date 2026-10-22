#!/usr/bin/env bash
set -u
cd /tmp/opencode/p2-10-round2/rust
OUT=/tmp/opencode/p2-10-round2
RED=crates/discord-core/src/state/voice/reducer.rs
cp "$RED" "$OUT/reducer.orig"
echo "control: full reducer suite"
flock /tmp/opencode/discord-cargo.lock cargo test --offline -p discord-core --test voice_state_reducer >"$OUT/control.log" 2>&1
echo "CONTROL_EXIT=$?"

python3 - "$RED" <<'PY'
import sys
p=sys.argv[1]
s=open(p).read()
old="""pub fn apply_snapshot(state: &mut State, voice_states: Vec<VoiceState>) {
    for voice in voice_states {"""
new="""pub fn apply_snapshot(state: &mut State, voice_states: Vec<VoiceState>) {
    state.voice_states.clear();
    for voice in voice_states {"""
assert s.count(old)==1
open(p,'w').write(s.replace(old,new))
PY
diff -u "$OUT/reducer.orig" "$RED" >"$OUT/clear-before-loop.diff"
flock /tmp/opencode/discord-cargo.lock cargo test --offline -p discord-core --test voice_state_reducer v09b_snapshot >"$OUT/clear-before-loop.log" 2>&1
echo "CLEAR_EXIT=$?"
cp "$OUT/reducer.orig" "$RED"

python3 - "$RED" <<'PY'
import sys
p=sys.argv[1]
s=open(p).read()
old="""    if voice.channel_id.is_some() {
        state.voice_states.insert(key, voice);
    } else {"""
new="""    if voice.channel_id.is_some() {
        if !state.voice_states.contains_key(&key) {
            state.voice_states.insert(key, voice);
        }
    } else {"""
assert s.count(old)==1
open(p,'w').write(s.replace(old,new))
PY
diff -u "$OUT/reducer.orig" "$RED" >"$OUT/first-wins.diff"
flock /tmp/opencode/discord-cargo.lock cargo test --offline -p discord-core --test voice_state_reducer v09c_snapshot >"$OUT/first-wins.log" 2>&1
echo "FIRST_EXIT=$?"
cp "$OUT/reducer.orig" "$RED"
md5sum "$RED" "$OUT/reducer.orig"
