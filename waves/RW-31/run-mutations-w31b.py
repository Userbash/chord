#!/usr/bin/env python3
"""Мутационная проверка контракта W31b (SQLite-store через FFI к `libsqlite3`).

Каждая мутация — один точечный дефект в продуктовом коде `discord-store`
(`src/lib.rs`). Ожидание: targeted-тесты `store_contract` падают (мутация «убита»).
`KILLED` — только если прогон вернул `test result: FAILED`, ненулевой exit code и **ноль**
`SKIP:` (иначе вердикт `INCONCLUSIVE`). Исходник восстанавливается из снимка, совпадение md5
до/после обязательно.

Запуск (из корня репозитория):
    CARGO_TARGET_DIR=/tmp/opencode/target-w31b python3 waves/RW-31/run-mutations-w31b.py

Код выхода: 0 — все мутации убиты и файлы восстановлены; 1 — выжившая мутация,
ошибка восстановления, не найден фрагмент или baseline не зелёный.
"""

import hashlib
import os
import re
import subprocess
import sys

REPO_ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
LIB = os.path.join(REPO_ROOT, "rust", "crates", "discord-store", "src", "lib.rs")
LOG_PATH = os.path.join(REPO_ROOT, "waves", "RW-31", "mutations-w31b.log")
WORKDIR = os.path.join(REPO_ROOT, "rust")
TARGET_DIR = os.environ.get("CARGO_TARGET_DIR", "/tmp/opencode/target-w31b")
TESTS = ["--test", "store_contract", "--", "--nocapture"]

# (id, имя, фрагмент-оригинал, фрагмент-мутация, все вхождения?, что должен поймать)
MUTATIONS = [
    (
        "01",
        "foreign_keys_off",
        'self.db.exec("PRAGMA foreign_keys = ON")?;',
        'self.db.exec("PRAGMA foreign_keys = OFF")?;',
        False,
        "s06: FK выключен, сообщение без канала молча сохраняется",
    ),
    (
        "02",
        "order_by_asc",
        "ORDER BY timestamp DESC, id DESC",
        "ORDER BY timestamp ASC, id ASC",
        True,
        "s04/s05: обратный порядок истории и пагинации",
    ),
    (
        "03",
        "limit_clamp_removed",
        "let limit = limit.min(MAX_MESSAGES_LIMIT);",
        "let limit = limit;",
        False,
        "s04: limit > 1000 не клампится до 1000",
    ),
    (
        "04",
        "upsert_to_insert",
        """ON CONFLICT(id) DO UPDATE SET
    channel_id = excluded.channel_id,
    author_id = excluded.author_id,
    content = excluded.content,
    timestamp = excluded.timestamp,
    edited_timestamp = excluded.edited_timestamp,
    flags = excluded.flags,
    deleted = excluded.deleted
""",
        "",
        False,
        "s03: повторный put_message падает на PK вместо обновления",
    ),
    (
        "05",
        "soft_delete_flag_cleared",
        '"UPDATE messages SET deleted = 1 WHERE id = ?1"',
        '"UPDATE messages SET deleted = 0 WHERE id = ?1"',
        False,
        "s07: soft delete не скрывает сообщение из messages",
    ),
    (
        "06",
        "bind_to_concatenation",
        """                let mut statement = self.db.prepare(SQL_MESSAGES_BEFORE)?;
                statement.bind_text(1, channel_id)?;
                statement.bind_i64(2, timestamp)?;
                statement.bind_text(3, id)?;
                statement.bind_i64(4, limit)?;""",
        """                let sql = format!(
                    "SELECT id, channel_id, author_id, content, timestamp, edited_timestamp, flags, deleted \\
                     FROM messages WHERE channel_id = '{channel_id}' AND deleted = 0 \\
                     AND (timestamp < {timestamp} OR (timestamp = {timestamp} AND id < '{id}')) \\
                     ORDER BY timestamp DESC, id DESC LIMIT {limit}"
                );
                let mut statement = self.db.prepare(&sql)?;""",
        False,
        "s10: channel_id/id подставляются в SQL конкатенацией, враждебный id ломает запрос",
    ),
    (
        "07",
        "user_version_two",
        'self.db.exec("PRAGMA user_version = 1")?;',
        'self.db.exec("PRAGMA user_version = 2")?;',
        False,
        "s01/s12: версия схемы не 1 (повторный migrate упирается в Migration/оракул)",
    ),
]

