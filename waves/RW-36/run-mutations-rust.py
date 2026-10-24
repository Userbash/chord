#!/usr/bin/env python3
"""RW-36 / QA-1: повторный прогон Rust-мутаций против усиленных тестов.

Проверяет, что red-first тесты ловят 7 ранее выживших мутантов (R1–R9).
Каждая мутация: бэкап → точечная замена → targeted cargo test → восстановление.
"""
import hashlib
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
MAIN = ROOT / "rust" / "gui-spike" / "src" / "main.rs"
MANIFEST = ROOT / "rust" / "gui-spike" / "Cargo.toml"

# (id, описание, точная подстрока, замена, тест, который обязан упасть)
MUTATIONS = [
    (
        "R1",
        "read_status_field: starts_with → contains",
        "status.lines().find(|line| line.starts_with(key))?",
        "status.lines().find(|line| line.contains(key))?",
        "field_matches_only_line_start",
    ),
    (
        "R2",
        "validate_ui_snapshot: as_u64 → as_i64 (отрицательные проходят)",
        ".and_then(serde_json::Value::as_u64)",
        ".and_then(serde_json::Value::as_i64)",
        "rejects_negative_values",
    ),
    (
        "R3",
        "validate_ui_snapshot: убрано обязательное поле settingsToggles",
        '    ("settingsToggles", 38),\n',
        "",
        "rejects_missing_each_required_field",
    ),
    (
        "R5",
        "validate_ui_snapshot: снята проверка типа (Some(1) для любого значения)",
        ".and_then(serde_json::Value::as_u64)\n            .ok_or_else",
        ".and_then(|_| Some(1u64))\n            .ok_or_else",
        "validates_ui_snapshot_contract",
    ),
    (
        "R6",
        "validate_ui_snapshot: ошибка парсинга JSON проглатывается",
        'serde_json::from_str(payload).map_err(|err| format!("некорректный UI-снимок: {err}"))?',
        "serde_json::from_str(payload).unwrap_or(serde_json::Value::Null)",
        "rejects_invalid_json",
    ),
    (
        "R7",
        "read_status_field: ошибка parse → Some(0) вместо None",
        "value.parse().ok()",
        "value.parse().ok().or(Some(0))",
        "non_numeric_value_is_none",
    ),
    (
        "R9",
        "parse_process_metrics: RSS/HWM перепутаны",
        'rss_kib: field("VmRSS:")?,\n        hwm_kib: field("VmHWM:")?,',
        'rss_kib: field("VmHWM:")?,\n        hwm_kib: field("VmRSS:")?,',
        "parse_process_metrics_maps_each_field",
    ),
    (
        "R10",
        "SNAPSHOT_FLOORS: убран порог memberRenderedRows",
        '    ("memberRenderedRows", 10),\n',
        "",
        "rejects_rendered_rows_below_floor",
    ),
    (
        "R11",
        "validate_bench_payload: снята проверка chat/members",
        '    for key in ["chat", "members"] {\n        if !value.get(key).is_some_and(serde_json::Value::is_object) {\n            return Err(format!("BENCH-снимок не содержит объект {key}"));\n        }\n    }\n',
        "",
        "rejects_bench_payload_without_chat_or_members",
    ),
]


def md5(path: Path) -> str:
    return hashlib.md5(path.read_bytes()).hexdigest()


def run_test(name: str) -> str:
    """Возвращает 'passed' | 'failed' | 'no-test' (0 тестов = фильтр не сработал)."""
    proc = subprocess.run(
        [
            "cargo",
            "test",
            "--offline",
            "--manifest-path",
            str(MANIFEST),
            "--bin",
            "discord-gui-spike",
            f"tests::{name}",
            "--",
            "--exact",
        ],
        capture_output=True,
        text=True,
    )
    combined = proc.stdout + proc.stderr
    if "0 passed" in combined and "0 failed" in combined:
        return "no-test"
    return "passed" if proc.returncode == 0 else "failed"


def main() -> int:
    original = MAIN.read_text()
    backup = MAIN.with_suffix(".rs.bak")
    shutil.copy2(MAIN, backup)
    before = md5(MAIN)
    caught = survived = invalid = 0
    print(f"# RW-36 Rust mutation run, md5_before={before}")
    try:
        for mid, desc, old, new, test in MUTATIONS:
            if old not in original:
                print(f"{mid} INVALID: подстрока не найдена — {desc}")
                invalid += 1
                continue
            MAIN.write_text(original.replace(old, new, 1))
            status = run_test(test)
            if status == "no-test":
                invalid += 1
                result = "no-test"
            elif status == "passed":
                survived += 1
                result = "survived"
            else:
                caught += 1
                result = "caught"
            print(f"{mid} {result}: {desc} (test={test})")
            # восстановление + обновление mtime, чтобы cargo не переиспользовал
            # артефакт мутированной сборки (грабли из первого прогона QA-1).
            MAIN.write_text(original)
    finally:
        MAIN.write_text(original)
        backup.unlink(missing_ok=True)
    after = md5(MAIN)
    print(
        f"# summary: caught={caught} survived={survived} invalid={invalid} "
        f"md5_after={after} restored={after == before}"
    )
    return 0 if after == before and survived == 0 and invalid == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
