#!/usr/bin/env python3
"""QA-1: матрица мутаций B07/RW-26, лейн P4-07 (RTCP/RTP/stats).

Каждая мутация — точечная правка продуктового кода. Ожидание: целевые тесты
падают (мутант пойман). Продукт восстанавливается из бэкапа и проверяется по md5.

Запуск: python3 waves/RW-26/run-mutations-p407.py
"""

from __future__ import annotations

import hashlib
import json
import os
import shutil
import subprocess
import time
from pathlib import Path

ROOT = Path("/home/user/orca/project")
RUST = ROOT / "rust"
OUT = ROOT / "waves/RW-26/mutations"
TARGET_DIR = "/home/user/.cache/discord-lab-target-rw26-mut"

# (id, файл, old, new, test-target, что именно ломается)
MUTATIONS = [
    (
        1,
        "rust/crates/discord-voice/src/rtcp.rs",
        "    (raw << 8) >> 8",
        "    raw",
        "rtcp_contract",
        "24-bit sign extension",
    ),
    (
        2,
        "rust/crates/discord-voice/src/rtcp.rs",
        "if offset > 0 && (available == 16 || available == 20) {",
        "if available == 16 || available == 20 {",
        "rtcp_contract",
        "truncated-first-packet vs trailer",
    ),
    (
        3,
        "rust/crates/discord-voice/src/rtcp.rs",
        "RTCP_RR => HEADER_LEN + 4 + reports,",
        "RTCP_RR => HEADER_LEN + reports,",
        "rtcp_contract",
        "RR minimum length",
    ),
    (
        4,
        "rust/crates/discord-voice/src/rtcp.rs",
        "if packets.len() > MAX_COMPOUND_PACKETS {",
        "if packets.len() >= MAX_COMPOUND_PACKETS {",
        "rtcp_contract",
        "compound packet limit",
    ),
    (
        5,
        "rust/crates/discord-voice/src/reception.rs",
        "self.jitter += (delta.abs() - self.jitter) / 16.0;",
        "self.jitter += (delta.abs() - self.jitter) / 8.0;",
        "reception_contract",
        "RFC 3550 jitter gain",
    ),
    (
        6,
        "rust/crates/discord-voice/src/reception.rs",
        "self.duplicates += 1;",
        "self.duplicates = 0;",
        "reception_contract",
        "duplicate counter",
    ),
    (
        7,
        "rust/crates/discord-voice/src/reception.rs",
        "} else if difference < -SEQ_HALF {",
        "} else if difference < -100_000 {",
        "reception_contract",
        "forward sequence wrap",
    ),
    (
        8,
        "rust/crates/discord-voice/src/stats.rs",
        "let hold_ms = f64::from(block.delay_since_last_sr) * 1000.0 / 65_536.0;",
        "let hold_ms = 0.0;",
        "stats_contract",
        "DLSR subtraction",
    ),
    (
        9,
        "rust/crates/discord-voice/src/stats.rs",
        "capacity: capacity.max(1),",
        "capacity,",
        "stats_contract",
        "zero-capacity window",
    ),
    (
        10,
        "rust/crates/discord-voice/src/stats.rs",
        "Some(last.cumulative_lost - first.cumulative_lost)",
        "Some(last.cumulative_lost)",
        "stats_contract",
        "loss delta window",
    ),
    (
        11,
        "rust/crates/discord-voice/src/reception.rs",
        "f64::from(header.timestamp.wrapping_sub(previous_timestamp) as i32)",
        "f64::from(header.timestamp) - f64::from(previous_timestamp)",
        "reception_contract",
        "RTP timestamp wrap",
    ),
]


def md5(path: Path) -> str:
    return hashlib.md5(path.read_bytes()).hexdigest()


def main() -> int:
    OUT.mkdir(parents=True, exist_ok=True)
    results = []
    for ident, rel, old, new, target, what in MUTATIONS:
        path = ROOT / rel
        original = path.read_text()
        if old not in original:
            results.append({"id": ident, "file": rel, "status": "ERROR: pattern not found"})
            print(f"M{ident}: pattern not found in {rel}", flush=True)
            continue
        before = md5(path)
        backup = OUT / f".m{ident}.bak"
        shutil.copy2(path, backup)
        mutated = original.replace(old, new, 1)
        path.write_text(mutated)
        started = time.time()
        proc = subprocess.run(
            ["cargo", "test", "--offline", "-p", "discord-voice", "--test", target],
            cwd=RUST,
            env={**os.environ, "CARGO_TARGET_DIR": TARGET_DIR},
            capture_output=True,
            text=True,
        )
        elapsed = round(time.time() - started, 1)
        log = OUT / f"m{ident:02d}-{target}.log"
        log.write_text(proc.stdout + "\n--- stderr ---\n" + proc.stderr)
        shutil.copy2(backup, path)
        backup.unlink()
        after = md5(path)
        caught = proc.returncode != 0 and after == before
        results.append(
            {
                "id": ident,
                "file": rel,
                "target": target,
                "what": what,
                "exit": proc.returncode,
                "caught": caught,
                "updated_md5": before,
                "restored_md5": after,
                "seconds": elapsed,
            }
        )
        print(f"M{ident}: exit={proc.returncode} caught={caught} ({what}) {elapsed}s", flush=True)

    caught = sum(1 for item in results if item.get("caught"))
    summary = {
        "wave": "RW-26",
        "card": "P4-07",
        "caught": caught,
        "total": len(results),
        "results": results,
    }
    (OUT / "MUTATIONS-P407.json").write_text(json.dumps(summary, indent=1))
    print(f"ИТОГ: {caught}/{len(results)} caught")
    return 0 if caught == len(results) else 1


if __name__ == "__main__":
    raise SystemExit(main())
