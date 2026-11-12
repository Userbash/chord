#!/usr/bin/env python3
"""Независимая перепроверка оракулов RW-07 (ревьюер).

- union 7 файлов msgs-*.jsonl: op0 -> потиповые числа, типы, суммы (json-парсер, не мини-парсер теста);
- префикс .1389 (s <= 36): потиповые числа;
- .hexl .1389: кадры/сжатые байты/распакованные байты (zstd CLI)/термы собственным ETF-декодером;
- таймлайн .1389: число диспатчей op0;
- сверка с константами тест-файла и lib.rs (regex).
"""
import json
import re
import struct
import subprocess
import sys
import tempfile
from collections import Counter
from pathlib import Path

LAB = Path("/home/user/clean-room-oracle/gw-proto")
TMP = Path("/tmp/opencode/review-rw-07")
TMP.mkdir(parents=True, exist_ok=True)

UNION_FILES = [
    "msgs-run1-57992_5112.jsonl",
    "msgs-run2-95974_1389.jsonl",
    "msgs-run2-95974_5532.jsonl",
    "msgs-run2-95974_8269.jsonl",
    "msgs-run2-95974_8608.jsonl",
    "msgs-run2-95974_10029.jsonl",
    "msgs-run2-95974_13072.jsonl",
]

fail = []


def check(cond, msg):
    print(("OK  " if cond else "FAIL") + " " + msg)
    if not cond:
        fail.append(msg)


# ---------------- 1. Union op0 ----------------
union_counts = Counter()
union_total = 0
union_s = []
no_t = 0
no_s = 0
per_file = {}
for name in UNION_FILES:
    path = LAB / name
    c = Counter()
    with path.open(encoding="utf-8") as fh:
        for line in fh:
            line = line.strip()
            if not line:
                continue
            obj = json.loads(line)
            if obj.get("op") != 0:
                continue
            t = obj.get("t")
            s = obj.get("s")
            if not isinstance(t, str) or t in ("nil", ""):
                no_t += 1
                continue
            if not isinstance(s, int):
                no_s += 1
                continue
            c[t] += 1
            union_counts[t] += 1
            union_total += 1
            union_s.append(s)
    per_file[name] = sum(c.values())

print("== UNION ==")
print("per-file op0:", per_file)
check(union_total == 1340, f"union op0 total = {union_total} (ожидается 1340)")
check(len(union_counts) == 21, f"union types = {len(union_counts)} (ожидается 21)")
check(no_t == 0, f"op0 без валидного t: {no_t}")
check(no_s == 0, f"op0 без int s: {no_s}")
print("union by type:", dict(sorted(union_counts.items(), key=lambda kv: -kv[1])))
print("union s range:", min(union_s), "..", max(union_s))

# ---------------- 2. Префикс .1389 s <= 36 ----------------
pre = Counter()
pre_total = 0
non_disp = 0
with (LAB / "msgs-run2-95974_1389.jsonl").open(encoding="utf-8") as fh:
    for line in fh:
        line = line.strip()
        if not line:
            continue
        obj = json.loads(line)
        if obj.get("op") != 0:
            continue
        s = obj.get("s")
        if isinstance(s, int) and s <= 36:
            pre[obj["t"]] += 1
            pre_total += 1
        if obj.get("op") in (10, 11):
            non_disp += 1
print("== PREFIX .1389 s<=36 ==")
check(pre_total == 36, f"префикс op0 s<=36 = {pre_total} (ожидается 36)")
print("prefix by type:", dict(sorted(pre.items(), key=lambda kv: -kv[1])))

# ---------------- 3. .hexl: кадры, байты, распаковка ----------------
hexl = LAB / "zstd-frames-95974_1389.hexl"
frames = []
for line in hexl.read_text().splitlines():
    line = line.strip()
    if line:
        frames.append(bytes.fromhex(line))
check(len(frames) == 137, f"frames = {len(frames)} (ожидается 137)")
compressed = sum(len(f) for f in frames)
check(compressed == 104066, f"compressed bytes = {compressed} (ожидается 104066)")
blob = b"".join(frames)
(TMP / "review-1389.zst").write_bytes(blob)
# Кадр захвата не завершён (end() не вызывался) — zstd CLI вернёт rc=1
# "premature end", но отдаст весь распакованный префикс; это и есть факт RW-02.
res = subprocess.run(["zstd", "-d", "-c", str(TMP / "review-1389.zst")],
                     capture_output=True)
if res.returncode != 0 and b"premature end" not in res.stderr:
    check(False, f"zstd -d: {res.stderr.decode()[:200]}")
    sys.exit(1)
