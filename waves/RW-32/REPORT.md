# REPORT RW-32 — B13 headless: P6-07 spellcheck + P6-02b hints

Волна: RW-32. Дата: 2026-10-22. Статус: принято после gate и ревью.
Контракты: `BATCH-TZ.md`, `TZ-W32a.md`, `TZ-W32b.md`.

## Состав и результаты

| Карточка | Воркер | Тесты | Мутации | Отчёт |
|---|---|---:|---:|---|
| P6-07 Spellcheck (hunspell FFI) | PLATFORM-2 / W32a | 13 | 8/8 | `REPORT-W32a.md` |
| P6-02b D-Bus hints `a{sv}` | PLATFORM-1 / W32b | 8 + 18 | 6/6 | `REPORT-W32b.md` |

## Что сделано

- **P6-07**: FFI к системному `libhunspell 1.7.5` (`#[link_name]` для `Hunspell_*`), безопасный
  `SpellChecker` (check/suggest/add/remove, `Drop` без двойного destroy, `!Send + !Sync`),
  токенайзер с байтовыми смещениями: Unicode, апостроф/дефис, пропуск URL/e-mail/`code`/
  `<@…>`/`<#…>`/`:emoji:`. Pre-validation путей (hunspell 1.7.5 не сигналит NULL на битом
  словаре). `unsafe` — только в `ffi_hunspell.rs`.
- **P6-02b**: `Urgency`/`Hints` → `a{sv}` (urgency `y`, category/desktop-entry `s`, resident `b`),
  пустые строки пропускаются, порядок фиксирован; `notify` делегирует в `notify_with_hints`;
  сигнатура `susssasa{sv}i` не менялась.

## Оракулы

- Spellcheck: libhunspell + синтетические `fixtures/hunspell/{test.aff,test.dic}`; CLI-оракула
  нет — проверяются семантические инварианты, детерминизм suggest, 100 циклов Drop/RSS.
- Hints: libgio/libdbus-мок (`waves/RW-32/dbus/mock_hints.py`) журналирует ключ:сигнатура:значение;
  `fixtures/dbus/hints.json` — байты `Notify` и снимок демаршалинга, генератор `gen-hints.py`.

## Гейт

`waves/RW-32/gate-cargo.log`: fmt 0, clippy 0, workspace **1031 passed / 0 failed**;
`waves/RW-32/gate-worker.log` — passed.

## Ограничения

- libhunspell/libsecret — системные библиотеки (динамическая линковка); словари в системе
  отсутствуют, тесты используют синтетический словарь.
- Hints — только минимум freedesktop; `image-path`, `sound-name`, `transient` не заявляются.
- fd-передача и abstract-адреса D-Bus — вне карточки (бэклог RW-30/31).


## Пост-ревью фиксы

- Codex P1: словарь `1\n@@@` больше не принимается — усилена валидация `.aff`/`.dic`,
  тест h10 расширен; red/green/мутации пересняты (8/8, SKIP=0).
- Codex P2: его sandbox не поднимает приватный D-Bus — hints проверены отдельным агентом на
  живой шине (см. отчёт W32b/журнал ревью).
- Финальный gate: `.gate/worker-2026-10-22T18-29-31-203Z.json`.
