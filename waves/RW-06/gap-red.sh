#!/usr/bin/env bash
# Закрытие дыр R4/R10/R11 независимого ревью: мутации → red новых тестов → restore (+touch) → md5.
set -eu
SRC=/home/sanya/orca/project/rust/crates/discord-core/src
B=/tmp/opencode/rw-06/impl-backup2
mkdir -p "$B"
cp "$SRC/resume.rs" "$SRC/ws.rs" "$B/"
md5sum "$SRC/resume.rs" "$SRC/ws.rs" > "$B/before.md5"
cp /home/sanya/orca/project/rust/crates/discord-core/tests/resume_reconnect.rs /tmp/opencode/rw-06/resume_reconnect-red-gap.rs

python3 - "$SRC/resume.rs" "$SRC/ws.rs" <<'PY'
import sys
resume, ws = sys.argv[1], sys.argv[2]

r = open(resume, encoding='utf-8').read()
a = 'pub const RESUME_MAX_DOWNTIME_MS: u64 = 30_000;'
b = 'pub const RESUME_MAX_DOWNTIME_MS: u64 = 300_000;'
assert a in r
r = r.replace(a, b)
# R11: перепутать варианты ошибок
o1 = 'return Err(ResumeError::EmptyToken);'
o2 = 'return Err(ResumeError::EmptySessionId);'
assert o1 in r and o2 in r
r = r.replace(o1, 'return Err(ResumeError::SWAP);') \
     .replace(o2, o1) \
     .replace('return Err(ResumeError::SWAP);', o2)
open(resume, 'w', encoding='utf-8').write(r)

w = open(ws, encoding='utf-8').read()
g = 'if *sent || pipe.session().hello().is_none() {'
assert g in w
w = w.replace(g, 'if *sent {')
open(ws, 'w', encoding='utf-8').write(w)
print("мутации R4/R10/R11 применены")
PY

cd /home/sanya/orca/project/rust
{ cargo test --offline -p discord-core --test resume_reconnect 2>&1 | grep -E "^test .* (FAILED|ok)$|test result"; echo "# EXIT=${PIPESTATUS[0]}"; md5sum crates/discord-core/tests/resume_reconnect.rs; } | tee /tmp/opencode/rw-06/red-gap-tests.txt

cp "$B/resume.rs" "$B/ws.rs" "$SRC/"
touch "$SRC/resume.rs" "$SRC/ws.rs"
echo "--- md5 после восстановления (должны совпасть с before.md5) ---"
md5sum "$SRC/resume.rs" "$SRC/ws.rs"
cat "$B/before.md5"