raw = res.stdout
total = len(raw)
check(total == 824816, f"распаковано всего = {total} (ожидается 824816)")
available = total - 65536
check(available == 759280, f"доступно (минус окно 65536) = {available} (ожидается 759280)")
prefix = raw[:available]
(TMP / "review-1389-prefix.bin").write_bytes(prefix)

# ---------------- 4. Собственный ETF-декодер ----------------
class Trunc(Exception):
    pass


def decode(buf, pos, depth=0):
    if depth > 64:
        raise ValueError("depth")
    if pos >= len(buf):
        raise Trunc()
    tag = buf[pos]
    pos += 1
    if tag == 0x61:
        if pos + 1 > len(buf):
            raise Trunc()
        return buf[pos], pos + 1
    if tag == 0x62:
        if pos + 4 > len(buf):
            raise Trunc()
        return struct.unpack_from(">i", buf, pos)[0], pos + 4
    if tag == 0x63:
        if pos + 31 > len(buf):
            raise Trunc()
        return ("float", buf[pos:pos + 31]), pos + 31
    if tag == 0x46:
        if pos + 8 > len(buf):
            raise Trunc()
        return struct.unpack_from(">d", buf, pos)[0], pos + 8
    if tag == 0x64:
        if pos + 2 > len(buf):
            raise Trunc()
        n = struct.unpack_from(">H", buf, pos)[0]
        pos += 2
        if pos + n > len(buf):
            raise Trunc()
        return ("atom", buf[pos:pos + n].decode("latin1")), pos + n
    if tag == 0x73:
        if pos + 1 > len(buf):
            raise Trunc()
        n = buf[pos]
        pos += 1
        if pos + n > len(buf):
            raise Trunc()
        return ("atom", buf[pos:pos + n].decode("latin1")), pos + n
    if tag == 0x76 or tag == 0x77:
        if tag == 0x76:
            if pos + 2 > len(buf):
                raise Trunc()
            n = struct.unpack_from(">H", buf, pos)[0]
            pos += 2
        else:
            if pos + 1 > len(buf):
                raise Trunc()
            n = buf[pos]
            pos += 1
        if pos + n > len(buf):
            raise Trunc()
        return ("atom", buf[pos:pos + n].decode("utf-8")), pos + n
    if tag == 0x6b:
        if pos + 2 > len(buf):
            raise Trunc()
        n = struct.unpack_from(">H", buf, pos)[0]
        pos += 2
        if pos + n > len(buf):
            raise Trunc()
        return ("str", buf[pos:pos + n].decode("latin1")), pos + n
    if tag == 0x6d:
        if pos + 4 > len(buf):
            raise Trunc()
        n = struct.unpack_from(">I", buf, pos)[0]
        pos += 4
        if pos + n > len(buf):
            raise Trunc()
        return ("bin", bytes(buf[pos:pos + n])), pos + n
    if tag == 0x6a:
        return None, pos
    if tag == 0x6e:
        if pos + 2 > len(buf):
            raise Trunc()
        n = buf[pos]
        sign = buf[pos + 1]
        pos += 2
        if pos + n > len(buf):
            raise Trunc()
        num = 0
        for i in range(n):
            num |= buf[pos + i] << (8 * i)
        pos += n
        if sign:
            num = -num
        return num, pos
    if tag == 0x6f:
        if pos + 5 > len(buf):
            raise Trunc()
        n = struct.unpack_from(">I", buf, pos)[0]
        sign = buf[pos + 4]
        pos += 5
        if pos + n > len(buf):
            raise Trunc()
        num = 0
        for i in range(n):
            num |= buf[pos + i] << (8 * i)
        pos += n
        if sign:
            num = -num
        return num, pos
    if tag == 0x68 or tag == 0x69:
        if tag == 0x68:
            if pos + 1 > len(buf):
                raise Trunc()
            n = buf[pos]
            pos += 1
        else:
            if pos + 4 > len(buf):
                raise Trunc()
            n = struct.unpack_from(">I", buf, pos)[0]
            pos += 4
        items = []
        for _ in range(n):
            v, pos = decode(buf, pos, depth + 1)
            items.append(v)
        return ("tuple", items), pos
    if tag == 0x6c:
        if pos + 4 > len(buf):
            raise Trunc()
        n = struct.unpack_from(">I", buf, pos)[0]
        pos += 4
        items = []
        for _ in range(n):
            v, pos = decode(buf, pos, depth + 1)
            items.append(v)
        if pos + 1 > len(buf):
            raise Trunc()
        if buf[pos] != 0x6a:
            raise ValueError(f"LIST_EXT tail is not NIL_EXT at {pos}")
        return ("list", items), pos + 1
    if tag == 0x74:
        if pos + 4 > len(buf):
            raise Trunc()
        n = struct.unpack_from(">I", buf, pos)[0]
        pos += 4
        pairs = {}
        for _ in range(n):
            k, pos = decode(buf, pos, depth + 1)
            v, pos = decode(buf, pos, depth + 1)
            pairs[k] = v
        return ("map", pairs), pos
    raise ValueError(f"unknown tag 0x{tag:02x} at {pos - 1}")


