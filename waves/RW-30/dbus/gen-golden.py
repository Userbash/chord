#!/usr/bin/env python3
"""Фиксирует «золотые» D-Bus байты и ответы сервиса уведомлений в `fixtures/dbus/*.json`.

Два независимых оракула (ни один не использует Rust-код W30b):

1. `wire.json` — байты сообщений, собранные **Gio.DBusMessage.to_blob()** (GLib/libgio,
   отдельная реализация D-Bus). Используется unit-тестами маршалинга/парсинга.
2. `notifications.json` — ответы и сигнатуры настоящего сервиса на приватном
   `dbus-daemon`, где сервер — `mock_service.py` на python3-dbus (libdbus), а клиент —
   Gio. Используется как эталон сигнатур `Notify`/`GetCapabilities`/… и форм ответов.

Запуск: `python3 waves/RW-30/dbus/gen-golden.py` (из корня репозитория).
Сеть не используется. Если `dbus-daemon`/libdbus недоступны, `notifications.json` не
перезаписывается, а генератор завершается с кодом 0 и пояснением.
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
MOCK = os.path.join(SCRIPT_DIR, "mock_service.py")

DESTINATION = "org.freedesktop.Notifications"
PATH = "/org/freedesktop/Notifications"
CAPABILITIES = ["body", "actions", "persistence", "icon-static"]
SERVER_INFO = ["MockNotify", "discord-lab", "1.0", "1.2"]
NOTIFY_ID = 100


def build_wire_messages():
    import gi

    gi.require_version("Gio", "2.0")
    from gi.repository import Gio, GLib

    messages = []

    notify = Gio.DBusMessage.new_method_call(
        DESTINATION, PATH, DESTINATION, "Notify"
    )
    notify.set_serial(7)
    notify.set_body(
        GLib.Variant(
            "(susssasa{sv}i)",
            (
                "Discord Lab",
                0,
                "",
                "Alice",
                "hello",
                ["open", "Open"],
                {},
                -1,
            ),
        )
    )
    messages.append(
        {
            "name": "notify_call",
            "kind": "method_call",
            "serial": 7,
            "path": PATH,
            "interface": DESTINATION,
            "member": "Notify",
            "destination": DESTINATION,
            "signature": "susssasa{sv}i",
            "body": [
                "Discord Lab",
                0,
                "",
                "Alice",
                "hello",
                ["open", "Open"],
                {},
                -1,
            ],
            "hex": bytes(notify.to_blob(Gio.DBusCapabilityFlags.NONE)).hex(),
        }
    )

    action = Gio.DBusMessage.new_signal(PATH, DESTINATION, "ActionInvoked")
    action.set_serial(3)
    action.set_body(GLib.Variant("(us)", (NOTIFY_ID, "open")))
    messages.append(
        {
            "name": "action_invoked_signal",
            "kind": "signal",
            "serial": 3,
            "path": PATH,
            "interface": DESTINATION,
            "member": "ActionInvoked",
            # Тело сообщения — последовательность типов, без внешней скобки кортежа.
            "signature": "us",
            "body": [NOTIFY_ID, "open"],
            "hex": bytes(action.to_blob(Gio.DBusCapabilityFlags.NONE)).hex(),
        }
    )

    capabilities = Gio.DBusMessage.new_method_reply(notify)
    capabilities.set_serial(4)
    capabilities.set_body(GLib.Variant("(as)", (CAPABILITIES,)))
    messages.append(
        {
            "name": "capabilities_reply",
            "kind": "method_return",
            "serial": 4,
            "reply_serial": 7,
            "signature": "as",
            "body": [CAPABILITIES],
            "hex": bytes(capabilities.to_blob(Gio.DBusCapabilityFlags.NONE)).hex(),
        }
    )

    error = Gio.DBusMessage.new_method_error_literal(
        notify,
        "org.freedesktop.Notifications.Error.MockRejected",
        "mock rejected notification",
    )
    error.set_serial(5)
    messages.append(
        {
            "name": "error_reply",
            "kind": "error",
            "serial": 5,
            "reply_serial": 7,
            "error_name": "org.freedesktop.Notifications.Error.MockRejected",
            "signature": "s",
            "body": ["mock rejected notification"],
            "hex": bytes(error.to_blob(Gio.DBusCapabilityFlags.NONE)).hex(),
        }
    )

    # a(ss) с двумя структурами: проверка выравнивания 8 у элементов и пар внутри.
    pairs = Gio.DBusMessage.new_signal("/lab/Mock", "lab.Mock.Pairs", "Pairs")
    pairs.set_serial(6)
    pairs.set_body(GLib.Variant("(a(ss))", ([("a", "b"), ("c", "d")],)))
    messages.append(
        {
            "name": "pairs_signal",
            "kind": "signal",
            "serial": 6,
            "path": "/lab/Mock",
            "interface": "lab.Mock.Pairs",
            "member": "Pairs",
            "signature": "a(ss)",
            "hex": bytes(pairs.to_blob(Gio.DBusCapabilityFlags.NONE)).hex(),
        }
    )

    return {
        "oracle": "Gio.DBusMessage.to_blob() (GLib %d.%d.%d)"
        % (GLib.MAJOR_VERSION, GLib.MINOR_VERSION, GLib.MICRO_VERSION),
        "endianness": "little",
        "note": "Байты собраны libgio; порядок заголовочных полей принадлежит оракулу.",
        "messages": messages,
    }


def have_environment():
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


def collect_notifications_replies():
    workdir = tempfile.mkdtemp(prefix="dbus-golden-")
    log_path = os.path.join(workdir, "calls.tsv")
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
            [
                sys.executable,
                MOCK,
                "--address",
                address,
                "--log",
                log_path,
                "--next-id",
                str(NOTIFY_ID),
            ],
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

        conn = Gio.DBusConnection.new_for_address_sync(
            address,
            Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT
            | Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION,
            None,
            None,
        )

        def call(member, signature, args, reply_type):
            params = GLib.Variant(signature, args) if signature else None
            reply = conn.call_sync(
                DESTINATION,
                PATH,
                DESTINATION,
                member,
                params,
                GLib.VariantType.new(reply_type) if reply_type else None,
                Gio.DBusCallFlags.NONE,
                5000,
                None,
            )
            return reply

        capabilities = call("GetCapabilities", "", (), "(as)").unpack()[0]
        info = call("GetServerInformation", "", (), "(ssss)").unpack()
        notify_id = call(
            "Notify",
            "(susssasa{sv}i)",
            ("Discord Lab", 0, "", "Alice", "hello", ["open", "Open", "reply", "Reply"], {}, -1),
            "(u)",
        ).unpack()[0]
        call("CloseNotification", "(u)", (notify_id,), "")

        with open(log_path, encoding="utf-8") as handle:
            journal = [line.rstrip("\n").split("\t") for line in handle if line.strip()]

        return {
            "oracle": "mock_service.py (python3-dbus/libdbus) на приватном dbus-daemon",
            "destination": DESTINATION,
            "path": PATH,
            "methods": [
                {
                    "member": "GetCapabilities",
                    "signature": "",
                    "reply_signature": "as",
                    "reply": list(capabilities),
                },
                {
                    "member": "GetServerInformation",
                    "signature": "",
                    "reply_signature": "(ssss)",
                    "reply": list(info),
                },
                {
                    "member": "Notify",
                    "signature": "susssasa{sv}i",
                    "reply_signature": "u",
                    "reply": notify_id,
                    "note": "actions на проводе — `as` парами id/label (спека freedesktop)",
                },
                {
                    "member": "CloseNotification",
                    "signature": "u",
                    "reply_signature": "",
                    "reply": None,
                },
            ],
            "signals": [
                {"member": "ActionInvoked", "signature": "(us)", "body": [NOTIFY_ID, "open"]},
                {"member": "NotificationClosed", "signature": "(uu)", "body": [NOTIFY_ID, 2]},
            ],
            "journal": journal,
        }
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


def write_json(path, payload):
    with open(path, "w", encoding="utf-8") as handle:
        json.dump(payload, handle, ensure_ascii=False, indent=2)
        handle.write("\n")


def main():
    os.makedirs(FIXTURES, exist_ok=True)

    wire = build_wire_messages()
    write_json(os.path.join(FIXTURES, "wire.json"), wire)
    print("wire.json: %d сообщений (%s)" % (len(wire["messages"]), wire["oracle"]))

    reason = have_environment()
    if reason is not None:
        print("notifications.json пропущен: %s" % reason)
        return 0

    replies = collect_notifications_replies()
    write_json(os.path.join(FIXTURES, "notifications.json"), replies)
    print("notifications.json: %d методов (%s)" % (len(replies["methods"]), replies["oracle"]))
    return 0


if __name__ == "__main__":
    sys.exit(main())
