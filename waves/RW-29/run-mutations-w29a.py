#!/usr/bin/env python3
"""Мутационная проверка тестов RW-29 / W29a (`timestamp_contract`).

Каждая мутация — точечная текстовая замена в продукте, затем targeted-прогон.
Ожидание: тест-набор падает (мутация убита). После каждой мутации файл
восстанавливается из исходных байтов и проверяется md5.

Запуск: python3 waves/RW-29/run-mutations-w29a.py
Лог:    waves/RW-29/mutations-w29a.log
"""

from __future__ import annotations

import hashlib
import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CRATE = ROOT / "rust/crates/discord-i18n"
LOG_PATH = ROOT / "waves/RW-29/mutations-w29a.log"
TEST_CMD = [
    "cargo",
    "test",
    "--offline",
    "-p",
    "discord-i18n",
    "--test",
    "timestamp_contract",
]
ENV = {**os.environ, "CARGO_TARGET_DIR": "/tmp/opencode/target-w29a"}

# Файлы, которые мутации не должны оставить изменёнными.
PRODUCT_FILES = [
    CRATE / "src/timestamp.rs",
    CRATE / "src/timestamp_data.rs",
    CRATE / "src/format.rs",
    CRATE / "src/lib.rs",
]

MUTATIONS = [
    {
        "id": "m01",
        "title": "стиль t → паттерн T (style→другой)",
        "file": "src/timestamp.rs",
        "old": "            Self::ShortTime => Some(0),",
        "new": "            Self::ShortTime => Some(1),",
    },
    {
        "id": "m02",
        "title": "ладдер единиц: WEEK 7 → 8 суток",
        "file": "src/timestamp.rs",
        "old": "const WEEK_SECS: u64 = 7 * DAY_SECS;",
        "new": "const WEEK_SECS: u64 = 8 * DAY_SECS;",
    },
    {
        "id": "m03",
        "title": "дефолт пустого стиля: f → d",
        "file": "src/timestamp.rs",
        "old": "        Some(b'>') => (TimestampStyle::ShortDateTime, index + 1),",
        "new": "        Some(b'>') => (TimestampStyle::ShortDate, index + 1),",
    },
    {
        "id": "m04",
        "title": "парсер токена: потерян знак минус",
        "file": "src/timestamp.rs",
        "old": "    let negative = bytes.first() == Some(&b'-');",
        "new": "    let negative = false;",
    },
    {
        "id": "m05",
        "title": "локаль-фолбэк: без проверки is_enabled",
        "file": "src/timestamp.rs",
        "old": (
            "    canonical_locale(locale)\n"
            "        .filter(|code| is_enabled(code))\n"
            "        .unwrap_or(DEFAULT_LOCALE)"
        ),
        "new": "    canonical_locale(locale).unwrap_or(DEFAULT_LOCALE)",
    },
    {
        "id": "m06",
        "title": "нормализация NBSP/NNBSP отключена",
        "file": "src/format.rs",
        "old": "    Some(out.replace(['\\u{202f}', '\\u{a0}'], \" \"))",
        "new": "    Some(out)",
    },
    {
        "id": "m07",
        "title": "relative: Auto → Always (нет «now»/special forms)",
        "file": "src/timestamp.rs",
        "old": "        RelativeNumeric::Auto,",
        "new": "        RelativeNumeric::Always,",
    },
    {
        "id": "m08",
        "title": "ладдер единиц: MONTH 30 → 31 суток",
        "file": "src/timestamp.rs",
        "old": "const MONTH_SECS: u64 = 30 * DAY_SECS;",
        "new": "const MONTH_SECS: u64 = 31 * DAY_SECS;",
    },
]


def md5(path: Path) -> str:
    return hashlib.md5(path.read_bytes()).hexdigest()


def run_tests() -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        TEST_CMD,
        cwd=ROOT / "rust",
        env=ENV,
        capture_output=True,
        text=True,
        timeout=1800,
        check=False,
    )


def failing_lines(output: str) -> list[str]:
    lines = [
        line.strip()
        for line in output.splitlines()
        if line.startswith("test ") and " FAILED" in line
    ]
    lines += [line.strip() for line in output.splitlines() if "panicked at" in line]
    return lines[:6]


def main() -> int:
    baseline = {path: md5(path) for path in PRODUCT_FILES}
    log: list[str] = [
        "# RW-29 / W29a — мутации `timestamp_contract`",
        f"# command: (cd rust && CARGO_TARGET_DIR=/tmp/opencode/target-w29a {' '.join(TEST_CMD)})",
        "",
        "## База (md5 до мутаций)",
        *[f"{baseline[path]}  {path.relative_to(ROOT)}" for path in PRODUCT_FILES],
        "",
    ]
    survivors: list[str] = []

    for mutation in MUTATIONS:
        path = CRATE / mutation["file"]
        original = path.read_bytes()
        original_md5 = hashlib.md5(original).hexdigest()
        text = original.decode("utf-8")
        occurrences = text.count(mutation["old"])
        header = [
            f"## {mutation['id']} — {mutation['title']}",
            f"file: {path.relative_to(ROOT)}",
            f"md5 before: {original_md5}",
        ]
        if occurrences != 1:
            header.append(f"RESULT: SKIPPED (совпадений old: {occurrences}, нужно ровно 1)")
            log.extend(header + [""])
            survivors.append(mutation["id"])
            continue

        mutated = text.replace(mutation["old"], mutation["new"], 1)
        try:
            path.write_text(mutated, encoding="utf-8")
            header.append(f"md5 mutated: {md5(path)}")
            result = run_tests()
            killed = result.returncode != 0
            header.append(f"exit: {result.returncode}")
            header.append("RESULT: KILLED" if killed else "RESULT: SURVIVED")
            if killed:
                header.extend(f"  {line}" for line in failing_lines(result.stdout + result.stderr))
            else:
                survivors.append(mutation["id"])
        finally:
            path.write_bytes(original)
            restored = md5(path)
            header.append(f"md5 restored: {restored} ({'ok' if restored == original_md5 else 'MISMATCH'})")
            if restored != original_md5:
                survivors.append(f"{mutation['id']}:restore")
        log.extend(header + [""])

    log.append("## Итог")
    log.append(f"мутаций: {len(MUTATIONS)}, выжило: {len(survivors)}")
    if survivors:
        log.append(f"выжившие: {', '.join(survivors)}")
    final = {path: md5(path) for path in PRODUCT_FILES}
    log.append(f"продукт восстановлен: {'да' if final == baseline else 'НЕТ'}")
    log.append("")

    LOG_PATH.write_text("\n".join(log), encoding="utf-8")
    print("\n".join(log))
    return 1 if survivors or final != baseline else 0


if __name__ == "__main__":
    sys.exit(main())
