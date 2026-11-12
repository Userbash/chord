#!/usr/bin/env python3
"""QA-1: матрица мутаций волны B06/RW-25 (P4-03, P7-02, P8-01).

Каждая мутация — точечная правка продуктового кода. Ожидание: целевые тесты
**падают** (мутант пойман). Непойманный мутант = пробел теста (missed).

Запуск:  python3 waves/RW-25/run-mutations.py
Продукт восстанавливается по md5 после каждой мутации.
"""

from __future__ import annotations

import hashlib
import json
import os
import shutil
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path("/home/user/orca/project")
RUST = ROOT / "rust"
OUT = ROOT / "waves/RW-25/mutations"
TARGET_DIR = "/tmp/opencode/target-mut"

# (id, файл, old, new, команда целевых тестов, ожидаемый маркер падения)
MUTATIONS: list[tuple[int, str, str, str, list[str], str]] = [
    # --- P4-03: SRTP ---
    (
        1,
        "rust/crates/discord-voice/src/srtp.rs",
        "let value = 0x8000_0000u32 | (self.counter as u32);",
        "let value = 0x0000_0000u32 | (self.counter as u32);",
        ["test", "--offline", "-p", "discord-voice", "--test", "srtp_contract"],
        "trailer",
    ),
    (
        2,
        "rust/crates/discord-voice/src/srtp.rs",
        "const AEAD_TAG_LEN: usize = 16;",
        "const AEAD_TAG_LEN: usize = 12;",
        ["test", "--offline", "-p", "discord-voice", "--test", "srtp_contract"],
        "tag",
    ),
    (
        3,
        "rust/crates/discord-voice/src/srtp.rs",
        "let out = self.gcm.encrypt(&nonce, aad, plaintext)?;",
        "let out = self.gcm.encrypt(&nonce, &[], plaintext)?;",
        ["test", "--offline", "-p", "discord-voice", "--test", "srtp_contract"],
        "aad",
    ),
    (
        4,
        "rust/crates/discord-voice/src/srtp.rs",
        "            let clear_len = rtp_header_len(packet)?;\n            let (ciphertext, tag) =\n                self.seal(&packet[clear_len..], &packet[..clear_len], trailer)?;",
        "            let clear_len = RTP_HEADER_MIN;\n            let (ciphertext, tag) =\n                self.seal(&packet[clear_len..], &packet[..clear_len], trailer)?;",
        ["test", "--offline", "-p", "discord-voice", "--test", "srtp_contract"],
        "clear",
    ),
    (
        5,
        "rust/crates/discord-voice/src/srtp.rs",
        "        header_len += 4 + 4 * ext_len_words;",
        "        header_len += 4;",
        ["test", "--offline", "-p", "discord-voice", "--test", "srtp_contract"],
        "ext",
    ),
    (
        6,
        "rust/crates/discord-voice/src/srtp.rs",
        "            self.counter += 1;\n            Ok(out)\n        } else {",
        "            Ok(out)\n        } else {",
        ["test", "--offline", "-p", "discord-voice", "--test", "srtp_contract"],
        "counter",
    ),
    (
        7,
        "rust/crates/discord-voice/src/srtp.rs",
        "                let out = self.xchacha.encrypt(&nonce, aad, plaintext)?;",
        "                let out = self.xchacha.encrypt(&nonce, &[], plaintext)?;",
        ["test", "--offline", "-p", "discord-voice", "--test", "srtp_contract"],
        "xchacha",
    ),
    (
        8,
        "rust/crates/discord-voice/src/crypto/aes.rs",
        "x ^ x.rotate_left(1) ^ x.rotate_left(2) ^ x.rotate_left(3) ^ x.rotate_left(4) ^ 0x63",
        "x ^ x.rotate_left(1) ^ x.rotate_left(2) ^ x.rotate_left(3) ^ x.rotate_left(4) ^ 0x62",
        ["test", "--offline", "-p", "discord-voice", "--test", "crypto_vectors"],
        "affine",
    ),
    (
        15,
        "rust/crates/discord-voice/src/crypto/aes.rs",
        "        a ^= 0x1b & high;",
        "        a ^= 0x1b & high ^ 0x01;",
        ["test", "--offline", "-p", "discord-voice", "--test", "crypto_vectors"],
        "gfmul",
    ),
    (
        16,
        "rust/crates/discord-voice/src/crypto/kdf.rs",
        "    let mut inner_pad = [0x36u8; 64];\n    let mut outer_pad = [0x5cu8; 64];",
        "    let mut inner_pad = [0x36u8; 64];\n    let mut outer_pad = [0x5du8; 64];",
        ["test", "--offline", "-p", "discord-voice", "--test", "crypto_vectors"],
        "hkdf",
    ),
    (
        17,
        "rust/crates/discord-voice/src/srtp.rs",
        "const SRTCP_CLEAR_LEN: usize = 8;",
        "const SRTCP_CLEAR_LEN: usize = 4;",
        ["test", "--offline", "-p", "discord-voice", "--test", "srtp_contract"],
        "srtcp",
    ),
    (
        18,
        "rust/crates/discord-voice/src/srtp.rs",
        "        if self.receive_seen.contains(&trailer) {\n            return Err(SrtpError::NonceReuse { trailer });\n        }",
        "        if false {\n            return Err(SrtpError::NonceReuse { trailer });\n        }",
        ["test", "--offline", "-p", "discord-voice", "--test", "srtp_contract"],
        "nonce",
    ),
    # --- P8-01: updater ---
    (
        9,
        "rust/crates/discord-updater/src/plan.rs",
        "    if !is_safe_component(artifact_name) {",
        "    if false {",
        ["test", "--offline", "-p", "discord-updater", "--test", "updater_contract"],
        "name",
    ),
    (
        10,
        "rust/crates/discord-updater/src/plan.rs",
        "    if !actual.eq_ignore_ascii_case(&expected) {\n        return Err(PlanError::DigestMismatch { artifact });\n    }",
        "    let _ = (actual, expected);",
        ["test", "--offline", "-p", "discord-updater", "--test", "updater_contract"],
        "digest",
    ),
    (
        11,
        "rust/crates/discord-updater/src/plan.rs",
        "    if name.contains('/') || name.contains('\\\\') {\n        return false;\n    }",
        "",
        ["test", "--offline", "-p", "discord-updater", "--test", "updater_contract"],
        "slash",
    ),
    (
        12,
        "rust/crates/discord-updater/src/manifest.rs",
        "                (None, Some(_)) => true,",
        "                (None, Some(_)) => false,",
        ["test", "--offline", "-p", "discord-updater", "--test", "updater_contract"],
        "version",
    ),
    (
        13,
        "rust/crates/discord-updater/src/plan.rs",
        "            return Err(PlanError::BackupCorrupted {",
        "            return Err(PlanError::NotNewer { current: String::new(), candidate: String::new() });\n            #[allow(unreachable_code)]\n            return Err(PlanError::BackupCorrupted {",
        ["test", "--offline", "-p", "discord-updater", "--test", "updater_contract"],
        "backup",
    ),
    # --- P7-02: перф-харнесс ---
    (
        14,
        "rust/crates/discord-perf/src/workload.rs",
        "REGRESSION_THRESHOLD_PCT: f64 = 10.0",
        "REGRESSION_THRESHOLD_PCT: f64 = 100.0",
        ["test", "--offline", "-p", "discord-perf", "--test", "perf_contract"],
        "threshold",
    ),
]


