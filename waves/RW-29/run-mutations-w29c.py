#!/usr/bin/env python3
"""RW-29 / W29c — targeted-мутации reconnect-политики с восстановлением по md5.

Мутируется только продукт (`src/reconnect.rs`); тесты, `Cargo.toml`/`Cargo.lock`
и прочие файлы не меняются. Каждая мутация откатывается из in-memory копии,
после прогона проверяется md5 продукта и теста.

Запуск:
    python3 waves/RW-29/run-mutations-w29c.py

Итог печатает строкой вида:
    caught=6/6 restored_all=True base_md5=reconnect:<md5>,test:<md5>
"""

import hashlib
import os
import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path("/home/user/orca/project")
RUST = ROOT / "rust"
SRC = RUST / "crates/discord-voice/src/reconnect.rs"
TEST = RUST / "crates/discord-voice/tests/reconnect_contract.rs"
TARGET = "/tmp/opencode/target-w29c"

MUTATIONS = [
    (
        "M1",
        "порядок бэкоффа перевёрнут (30с → 1с)",
        "pub const BACKOFF_MS: [u64; 6] = [1_000, 2_000, 4_000, 8_000, 16_000, 30_000];",
        "pub const BACKOFF_MS: [u64; 6] = [30_000, 16_000, 8_000, 4_000, 2_000, 1_000];",
    ),
    (
        "M2",
        "op9-задержка 1000 → 500 мс",
        "pub const INVALID_SESSION_DELAY_MS: u64 = 1_000;",
        "pub const INVALID_SESSION_DELAY_MS: u64 = 500;",
    ),
    (
        "M3",
        "on_established не сбрасывает attempt",
        "    pub fn on_established(&mut self) {\n        self.attempt = 0;\n    }",
        "    pub fn on_established(&mut self) {}",
    ),
    (
        "M4",
        "лимит попыток проверяется как attempt > max (off-by-one)",
        "if self.attempt >= self.max_attempts {",
        "if self.attempt > self.max_attempts {",
    ),
    (
        "M5",
        "ветка resumable инвертирована (Resume ↔ Identify)",
        "        let action = if resumable {",
        "        let action = if !resumable {",
    ),
    (
        "M6",
        "GiveUp возвращает бэкофф вместо нулевой задержки",
        "                action: ReconnectAction::GiveUp,\n                delay_ms: 0,",
        "                action: ReconnectAction::GiveUp,\n                delay_ms: backoff_ms(self.attempt),",
    ),
]


def md5(path: pathlib.Path) -> str:
    return hashlib.md5(path.read_bytes()).hexdigest()


def run_tests() -> tuple[int, str]:
    proc = subprocess.run(
        ["cargo", "test", "--offline", "-p", "discord-voice", "--test", "reconnect_contract"],
        cwd=RUST,
        env={**os.environ, "CARGO_TARGET_DIR": TARGET},
        capture_output=True,
        text=True,
        timeout=600,
    )
    return proc.returncode, proc.stdout + proc.stderr


def main() -> int:
    original = SRC.read_text(encoding="utf-8")
    base = md5(SRC)
    test_md5 = md5(TEST)
    print(f"base_md5=reconnect:{base},test:{test_md5}")

    caught = 0
    restored_all = True
    for ident, description, old, new in MUTATIONS:
        count = original.count(old)
        if count != 1:
            print(f"{ident}: FAIL anchor count {count} != 1 ({description})", flush=True)
            return 2
        SRC.write_text(original.replace(old, new), encoding="utf-8")
        try:
            code, output = run_tests()
        finally:
            SRC.write_text(original, encoding="utf-8")
            # mtime должен быть новее артефакта мутанта, иначе cargo переиспользует его
            SRC.touch()
        restored = md5(SRC) == base
        restored_all = restored_all and restored

        failed = re.findall(r"^test (\S+) \.\.\. FAILED$", output, re.MULTILINE)
        compile_error = "error[E" in output or "could not compile" in output
        if code == 0:
            verdict = "MISSED"
        elif compile_error and not failed:
            verdict = "INCONCLUSIVE (compile)"
        else:
            verdict = "caught"
            caught += 1
        tests = ",".join(failed) if failed else "—"
        print(
            f"{ident}: exit={code} {verdict} restored={restored} [{tests}] — {description}",
            flush=True,
        )
        if not restored:
            print(f"{ident}: RESTORE FAILED", file=sys.stderr)
            return 3

    total = len(MUTATIONS)
    print(
        f"caught={caught}/{total} restored_all={restored_all} "
        f"base_md5=reconnect:{base},test:{test_md5}"
    )
    return 0 if caught == total and restored_all else 1


if __name__ == "__main__":
    sys.exit(main())
