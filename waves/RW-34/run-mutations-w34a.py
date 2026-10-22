#!/usr/bin/env python3
"""Мутационная проверка контракта W34a (D-Bus abstract-адреса).

Каждая мутация — один точечный дефект в продуктовом коде `discord-platform`
(`src/dbus/mod.rs`): разбор адреса (`path=`/`abstract=`, список `;`, NUL, лимит 107 байт,
граница по `,`) и `connect_addr` для abstract-сокета. Ожидание: targeted-тесты
`dbus_contract` падают (`test result: FAILED`).

Правила вердикта:

- `KILLED` — только при ненулевом exit code и строке `test result: FAILED`;
- `INCONCLUSIVE` — если тесты прошли, но часть ушла в `SKIP:` (стенд недоступен);
- `SURVIVED` — тесты зелёные, `SKIP:` нет.

Baseline обязан быть зелёным с нулём `SKIP:`: abstract-стенд (приватный `dbus-daemon` +
libdbus-мок) в этом окружении поднимается по-настоящему. Исходники восстанавливаются из
снимка, совпадение md5 до/после обязательно; тестовый файл тоже под защитой md5, чужие
файлы не трогаются.

Запуск (из корня репозитория):
    python3 waves/RW-34/run-mutations-w34a.py

Код выхода: 0 — все мутации убиты и файлы восстановлены; 1 — выжившая мутация,
ошибка восстановления/поиска фрагмента или baseline не зелёный.
"""

import hashlib
import os
import re
import subprocess
import sys

REPO_ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
CRATE = os.path.join(REPO_ROOT, "rust", "crates", "discord-platform")
SOURCE = os.path.join(CRATE, "src", "dbus", "mod.rs")
TEST = os.path.join(CRATE, "tests", "dbus_contract.rs")
LOG_PATH = os.path.join(REPO_ROOT, "waves", "RW-34", "mutations-w34a.log")
WORKDIR = os.path.join(REPO_ROOT, "rust")
TARGET_DIR = os.environ.get("CARGO_TARGET_DIR", "/tmp/opencode/target-w34a")
TESTS = ["--test", "dbus_contract", "--", "--nocapture"]

# (id, имя, фрагмент-оригинал, фрагмент-мутация, что должен поймать)
MUTATIONS = [
    (
        "01",
        "path_nul_not_rejected",
        """        if value.is_empty() || value.contains('\\0') {""",
        """        if value.is_empty() {""",
        "a03: `unix:path=/tmp/\\0bus` разбирается в Path с NUL вместо None",
    ),
    (
        "02",
        "abstract_empty_name_allowed",
        "if bytes.is_empty() || bytes.len() > MAX_ABSTRACT_NAME_BYTES || bytes.contains(&0) {",
        "if bytes.len() > MAX_ABSTRACT_NAME_BYTES || bytes.contains(&0) {",
        "a03: `unix:abstract=` даёт пустое имя вместо None",
    ),
    (
        "03",
        "abstract_limit_off_by_one",
        "bytes.len() > MAX_ABSTRACT_NAME_BYTES",
        "bytes.len() >= MAX_ABSTRACT_NAME_BYTES",
        "a02: имя ровно в 107 байт (предел ядра) отвергается",
    ),
    (
        "04",
        "abstract_name_swallows_guid",
        """                if let Some(name) = field.strip_prefix("abstract=") {""",
        """                if let Some(name) = rest.strip_prefix("abstract=") {""",
        "a02/a04: имя abstract забирает `,guid=…` из адреса, напечатанного dbus-daemon",
    ),
    (
        "05",
        "address_list_first_entry_only",
        "for entry in value.split(';') {",
        "for entry in value.split(';').take(1) {",
        "a03: список `;` не перебирается — `tcp:…;unix:abstract=…` даёт None",
    ),
    (
        "06",
        "connect_addr_name_truncated",
        "let target = std::os::unix::net::SocketAddr::from_abstract_name(name)",
        "let target = std::os::unix::net::SocketAddr::from_abstract_name(&name[..1])",
        "a04/a05: connect_addr идёт в усечённое abstract-имя (connection refused)",
    ),
]

TEST_LINE = re.compile(r"^test (\S+) \.\.\. FAILED$", re.MULTILINE)
SUMMARY = re.compile(r"^test result: FAILED\..*$", re.MULTILINE)
RESULT_LINE = re.compile(r"^test result: (ok|FAILED)\.", re.MULTILINE)
SKIP_LINE = re.compile(r"^SKIP: ", re.MULTILINE)


def md5(path):
    with open(path, "rb") as handle:
        return hashlib.md5(handle.read()).hexdigest()


