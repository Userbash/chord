#!/usr/bin/env bash
# Independent red-first reproduction on the current tests, using temporary stubs.
set -u
SRC=/home/sanya/orca/project/rust/crates/discord-core/src
OUT=/tmp/opencode/review-rw-0506
BK=$OUT/red-backup
mkdir -p "$BK"
cp "$SRC/resume.rs" "$SRC/ws.rs" "$SRC/identify.rs" "$SRC/ready.rs" "$BK/"
md5sum "$SRC/resume.rs" "$SRC/ws.rs" "$SRC/identify.rs" "$SRC/ready.rs" > "$BK/before.md5"

restore() {
  cp "$BK/resume.rs" "$SRC/resume.rs"
  cp "$BK/ws.rs" "$SRC/ws.rs"
  cp "$BK/identify.rs" "$SRC/identify.rs"
  cp "$BK/ready.rs" "$SRC/ready.rs"
}
trap restore EXIT

# ---- RW-06 red: same stubs as the worker's red-final.sh ----
python3 - "$SRC/resume.rs" "$SRC/ws.rs" <<'PY'
import sys
resume, ws = sys.argv[1], sys.argv[2]
r = open(resume, encoding='utf-8').read()
blocks = [
    ('pub fn build_resume_frame(params: &ResumeParams) -> Result<Vec<u8>, ResumeError> {', 'let _ = params;'),
    ('pub fn plan_reconnect(\n    state: Option<&ResumeState>,\n    downtime_ms: u64,\n    invalid_session: Option<bool>,\n) -> ReconnectAction {', 'let _ = (state, downtime_ms, invalid_session);'),
]
for start, prelude in blocks:
    i = r.index(start); j = r.index('\n}\n', i)
    r = r[:i] + start + '\n    ' + prelude + '\n    todo!("review red reproduction")' + r[j:]
open(resume, 'w', encoding='utf-8').write(r)

w = open(ws, encoding='utf-8').read()
old = '''    let frame = if let Some(params) = resume {
        build_resume_frame(params).map_err(TransportError::Resume)?
    } else if let Some(params) = opts.identify.as_ref() {'''
new = '''    let frame = if let Some(params) = resume {
        let _ = params;
        todo!("review red reproduction")
    } else if let Some(params) = opts.identify.as_ref() {'''
assert old in w
w = w.replace(old, new)
for start in ('    pub fn resume_state(&self) -> Option<ResumeState> {',
              '    pub fn last_invalid_session_resumable(&self) -> Option<bool> {'):
    i = w.index(start); j = w.index('\n    }\n', i)
    w = w[:i] + start + '\n        todo!("review red reproduction")\n' + w[j:]
open(ws, 'w', encoding='utf-8').write(w)
PY
cd /home/sanya/orca/project/rust
{ cargo test --offline -p discord-core --test resume_reconnect 2>&1; echo "# EXIT=$?"; md5sum crates/discord-core/tests/resume_reconnect.rs; } > "$OUT/red-repro-rw06.txt"
restore

# ---- RW-05 red: stub the two data builders used by the tests ----
python3 - "$SRC/identify.rs" "$SRC/ready.rs" <<'PY'
import sys
identify, ready = sys.argv[1], sys.argv[2]
s = open(identify, encoding='utf-8').read()
start = '    pub fn for_rw_client() -> Self {'
i = s.index(start); j = s.index('\n    }\n', i)
s = s[:i] + start + '\n        todo!("review red reproduction")\n' + s[j:]
start2 = 'pub fn qos_token_client_provided(is_active: bool) -> Vec<u8> {'
i = s.index(start2); j = s.index('\n}\n', i)
s = s[:i] + start2 + '\n    let _ = is_active;\n    todo!("review red reproduction")\n' + s[j:]
open(identify, 'w', encoding='utf-8').write(s)

r = open(ready, encoding='utf-8').read()
start = '    pub fn from_dispatch(value: &EtfValue) -> Option<ReadySummary> {'
i = r.index(start); j = r.index('\n    }\n', i)
r = r[:i] + start + '\n        let _ = value;\n        todo!("review red reproduction")\n' + r[j:]
open(ready, 'w', encoding='utf-8').write(r)
PY
{ cargo test --offline -p discord-core --test identify_ready 2>&1; echo "# EXIT=$?"; md5sum crates/discord-core/tests/identify_ready.rs; } > "$OUT/red-repro-rw05.txt"
restore
echo "--- md5 after restore ---"
md5sum "$SRC/resume.rs" "$SRC/ws.rs" "$SRC/identify.rs" "$SRC/ready.rs"
