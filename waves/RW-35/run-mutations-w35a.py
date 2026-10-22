#!/usr/bin/env python3
"""Мутационная проверка контракта W35a (fd-передача D-Bus + sleep-inhibitor).

Запускает целевые тесты на каждом дефекте и считает мутацию убитой **только** при
`test result: FAILED` (ошибка сборки/таймаут раннера — не KILLED). Перед мутацией файл
запоминается вместе с md5 и восстанавливается в `finally`; после восстановления md5
сверяется. Полный stdout тестов печатается в консоль (в файл лога не перенаправляется),
в `mutations-w35a.log` пишется только сводка.

CLI:
    run-mutations-w35a.py [--repo PATH] [--filter NAME]

Код выхода: 0 — baseline зелёный и все мутации убиты; 1 — есть выжившая/ошибка.
"""

import argparse
import hashlib
import os
import subprocess
import sys
from pathlib import Path

TARGET_DIR = "/tmp/opencode/target-w35a"
TEST_ARGS = [
    "cargo",
    "test",
    "--offline",
    "-p",
    "discord-platform",
    "--test",
    "dbus_contract",
    "--test",
    "power_contract",
    "--",
    "--nocapture",
]

# (имя, файл относительно rust/, точная замена, ожидаемо убивающие тесты)
MUTATIONS = [
    (
        "m1-unix-fd-index",
        "crates/discord-platform/src/dbus/message.rs",
        (
            """                let index = u32::try_from(self.fds.len())
                    .map_err(|_| DbusError::Protocol("too many unix fds in message"))?;""",
            """                let index = 0u32; // MUTATION m1: индекс всегда 0""",
        ),
        ["f09_call_with_fds_sends_two_pipe_fds_to_service"],
    ),
    (
        "m2-inhibitor-close",
        "crates/discord-platform/src/power.rs",
        (
            """    fn drop(&mut self) {
        fd::close_fd(self.fd);
    }""",
            """    fn drop(&mut self) {
        // MUTATION m2: fd ингибитора не закрывается
    }""",
        ),
        ["f03_drop_closes_inhibitor_fd_exactly_once"],
    ),
    (
        "m3-msg-ctrunc",
        "crates/discord-platform/src/dbus/fd.rs",
        (
            "if message.msg_flags & libc::MSG_CTRUNC != 0 {",
            "if false { // MUTATION m3: MSG_CTRUNC игнорируется",
        ),
        ["f06_control_truncation_is_protocol_and_closes_received_fds"],
    ),
    (
        "m4-sendmsg-fds",
        "crates/discord-platform/src/dbus/fd.rs",
        (
            "    if !fds.is_empty() {\n        let payload = fds.len() * INT_BYTES;",
            "    if false {\n        let payload = fds.len() * INT_BYTES; // MUTATION m4",
        ),
        ["f09_call_with_fds_sends_two_pipe_fds_to_service"],
    ),
    (
        "m5-empty-why",
        "crates/discord-platform/src/power.rs",
        (
            """        if why.is_empty() {
            return Err(DbusError::Protocol("empty why"));
        }""",
            "        // MUTATION m5: проверка пустого why удалена",
        ),
        ["f08_empty_why_is_protocol_and_never_reaches_bus"],
    ),
    (
        "m6-unix-fds-header",
        "crates/discord-platform/src/dbus/message.rs",
        (
            "let fd_count = fds_in_body(body).len();",
            "let fd_count = 0; // MUTATION m6: поле UNIX_FDS не пишется",
        ),
        ["f09_call_with_fds_sends_two_pipe_fds_to_service"],
    ),
]

# Признаки того, что среда стенда недоступна (baseline тогда не доказательство).
HARNESS_SKIP_MARKERS = [
    "SKIP: нет dbus-daemon",
    "SKIP: нет python3",
    "mock_logind.py завершился",
    "mock_logind.py не поднялся",
    "мок-сервис завершился",
    "мок-питание завершился",
]


def md5(data: bytes) -> str:
    return hashlib.md5(data).hexdigest()


