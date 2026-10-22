#!/usr/bin/env python3
"""Мутационная проверка контракта W33a (keyring через FFI к `libsecret 0.21`).

Каждая мутация — один точечный дефект в продуктовом коде `discord-platform`
(`src/keyring.rs` или `src/ffi_secret.rs`). Ожидание: targeted-тесты `keyring_contract`
падают (`test result: FAILED`). `KILLED` — только при ненулевом exit code и `FAILED`;
если live-кейсы ушли в `SKIP` (нет сервиса/флага), вердикт `INCONCLUSIVE`, а не `KILLED`.

Прогоны идут с `DISCORD_LAB_KEYRING_LIVE=1` (реальный ksecretd + `secret-tool`), поэтому
baseline обязан быть зелёным с нулём `SKIP:`. Исходники восстанавливаются из снимка,
совпадение md5 до/после обязательно; чужие файлы не трогаются.

Запуск (из корня репозитория):
    python3 waves/RW-33/run-mutations-w33a.py

Код выхода: 0 — все мутации убиты и файлы восстановлены; 1 — выжившая мутация,
ошибка восстановления, не найден фрагмент или baseline не зелёный.
"""

import hashlib
import os
import re
import subprocess
import sys

REPO_ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
CRATE = os.path.join(REPO_ROOT, "rust", "crates", "discord-platform")
KEYRING = os.path.join(CRATE, "src", "keyring.rs")
FFI = os.path.join(CRATE, "src", "ffi_secret.rs")
LIB = os.path.join(CRATE, "src", "lib.rs")
TEST = os.path.join(CRATE, "tests", "keyring_contract.rs")
LOG_PATH = os.path.join(REPO_ROOT, "waves", "RW-33", "mutations-w33a.log")
WORKDIR = os.path.join(REPO_ROOT, "rust")
TARGET_DIR = os.environ.get("CARGO_TARGET_DIR", "/tmp/opencode/target-w33a")
TESTS = ["--test", "keyring_contract", "--", "--nocapture"]

# (id, имя, файл, фрагмент-оригинал, фрагмент-мутация, что должен поймать)
MUTATIONS = [
    (
        "01",
        "schema_attribute_renamed",
        KEYRING,
        'const ATTRIBUTE_KEY: &str = "key";',
        'const ATTRIBUTE_KEY: &str = "ke";',
        "k07/k08: CLI secret-tool ищет записи по атрибуту `key`, а мы пишем `ke`",
    ),
    (
        "02",
        "store_not_persisted",
        KEYRING,
        """        self.service
            .store(&self.schema, ATTRIBUTE_KEY, key, LABEL, secret)
            .map_err(KeyringError::from)""",
        "        Ok(())",
        "k02/k05/k07: store сообщает успех, но запись не создаётся",
    ),
    (
        "03",
        "clear_always_true",
        KEYRING,
        """        self.service
            .clear(&self.schema, ATTRIBUTE_KEY, key)
            .map_err(KeyringError::from)""",
        "        Ok(true)",
        "k11: повторный clear отсутствующей записи возвращает true",
    ),
    (
        "04",
        "lookup_null_is_error",
        FFI,
        """        let Some(value) = NonNull::new(value) else {
            return Ok(None);
        };
        let mut length: usize = 0;""",
        """        let Some(value) = NonNull::new(value) else {
            return Err(FfiError::new(0, "мутация: NULL вместо Ok(None)"));
        };
        let mut length: usize = 0;""",
        "k03: отсутствие записи становится ошибкой, а не Ok(None)",
    ),
    (
        "05",
        "secret_length_truncated",
        FFI,
        "let bytes = unsafe { slice::from_raw_parts(data.cast::<u8>(), length) }.to_vec();",
        "let bytes = unsafe { slice::from_raw_parts(data.cast::<u8>(), length.saturating_sub(1)) }.to_vec();",
        "k02: значение секрета обрезано на 1 байт",
    ),
    (
        "06",
        "debug_prints_pointer",
        KEYRING,
        """        formatter
            .debug_struct("Keyring")
            .field("schema", &SCHEMA_NAME)
            .field("label", &LABEL)
            .finish_non_exhaustive()""",
        """        formatter
            .debug_struct("Keyring")
            .field("service", &format!("{:p}", &self.service))
            .field("label", &LABEL)
            .finish_non_exhaustive()""",
        "k12: ручная редакция Debug заменена печатью адреса внутреннего объекта",
    ),
    (
        "07",
        "empty_key_allowed",
        KEYRING,
        "    if key.is_empty() || key.contains('\\0') {",
        "    if key.contains('\\0') {",
        "k09: пустой ключ больше не отвергается (InvalidKey)",
    ),
]

