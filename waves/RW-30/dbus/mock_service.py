#!/usr/bin/env python3
"""Мок-сервер `org.freedesktop.Notifications` на libdbus для контрактных тестов W30b.

Оракул независим от Rust-реализации: сервис и транспорт — python3-dbus (libdbus) на
приватном `dbus-daemon`. Мок отвечает на `Notify`/`CloseNotification`/`GetCapabilities`/
`GetServerInformation`, умеет эмитить `ActionInvoked`/`NotificationClosed`, «чужой» сигнал
и падать ошибкой (для ветки `DbusError::Remote`).

CLI:
    mock_service.py --address <unix:path=...> --log <file> [--fail-notify]
                    [--next-id 101] [--capabilities body,actions,persistence]

Журнал вызовов — TSV-строки (первое поле — член вызова):
    READY
    GetCapabilities
    GetServerInformation
    Notify\t<app_name>\t<replaces_id>\t<app_icon>\t<summary>\t<body>\t<hints_len>\t<actions>\t<timeout>
    CloseNotification\t<id>
    EmitAction\t<id>\t<action>
    EmitClosed\t<id>\t<reason>
    EmitForeign\t<id>

Управляющий интерфейс `lab.Mock.Notifications` на `/lab/Mock`: `EmitAction(u, s)`,
`EmitClosed(u, u)`, `EmitForeign(u)`.

Сервис ничего не пишет в stderr в штатном режиме (тест читает журнал файлом).
"""

import argparse
import os
import sys

import dbus
import dbus.lowlevel
import dbus.service
from dbus.mainloop.glib import DBusGMainLoop
from gi.repository import GLib

DESTINATION = "org.freedesktop.Notifications"
PATH = "/org/freedesktop/Notifications"
IFACE = "org.freedesktop.Notifications"
CONTROL_PATH = "/lab/Mock"
CONTROL_IFACE = "lab.Mock.Notifications"
CONTROL_DESTINATION = "lab.Mock"


class MockNotifications(dbus.service.Object):
    def __init__(self, bus, log_path, fail_notify, next_id, capabilities):
        super().__init__(bus, PATH)
        self._bus = bus
        self._log_path = log_path
        self._fail_notify = fail_notify
        self._next_id = next_id
        self._capabilities = capabilities
        self._control = MockControl(bus, self)

    def _log(self, *fields):
        with open(self._log_path, "a", encoding="utf-8") as handle:
            handle.write("\t".join(str(field) for field in fields) + "\n")
            handle.flush()
            os.fsync(handle.fileno())

    @staticmethod
    def _format_actions(actions):
        # Спека freedesktop: `as` — плоский список парами (id, label, id, label, …).
        flat = [str(item) for item in actions]
        pairs = []
        for index in range(0, len(flat) - 1, 2):
            pairs.append("%s:%s" % (flat[index], flat[index + 1]))
        return "|".join(pairs)

    @dbus.service.method(IFACE, in_signature="susssasa{sv}i", out_signature="u")
    def Notify(self, app_name, replaces_id, app_icon, summary, body, actions, hints, expire_timeout):
        self._log(
            "Notify",
            app_name,
            int(replaces_id),
            app_icon,
            summary,
            body,
            len(hints),
            self._format_actions(actions),
            int(expire_timeout),
        )
        if self._fail_notify:
            # Ветка DbusError::Remote: имя ошибки и текст уходят клиенту как ERROR-ответ.
            raise dbus.exceptions.DBusException(
                "mock rejected notification",
                name="org.freedesktop.Notifications.Error.MockRejected",
            )
        if int(replaces_id) != 0:
            return dbus.UInt32(int(replaces_id))
        self._next_id += 1
        return dbus.UInt32(self._next_id - 1)

    @dbus.service.method(IFACE, in_signature="u", out_signature="")
    def CloseNotification(self, notification_id):
        self._log("CloseNotification", int(notification_id))

    @dbus.service.method(IFACE, in_signature="", out_signature="as")
    def GetCapabilities(self):
        self._log("GetCapabilities")
        return dbus.Array(self._capabilities, signature="s")

    @dbus.service.method(IFACE, in_signature="", out_signature="ssss")
    def GetServerInformation(self):
        self._log("GetServerInformation")
        return dbus.String("MockNotify"), dbus.String("discord-lab"), dbus.String("1.0"), dbus.String("1.2")

    @dbus.service.signal(IFACE, signature="us")
    def ActionInvoked(self, notification_id, action_key):
        pass

    @dbus.service.signal(IFACE, signature="uu")
    def NotificationClosed(self, notification_id, reason):
        pass

    def emit_action(self, notification_id, action_key):
        self._log("EmitAction", notification_id, action_key)
        self.ActionInvoked(dbus.UInt32(notification_id), dbus.String(action_key))

    def emit_closed(self, notification_id, reason):
        self._log("EmitClosed", notification_id, reason)
        self.NotificationClosed(dbus.UInt32(notification_id), dbus.UInt32(reason))

    def emit_foreign(self, notification_id):
        """Шлёт два «чужих» сигнала: другой интерфейс и ActionInvoked с чужим телом."""
        self._log("EmitForeign", notification_id)
        ping = dbus.lowlevel.SignalMessage(CONTROL_PATH, "lab.Mock.Ping", "Ping")
        ping.append(dbus.String("foreign"), signature="s")
        self._bus.send_message(ping)

        # Тот же интерфейс/член, но тело `s` вместо `(us)`: клиент обязан пропустить сигнал.
        bogus = dbus.lowlevel.SignalMessage(PATH, IFACE, "ActionInvoked")
        bogus.append(dbus.String("not-a-struct"), signature="s")
        self._bus.send_message(bogus)


