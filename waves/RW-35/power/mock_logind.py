#!/usr/bin/env python3
"""Мок `org.freedesktop.login1.Manager` с передачей unix-fd (SCM_RIGHTS) для W35a.

Оракул не зависит от Rust-реализации: сервис поднят на python3-dbus (libdbus) поверх
приватного `dbus-daemon`. Контрактные и/или тестовые методы:

- `Inhibit(what: s, who: s, why: s, mode: s) -> h` (login1.Manager): журналирует
  аргументы и возвращает реальный fd (memfd). Режимы (`--inhibit-mode`/`SetInhibitMode`):
  `ok` — fd; `error` — D-Bus ERROR (`org.freedesktop.login1.NotAuthorized`);
  `no-fd` — D-Bus ERROR (`org.freedesktop.DBus.Error.Failed`): сервер не может
  выделить fd, клиент обязан получить ошибку и закрыть всё полученное.
- `TwoFds() -> hh` (управляющий интерфейс): два fd в одном сообщении (memfd с
  содержимым `first`/`second`) — проверка очереди fd на сообщение.
- `AcceptFds(first: h, second: h) -> s` (управляющий интерфейс): принимает два fd,
  читает их содержимое и пишет в журнал — проверка отправки SCM_RIGHTS клиентом.
- `SetInhibitMode(mode: s)`: переключает режим Inhibit во время теста.

Журнал — TSV-строки (первое поле — операция):
    READY
    Inhibit\t<what>\t<who>\t<why>\t<mode>
    SetInhibitMode\t<mode>
    TwoFds
    AcceptFds\t<content1>\t<content2>

CLI:
    mock_logind.py --address <unix:path=...> --log <file>
                   [--inhibit-mode ok|error|no-fd]

Сервис ничего не пишет в stderr в штатном режиме: тест читает журнал файлом.
"""

import argparse
import os
import sys

import dbus
import dbus.service
import dbus.types
from dbus.mainloop.glib import DBusGMainLoop
from gi.repository import GLib

LOGIN1_DESTINATION = "org.freedesktop.login1"
LOGIN1_PATH = "/org/freedesktop/login1"
LOGIN1_IFACE = "org.freedesktop.login1.Manager"

CONTROL_DESTINATION = "lab.Mock.Login1"
CONTROL_PATH = "/lab/Mock"
CONTROL_IFACE = "lab.Mock.Login1"

INHIBIT_MODES = ("ok", "error", "no-fd")


def make_memfd(name, payload):
    """Создаёт memfd с данными и возвращает его на чтение с начала."""
    fd = os.memfd_create(name, flags=0)
    os.write(fd, payload)
    os.lseek(fd, 0, os.SEEK_SET)
    return fd


def read_all(fd):
    """Читает fd до EOF (для fd, пришедших от клиента)."""
    chunks = []
    while True:
        chunk = os.read(fd, 4096)
        if not chunk:
            break
        chunks.append(chunk)
    return b"".join(chunks)


class MockLogin1(dbus.service.Object):
    def __init__(self, bus, log_path, inhibit_mode):
        super().__init__(bus, LOGIN1_PATH)
        self._log_path = log_path
        self._inhibit_mode = inhibit_mode

    def _log(self, *fields):
        with open(self._log_path, "a", encoding="utf-8") as handle:
            handle.write("\t".join(str(field) for field in fields) + "\n")
            handle.flush()
            os.fsync(handle.fileno())

    def set_mode(self, mode):
        self._inhibit_mode = mode
        self._log("SetInhibitMode", mode)

    @dbus.service.method(LOGIN1_IFACE, in_signature="ssss", out_signature="h")
    def Inhibit(self, what, who, why, mode):
        # Журнал пишется до ответа: тест видит аргументы даже при ERROR.
        self._log("Inhibit", str(what), str(who), str(why), str(mode))
        if self._inhibit_mode == "error":
            raise dbus.exceptions.DBusException(
                "mock inhibit denied",
                name="org.freedesktop.login1.NotAuthorized",
            )
        if self._inhibit_mode == "no-fd":
            # Сервер не смог выделить fd: клиент обязан получить D-Bus ERROR, а не панику.
            raise dbus.exceptions.DBusException(
                "mock cannot allocate an inhibit fd",
                name="org.freedesktop.DBus.Error.Failed",
            )
        return dbus.types.UnixFd(make_memfd("discord-lab-inhibit", b"inhibit"))


class MockControl(dbus.service.Object):
    def __init__(self, bus, log_path, login1):
        super().__init__(bus, CONTROL_PATH)
        self._log_path = log_path
        self._login1 = login1

    def _log(self, *fields):
        with open(self._log_path, "a", encoding="utf-8") as handle:
            handle.write("\t".join(str(field) for field in fields) + "\n")
            handle.flush()
            os.fsync(handle.fileno())

    @dbus.service.method(CONTROL_IFACE, in_signature="s", out_signature="")
    def SetInhibitMode(self, mode):
        mode = str(mode)
        if mode not in INHIBIT_MODES:
            raise dbus.exceptions.DBusException(
                "unknown inhibit mode %r" % mode,
                name="lab.Mock.Error.BadMode",
            )
        self._login1.set_mode(mode)

    @dbus.service.method(CONTROL_IFACE, in_signature="", out_signature="hh")
    def TwoFds(self):
        self._log("TwoFds")
        return (
            dbus.types.UnixFd(make_memfd("discord-lab-first", b"first")),
            dbus.types.UnixFd(make_memfd("discord-lab-second", b"second")),
        )

    @dbus.service.method(CONTROL_IFACE, in_signature="hh", out_signature="s")
    def AcceptFds(self, first, second):
        first_fd = first.take()
        second_fd = second.take()
        try:
            content_first = read_all(first_fd).decode("utf-8", "replace")
            content_second = read_all(second_fd).decode("utf-8", "replace")
        finally:
            os.close(first_fd)
            os.close(second_fd)
        self._log("AcceptFds", content_first, content_second)
        return dbus.String("ok")


def main(argv):
    parser = argparse.ArgumentParser(
        description="Мок org.freedesktop.login1.Manager с unix-fd (libdbus)"
    )
    parser.add_argument("--address", required=True, help="адрес приватной шины")
    parser.add_argument("--log", required=True, help="файл журнала вызовов (TSV)")
    parser.add_argument(
        "--inhibit-mode",
        default="ok",
        choices=INHIBIT_MODES,
        help="поведение Inhibit: fd / ERROR / ответ без fd",
    )
    args = parser.parse_args(argv)

    if os.path.exists(args.log):
        os.remove(args.log)

    DBusGMainLoop(set_as_default=True)
    bus = dbus.bus.BusConnection(args.address)
    login1 = MockLogin1(bus, args.log, args.inhibit_mode)
    control = MockControl(bus, args.log, login1)

    login1_name = dbus.service.BusName(LOGIN1_DESTINATION, bus=bus)
    control_name = dbus.service.BusName(CONTROL_DESTINATION, bus=bus)
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
