#!/usr/bin/env python3
"""Мутационная проверка контракта W32a (spellcheck через FFI к `libhunspell-1.7`).

Каждая мутация — один точечный дефект в продуктовом коде `discord-platform`
(`src/spellcheck.rs` или `src/ffi_hunspell.rs`). Ожидание: targeted-тесты
`spellcheck_contract` падают (мутация «убита»). `KILLED` — только если прогон вернул
`test result: FAILED`, ненулевой exit code и **ноль** `SKIP:` (иначе `INCONCLUSIVE`).
Исходник восстанавливается из снимка, совпадение md5 до/после обязательно. Фикстуры
словаря не должны меняться: тесты работают с temp-копией, md5 фикстур сверяется.

Запуск (из корня репозитория):
    python3 waves/RW-32/run-mutations-w32a.py > /tmp/opencode/w32a-mutations.out

Код выхода: 0 — все мутации убиты и файлы восстановлены; 1 — выжившая мутация,
ошибка восстановления, не найден фрагмент, изменённая фикстура или baseline не зелёный.
"""

import hashlib
import os
import re
import subprocess
import sys

REPO_ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
CRATE = os.path.join(REPO_ROOT, "rust", "crates", "discord-platform")
SPELLCHECK = os.path.join(CRATE, "src", "spellcheck.rs")
FFI = os.path.join(CRATE, "src", "ffi_hunspell.rs")
FIXTURES = [
    os.path.join(REPO_ROOT, "fixtures", "hunspell", "test.aff"),
    os.path.join(REPO_ROOT, "fixtures", "hunspell", "test.dic"),
]
LOG_PATH = os.path.join(REPO_ROOT, "waves", "RW-32", "mutations-w32a.log")
WORKDIR = os.path.join(REPO_ROOT, "rust")
TARGET_DIR = os.environ.get("CARGO_TARGET_DIR", "/tmp/opencode/target-w32a")
TESTS = ["--test", "spellcheck_contract", "--", "--nocapture"]

# (id, имя, файл, фрагмент-оригинал, фрагмент-мутация, что должен поймать)
MUTATIONS = [
    (
        "01",
        "word_boundary_trailing_hyphen",
        SPELLCHECK,
        """            let word_char =
                current.is_alphabetic() || (current == '\\'' || current == '-') && next_is_letter;""",
        """            let word_char = current.is_alphabetic() || current == '\\'' || current == '-';""",
        "h07: дефис в конце слова приклеивается к слову",
    ),
    (
        "02",
        "url_skip_removed",
        SPELLCHECK,
        '        if chunk.contains("://") || chunk.contains(\'@\') {',
        "        if chunk.contains('@') {",
        "h08: URL разбирается на слова (https/example/com/…)",
    ),
    (
        "03",
        "suggest_limit_zero",
        SPELLCHECK,
        "const MAX_SUGGESTIONS: usize = 10;",
        "const MAX_SUGGESTIONS: usize = 0;",
        "h04: suggest не возвращает вариантов, hello не находится",
    ),
    (
        "04",
        "add_ignored",
        FFI,
        "        unsafe { hunspell_add(self.raw.as_ptr(), c_word.as_ptr()) == 0 }",
        "        true",
        "h05: add сообщает успех, но слово не попадает в словарь",
    ),
    (
        "05",
        "drop_without_destroy",
        FFI,
        "        unsafe { hunspell_destroy(self.raw.as_ptr()) };",
        "        // мутация: hunspell_destroy не вызывается",
        "h11: 100 циклов create/drop утекают память (RSS-порог)",
    ),
    (
        "06",
        "nul_guard_removed",
        SPELLCHECK,
        "    if text.contains('\\0') {",
        "    if false && text.contains('\\0') {",
        "h12: NUL в пути больше не даёт SpellError::Nul",
    ),
    (
        "07",
        "mention_skip_removed",
        SPELLCHECK,
        "        if character == '<' && is_mention_start(&positions, index) {",
        "        if false && character == '<' && is_mention_start(&positions, index) {",
        "h08: <@alice>/<#general> дают слова alice/general",
    ),
    (
        "08",
        "spell_always_true",
        FFI,
        "        unsafe { hunspell_spell(self.raw.as_ptr(), c_word.as_ptr()) != 0 }",
        """        unsafe {
            let _ = hunspell_spell(self.raw.as_ptr(), c_word.as_ptr());
            true
        }""",
        "h03: check принимает любое слово (helo/приветъ)",
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
    print("\n".join(lines[-10:]))
    print("\nлог: %s" % LOG_PATH)


def main():
    lines = [
        "# W32a — мутации spellcheck (`discord-platform/src/spellcheck.rs`, `src/ffi_hunspell.rs`)",
        "",
        "targeted: cargo test --offline -p discord-platform --test spellcheck_contract -- --nocapture",
        "CARGO_TARGET_DIR=%s" % TARGET_DIR,
        "",
    ]
    failures = []
    sources = {}
    source_md5 = {}
    for path in (SPELLCHECK, FFI):
        with open(path, encoding="utf-8") as handle:
            sources[path] = handle.read()
        source_md5[path] = md5(path)
    fixture_md5 = {path: md5(path) for path in FIXTURES}
    lines.append("md5 spellcheck.rs: %s" % source_md5[SPELLCHECK])
    lines.append("md5 ffi_hunspell.rs: %s" % source_md5[FFI])
    for path, digest in fixture_md5.items():
        lines.append("md5 %s: %s" % (os.path.relpath(path, REPO_ROOT), digest))
    lines.append("")

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

    for path, digest in fixture_md5.items():
        if md5(path) != digest:
            failures.append("фикстура изменена тестом: %s" % os.path.relpath(path, REPO_ROOT))
    residual = [path for path, digest in source_md5.items() if md5(path) != digest]
    if residual:
        failures.append("не восстановлены: %s" % ", ".join(os.path.basename(p) for p in residual))

    lines.append("## Итог")
    lines.append("мутаций: %d" % len(MUTATIONS))
    if failures:
        lines.append("проблемы: %s" % "; ".join(failures))
        write_log(lines)
        return 1
    lines.append("все мутации убиты, все файлы и фикстуры восстановлены")
    write_log(lines)
    return 0


if __name__ == "__main__":
    sys.exit(main())
