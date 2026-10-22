#!/usr/bin/env python3
"""Мок-сервисы питания на libdbus для контрактных тестов W31a.

Оракул независим от Rust-реализации: сервисы и транспорт — python3-dbus (libdbus) на
приватном `dbus-daemon`. На одной шине поднимаются два well-known имени:

- `org.freedesktop.ScreenSaver` (`/org/freedesktop/ScreenSaver`): `Inhibit`, `UnInhibit`,
  алиас `Uninhibit` (fallback), `IsActive`, `GetSessionIdleTime`. `GetSessionIdleTime`
  умеет отвечать значением или D-Bus-ошибкой (`UnknownMethod`/`UnknownInterface`/своя),
  чтобы проверить ветку `Ok(None)`/`Err` контракта.
- `org.freedesktop.login1` (`/org/freedesktop/login1`, интерфейс Manager): сигнал
  `PrepareForSleep(b)`; методами не пользуется — клиент только `AddMatch`.

Управляющий интерфейс `lab.Mock.Power` на `/lab/Mock`: `EmitPrepareForSleep(b)`,
`EmitForeign()`, `SetActive(b)`, `SetIdleMode(s)`, `SetIdleSeconds(u)`.

CLI:
    mock_power.py --address <unix:path=...> --log <file>
                  [--cookie-start 41] [--active] [--idle-seconds 1234]
                  [--idle-mode value|unknown-method|unknown-interface|error]
                  [--force-uninhibit-alias]

Журнал вызовов — TSV-строки (первое поле — член вызова):
    READY
    Inhibit\t<app_name>\t<reason>\t<cookie>
    UnInhibit\t<cookie>
    Uninhibit\t<cookie>
    IsActive\t<0|1>
    GetSessionIdleTime\t<mode>\t<seconds>
    EmitPrepareForSleep\t<1|0>
    EmitForeign
    SetActive\t<1|0>
    SetIdleMode\t<mode>
    SetIdleSeconds\t<seconds>

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

SCREENSAVER_DESTINATION = "org.freedesktop.ScreenSaver"
SCREENSAVER_PATH = "/org/freedesktop/ScreenSaver"
SCREENSAVER_IFACE = "org.freedesktop.ScreenSaver"

LOGIN1_DESTINATION = "org.freedesktop.login1"
LOGIN1_PATH = "/org/freedesktop/login1"
LOGIN1_IFACE = "org.freedesktop.login1.Manager"

CONTROL_PATH = "/lab/Mock"
CONTROL_IFACE = "lab.Mock.Power"
CONTROL_DESTINATION = "lab.Mock"

IDLE_MODES = ("value", "unknown-method", "unknown-interface", "error")


class MockScreenSaver(dbus.service.Object):
    def __init__(self, bus, log_path, cookie_start, active, idle_seconds, idle_mode, force_alias):
        super().__init__(bus, SCREENSAVER_PATH)
        self._log_path = log_path
        self._next_cookie = cookie_start
        self._issued = set()
        self._active = active
        self._idle_seconds = idle_seconds
        self._idle_mode = idle_mode
        self._force_uninhibit_alias = force_alias

    def _log(self, *fields):
        with open(self._log_path, "a", encoding="utf-8") as handle:
            handle.write("\t".join(str(field) for field in fields) + "\n")
            handle.flush()
            os.fsync(handle.fileno())

    def _consume_cookie(self, cookie):
        if cookie not in self._issued:
            # Неизвестный cookie — ошибка сервера: клиент обязан вернуть Err, не паниковать.
            raise dbus.exceptions.DBusException(
                "unknown cookie %d" % cookie,
                name="org.freedesktop.ScreenSaver.Error.InvalidCookie",
            )
        self._issued.discard(cookie)

    @dbus.service.method(SCREENSAVER_IFACE, in_signature="ss", out_signature="u")
    def Inhibit(self, app_name, reason):
        cookie = self._next_cookie
        self._next_cookie += 1
        self._issued.add(cookie)
        self._log("Inhibit", str(app_name), str(reason), cookie)
        return dbus.UInt32(cookie)

    @dbus.service.method(SCREENSAVER_IFACE, in_signature="u", out_signature="")
    def UnInhibit(self, cookie):
        self._log("UnInhibit", int(cookie))
        if self._force_uninhibit_alias:
            # Сервис без `UnInhibit` (только `Uninhibit`): клиент обязан попробовать алиас.
            raise dbus.exceptions.DBusException(
                "mock exposes only Uninhibit",
                name="org.freedesktop.DBus.Error.UnknownMethod",
            )
        self._consume_cookie(int(cookie))

    @dbus.service.method(SCREENSAVER_IFACE, in_signature="u", out_signature="")
    def Uninhibit(self, cookie):
        self._log("Uninhibit", int(cookie))
        self._consume_cookie(int(cookie))

    @dbus.service.method(SCREENSAVER_IFACE, in_signature="", out_signature="b")
    def IsActive(self):
        self._log("IsActive", 1 if self._active else 0)
        return dbus.Boolean(self._active)

    @dbus.service.method(SCREENSAVER_IFACE, in_signature="", out_signature="u")
    def GetSessionIdleTime(self):
        self._log("GetSessionIdleTime", self._idle_mode, self._idle_seconds)
        if self._idle_mode == "unknown-method":
            raise dbus.exceptions.DBusException(
                "GetSessionIdleTime is not implemented",
                name="org.freedesktop.DBus.Error.UnknownMethod",
            )
        if self._idle_mode == "unknown-interface":
            raise dbus.exceptions.DBusException(
                "org.freedesktop.ScreenSaver is not implemented",
                name="org.freedesktop.DBus.Error.UnknownInterface",
            )
        if self._idle_mode == "error":
            raise dbus.exceptions.DBusException(
                "mock idle failure",
                name="org.freedesktop.ScreenSaver.Error.MockFailure",
            )
        return dbus.UInt32(self._idle_seconds)

    # --- управление из теста (не входит в контракт ScreenSaver) -----------------

    def set_active(self, active):
        self._active = bool(active)
        self._log("SetActive", 1 if self._active else 0)

    def set_idle_mode(self, mode):
        self._idle_mode = mode
        self._log("SetIdleMode", mode)

    def set_idle_seconds(self, seconds):
        self._idle_seconds = int(seconds)
        self._log("SetIdleSeconds", int(seconds))


class MockLogin1(dbus.service.Object):
    def __init__(self, bus, log_path):
        super().__init__(bus, LOGIN1_PATH)
        self._bus = bus
        self._log_path = log_path

    def _log(self, *fields):
        with open(self._log_path, "a", encoding="utf-8") as handle:
            handle.write("\t".join(str(field) for field in fields) + "\n")
            handle.flush()
            os.fsync(handle.fileno())

    @dbus.service.signal(LOGIN1_IFACE, signature="b")
    def PrepareForSleep(self, start):
        pass

    def emit_prepare_for_sleep(self, start):
        self._log("EmitPrepareForSleep", 1 if start else 0)
        self.PrepareForSleep(dbus.Boolean(start))

    def emit_foreign(self):
        """Шлёт чужие сигналы: другой интерфейс и PrepareForSleep с телом `s`."""
        self._log("EmitForeign")
        ping = dbus.lowlevel.SignalMessage(CONTROL_PATH, "lab.Mock.Ping", "Ping")
        ping.append(dbus.String("foreign"), signature="s")
        self._bus.send_message(ping)

        # Верный путь/интерфейс/член, но тело `s` вместо `b`: клиент обязан пропустить.
        bogus = dbus.lowlevel.SignalMessage(LOGIN1_PATH, LOGIN1_IFACE, "PrepareForSleep")
        bogus.append(dbus.String("not-a-bool"), signature="s")
        self._bus.send_message(bogus)


class MockControl(dbus.service.Object):
    def __init__(self, bus, screensaver, login1):
        super().__init__(bus, CONTROL_PATH)
        self._screensaver = screensaver
        self._login1 = login1

    @dbus.service.method(CONTROL_IFACE, in_signature="b", out_signature="")
    def EmitPrepareForSleep(self, start):
        self._login1.emit_prepare_for_sleep(bool(start))

    @dbus.service.method(CONTROL_IFACE, in_signature="", out_signature="")
    def EmitForeign(self):
        self._login1.emit_foreign()

    @dbus.service.method(CONTROL_IFACE, in_signature="b", out_signature="")
    def SetActive(self, active):
        self._screensaver.set_active(bool(active))

    @dbus.service.method(CONTROL_IFACE, in_signature="s", out_signature="")
    def SetIdleMode(self, mode):
        mode = str(mode)
        if mode not in IDLE_MODES:
            raise dbus.exceptions.DBusException(
                "unknown idle mode %r" % mode,
                name="lab.Mock.Error.BadMode",
            )
        self._screensaver.set_idle_mode(mode)

    @dbus.service.method(CONTROL_IFACE, in_signature="u", out_signature="")
    def SetIdleSeconds(self, seconds):
        self._screensaver.set_idle_seconds(int(seconds))


def main(argv):
    parser = argparse.ArgumentParser(description="Мок org.freedesktop.ScreenSaver/login1 на libdbus")
    parser.add_argument("--address", required=True, help="адрес приватной шины")
    parser.add_argument("--log", required=True, help="файл журнала вызовов (TSV)")
    parser.add_argument("--cookie-start", type=int, default=41, help="первый выданный cookie")
    parser.add_argument("--active", action="store_true", help="начальное значение IsActive")
    parser.add_argument("--idle-seconds", type=int, default=1234, help="значение GetSessionIdleTime")
    parser.add_argument(
        "--idle-mode",
        default="value",
        choices=IDLE_MODES,
        help="поведение GetSessionIdleTime",
    )
    parser.add_argument(
        "--force-uninhibit-alias",
        action="store_true",
        help="UnInhibit отвечает UnknownMethod (сервис знает только Uninhibit)",
    )
    args = parser.parse_args(argv)

    if os.path.exists(args.log):
        os.remove(args.log)

    DBusGMainLoop(set_as_default=True)
    bus = dbus.bus.BusConnection(args.address)
    screensaver = MockScreenSaver(
        bus,
        args.log,
        args.cookie_start,
        args.active,
        args.idle_seconds,
        args.idle_mode,
        args.force_uninhibit_alias,
    )
    login1 = MockLogin1(bus, args.log)
    MockControl(bus, screensaver, login1)

    # Имена занимаются после активации главного цикла; READY пишем только когда оба наши.
    screensaver_name = dbus.service.BusName(SCREENSAVER_DESTINATION, bus=bus)
    login1_name = dbus.service.BusName(LOGIN1_DESTINATION, bus=bus)
    control_name = dbus.service.BusName(CONTROL_DESTINATION, bus=bus)
    assert screensaver_name is not None
    assert login1_name is not None
    assert control_name is not None

    with open(args.log, "a", encoding="utf-8") as handle:
        handle.write("READY\n")
        handle.flush()
        os.fsync(handle.fileno())

    GLib.MainLoop().run()
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
