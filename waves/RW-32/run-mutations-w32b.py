#!/usr/bin/env python3
"""Мутационная проверка контрактов W32b (hints `a{sv}` для уведомлений).

Каждая мутация — один точечный дефект в `src/notifications.rs` (аддитивный код P6-02b).
Ожидание: targeted-тесты `notifications_contract` и `dbus_contract` падают (мутация
«убита»), причём без harness-SKIP. Файл восстанавливается из снимка, совпадение md5
до/после обязательно.

Запуск (из корня репозитория):
    CARGO_TARGET_DIR=/tmp/opencode/target-w32b python3 waves/RW-32/run-mutations-w32b.py

stdout не перенаправляется в `mutations-w32b.log`: скрипт сам пишет туда журнал,
а в stdout печатает только итог.

Код выхода: 0 — все мутации убиты и файлы восстановлены; 1 — есть выжившая мутация,
ошибка восстановления, harness-SKIP в baseline или сбой запуска тестов.
"""

import hashlib
import os
import re
import subprocess
import sys

REPO_ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
CRATE = os.path.join(REPO_ROOT, "rust", "crates", "discord-platform")
NOTIFICATIONS = os.path.join(CRATE, "src", "notifications.rs")
LOG_PATH = os.path.join(REPO_ROOT, "waves", "RW-32", "mutations-w32b.log")
WORKDIR = os.path.join(REPO_ROOT, "rust")
TESTS = ["notifications_contract", "dbus_contract"]

# (id, имя, файл, фрагмент-оригинал, фрагмент-мутация, что должен поймать)
MUTATIONS = [
    (
        "01",
        "urgency_variant_u32",
        NOTIFICATIONS,
        'entries.push(variant_entry("urgency", Value::Byte(urgency.code())));',
        'entries.push(variant_entry(\n                "urgency",\n                Value::U32(u32::from(urgency.code())),\n            ));',
        "q02/q05/q08: вариант urgency `y` заменён на `u` (тип варианта)",
    ),
    (
        "02",
        "urgency_key_typo",
        NOTIFICATIONS,
        'entries.push(variant_entry("urgency", Value::Byte(urgency.code())));',
        'entries.push(variant_entry("urgentcy", Value::Byte(urgency.code())));',
        "q02/q05: ключ `urgency` с опечаткой (ключ записи)",
    ),
    (
        "03",
        "empty_strings_not_skipped",
        NOTIFICATIONS,
        """    match value.as_deref() {
        Some(text) if !text.is_empty() => Some(text),
        _ => None,
    }""",
        """    value.as_deref()""",
        "q06: пустые category/desktop-entry уходят на шину (пропуск пустой строки)",
    ),
    (
        "04",
        "desktop_entry_key_underscore",
        NOTIFICATIONS,
        '"desktop-entry",',
        '"desktop_entry",',
        "q04/q05: ключ `desktop_entry` вместо `desktop-entry`",
    ),
    (
        "05",
        "resident_dropped",
        NOTIFICATIONS,
        """        if let Some(resident) = self.resident {
            entries.push(variant_entry("resident", Value::Bool(resident)));
        }
""",
        "",
        "q04/q05: запись `resident` потеряна (наличие)",
    ),
    (
        "06",
        "desktop_entry_before_category",
        NOTIFICATIONS,
        """        if let Some(category) = non_empty(&self.category) {
            entries.push(variant_entry(
                "category",
                Value::String(category.to_owned()),
            ));
        }
        if let Some(desktop_entry) = non_empty(&self.desktop_entry) {
            entries.push(variant_entry(
                "desktop-entry",
                Value::String(desktop_entry.to_owned()),
            ));
        }""",
        """        if let Some(desktop_entry) = non_empty(&self.desktop_entry) {
            entries.push(variant_entry(
                "desktop-entry",
                Value::String(desktop_entry.to_owned()),
            ));
        }
        if let Some(category) = non_empty(&self.category) {
            entries.push(variant_entry(
                "category",
                Value::String(category.to_owned()),
            ));
        }""",
        "q05/q08: порядок записей изменён (desktop-entry раньше category)",
    ),
]

TEST_LINE = re.compile(r"^test (\S+) \.\.\. FAILED$", re.MULTILINE)
SUMMARY = re.compile(r"^test result: FAILED\..*$", re.MULTILINE)


def md5(path):
    with open(path, "rb") as handle:
        return hashlib.md5(handle.read()).hexdigest()