TEST_LINE = re.compile(r"^test (\S+) \.\.\. FAILED$", re.MULTILINE)
SUMMARY = re.compile(r"^test result: FAILED\..*$", re.MULTILINE)
RESULT_LINE = re.compile(r"^test result: (ok|FAILED)\.", re.MULTILINE)


def md5(path):
    with open(path, "rb") as handle:
        return hashlib.md5(handle.read()).hexdigest()


def run_tests():
    env = dict(os.environ)
    env["CARGO_TARGET_DIR"] = TARGET_DIR
    env["DISCORD_LAB_KEYRING_LIVE"] = "1"
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


def cleanup_synthetic():
    """Убирает синтетические записи схемы после мутанта (CLI-оракул, не наш код).

    Мутация `clear_always_true` делает `clear` (в том числе в `Drop` тестовой записи)
    no-op, поэтому записи прогона удаляются независимым `secret-tool`.
    """
    try:
        completed = subprocess.run(
            ["secret-tool", "clear", "xdg:schema", "com.discordlab.Token"],
            capture_output=True,
            text=True,
            check=False,
            timeout=60,
        )
    except (OSError, subprocess.SubprocessError) as error:
        return "пропущено (%s)" % error
    # secret-tool exit 1 = «совпадений нет» (записи уже удалены) — это нормальная очистка.
    if completed.returncode in (0, 1):
        return "ok (exit=%d)" % completed.returncode
    return "код %d" % completed.returncode


def write_log(lines):
    os.makedirs(os.path.dirname(LOG_PATH), exist_ok=True)
    with open(LOG_PATH, "w", encoding="utf-8") as handle:
        handle.write("\n".join(lines) + "\n")
    print("\n".join(lines[-12:]))
    print("\nлог: %s" % LOG_PATH)


def main():
    lines = [
        "# W33a — мутации keyring (`discord-platform/src/keyring.rs`, `src/ffi_secret.rs`)",
        "",
        "targeted: cargo test --offline -p discord-platform --test keyring_contract -- --nocapture",
        "live: DISCORD_LAB_KEYRING_LIVE=1 (реальный ksecretd + secret-tool)",
        "CARGO_TARGET_DIR=%s" % TARGET_DIR,
        "",
    ]
    failures = []
    sources = {}
    source_md5 = {}
    protected = [KEYRING, FFI, LIB, TEST]
    for path in protected:
        with open(path, encoding="utf-8") as handle:
            sources[path] = handle.read()
        source_md5[path] = md5(path)
    for path in (KEYRING, FFI, LIB, TEST):
        lines.append("md5 %s: %s" % (os.path.relpath(path, REPO_ROOT), source_md5[path]))
    lines.append("")

    baseline_code, baseline_output = run_tests()
    baseline_skips = baseline_output.count("SKIP:")
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
            "ВНИМАНИЕ: baseline содержит SKIP (%d) — live-кейсы не проверены, мутации могут быть INCONCLUSIVE"
            % baseline_skips
        )
    lines.append("очистка синтетических записей: %s" % cleanup_synthetic())
    lines.append("")

    for mutation_id, name, path, original, mutated, expectation in MUTATIONS:
        before_md5 = md5(path)
        with open(path, encoding="utf-8") as handle:
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

        lines.append("## %s %s (%s)" % (mutation_id, name, os.path.basename(path)))
        lines.append("ждём: %s" % expectation)
        lines.append("md5 до: %s" % before_md5)

        try:
            with open(path, "w", encoding="utf-8") as handle:
                handle.write(source.replace(original, mutated, 1))
            code, output = run_tests()
            failing = sorted(set(TEST_LINE.findall(output)))
            skips = output.count("SKIP:")
            failed_result = "test result: FAILED" in output
            if failed_result and code != 0 and skips == 0:
                verdict = "KILLED"
            elif failed_result and code != 0:
                verdict = "KILLED (с SKIP=%d)" % skips
            elif skips:
                verdict = "INCONCLUSIVE (SKIP=%d)" % skips
            else:
                verdict = "SURVIVED"
            lines.append("exit: %d, SKIP=%d — %s" % (code, skips, verdict))
            if failing:
                lines.append("упавшие тесты: %s" % ", ".join(failing))
            for match in SUMMARY.findall(output):
                lines.append("  %s" % match)
            if verdict != "KILLED" and not verdict.startswith("KILLED "):
                tail = [line for line in output.splitlines() if line.startswith("error")]
                lines.append("вывод: %s" % " | ".join(tail[:3]))
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
            lines.append("очистка синтетических записей: %s" % cleanup_synthetic())
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
