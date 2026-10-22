# REPORT RW-34 — B15 headless: abstract D-Bus + расширенные hints

Волна: RW-34. Дата: 2026-10-23. Статус: принято после gate и ревью.
Контракты: `BATCH-TZ.md`, `TZ-W34a.md`, `TZ-W34b.md`.

## Результат

| Карточка | Воркер | Тесты | Мутации |
|---|---|---:|---:|
| P6-02c abstract-адреса D-Bus | PLATFORM-1 / W34a | 15 (dbus_contract) | 6/6 |
| P6-02d hints ext | PLATFORM-2 / W34b | 25 (notifications_contract) | 6/6 |

## Что сделано

- `BusAddress::{Path, Abstract}`: `parse` списка `;` (первый поддерживаемый), отказ `tcp:`/NUL/имён
  > 107 Б; `Connection::connect` для abstract — `SocketAddrExt::from_abstract_name` +
  `connect_addr` (stable, без `unsafe`); path-ветка сохранена.
- `Hints` расширены `image_path`, `sound_name`, `transient`, `suppress_sound`; порядок
  urgency → category → desktop-entry → image-path → sound-name → resident → transient →
  suppress-sound; пустые строки пропускаются, `Some(false)` пишется; сигнатура Notify не менялась.

## Оракулы

- Приватный `dbus-daemon --address=unix:abstract=…` + libdbus-мок (журнал вызовов, Hello).
- libgio-байты `Notify` (`fixtures/dbus/hints-ext.json`) + демаршалинг libdbus;
  `mock_hints2.py` — отдельная копия мока, оригинал RW-32 не тронут.

## Гейт

`waves/RW-34/gate-cargo.log`: fmt 0, clippy 0, workspace **1057 passed / 0 failed**;
`waves/RW-34/gate-worker.log` — passed.

## Ограничения

- abstract — только Linux; percent-encoding и `tcp:` не поддержаны (задокументировано).
- fd-передача (`SCM_RIGHTS`) не входит: `SocketAncillary` в текущем Rust нестабилен, нужен
  отдельный ffi-модуль или зависимость `libc` (решение владельца).
- hints — минимум freedesktop + расширения; `|`/`:` в значениях журнала мока не экранируются.