def run_tests():
    env = dict(os.environ)
    env["CARGO_TARGET_DIR"] = TARGET_DIR
    command = ["cargo", "test", "--offline", "-p", "discord-platform"] + TESTS
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
    print("\n".join(lines[-14:]))
    print("\nлог: %s" % LOG_PATH)


def main():
    lines = [
        "# W34a — мутации D-Bus abstract-адресов (`discord-platform/src/dbus/mod.rs`)",
        "",
        "targeted: cargo test --offline -p discord-platform --test dbus_contract -- --nocapture",
        "CARGO_TARGET_DIR=%s" % TARGET_DIR,
        "",
    ]
    failures = []
    sources = {}
    source_md5 = {}
    for path in (SOURCE, TEST):
        with open(path, encoding="utf-8") as handle:
            sources[path] = handle.read()
        source_md5[path] = md5(path)
        lines.append("md5 %s: %s" % (os.path.relpath(path, REPO_ROOT), source_md5[path]))
    lines.append("")

    baseline_code, baseline_output = run_tests()
    baseline_skips = len(SKIP_LINE.findall(baseline_output))
    baseline_result = RESULT_LINE.search(baseline_output)
    baseline_ok = baseline_code == 0 and (
        baseline_result is not None and baseline_result.group(1) == "ok"
    )
    lines.append(
        "baseline: exit=%d, SKIP=%d, test result=%s"
        % (
            baseline_code,
            baseline_skips,
            baseline_result.group(1) if baseline_result else "нет",
        )
    )
    if not baseline_ok:
        lines.append("ОСТАНОВ: базовый прогон не зелёный")
        lines.append(baseline_output[-4000:])
        write_log(lines)
        return 1
    if baseline_skips:
        lines.append(
            "ОСТАНОВ: baseline содержит SKIP (%d) — abstract-стенд не проверен, "
            "мутации были бы INCONCLUSIVE" % baseline_skips
        )
        write_log(lines)
        return 1
    lines.append("")

    for mutation_id, name, original, mutated, expectation in MUTATIONS:
        before_md5 = md5(SOURCE)
        with open(SOURCE, encoding="utf-8") as handle:
            source = handle.read()
        occurrences = source.count(original)
        if occurrences != 1:
            detail = (
                "фрагмент не найден"
                if occurrences == 0
                else "фрагмент не уникален (%d)" % occurrences
            )
            failures.append("%s: %s" % (mutation_id, detail))
            lines.append("## %s %s — ОШИБКА: %s" % (mutation_id, name, detail))
            continue

        lines.append("## %s %s (src/dbus/mod.rs)" % (mutation_id, name))
        lines.append("ждём: %s" % expectation)
        lines.append("md5 до: %s" % before_md5)

        try:
            with open(SOURCE, "w", encoding="utf-8") as handle:
                handle.write(source.replace(original, mutated, 1))
            code, output = run_tests()
            failing = sorted(set(TEST_LINE.findall(output)))
            skips = len(SKIP_LINE.findall(output))
            failed_result = "test result: FAILED" in output
            if failed_result and code != 0:
                verdict = "KILLED" if skips == 0 else "KILLED (с SKIP=%d)" % skips
            elif skips:
                verdict = "INCONCLUSIVE (SKIP=%d)" % skips
            else:
                verdict = "SURVIVED"
            lines.append("exit: %d, SKIP=%d — %s" % (code, skips, verdict))
            if failing:
                lines.append("упавшие тесты: %s" % ", ".join(failing))
            for match in SUMMARY.findall(output):
                lines.append("  %s" % match)
            if not verdict.startswith("KILLED"):
                tail = [line for line in output.splitlines() if line.startswith("error")]
                lines.append("вывод: %s" % " | ".join(tail[:3]))
                failures.append("%s %s: %s" % (mutation_id, name, verdict))
        finally:
            with open(SOURCE, "w", encoding="utf-8") as handle:
                handle.write(source)
            after_md5 = md5(SOURCE)
            lines.append(
                "md5 после restore: %s — %s"
                % (after_md5, "OK" if after_md5 == before_md5 else "РАСХОЖДЕНИЕ")
            )
            if after_md5 != before_md5:
                failures.append("%s: md5 после восстановления не совпал" % mutation_id)
        lines.append("")

    residual = [path for path, digest in source_md5.items() if md5(path) != digest]
    if residual:
        failures.append(
            "не восстановлены: %s" % ", ".join(os.path.basename(path) for path in residual)
        )

    lines.append("## Итог")
    lines.append("мутаций: %d" % len(MUTATIONS))
    if failures:
        lines.append("проблемы: %s" % "; ".join(failures))
        lines.append("вердикт: FAIL")
        write_log(lines)
        return 1
    lines.append("вердикт: OK — все мутации убиты, файлы восстановлены")
    write_log(lines)
    return 0


if __name__ == "__main__":
    sys.exit(main())
