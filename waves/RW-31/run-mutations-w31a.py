#!/usr/bin/env python3
"""Мутационная проверка контракта W31a (питание и idle).

Каждая мутация — один точечный дефект в `discord-platform/src/power.rs`. Ожидание:
targeted-тесты `power_contract` падают (мутация «убита»). Файл восстанавливается из
снимка, совпадение md5 до/после обязательно.

Запуск (из корня репозитория):
    CARGO_TARGET_DIR=/tmp/opencode/target-w31a python3 waves/RW-31/run-mutations-w31a.py

Код выхода: 0 — все мутации убиты и файл восстановлен; 1 — выжившая мутация,
ошибка восстановления или сбой запуска тестов.
"""

import hashlib
import os
import re
import subprocess
import sys

REPO_ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
CRATE = os.path.join(REPO_ROOT, "rust", "crates", "discord-platform")
POWER = os.path.join(CRATE, "src", "power.rs")
LOG_PATH = os.path.join(REPO_ROOT, "waves", "RW-31", "mutations-w31a.log")
WORKDIR = os.path.join(REPO_ROOT, "rust")
TESTS = ["--test", "power_contract"]

# (id, имя, фрагмент-оригинал, фрагмент-мутация, что должен поймать)
MUTATIONS = [
    (
        "01",
        "policy_idle_boundary_off_by_one",
        """        } else if idle_secs >= self.idle_after_secs {""",
        """        } else if idle_secs > self.idle_after_secs {""",
        "p01: граница idle_after (600) съезжает на 601",
    ),
    (
        "02",
        "sleep_flag_reversed",
        """        [Value::Bool(true)] => Some(PowerEvent::GoingToSleep),
        [Value::Bool(false)] => Some(PowerEvent::WakingUp),""",
        """        [Value::Bool(true)] => Some(PowerEvent::WakingUp),
        [Value::Bool(false)] => Some(PowerEvent::GoingToSleep),""",
        "p06: true/false у PrepareForSleep перепутаны",
    ),
    (
        "03",
        "timeout_becomes_error",
        """                Err(DbusError::Timeout) => return Ok(None),""",
        """                Err(DbusError::Timeout) => return Err(DbusError::Timeout),""",
        "p08: таймаут next_event становится ошибкой вместо Ok(None)",
    ),
    (
        "04",
        "unknown_method_is_fatal",
        """fn is_unknown_member(name: &str) -> bool {
    matches!(
        name,
        "org.freedesktop.DBus.Error.UnknownMethod" | "org.freedesktop.DBus.Error.UnknownInterface"
    )
}""",
        """fn is_unknown_member(name: &str) -> bool {
    let _ = name;
    false
}""",
        "p05: UnknownMethod/UnknownInterface больше не дают Ok(None)",
    ),
    (
        "05",
        "cookie_off_by_one",
        """            Some(Value::U32(cookie)) => Ok(cookie),""",
        """            Some(Value::U32(cookie)) => Ok(cookie.wrapping_add(1)),""",
        "p02/p03: cookie ингибитора сдвинут на 1",
    ),
    (
        "06",
        "signal_filter_accepts_garbage",
        """        [Value::Bool(true)] => Some(PowerEvent::GoingToSleep),
        [Value::Bool(false)] => Some(PowerEvent::WakingUp),
        _ => None,""",
        """        [Value::Bool(true)] => Some(PowerEvent::GoingToSleep),
        [Value::Bool(false)] => Some(PowerEvent::WakingUp),
        _ => Some(PowerEvent::GoingToSleep),""",
        "p07: PrepareForSleep с чужим телом принимается за событие",
    ),
    (
        "07",
        "uninhibit_fallback_removed",
        """            Err(DbusError::Remote { name, .. }) if is_unknown_member(&name) => {
                self.call(UNINHIBIT_FALLBACK_METHOD, "u", &body)?;
                Ok(())
            }""",
        """            Err(DbusError::Remote { name, .. }) if is_unknown_member(&name) => {
                Err(DbusError::Remote {
                    name,
                    message: "fallback disabled".to_owned(),
                })
            }""",
        "p03: нет fallback UnInhibit → Uninhibit",
    ),
]

TEST_LINE = re.compile(r"^test (\S+) \.\.\. FAILED$", re.MULTILINE)
SUMMARY = re.compile(r"^test result: FAILED\..*$", re.MULTILINE)


def md5(path):
    with open(path, "rb") as handle:
        return hashlib.md5(handle.read()).hexdigest()


def count_skips(output: str) -> int:
    """SKIP-строки harness без учёта намеренных live-skip (p11/n16)."""
    return sum(
        1
        for line in output.splitlines()
        if "SKIP:" in line and "live" not in line.lower()
    )


def run_tests():
    env = dict(os.environ)
    env.setdefault("CARGO_TARGET_DIR", "/tmp/opencode/target-w31a")
    command = ["cargo", "test", "--offline", "-p", "discord-platform"] + TESTS + ["--", "--nocapture"]
    completed = subprocess.run(
        command,
        cwd=WORKDIR,
        env=env,
        capture_output=True,
        text=True,
        check=False,
    )
    return completed.returncode, completed.stdout + completed.stderr


def main():
    lines = ["# W31a — мутации питания/idle", ""]
    failures = []

    baseline_code, baseline_output = run_tests()
    baseline_skips = count_skips(baseline_output)
    baseline_ok = baseline_code == 0 and "test result: ok" in baseline_output
    lines.append("baseline: exit=%d, SKIP=%d" % (baseline_code, baseline_skips))
    if not baseline_ok or baseline_skips:
        lines.append("ОСТАНОВ: базовый прогон не зелёный или содержит SKIP")
        lines.append(baseline_output[-4000:])
        write_log(lines)
        return 1
    lines.append("")

    for mutation_id, name, original, mutated, expectation in MUTATIONS:
        before_md5 = md5(POWER)
        with open(POWER, encoding="utf-8") as handle:
            source = handle.read()
        if original not in source:
            failures.append("%s: фрагмент не найден" % mutation_id)
            lines.append("## %s %s — ОШИБКА: фрагмент не найден" % (mutation_id, name))
            continue

        lines.append("## %s %s" % (mutation_id, name))
        lines.append("ждём: %s" % expectation)
        lines.append("md5 до: %s" % before_md5)

        try:
            with open(POWER, "w", encoding="utf-8") as handle:
                handle.write(source.replace(original, mutated, 1))
            code, output = run_tests()
            failing = sorted(set(TEST_LINE.findall(output)))
            skips = count_skips(output)
            killed = code != 0 and "test result: FAILED" in output and skips == 0
            verdict = "KILLED" if killed else ("INCONCLUSIVE (SKIP)" if skips else "SURVIVED")
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
            with open(POWER, "w", encoding="utf-8") as handle:
                handle.write(source)
            after_md5 = md5(POWER)
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