def field(value, key):
    if isinstance(value, tuple) and value[0] == "map":
        return value[1].get(("atom", key))
    return None


offset = 0
terms = 0
routed_pre = Counter()
non_dispatch_ops = []
while offset < len(prefix):
    if prefix[offset] != 0x83:
        check(False, f"терм {terms}: нет 0x83 на смещении {offset}")
        break
    try:
        value, end = decode(prefix, offset + 1)
    except Trunc:
        break
    except ValueError as exc:
        check(False, f"терм {terms} на смещении {offset}: {exc}")
        break
    offset = end
    terms += 1
    op = field(value, "op")
    if op == 0:
        routed_pre[field(value, "t")[1]] += 1
    else:
        non_dispatch_ops.append(op)
leftover = len(prefix) - offset
print("== TERMS .1389 (доступный префикс) ==")
check(terms == 44, f"термов = {terms} (ожидается 44)")
check(len(routed_pre) > 0 and sum(routed_pre.values()) == 36,
      f"op0 в префиксе = {sum(routed_pre.values())} (ожидается 36)")
check(len(non_dispatch_ops) == 8, f"не-диспатчей = {len(non_dispatch_ops)} (ожидается 8)")
check(all(op in (10, 11) for op in non_dispatch_ops),
      f"не-диспатчи только op10/op11: {non_dispatch_ops}")
check(leftover == 704, f"остаток (обрезанный терм) = {leftover} (ожидается 704)")
check(prefix[offset:offset + 1] == b"\x83", "остаток начинается с 0x83 (обрезанный терм)")
print("prefix by type (собственный декодер):",
      dict(sorted(routed_pre.items(), key=lambda kv: -kv[1])))

# ---------------- 5. Таймлайн ----------------
tl_dispatch = 0
tl_total = 0
for line in (LAB / "timeline-95974_1389.tsv").read_text().splitlines():
    if line.startswith("#") or not line.strip():
        continue
    tl_total += 1
    cols = line.split("\t")
    if len(cols) >= 3 and cols[2] == "0":
        tl_dispatch += 1
check(tl_dispatch == 113, f"таймлайн: диспатчей op0 = {tl_dispatch} (ожидается 113)")
print("timeline rows:", tl_total, "op0:", tl_dispatch)

# ---------------- 6. Сверка с константами ----------------
test_src = Path("/home/user/orca/project/rust/crates/discord-core/tests/dispatch_router.rs").read_text()
lib_src = Path("/home/user/orca/project/rust/crates/discord-core/src/lib.rs").read_text()


def parse_names(block):
    return re.findall(r'"([A-Z_]+)"', block)


m = re.search(r"const LAB_TYPE_NAMES: \[&str; 21\] = \[(.*?)\];", test_src, re.S)
test_names = parse_names(m.group(1))
check(len(test_names) == 21 and set(test_names) == set(union_counts),
      "LAB_TYPE_NAMES == типы из lab-файлов (как множество)")

m = re.search(r"const LAB_TYPE_COUNTS: \[\(&str, u64\); 21\] = \[(.*?)\];", test_src, re.S)
test_counts = dict((n, int(c)) for n, c in re.findall(r'\("([A-Z_]+)", (\d+)\)', m.group(1)))
check(test_counts == dict(union_counts), "LAB_TYPE_COUNTS == мои union-числа")

m = re.search(r"const LAB_STREAM_COUNTS: \[\(&str, u64\); 9\] = \[(.*?)\];", test_src, re.S)
test_stream = dict((n, int(c)) for n, c in re.findall(r'\("([A-Z_]+)", (\d+)\)', m.group(1)))
check(test_stream == dict(routed_pre), "LAB_STREAM_COUNTS == мои числа префикса (собственный декодер)")

m = re.search(r"DISPATCH_COUNT_R7: u32 = ([\d_]+)", lib_src)
lib_total = int(m.group(1).replace("_", ""))
check(lib_total == union_total, f"DISPATCH_COUNT_R7 = {lib_total} == union {union_total}")
m = re.search(r"DISPATCH_TYPE_COUNT_R7: usize = (\d+)", lib_src)
check(int(m.group(1)) == len(union_counts), "DISPATCH_TYPE_COUNT_R7 == 21")

print()
if fail:
    print("ИТОГ: ПРОВАЛ —", len(fail), "несовпадений")
    for f in fail:
        print(" -", f)
    sys.exit(1)
print("ИТОГ: все независимые оракулы сошлись")
