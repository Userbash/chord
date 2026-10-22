#!/usr/bin/env python3
"""Мутационная проверка контрактов W34b (расширенные hints `a{sv}`).

Каждая мутация — один точечный дефект в аддитивном коде P6-02d
(`src/notifications.rs`, поля `image_path`/`sound_name`/`transient`/`suppress_sound`).
Ожидание: targeted-тесты `notifications_contract` падают (мутация «убита»), причём без
harness-SKIP. Файл восстанавливается из снимка, совпадение md5 до/после обязательно.

Запуск (из корня репозитория):
    CARGO_TARGET_DIR=/tmp/opencode/target-w34b python3 waves/RW-34/run-mutations-w34b.py

stdout не перенаправляется в `mutations-w34b.log`: скрипт сам пишет туда журнал,
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
LOG_PATH = os.path.join(REPO_ROOT, "waves", "RW-34", "mutations-w34b.log")
WORKDIR = os.path.join(REPO_ROOT, "rust")
TESTS = ["notifications_contract"]

IMAGE_PATH_BLOCK = """        if let Some(image_path) = non_empty(&self.image_path) {
            entries.push(variant_entry(
                "image-path",
                Value::String(image_path.to_owned()),
            ));
        }
        if let Some(sound_name) = non_empty(&self.sound_name) {
            entries.push(variant_entry(
                "sound-name",
                Value::String(sound_name.to_owned()),
            ));
        }"""

SUPPRESS_BLOCK = """        if let Some(suppress_sound) = self.suppress_sound {
            entries.push(variant_entry("suppress-sound", Value::Bool(suppress_sound)));
        }
"""

TRANSIENT_BLOCK = """        if let Some(transient) = self.transient {
            entries.push(variant_entry("transient", Value::Bool(transient)));
        }"""

# (id, имя, файл, фрагмент-оригинал, фрагмент-мутация, что должен поймать)
MUTATIONS = [
    (
        "01",
        "image_path_key_underscore",
        NOTIFICATIONS,
        """        if let Some(image_path) = non_empty(&self.image_path) {
            entries.push(variant_entry(
                "image-path",""",
        """        if let Some(image_path) = non_empty(&self.image_path) {
            entries.push(variant_entry(
                "image_path",""",
        "e01/e02/e07: ключ `image_path` вместо `image-path` (ключ записи)",
    ),
    (
        "02",
        "transient_variant_byte",
        NOTIFICATIONS,
        'entries.push(variant_entry("transient", Value::Bool(transient)));',
        'entries.push(variant_entry(\n                "transient",\n                Value::Byte(u8::from(transient)),\n            ));',
        "e01/e03/e07: вариант transient `b` заменён на `y` (тип варианта)",
    ),
    (
        "03",
        "empty_image_path_not_skipped",
        NOTIFICATIONS,
        "if let Some(image_path) = non_empty(&self.image_path) {",
        "if let Some(image_path) = self.image_path.as_deref() {",
        "e04: пустой image-path уходит на шину (пропуск пустой строки)",
    ),
    (
        "04",
        "sound_name_before_image_path",
        NOTIFICATIONS,
        IMAGE_PATH_BLOCK,
        """        if let Some(sound_name) = non_empty(&self.sound_name) {
            entries.push(variant_entry(
                "sound-name",
                Value::String(sound_name.to_owned()),
            ));
        }
        if let Some(image_path) = non_empty(&self.image_path) {
            entries.push(variant_entry(
                "image-path",
                Value::String(image_path.to_owned()),
            ));
        }""",
        "e05/e07: порядок записей изменён (sound-name раньше image-path)",
    ),
    (
        "05",
        "suppress_sound_dropped",
        NOTIFICATIONS,
        SUPPRESS_BLOCK,
        "",
        "e01/e05/e07: запись `suppress-sound` потеряна (наличие)",
    ),
    (
        "06",
        "transient_false_skipped",
        NOTIFICATIONS,
        TRANSIENT_BLOCK,
        """        if let Some(transient) = self.transient {
            if transient {
                entries.push(variant_entry("transient", Value::Bool(transient)));
            }
        }""",
        "e03/e07: `Some(false)` пропущен вместо записи `b:0` (значение false)",
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
    env = dict(os.environ)
    env.setdefault("CARGO_TARGET_DIR", "/tmp/opencode/target-w34b")
    code = 0
    chunks = []
    for target in TESTS:
        completed = subprocess.run(
            [
                "cargo",
                "test",
                "--offline",
                "-p",
                "discord-platform",
                "--test",
                target,
                "--",
                "--nocapture",
            ],
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
    lines = ["# W34b — мутации расширенных hints `a{sv}`", ""]
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
        if source.count(original) != 1:
            failures.append("%s: фрагмент не найден однозначно" % mutation_id)
            lines.append(
                "## %s %s — ОШИБКА: фрагмент найден %d раз"
                % (mutation_id, name, source.count(original))
            )
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
