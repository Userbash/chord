#!/usr/bin/env python3
"""Фиксирует эталон для `hints` (`a{sv}`) в `fixtures/dbus/hints.json` (W32b).

Два независимых оракула (Rust-код W32b в них не участвует):

1. **libgio**: полные байты method call `Notify(susssasa{sv}i)` для каждого набора hints
   (`Gio.DBusMessage.to_blob()`). Rust-тест собирает то же сообщение нашим маршалером и
   сравнивает hex побайтово — это проверка выравнивания `a{sv}`, сигнатур вариантов и
   порядка записей.
2. **libdbus**: приватный `dbus-daemon` + `mock_hints.py`; Gio-клиент отправляет те же
   наборы, журнал мока (`ключ:сигнатура:значение`) попадает в фикстуру как снимок того,
   что libdbus реально демаршалил.

Запуск: `python3 waves/RW-32/dbus/gen-hints.py` (из корня репозитория).
Сеть не используется. Если `dbus-daemon`/`python3-dbus` недоступны, байты libgio всё равно
пишутся, журнал пропускается с пояснением (код выхода 0).
"""

import json
import os
import shutil
import subprocess
import sys
import tempfile
import time

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
REPO_ROOT = os.path.abspath(os.path.join(SCRIPT_DIR, "..", "..", ".."))
FIXTURES = os.path.join(REPO_ROOT, "fixtures", "dbus")
MOCK = os.path.join(SCRIPT_DIR, "mock_hints.py")
FIXTURE = os.path.join(FIXTURES, "hints.json")

DESTINATION = "org.freedesktop.Notifications"
PATH = "/org/freedesktop/Notifications"
SIGNATURE = "susssasa{sv}i"
NOTIFY_ID = 100

# Общие поля Notify для всех эталонных сообщений (hints — единственная переменная).
APP_NAME = "Discord Lab"
SUMMARY = "Alice"
BODY = "hello"
ACTIONS = ["open", "Open", "reply", "Reply"]
TIMEOUT = -1
SERIAL = 7

# Порядок записей в наборе — тот же, что требует контракт: urgency, category,
# desktop-entry, resident. libgio сериализует dict в порядке вставки python.
CASES = [
    {"name": "empty", "hints": []},
    {"name": "urgency_low", "hints": [("urgency", "y", 0)]},
    {"name": "urgency_normal", "hints": [("urgency", "y", 1)]},
    {"name": "urgency_critical", "hints": [("urgency", "y", 2)]},
    {"name": "category", "hints": [("category", "s", "email.arrived")]},
    {
        "name": "desktop_entry_resident",
        "hints": [("desktop-entry", "s", "discord-lab"), ("resident", "b", True)],
    },
    {"name": "resident_false", "hints": [("resident", "b", False)]},
    {
        "name": "all",
        "hints": [
            ("urgency", "y", 2),
            ("category", "s", "im.received"),
            ("desktop-entry", "s", "discord-lab"),
            ("resident", "b", True),
        ],
    },
]


def variant_payload(hints):
    """Список (key, signature, value) → dict for GLib `a{sv}`."""
    import gi

    gi.require_version("Gio", "2.0")
    from gi.repository import GLib

    return {key: GLib.Variant(signature, value) for key, signature, value in hints}


def build_notify_blob(hints):
    import gi

    gi.require_version("Gio", "2.0")
    from gi.repository import Gio, GLib

    message = Gio.DBusMessage.new_method_call(DESTINATION, PATH, DESTINATION, "Notify")
    message.set_serial(SERIAL)
    message.set_body(
        GLib.Variant(
            "(%s)" % SIGNATURE,
            (
                APP_NAME,
                0,
                "",
                SUMMARY,
                BODY,
                ACTIONS,
                variant_payload(hints),
                TIMEOUT,
            ),
        )
    )
    return bytes(message.to_blob(Gio.DBusCapabilityFlags.NONE)).hex()


def environment_problem():
    if shutil.which("dbus-daemon") is None:
        return "нет dbus-daemon"
    probe = subprocess.run(
        [sys.executable, "-c", "import dbus, gi"],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        check=False,
    )
    if probe.returncode != 0:
        return "нет python3-dbus/gi"
    return None


