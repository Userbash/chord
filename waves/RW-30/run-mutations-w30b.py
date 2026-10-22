#!/usr/bin/env python3
"""Мутационная проверка контрактов W30b (D-Bus-уведомления).

Каждая мутация — один точечный дефект в продуктовом коде `discord-platform`
(`src/dbus/*.rs`, `src/notifications.rs`). Ожидание: targeted-тесты `dbus_contract` и
`notifications_contract` падают (мутация «убита»). Файл восстанавливается из снимка,
совпадение md5 до/после обязательно.

Запуск (из корня репозитория):
    CARGO_TARGET_DIR=/tmp/opencode/target-w30b python3 waves/RW-30/run-mutations-w30b.py

Код выхода: 0 — все мутации убиты и файлы восстановлены; 1 — есть выжившая мутация,
ошибка восстановления или сбой запуска тестов.
"""

import hashlib
import os
import re
import subprocess
import sys

REPO_ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
CRATE = os.path.join(REPO_ROOT, "rust", "crates", "discord-platform")
AUTH = os.path.join(CRATE, "src", "dbus", "auth.rs")
MESSAGE = os.path.join(CRATE, "src", "dbus", "message.rs")
MOD = os.path.join(CRATE, "src", "dbus", "mod.rs")
NOTIFICATIONS = os.path.join(CRATE, "src", "notifications.rs")
LOG_PATH = os.path.join(REPO_ROOT, "waves", "RW-30", "mutations-w30b.log")
WORKDIR = os.path.join(REPO_ROOT, "rust")
TESTS = ["--test", "dbus_contract", "--test", "notifications_contract"]

# (id, имя, файл, фрагмент-оригинал, фрагмент-мутация, что должен поймать)
MUTATIONS = [
    (
        "01",
        "auth_hex_of_number",
        AUTH,
        """    let decimal = uid.to_string();
    let mut hex = String::with_capacity(decimal.len() * 2);
    for byte in decimal.as_bytes() {
        hex.push_str(&format!("{byte:02x}"));
    }
    format!("AUTH EXTERNAL {hex}")""",
        """    format!("AUTH EXTERNAL {uid:x}")""",
        "n02/n02b: hex от числа вместо hex от десятичной записи uid",
    ),
    (
        "02",
        "struct_alignment_4",
        MESSAGE,
        "        b'x' | b't' | b'd' | b'(' | b'{' => 8,",
        "        b'x' | b't' | b'd' | b'(' | b'{' => 4,",
        "n03/n04/n05: выравнивание struct/dict-entry 8 → 4",
    ),
    (
        "03",
        "reply_serial_off_by_one",
        MOD,
        """                MessageType::MethodReturn => {
                    if message.reply_serial() == Some(serial) {
                        return Ok(message.body);
                    }
                }""",
        """                MessageType::MethodReturn => {
                    if message.reply_serial() == Some(serial.wrapping_add(1)) {
                        return Ok(message.body);
                    }
                }""",
        "n02/n07…: ответ не сопоставляется по reply_serial",
    ),
    (
        "04",
        "error_reply_as_success",
        MOD,
        """                        return Err(DbusError::Remote {
                            name,
                            message: text,
                        });""",
        """                        let _ = (name, text);
                        return Ok(message.body);""",
        "n11: ERROR-ответ превращается в успех вместо DbusError::Remote",
    ),
    (
        "05",
        "actions_label_id_swapped",
        NOTIFICATIONS,
        """            actions.push(Value::String(action_id.clone()));
            actions.push(Value::String(label.clone()));""",
        """            actions.push(Value::String(label.clone()));
            actions.push(Value::String(action_id.clone()));""",
        "n09: пары actions перепутаны (label вместо id)",
    ),
    (
        "06",
        "match_rule_without_closed",
        NOTIFICATIONS,
        """        for member in ["ActionInvoked", "NotificationClosed"] {""",
        """        for member in ["ActionInvoked", "ActionInvoked"] {""",
        "n12: нет AddMatch на NotificationClosed",
    ),
    (
        "07",
        "event_filter_accepts_garbage",
        NOTIFICATIONS,
        """        [Value::U32(id), Value::String(action)] => Some(NotificationEvent::Action {
            id: *id,
            action: action.clone(),
        }),
        _ => None,
    }
}""",
        """        [Value::U32(id), Value::String(action)] => Some(NotificationEvent::Action {
            id: *id,
            action: action.clone(),
        }),
        _ => Some(NotificationEvent::Action {
            id: 0,
            action: String::new(),
        }),
    }
}""",
        "n13: чужой сигнал с телом `s` принимается за событие",
    ),
    (
        "08",
        "serial_never_advances",
        MOD,
        """        self.serial = self.serial.wrapping_add(1);
        if self.serial == 0 {
            self.serial = 1;
        }
        self.serial""",
        """        0""",
        "серийный номер не растёт: method call с serial=0 не собирается",
    ),
    (
        "09",
        "empty_summary_guard_removed",
        NOTIFICATIONS,
        """        if notification.summary.is_empty() {
            return Err(DbusError::Protocol("empty summary"));
        }

""",
        "",
        "n08b: пустой summary уходит на шину вместо DbusError::Protocol",
    ),
    (
        "10",
        "hints_not_empty",
        NOTIFICATIONS,
        "            Value::Dict(Vec::new()), // hints: пустой a{sv}",
        """            Value::Dict(vec![(
                Value::String("urgency".to_owned()),
                Value::Variant(Box::new(Value::Byte(1))),
            )]),""",
        "n08: hints перестаёт быть пустым a{sv}",
    ),
]

TEST_LINE = re.compile(r"^test (\S+) \.\.\. FAILED$", re.MULTILINE)
SUMMARY = re.compile(r"^test result: FAILED\..*$", re.MULTILINE)


def md5(path):
    with open(path, "rb") as handle:
        return hashlib.md5(handle.read()).hexdigest()


def run_tests():
    env = dict(os.environ)
    env.setdefault("CARGO_TARGET_DIR", "/tmp/opencode/target-w30b")
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


def main():
    lines = ["# W30b — мутации D-Bus/уведомлений", ""]
    failures = []

    baseline_code, baseline_output = run_tests()
    lines.append("baseline: exit=%d" % baseline_code)
    if baseline_code != 0:
        lines.append("ОСТАНОВ: базовый прогон не зелёный")
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
            killed = code != 0
            lines.append("exit: %d — %s" % (code, "KILLED" if killed else "SURVIVED"))
            if failing:
                lines.append("упавшие тесты: %s" % ", ".join(failing))
            for match in SUMMARY.findall(output):
                lines.append("  %s" % match)
            if not failing:
                tail = [line for line in output.splitlines() if line.startswith("error")]
                lines.append("вывод: %s" % " | ".join(tail[:3]))
            if not killed:
                failures.append("%s %s: мутация выжила" % (mutation_id, name))
        finally:
            with open(path, "w", encoding="utf-8") as handle:
                handle.write(source)
            after_md5 = md5(path)
            lines.append("md5 после restore: %s — %s" % (after_md5, "OK" if after_md5 == before_md5 else "РАСХОЖДЕНИЕ"))
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
