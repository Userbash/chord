#!/usr/bin/env python3
"""Генератор фикстуры W34b (не в файловой матрице карточки; хранится в отчёте).

Запуск из корня репозитория: `python3 gen-hints-ext.py`.
Пишет `fixtures/dbus/hints-ext.json`: байты `Notify` от libgio + снимок журнала
`waves/RW-34/dbus/mock_hints2.py` (libdbus). Сеть не используется.
"""

import json
import os
import shutil
import subprocess
import sys
import tempfile
import time

REPO = os.getcwd()  # запуск из корня репозитория
MOCK = os.path.join(REPO, "waves", "RW-34", "dbus", "mock_hints2.py")
FIXTURE = os.path.join(REPO, "fixtures", "dbus", "hints-ext.json")

DESTINATION = "org.freedesktop.Notifications"
PATH = "/org/freedesktop/Notifications"
SIGNATURE = "susssasa{sv}i"
NOTIFY_ID = 100
APP_NAME, SUMMARY, BODY = "Discord Lab", "Alice", "hello"
ACTIONS = ["open", "Open", "reply", "Reply"]
TIMEOUT, SERIAL = -1, 7

CASES = [
    {"name": "all_eight", "hints": [
        ("urgency", "y", 1),
        ("category", "s", "im.received"),
        ("desktop-entry", "s", "discord-lab"),
        ("image-path", "s", "/usr/share/icons/discord-lab.png"),
        ("sound-name", "s", "message-new-instant"),
        ("resident", "b", True),
        ("transient", "b", True),
        ("suppress-sound", "b", True),
    ]},
    {"name": "image_and_sound", "hints": [
        ("image-path", "s", "file:///tmp/avatar.png"),
        ("sound-name", "s", "alarm-clock-elapsed"),
    ]},
    {"name": "bools_false", "hints": [
        ("resident", "b", False),
        ("transient", "b", False),
        ("suppress-sound", "b", False),
    ]},
    {"name": "empties_normalized", "hints": [
        ("urgency", "y", 0),
        ("resident", "b", True),
    ]},
    {"name": "unicode", "hints": [
        ("category", "s", "im.\U0001F4EC"),
        ("image-path", "s", "/tmp/\u0438\u043a\u043e\u043d\u043a\u0430.png"),
        ("sound-name", "s", "\u0437\u0432\u0443\u043a-\u0443\u0432\u0435\u0434\u043e\u043c\u043b\u0435\u043d\u0438\u044f"),
    ]},
    {"name": "sound_and_image_alt", "hints": [
        ("image-path", "s", "/tmp/bell.png"),
        ("sound-name", "s", "bell"),
    ]},
]


def variant_payload(hints):
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
    message.set_body(GLib.Variant("(%s)" % SIGNATURE, (
        APP_NAME, 0, "", SUMMARY, BODY, ACTIONS, variant_payload(hints), TIMEOUT,
    )))
    return bytes(message.to_blob(Gio.DBusCapabilityFlags.NONE)).hex()


def collect_journal():
    workdir = tempfile.mkdtemp(prefix="dbus-hints-ext-")
    log_path = os.path.join(workdir, "journal.tsv")
    daemon_pid = None
    mock = None
    try:
        started = subprocess.run(
            ["dbus-daemon", "--session", "--print-address=1", "--print-pid=1", "--fork"],
            capture_output=True, text=True, check=True)
        lines = [line for line in started.stdout.splitlines() if line.strip()]
        address = lines[0]
        daemon_pid = int(lines[1])
        mock = subprocess.Popen(
            [sys.executable, MOCK, "--address", address, "--log", log_path,
             "--next-id", str(NOTIFY_ID)],
            stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, text=True)
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
            None, None)
        for case in CASES:
            connection.call_sync(
                DESTINATION, PATH, DESTINATION, "Notify",
                GLib.Variant("(%s)" % SIGNATURE, (
                    APP_NAME, 0, "", SUMMARY, BODY, ACTIONS,
                    variant_payload(case["hints"]), TIMEOUT)),
                GLib.VariantType.new("(u)"), Gio.DBusCallFlags.NONE, 5000, None)
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
    payload = {
        "oracle": "Gio.DBusMessage.to_blob() (libgio) + mock_hints2.py (python3-dbus/libdbus)",
        "endianness": "little",
        "signature": SIGNATURE,
        "hints_signature": "a{sv}",
        "notify": {
            "app_name": APP_NAME, "summary": SUMMARY, "body": BODY,
            "actions": ACTIONS, "timeout_ms": TIMEOUT, "serial": SERIAL,
        },
        "note": (
            "notify_hex — полное сообщение Notify, собранное libgio; порядок hints в dict "
            "задан контрактом W34b (urgency, category, desktop-entry, image-path, "
            "sound-name, resident, transient, suppress-sound). journal — то, что libdbus "
            "демаршалировал в моке mock_hints2.py."
        ),
        "cases": [
            {
                "name": case["name"],
                "hints": [
                    {"key": key, "signature": signature, "value": value}
                    for key, signature, value in case["hints"]
                ],
                "notify_hex": build_notify_blob(case["hints"]),
            }
            for case in CASES
        ],
    }
    try:
        payload["journal"] = collect_journal()
    except Exception as error:  # noqa: BLE001 — стенд не должен ломать генератор
        payload["journal_error"] = str(error)
        print("journal пропущен: %s" % error)

    os.makedirs(os.path.dirname(FIXTURE), exist_ok=True)
    with open(FIXTURE, "w", encoding="utf-8") as handle:
        json.dump(payload, handle, ensure_ascii=False, indent=2)
        handle.write("\n")
    print("hints-ext.json: %d наборов" % len(payload["cases"]))
    return 0


if __name__ == "__main__":
    sys.exit(main())