def run_tests(repo: Path) -> subprocess.CompletedProcess:
    env = dict(os.environ)
    env["CARGO_TARGET_DIR"] = TARGET_DIR
    env.pop("DISCORD_LAB_POWER_LIVE", None)
    return subprocess.run(
        TEST_ARGS,
        cwd=repo / "rust",
        env=env,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        timeout=900,
    )


def result_lines(output: str) -> list:
    return [line.strip() for line in output.splitlines() if "test result:" in line]


def failing_tests(output: str) -> list:
    names = []
    for line in output.splitlines():
        line = line.strip()
        if line.startswith("---- ") and line.endswith(" stdout ----"):
            names.append(line[len("---- ") : -len(" stdout ----")])
    return names


def main(argv):
    parser = argparse.ArgumentParser(description="Мутации W35a")
    parser.add_argument("--repo", default=str(Path(__file__).resolve().parents[2]))
    parser.add_argument("--filter", default="", help="прогнать только мутации с подстрокой")
    args = parser.parse_args(argv)

    repo = Path(args.repo).resolve()
    log_path = repo / "waves/RW-35/mutations-w35a.log"
    log = []

    def emit(line):
        print(line)
        log.append(line)

    emit("# W35a mutations: baseline + %d мутаций" % len(MUTATIONS))
    emit("# тесты: " + " ".join(TEST_ARGS))

    baseline = run_tests(repo)
    print(baseline.stdout)
    lines = result_lines(baseline.stdout)
    harness_skip = [m for m in HARNESS_SKIP_MARKERS if m in baseline.stdout]
    baseline_ok = (
        baseline.returncode == 0
        and len([line for line in lines if line.startswith("test result: ok")]) == 2
        and not any(line.startswith("test result: FAILED") for line in lines)
        and not harness_skip
    )
    emit("BASELINE: %s (%s)" % ("OK" if baseline_ok else "BAD", "; ".join(lines)))
    if harness_skip:
        emit("BASELINE harness-skip: %s" % harness_skip)
    if not baseline_ok:
        emit("ABORT: baseline не зелёный — мутации не считаются")
        log_path.write_text("\n".join(log) + "\n", encoding="utf-8")
        return 1

    killed = 0
    for name, relative, (old, new), expected in MUTATIONS:
        if args.filter and args.filter not in name:
            continue
        path = repo / "rust" / relative
        original = path.read_bytes()
        original_md5 = md5(original)
        source = original.decode("utf-8")
        if source.count(old) != 1:
            emit("%s: ERROR — якорь найден %d раз в %s" % (name, source.count(old), relative))
            continue
        path.write_text(source.replace(old, new, 1), encoding="utf-8")
        try:
            print("\n" + "=" * 78)
            print("# MUTATION %s (%s)" % (name, relative))
            print("=" * 78)
            run = run_tests(repo)
            print(run.stdout)
        finally:
            path.write_bytes(original)
            restored_md5 = md5(path.read_bytes())
            if restored_md5 != original_md5:
                raise SystemExit(
                    "FATAL: %s не восстановлен (%s != %s)"
                    % (path, restored_md5, original_md5)
                )
        lines = result_lines(run.stdout)
        failed = [line for line in lines if line.startswith("test result: FAILED")]
        if failed:
            killed += 1
            status = "KILLED"
        elif not lines:
            status = "BUILD-ERROR"
        else:
            status = "SURVIVED"
        emit(
            "%s: %s | %s | expected: %s"
            % (
                name,
                status,
                "; ".join(lines) if lines else "нет test result (сборка?)",
                ", ".join(expected),
            )
        )
        failing = failing_tests(run.stdout)
        if failing:
            emit("  упали: %s" % ", ".join(failing))

    selected = len([m for m in MUTATIONS if not args.filter or args.filter in m[0]])
    emit("ИТОГО: KILLED %d/%d" % (killed, selected))
    log_path.write_text("\n".join(log) + "\n", encoding="utf-8")
    print("\nлог: %s" % log_path)
    return 0 if killed == selected else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