def collect_journal():
    """Отправляет все наборы на libdbus-мок и возвращает строки журнала `Notify`."""
    workdir = tempfile.mkdtemp(prefix="dbus-hints-")
    log_path = os.path.join(workdir, "journal.tsv")
    daemon_pid = None
    mock = None
    try:
        started = subprocess.run(
            ["dbus-daemon", "--session", "--print-address=1", "--print-pid=1", "--fork"],
            capture_output=True,
            text=True,
            check=True,
        )
        lines = [line for line in started.stdout.splitlines() if line.strip()]
        address = lines[0]
        daemon_pid = int(lines[1])

        mock = subprocess.Popen(
            [sys.executable, MOCK, "--address", address, "--log", log_path, "--next-id", str(NOTIFY_ID)],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.PIPE,
            text=True,
        )
        deadline = time.time() + 10.0
        while time.time() < deadline:
            if os.path.exists(log_path):
                with open(log_path, encoding="utf-8") as handle:
                    if "READY" in handle.read():
                        break
            if mock.poll() is not None:
                raise RuntimeError("мок-сервис завершился: %s" % mock.stderr.read())
            time.sleep(0.05)
        else:
            raise RuntimeError("мок-сервис не поднялся за 10 с")

        import gi

        gi.require_version("Gio", "2.0")
        from gi.repository import Gio, GLib

        connection = Gio.DBusConnection.new_for_address_sync(
            address,
            Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT
            | Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION,
            None,
            None,
        )
        for case in CASES:
            connection.call_sync(
                DESTINATION,
                PATH,
                DESTINATION,
                "Notify",
                GLib.Variant(
                    "(%s)" % SIGNATURE,
                    (
                        APP_NAME,
                        0,
                        "",
                        SUMMARY,
                        BODY,
                        ACTIONS,
                        variant_payload(case["hints"]),
                        TIMEOUT,
                    ),
                ),
                GLib.VariantType.new("(u)"),
                Gio.DBusCallFlags.NONE,
                5000,
                None,
            )

        with open(log_path, encoding="utf-8") as handle:
            return [line.rstrip("\n").split("\t") for line in handle if line.strip()]
    finally:
        if mock is not None and mock.poll() is None:
            mock.terminate()
            try:
                mock.wait(timeout=5)
            except subprocess.TimeoutExpired:
                mock.kill()
        if daemon_pid is not None:
            subprocess.run(["kill", str(daemon_pid)], check=False)
        shutil.rmtree(workdir, ignore_errors=True)


def main():
    os.makedirs(FIXTURES, exist_ok=True)

    cases = [
        {
            "name": case["name"],
            "hints": [
                {"key": key, "signature": signature, "value": value}
                for key, signature, value in case["hints"]
            ],
            "notify_hex": build_notify_blob(case["hints"]),
        }
        for case in CASES
    ]

    payload = {
        "oracle": "Gio.DBusMessage.to_blob() (libgio) + mock_hints.py (python3-dbus/libdbus)",
        "endianness": "little",
        "signature": SIGNATURE,
        "hints_signature": "a{sv}",
        "notify": {
            "app_name": APP_NAME,
            "summary": SUMMARY,
            "body": BODY,
            "actions": ACTIONS,
            "timeout_ms": TIMEOUT,
            "serial": SERIAL,
        },
        "note": (
            "notify_hex — полное сообщение Notify, собранное libgio; порядок hints в "
            "dict задан контрактом (urgency, category, desktop-entry, resident). "
            "journal — то, что libdbus демаршалировал в моке."
        ),
        "cases": cases,
    }

    problem = environment_problem()
    if problem is None:
        try:
            payload["journal"] = collect_journal()
        except Exception as error:  # noqa: BLE001 — генератор не должен падать из-за стенда
            payload["journal_error"] = str(error)
            print("journal пропущен: %s" % error)
    else:
        payload["journal_error"] = problem
        print("journal пропущен: %s" % problem)

    with open(FIXTURE, "w", encoding="utf-8") as handle:
        json.dump(payload, handle, ensure_ascii=False, indent=2)
        handle.write("\n")
    print("hints.json: %d наборов (%s)" % (len(cases), payload["oracle"]))
    return 0


if __name__ == "__main__":
    sys.exit(main())