TEST_LINE = re.compile(r"^test (\S+) \.\.\. FAILED$", re.MULTILINE)
SUMMARY = re.compile(r"^test result: FAILED\..*$", re.MULTILINE)


def md5(path):
    with open(path, "rb") as handle:
        return hashlib.md5(handle.read()).hexdigest()


def run_tests():
    env = dict(os.environ)
    env["CARGO_TARGET_DIR"] = TARGET_DIR
    command = ["cargo", "test", "--offline", "-p", "discord-store"] + TESTS
    completed = subprocess.run(
        command,
        cwd=WORKDIR,
        env=env,
        capture_output=True,
        text=True,
        check=False,
    )
    return completed.returncode, completed.stdout + completed.stderr


def write_log(lines):
    os.makedirs(os.path.dirname(LOG_PATH), exist_ok=True)
    with open(LOG_PATH, "w", encoding="utf-8") as handle:
        handle.write("\n".join(lines) + "\n")
    print("\n".join(lines[-8:]))
    print("\nлог: %s" % LOG_PATH)


def main():
    lines = [
        "# W31b — мутации SQLite-store (`discord-store/src/lib.rs`)",
        "",
        "targeted: cargo test --offline -p discord-store --test store_contract -- --nocapture",
        "CARGO_TARGET_DIR=%s" % TARGET_DIR,
        "",
    ]
    failures = []

    baseline_code, baseline_output = run_tests()
    baseline_skips = baseline_output.count("SKIP:")
    baseline_ok = "test result: ok" in baseline_output
    lines.append(
        "baseline: exit=%d, SKIP=%d, test result ok=%s"
        % (baseline_code, baseline_skips, "да" if baseline_ok else "нет")
    )
    if baseline_code != 0 or not baseline_ok or baseline_skips:
        lines.append("ОСТАНОВ: базовый прогон не зелёный или содержит SKIP")
        lines.append(baseline_output[-4000:])
        write_log(lines)
        return 1
    lines.append("")

    for mutation_id, name, original, mutated, all_occurrences, expectation in MUTATIONS:
        before_md5 = md5(LIB)
        with open(LIB, encoding="utf-8") as handle:
            source = handle.read()
        occurrences = source.count(original)
        if occurrences == 0 or (occurrences > 1 and not all_occurrences):
            detail = "фрагмент не найден" if occurrences == 0 else "фрагмент не уникален (%d)" % occurrences
            failures.append("%s: %s" % (mutation_id, detail))
            lines.append("## %s %s — ОШИБКА: %s" % (mutation_id, name, detail))
            continue

        lines.append("## %s %s" % (mutation_id, name))
        lines.append("ждём: %s" % expectation)
        lines.append("вхождений: %d" % occurrences)
        lines.append("md5 до: %s" % before_md5)

        try:
            replace_count = -1 if all_occurrences else 1
            with open(LIB, "w", encoding="utf-8") as handle:
                handle.write(source.replace(original, mutated, replace_count))
            code, output = run_tests()
            failing = sorted(set(TEST_LINE.findall(output)))
            skips = output.count("SKIP:")
            failed_result = "test result: FAILED" in output
            killed = code != 0 and failed_result and skips == 0
            if skips:
                verdict = "INCONCLUSIVE (SKIP)"
            elif killed:
                verdict = "KILLED"
            else:
                verdict = "SURVIVED"
            lines.append("exit: %d, SKIP=%d — %s" % (code, skips, verdict))
            if failing:
                lines.append("упавшие тесты: %s" % ", ".join(failing))
            for match in SUMMARY.findall(output):
                lines.append("  %s" % match)
            if not killed:
                tail = [line for line in output.splitlines() if line.startswith("error")]
                lines.append("вывод: %s" % " | ".join(tail[:3]))
                failures.append("%s %s: %s" % (mutation_id, name, verdict))
        finally:
            with open(LIB, "w", encoding="utf-8") as handle:
                handle.write(source)
            after_md5 = md5(LIB)
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


if __name__ == "__main__":
    sys.exit(main())