class MockControl(dbus.service.Object):
    def __init__(self, bus, notifications):
        super().__init__(bus, CONTROL_PATH)
        self._notifications = notifications

    @dbus.service.method(CONTROL_IFACE, in_signature="us", out_signature="")
    def EmitAction(self, notification_id, action_key):
        self._notifications.emit_action(int(notification_id), str(action_key))

    @dbus.service.method(CONTROL_IFACE, in_signature="uu", out_signature="")
    def EmitClosed(self, notification_id, reason):
        self._notifications.emit_closed(int(notification_id), int(reason))

    @dbus.service.method(CONTROL_IFACE, in_signature="u", out_signature="")
    def EmitForeign(self, notification_id):
        self._notifications.emit_foreign(int(notification_id))


def main(argv):
    parser = argparse.ArgumentParser(description="Мок org.freedesktop.Notifications на libdbus")
    parser.add_argument("--address", required=True, help="адрес приватной сессионной шины")
    parser.add_argument("--log", required=True, help="файл журнала вызовов (TSV)")
    parser.add_argument("--fail-notify", action="store_true", help="Notify отвечает D-Bus-ошибкой")
    parser.add_argument("--next-id", type=int, default=100, help="первый выданный id уведомления")
    parser.add_argument(
        "--capabilities",
        default="body,actions,persistence,icon-static",
        help="список возможностей через запятую",
    )
    args = parser.parse_args(argv)

    if os.path.exists(args.log):
        os.remove(args.log)

    DBusGMainLoop(set_as_default=True)
    bus = dbus.bus.BusConnection(args.address)
    capabilities = [item for item in args.capabilities.split(",") if item]
    MockNotifications(bus, args.log, args.fail_notify, args.next_id, capabilities)

    # Имя занимается после активации главного цикла; READY пишем только когда имя наше.
    name = dbus.service.BusName(DESTINATION, bus=bus)
    assert name is not None
    # Управляющий интерфейс доступен по отдельному well-known имени `lab.Mock`.
    control_name = dbus.service.BusName(CONTROL_DESTINATION, bus=bus)
    assert control_name is not None

    with open(args.log, "a", encoding="utf-8") as handle:
        handle.write("READY\n")
        handle.flush()
        os.fsync(handle.fileno())

    GLib.MainLoop().run()
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