def count_skips(output: str) -> int:
    """SKIP-строки harness без учёта намеренных live-skip (n16, флаг выключен)."""
    return sum(
        1
        for line in output.splitlines()
        if "SKIP:" in line and "live" not in line.lower()
    )


def run_tests():
    """Прогоняет оба test-таргета по отдельности.

    `cargo test` с несколькими `--test` останавливается на первом упавшем бинарнике,
    поэтому мутация, убитая mock-оракулом (`notifications_contract`), не должна скрывать
    проверку байтов (`dbus_contract`) — и наоборот.
    """
    env = dict(os.environ)
    env.setdefault("CARGO_TARGET_DIR", "/tmp/opencode/target-w32b")
    code = 0
    chunks = []
    for target in TESTS:
        command = [
            "cargo",
            "test",
            "--offline",
            "-p",
            "discord-platform",
            "--test",
            target,
            "--",
            "--nocapture",
        ]
        completed = subprocess.run(
            command,
            cwd=WORKDIR,
            env=env,
            capture_output=True,
            text=True,
            check=False,
        )
        if completed.returncode != 0:
            code = completed.returncode
        chunks.append("### %s\n%s%s" % (target, completed.stdout, completed.stderr))
    return code, "\n".join(chunks)


def main():
    lines = ["# W32b — мутации hints `a{sv}`", ""]
    failures = []

    baseline_code, baseline_output = run_tests()
    baseline_skips = count_skips(baseline_output)
    lines.append("baseline: exit=%d, SKIP=%d" % (baseline_code, baseline_skips))
    if baseline_code != 0 or "test result: ok" not in baseline_output or baseline_skips:
        lines.append("ОСТАНОВ: базовый прогон не зелёный или содержит SKIP")
        lines.append(baseline_output[-4000:])
        write_log(lines)
        return 1
    lines.append("")

    for mutation_id, name, path, original, mutated, expectation in MUTATIONS:
        before_md5 = md5(path)
        with open(path, encoding="utf-8") as handle:
            source = handle.read()
        if original not in source:
            failures.append("%s: фрагмент не найден" % mutation_id)
            lines.append("## %s %s — ОШИБКА: фрагмент не найден" % (mutation_id, name))
            continue

        lines.append("## %s %s" % (mutation_id, name))
        lines.append("ждём: %s" % expectation)
        lines.append("md5 до: %s" % before_md5)

        try:
            with open(path, "w", encoding="utf-8") as handle:
                handle.write(source.replace(original, mutated, 1))
            code, output = run_tests()
            failing = sorted(set(TEST_LINE.findall(output)))
            skips = count_skips(output)
            killed = code != 0 and "test result: FAILED" in output and skips == 0
            verdict = (
                "KILLED"
                if killed
                else ("INCONCLUSIVE (SKIP)" if skips else "SURVIVED")
            )
            lines.append("exit: %d, SKIP=%d — %s" % (code, skips, verdict))
            if failing:
                lines.append("упавшие тесты: %s" % ", ".join(failing))
            for match in SUMMARY.findall(output):
                lines.append("  %s" % match)
            if not failing:
                tail = [line for line in output.splitlines() if line.startswith("error")]
                lines.append("вывод: %s" % " | ".join(tail[:3]))
            if not killed:
                failures.append("%s %s: %s" % (mutation_id, name, verdict))
        finally:
            with open(path, "w", encoding="utf-8") as handle:
                handle.write(source)
            after_md5 = md5(path)
            lines.append(
                "md5 после restore: %s — %s"
                % (after_md5, "OK" if after_md5 == before_md5 else "РАСХОЖДЕНИЕ")
            )
            if after_md5 != before_md5:
                failures.append("%s: md5 после восстановления не совпал" % mutation_id)
        lines.append("")

    lines.append("## Итог")
    lines.append("мутаций: %d" % len(MUTATIONS))
    if failures:
        lines.append("проблемы: %s" % "; ".join(failures))
        write_log(lines)
        return 1
    lines.append("все мутации убиты, все файлы восстановлены")
    write_log(lines)
    return 0


def write_log(lines):
    os.makedirs(os.path.dirname(LOG_PATH), exist_ok=True)
    with open(LOG_PATH, "w", encoding="utf-8") as handle:
        handle.write("\n".join(lines) + "\n")
    print("\n".join(lines[-8:]))
    print("\nлог: %s" % LOG_PATH)


if __name__ == "__main__":
    sys.exit(main())
