#!/usr/bin/env bash
set -u
SRC=/home/user/orca/project/rust/crates/discord-core/src
OUT=/tmp/opencode/review-rw-0506
BK=$OUT/red5-backup
mkdir -p "$BK"
cp "$SRC/identify.rs" "$SRC/ready.rs" "$SRC/ws.rs" "$BK/"
restore(){ cp "$BK/identify.rs" "$SRC/identify.rs"; cp "$BK/ready.rs" "$SRC/ready.rs"; cp "$BK/ws.rs" "$SRC/ws.rs"; }
trap restore EXIT
python3 - "$SRC/identify.rs" "$SRC/ready.rs" "$SRC/ws.rs" <<'PY'
import sys
identify, ready, ws = sys.argv[1], sys.argv[2], sys.argv[3]
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

w = open(ws, encoding='utf-8').read()
old = '''                    // READY (op0 `t: "READY"`) — сводка для вызывающего; не-READY → `None`.
                    if let Some(ready) = ReadySummary::from_dispatch(&value) {
                        self.last_ready = Some(ready);
                    }
'''
assert old in w
w = w.replace(old, '')
open(ws, 'w', encoding='utf-8').write(w)
PY
cd /home/user/orca/project/rust
{ cargo test --offline -p discord-core --test identify_ready 2>&1; echo "# EXIT=$?"; md5sum crates/discord-core/tests/identify_ready.rs; } > "$OUT/red-repro-rw05b.txt"
restore
