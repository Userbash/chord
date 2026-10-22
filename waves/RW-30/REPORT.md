# REPORT RW-30 — B11 headless: P7-01d DurationFormat + P6-02 D-Bus-уведомления

Волна: RW-30. Дата: 2026-10-22. Статус: **в ревью/приёмке** → принято после гейта и ревью.
Контракты: `BATCH-TZ.md`, `TZ-W30a.md`, `TZ-W30b.md`; RE: `RE-NOTIFICATIONS.md`.

## Состав и результаты

| Карточка | Воркер | Тесты | Мутации | Отчёт |
|---|---|---:|---:|---|
| P7-01d `DurationFormat` | CLIENT-1 / W30a | 13 | 8/8 | `REPORT-W30a.md` |
| P6-02 D-Bus-уведомления | PLATFORM-1 / W30b | 7 + 11 | 10/10 | `REPORT-W30b.md` |

## Что сделано

- **DurationFormat**: 31 локаль × 3 стиля × 8 единиц (4464 паттерна, 93 набора разделителей),
  оракул 4092 кейса, генератор с проверкой гипотезы склейки (18 600 проверок, 0 расхождений).
  Плюральные категории — `plural_category_integer`, группировка — `format_integer_unsigned`.
- **D-Bus**: собственный клиент на `std` (auth EXTERNAL, `Hello`, method call/reply, `AddMatch`,
  сигналы), `org.freedesktop.Notifications` (`Notify`, `CloseNotification`, `GetCapabilities`,
  `GetServerInformation`, `ActionInvoked`/`NotificationClosed`). Без `unsafe` и новых крейтов.
- RE-вывод: на Linux Discord не загружает `discord_notifications` (`main-arch.md` §3.2) — своя
  реализация обязательна; wire-совместимость с Electron-модулем не заявляется.

## Оракулы

- ICU: Node 24.21 (`Intl.DurationFormat`, `Intl.PluralRules`), независимая сверка ревьюером —
  6144 кейса, 0 расхождений; пересчёт оракула 4092/4092.
- D-Bus: приватный `dbus-daemon` + мок-сервис на libdbus (`python3-dbus`); golden five wire
  messages побайтово сверены с libdbus `marshal/demarshal`.
- Live-уведомления не отправлялись (`DISCORD_LAB_LIVE_NOTIFY` не выставлялся).

## Ревью

- W30a: независимый агент — **принято**, P0/P1/P2 = 0; P3 закрыты (md5, red/green на финальных
  байтах, усиление тестов `ar`/10⁷, baseline-проверка мутаций).
- W30b: codex — «доработка» (P1 устаревшие md5, P2 фикстура `GetServerInformation`,
  P2 мутации с SKIP); всё закрыто: md5 пересчитаны, сигнатура исправлена на `ssss`,
  runner требует зелёный baseline без SKIP и `FAILED` для KILLED → **10/10, SKIP=0**,
  red/green перегенерены. Код продукта не изменялся по существу.

## Гейт

`waves/RW-30/gate-cargo.log`: fmt 0, clippy 0, workspace **984 passed / 0 failed** (до финальных
правок; финальный прогон — после закрытия P2), `waves/RW-30/gate-worker.log` — passed.

## Ограничения

- `hints` — всегда пустой `a{sv}`; abstract/tcp-адреса, big-endian, fd-передача (`h`) и
  percent-кодирование адреса не поддержаны (бэклог).
- Точные пороги idle→status не подтверждены RE (будут конфигом в P6-06).
- DurationFormat привязан к ICU 78 (Node 24.21); при смене ICU данные перегенерировать.
- Wire-совместимость с клиентом Discord не заявляется.
