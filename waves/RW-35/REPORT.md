# REPORT RW-35 — B16 headless: fd-передача D-Bus + logind sleep-inhibitor

Волна: RW-35. Дата: 2026-10-23. Статус: принято после gate и ревью.
Контракт: `TZ-W35a.md`; `BATCH-TZ.md`. Решение владельца: `libc = "0.2"` (офлайн-кэш).

## Результат

| Карточка | Воркер | Тесты | Мутации |
|---|---|---:|---:|
| P6-06c fd-передача + sleep-inhibitor | PLATFORM-1 / W35a | dbus 21, power 16 | 6/6 |

## Что сделано

- `dbus/fd.rs`: `sendmsg`/`recvmsg` с `SCM_RIGHTS`, `CMSG_*`, `MSG_CTRUNC`; лимит 16 fd на
  приём / 253 на отправку; все fd закрываются на error-путях.
- `Value::UnixFd`, `parse_with_fds`/`build_with_fds`, заголовок `UNIX_FDS` (код 9),
  `Connection::call_with_fds`; `call` закрывает fd из ответа и возвращает `Protocol`.
- `PowerMonitor::inhibit_sleep` → `org.freedesktop.login1.Manager.Inhibit("sleep", …)` → fd;
  `SleepInhibitor` с `fd()` и `Drop → close` ровно один раз.
- `NEGOTIATE_UNIX_FD` реализован в `dbus/mod.rs` (auth.rs был вне файловой матрицы) — отклонение
  задокументировано, перенос в `auth.rs` — отдельная карточка/бэклог.

## Оракулы

- Приватный `dbus-daemon` + `waves/RW-35/power/mock_logind.py` (python3-dbus): `Inhibit → h`,
  `TwoFds hh`, `AcceptFds hh`, режимы error/no-fd; журнал аргументов.
- Live `systemd-logind` по `DISCORD_LAB_POWER_LIVE=1`: реальный fd получен, ингибитор сразу снят.

## Гейт

`waves/RW-35/gate-cargo.log`: fmt 0, clippy 0, workspace **1067 passed / 0 failed**;
`waves/RW-35/gate-worker.log` — passed.

## Ограничения

- fd-передача Linux-only; fd не наследуется через fork; live-тест не удерживает сон.
- `libc` — новая офлайн-зависимость (разрешена владельцем), `unsafe` изолирован в `dbus/fd.rs`.
- Координация `NEGOTIATE_UNIX_FD` дублирует часть auth-логики — риск для будущего рефакторинга.
