#!/usr/bin/env python3
"""QA-1: матрица мутаций B07/RW-26, лейн P4-11 (регионы/latency).

Каждая мутация — точечная правка продуктового кода. Ожидание: `regions_contract`
падает (мутант пойман). Продукт восстанавливается из бэкапа и проверяется по md5.

Запуск: python3 waves/RW-26/run-mutations-p411.py
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

# (id, файл, old, new, target, что именно ломается)
MUTATIONS = [
    (
        1,
        "rust/crates/discord-voice/src/regions.rs",
        "Some(values[(values.len() - 1) / 2])",
        "Some(values[values.len() / 2])",
        "regions_contract",
        "median even-count rounding",
    ),
    (
        2,
        "rust/crates/discord-voice/src/regions.rs",
        "lm.is_none()\n            .cmp(&rm.is_none())",
        "rm.is_none()\n            .cmp(&lm.is_none())",
        "regions_contract",
        "unmeasured regions must follow measured",
    ),
    (
        3,
        "rust/crates/discord-voice/src/regions.rs",
        ".then_with(|| geo_pos(left).cmp(&geo_pos(right)))",
        "",
        "regions_contract",
        "geo tie-break before lexical",
    ),
    (
        4,
        "rust/crates/discord-voice/src/regions.rs",
        "if port_text != port.to_string() || !(1..=65_535).contains(&port) {",
        "if !(1..=65_535).contains(&port) {",
        "regions_contract",
        "canonical port spelling",
    ),
    (
        5,
        "rust/crates/discord-voice/src/regions.rs",
        "if seen_ips.insert(endpoint.to_owned()) {\n                ips.push(endpoint.to_owned());\n            }",
        "ips.push(endpoint.to_owned());",
        "regions_contract",
        "endpoint deduplication",
    ),
    (
        6,
        "rust/crates/discord-voice/src/regions.rs",
        "region.ips[(seq as usize) % region.ips.len()].clone()",
        "region.ips[0].clone()",
        "regions_contract",
        "probe endpoint rotation",
    ),
    (
        7,
        "rust/crates/discord-voice/src/regions.rs",
        "(rtt <= timeout_ms).then_some(rtt)",
        "(rtt >= timeout_ms).then_some(rtt)",
        "regions_contract",
        "probe timeout comparison",
    ),
    (
        8,
        "rust/crates/discord-voice/src/regions.rs",
        "crate::select_region(&latency.measured(), geo, manual)",
        "crate::select_region(&latency.measured(), geo, None)",
        "regions_contract",
        "manual region honoured/rejected",
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
        path.write_text(original.replace(old, new, 1))
        started = time.time()
        proc = subprocess.run(
            ["cargo", "test", "--offline", "-p", "discord-voice", "--test", target],
            cwd=RUST,
            env={**os.environ, "CARGO_TARGET_DIR": TARGET_DIR},
            capture_output=True,
            text=True,
        )
        elapsed = round(time.time() - started, 1)
        log = OUT / f"p411-m{ident:02d}-{target}.log"
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
        "card": "P4-11",
        "caught": caught,
        "total": len(results),
        "results": results,
    }
    (OUT / "MUTATIONS-P411.json").write_text(json.dumps(summary, indent=1))
    print(f"ИТОГ: {caught}/{len(results)} caught")
    return 0 if caught == len(results) else 1


if __name__ == "__main__":
    raise SystemExit(main())
