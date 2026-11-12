#!/usr/bin/env bash
# Мутационное само-ревью RW-05: ломаем реализацию — смотрим, ловят ли тесты.
# Каждая мутация: применить → cargo test -p discord-core → восстановить из бэкапа.
set -u
SRC=/home/user/orca/project/rust/crates/discord-core/src
B=/tmp/opencode/rw-05/mut
mkdir -p "$B"
cp "$SRC/identify.rs" "$SRC/ready.rs" "$SRC/ws.rs" "$B/"
md5sum "$SRC/identify.rs" "$SRC/ready.rs" "$SRC/ws.rs" > "$B/before.md5"

restore() {
    cp "$B/identify.rs" "$SRC/identify.rs"
    cp "$B/ready.rs" "$SRC/ready.rs"
    cp "$B/ws.rs" "$SRC/ws.rs"
}

apply() { # $1=file $2=old $3=new ; replace ALL occurrences
    python3 - "$1" "$2" "$3" <<'PY'
import sys
path, old, new = sys.argv[1], sys.argv[2], sys.argv[3]
text = open(path, encoding='utf-8').read()
if old not in text:
    print(f"NO-MATCH: {old[:60]!r} in {path}")
    sys.exit(3)
open(path, 'w', encoding='utf-8').write(text.replace(old, new))
PY
}

run() { # $1=id $2=desc
    local log="$B/$1.txt"
    ( cd /home/user/orca/project/rust && cargo test --offline -p discord-core ) > "$log" 2>&1
    local status
    if grep -qE "^(error(\[|:)|error\[)" "$log"; then status="COMPILE-FAIL"
    elif grep -q "test result: FAILED" "$log"; then status="CAUGHT"
    else status="SURVIVED"; fi
    local names
    names=$(grep -E "^test .* FAILED" "$log" | sed 's/^test //; s/ \.\.\. FAILED//' | paste -sd, -)
    printf '%s | %s | %s | %s\n' "$1" "$status" "$names" "$2" >> "$B/results.txt"
    echo "=== $1: $status [$2] failing: ${names:-—}"
    restore
}

printf '' > "$B/results.txt"

apply "$SRC/ws.rs" 'build_identify_frame' 'build_identify'
run M1 "ws: слать payload без конверта {op,d}"
apply "$SRC/identify.rs" 'pub const CAPABILITIES_ACTIVE: u64 = 1_734_653;' 'pub const CAPABILITIES_ACTIVE: u64 = 1_734_652;'
run M2 "identify: capabilities-1"
apply "$SRC/identify.rs" '(key("compress"), bool_atom(p.compress)),' '(key("compress"), bool_atom(true)),'
run M3 "identify: compress всегда true"
apply "$SRC/identify.rs" '(key("gateway_connect_reasons"), EtfValue::Str(p.gateway_connect_reasons.clone())),' '(key("gateway_connect_reasons"), EtfValue::List(vec![EtfValue::Str(p.gateway_connect_reasons.clone())])),'
run M4 "identify: connect_reasons списком"
apply "$SRC/identify.rs" '    if let Some(installation_id) = &p.installation_id {
        pairs.push((
            key("installation_id"),
            EtfValue::Str(installation_id.clone()),
        ));
    }
' ''
run M5 "identify: installation_id не попадает"
apply "$SRC/ready.rs" '                    EtfValue::Int(number) => Some(number.to_string()),
                    EtfValue::Str(text) => Some(text.clone()),
' '                    EtfValue::Int(number) => Some(number.to_string()),
'
run M6 "ready: user_id только Int"
apply "$SRC/ready.rs" '        EtfValue::Int(1) => Some(true),
' ''
run M7 "ready: partial int-1 не читается"
apply "$SRC/ready.rs" 'top_keys: d.as_map().map_or(0, <[_]>::len),' 'top_keys: d.as_map().map_or(0, <[_]>::len) + 1,'
run M8 "ready: top_keys+1"
apply "$SRC/ws.rs" 'if *sent || pipe.session().hello().is_none() {' 'if pipe.session().hello().is_none() {'
run M9 "ws: IDENTIFY на каждый HELLO (без флага)"
apply "$SRC/ws.rs" 'if *sent || pipe.session().hello().is_none() {' 'if *sent {'
run M10 "ws: слать IDENTIFY до HELLO"
apply "$SRC/ws.rs" 'self.last_ready = Some(ready);' 'let _ = ready;'
run M11 "ws: last_ready не запоминается"

echo "--- md5 после восстановления ---"
md5sum "$SRC/identify.rs" "$SRC/ready.rs" "$SRC/ws.rs"
