#!/usr/bin/env python3
"""Мутационная проверка контракта W30a (`format_duration`, RW-30).

Каждая мутация — один строковый патч в `src/duration.rs`; тест
`duration_contract` обязан упасть (exit != 0). Файл восстанавливается из
байтового снимка, затем сверяется md5. Мутации, выжившие (exit == 0),
считаются провалом прогона.
"""

from __future__ import annotations

import hashlib
import os
import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2]
SRC = ROOT / "rust/crates/discord-i18n/src/duration.rs"
LOG = ROOT / "waves/RW-30/mutations-w30a.log"
TARGET_DIR = "/tmp/opencode/target-w30a"

# (id, описание, точная подстрока, замена)
MUTATIONS = [
    (
        "m01",
        "порядок единиц: hours ↔ minutes",
        '("hour", self.hours),\n            ("minute", self.minutes),',
        '("minute", self.minutes),\n            ("hour", self.hours),',
    ),
    (
        "m02",
        "join2 заменён на join_middle",
        "out.push_str(joins.join2);",
        "out.push_str(joins.join_middle);",
    ),
    (
        "m03",
        "join_middle заменён на join_last",
        "for chunk in middle {\n                out.push_str(joins.join_middle);",
        "for chunk in middle {\n                out.push_str(joins.join_last);",
    ),
    (
        "m04",
        "нулевые единицы больше не опускаются",
        "if value == 0 {",
        "if false {",
    ),
    (
        "m05",
        "плюральная категория всегда other",
        "plural_category_integer(row.locale, value, PluralType::Cardinal).as_str()",
        '"other"',
    ),
    (
        "m06",
        "фолбэк локали ведёт в ru вместо en-US",
        ".unwrap_or(DEFAULT_LOCALE);",
        '.unwrap_or("ru");',
    ),
    (
        "m07",
        "группировка отключена (value.to_string())",
        "format_integer_unsigned(value, row.locale)",
        "value.to_string()",
    ),
    (
        "m08",
        "нормализация NBSP/NNBSP удалена",
        "rendered.replace(['\\u{202f}', '\\u{a0}'], \" \")",
        "rendered",
    ),
]


def md5(path: pathlib.Path) -> str:
    return hashlib.md5(path.read_bytes()).hexdigest()


def run_test() -> tuple[int, str]:
    env = dict(os.environ, CARGO_TARGET_DIR=TARGET_DIR)
    proc = subprocess.run(
        [
            "cargo",
            "test",
            "--offline",
            "-p",
            "discord-i18n",
            "--test",
            "duration_contract",
        ],
        cwd=ROOT / "rust",
        env=env,
        capture_output=True,
        text=True,
    )
    output = proc.stdout + proc.stderr
    failing = [
        line.strip()
        for line in output.splitlines()
        if line.strip().startswith("d") and "..." in line and "FAILED" not in line
    ]
    summary = next(
        (line.strip() for line in output.splitlines() if line.startswith("test result:")),
        "test result: <нет строки>",
    )
    detail = f"{summary}; первые падения: {failing[:3]}" if failing else summary
    return proc.returncode, detail


def main() -> int:
    baseline = SRC.read_bytes()
    baseline_md5 = hashlib.md5(baseline).hexdigest()
    lines = [
        "# RW-30 W30a: мутации `format_duration`",
        f"# baseline md5 duration.rs: {baseline_md5}",
        "# мутация | exit | вердикт | деталь",
    ]
    baseline_code, baseline_detail = run_test()
    lines.append(f"# baseline: exit={baseline_code} | {baseline_detail}")
    if baseline_code != 0 or "test result: ok" not in baseline_detail:
        lines.append("# baseline не зелёный — мутации не запускаются")
        print("baseline is not green:", baseline_detail)
        return 3
    killed = 0
    try:
        for mutation_id, description, old, new in MUTATIONS:
            text = baseline.decode("utf-8")
            count = text.count(old)
            if count != 1:
                lines.append(f"{mutation_id} | - | ERROR | подстрока встречается {count} раз: {description}")
                print(f"{mutation_id}: ERROR (совпадений {count})")
                continue
            SRC.write_text(text.replace(old, new), encoding="utf-8")
            code, detail = run_test()
            SRC.write_bytes(baseline)
            restored = md5(SRC) == baseline_md5
            verdict = (
                "KILLED"
                if code != 0 and "FAILED" in detail
                else ("SURVIVED" if code == 0 else "INCONCLUSIVE")
            )
            if verdict == "KILLED":
                killed += 1
            lines.append(f"{mutation_id} | exit={code} | {verdict} | {description} | {detail}")
            if not restored:
                lines.append(f"{mutation_id} | - | ERROR | restore по md5 не совпал")
                print(f"{mutation_id}: restore FAILED")
                return 2
            print(f"{mutation_id}: {verdict} (exit={code}) {description}")
    finally:
        SRC.write_bytes(baseline)

    total = len(MUTATIONS)
    restored_md5 = md5(SRC)
    lines.append(f"# итог: killed {killed}/{total}")
    lines.append(f"# restore md5 duration.rs: {restored_md5} ({'OK' if restored_md5 == baseline_md5 else 'FAIL'})")
    LOG.write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"killed {killed}/{total}; restored md5 {restored_md5}")
    return 0 if killed == total and restored_md5 == baseline_md5 else 1


if __name__ == "__main__":
    sys.exit(main())