def md5(path: Path) -> str:
    return hashlib.md5(path.read_bytes()).hexdigest()


def run(cmd: list[str]) -> subprocess.CompletedProcess[str]:
    env = {**os.environ, "CARGO_TARGET_DIR": TARGET_DIR}
    return subprocess.run(
        cmd, cwd=str(RUST), capture_output=True, text=True, env=env, timeout=1800
    )


def main() -> int:
    OUT.mkdir(parents=True, exist_ok=True)
    results: list[dict[str, object]] = []
    baseline = run(["cargo", "test", "--offline", "-p", "discord-voice", "-p", "discord-updater", "-p", "discord-perf"])
    if baseline.returncode != 0:
        print("БАЗА ЗЕЛЁНАЯ ПРОВЕРКА НЕ ПРОШЛА — матрица не запускается")
        print(baseline.stdout[-3000:])
        return 2
    print("база зелёная, старт матрицы")

    for mid, rel, old, new, cmd, marker in MUTATIONS:
        path = ROOT / rel
        original = path.read_text(encoding="utf-8")
        digest = md5(path)
        if old not in original:
            results.append({"id": mid, "file": rel, "status": "pattern-missing", "marker": marker})
            print(f"M{mid}: ПАТТЕРН НЕ НАЙДЕН в {rel} — мутация не применена")
            continue
        mutated = original.replace(old, new, 1)
        path.write_text(mutated, encoding="utf-8")
        started = time.monotonic()
        try:
            result = run(["cargo", *cmd])
            code = result.returncode
            output = result.stdout + result.stderr
        except subprocess.TimeoutExpired:
            code = 124
            output = "timeout"
        elapsed = time.monotonic() - started
        path.write_text(original, encoding="utf-8")
        restored = md5(path) == digest
        caught = code != 0
        status = "caught" if caught else "MISSED"
        results.append(
            {
                "id": mid,
                "file": rel,
                "status": status,
                "exit_code": code,
                "restored": restored,
                "elapsed_s": round(elapsed, 1),
                "marker": marker,
                "log": f"waves/RW-25/mutations/m{mid:02d}.log",
            }
        )
        print(f"M{mid}: {status} (exit={code}, {elapsed:.1f}s, restore={restored})")
        (OUT / f"m{mid:02d}.log").write_text(
            f"# мутация M{mid}: {rel}\n# old: {old[:120]!r}\n# new: {new[:120]!r}\n"
            f"# команда: cargo {' '.join(cmd)}\n# exit code: {code}\n# восстановлено по md5: {restored}\n\n{output[-4000:]}\n",
            encoding="utf-8",
        )

    caught = sum(1 for r in results if r["status"] == "caught")
    missed = sum(1 for r in results if r["status"] == "MISSED")
    broken = sum(1 for r in results if r["status"] == "pattern-missing")
    (OUT / "mutations.json").write_text(
        json.dumps({"results": results, "caught": caught, "missed": missed, "pattern_missing": broken}, ensure_ascii=False, indent=2),
        encoding="utf-8",
    )
    print(f"\nитого: поймано {caught}, пропущено {missed}, шаблон не найден {broken}")
    return 0 if missed == 0 and broken == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
