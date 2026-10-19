#!/usr/bin/env bash
# Red на финальной редакции тестов: временно «заглушить» реализацию, прогнать, восстановить.
set -eu
SRC=/home/sanya/orca/project/rust/crates/discord-core/src
B=/tmp/opencode/rw-06/impl-backup
mkdir -p "$B"
cp "$SRC/resume.rs" "$SRC/ws.rs" "$SRC/lib.rs" "$B/"
md5sum "$SRC/resume.rs" "$SRC/ws.rs" "$SRC/lib.rs" > "$B/before.md5"

python3 - "$SRC/resume.rs" "$SRC/ws.rs" <<'PY'
import sys
resume, ws = sys.argv[1], sys.argv[2]

r = open(resume, encoding='utf-8').read()
blocks = [
    ('pub fn build_resume_frame(params: &ResumeParams) -> Result<Vec<u8>, ResumeError> {',
     'let _ = params;'),
    ('pub fn plan_reconnect(\n    state: Option<&ResumeState>,\n    downtime_ms: u64,\n    invalid_session: Option<bool>,\n) -> ReconnectAction {',
     'let _ = (state, downtime_ms, invalid_session);'),
]
for start, prelude in blocks:
    i = r.index(start)
    j = r.index('\n}\n', i)
    r = r[:i] + start + '\n    ' + prelude + '\n    todo!("RW-06 (red, финальная редакция тестов)")' + r[j:]
open(resume, 'w', encoding='utf-8').write(r)

w = open(ws, encoding='utf-8').read()
rep = [
    ('    let frame = if let Some(params) = resume {\n        build_resume_frame(params).map_err(TransportError::Resume)?\n    } else if let Some(params) = opts.identify.as_ref() {',
     '    let frame = if let Some(params) = resume {\n        let _ = params;\n        todo!("RW-06 (red, финальная редакция тестов)")\n    } else if let Some(params) = opts.identify.as_ref() {'),
]
for old, new in rep:
    assert old in w, old[:60]
    w = w.replace(old, new)

for start in ('    pub fn resume_state(&self) -> Option<ResumeState> {',
              '    pub fn last_invalid_session_resumable(&self) -> Option<bool> {'):
    i = w.index(start)
    j = w.index('\n    }\n', i)
    w = w[:i] + start + '\n        todo!("RW-06 (red, финальная редакция тестов)")\n' + w[j:]
open(ws, 'w', encoding='utf-8').write(w)
print("заглушки применены")
PY

cd /home/sanya/orca/project/rust
{ cargo test --offline -p discord-core --test resume_reconnect 2>&1 | grep -E "^test |test result|^error" | head -20; echo "# EXIT=${PIPESTATUS[0]}"; md5sum crates/discord-core/tests/resume_reconnect.rs; } | tee /tmp/opencode/rw-06/red.txt

cp "$B/resume.rs" "$B/ws.rs" "$B/lib.rs" "$SRC/"
echo "--- md5 после восстановления ---"
md5sum "$SRC/resume.rs" "$SRC/ws.rs" "$SRC/lib.rs"
