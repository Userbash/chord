# BATCH RW-28 — P7-01c RelativeTimeFormat (headless)

Дата: 2026-10-22. Team Lead: TL-1. Tech Lead: TL-2. Статус: **frozen**.

## 1. Состав

| Карточка | Seat | Файлы (только эти) | Зависит от | Оракул |
|---|---|---|---|---|
| P7-01c RelativeTimeFormat | CLIENT-1 / W28a | `discord-i18n/src/{relative.rs,relative_data.rs,lib.rs}`, `discord-i18n/tests/relative_contract.rs`, `discord-i18n/tests/data/relative-oracle.json`, `waves/RW-28/gen-relative-data.mjs` | P7-01 `[x]` | Node 24 ICU: `Intl.RelativeTimeFormat` + `Intl.PluralRules` |

Общий файл: `discord-i18n/src/lib.rs` — только `pub mod relative;` + `pub use`.
Запрещено: чужие модули/тесты, `Cargo.toml`/`Cargo.lock`, другие крейты, доки
`STATE/STATUS/ROADMAP/TASKS/map`, golden, сеть, live Discord, `git commit/push`, `unsafe`.

## 2. Контракт

Полные сигнатуры и инварианты — `TZ-W28a.md` §1. Данные генерируются, не правятся руками.

## 3. Кейсы

`TZ-W28a.md` §3 (t01–t11). Бюджет 11–14 тестов, мутации 6–8.

## 4. Команды и DoD

`TZ-W28a.md` §4–§5. Полный gate — один раз после green; review после gate.
