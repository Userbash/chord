# BATCH RW-35 — fd-передача D-Bus + logind sleep-inhibitor (headless)

Дата: 2026-10-23. Team Lead: TL-1. Статус: **frozen**.
Решение владельца: разрешена офлайн-зависимость `libc 0.2.19x` (есть в кэше).

## 1. Состав

| Карточка | Seat | Файлы | Оракул |
|---|---|---|---|
| P6-06c fd-передача + sleep-inhibitor | PLATFORM-1 / W35a | `discord-platform/src/{dbus/mod.rs,dbus/message.rs,dbus/fd.rs,power.rs}`, `Cargo.toml` (+`libc`), `tests/{dbus_contract.rs,power_contract.rs}`, `waves/RW-35/power/mock_logind.py`; следствие зависимости: `rust/Cargo.lock`, `rust/dist/sbom.spdx.json` | приватный `dbus-daemon` + libdbus-мок, отдающий `UnixFd` |

Запрещено: другие крейты, workspace `Cargo.toml`/`Cargo.lock` (кроме `cargo`-резолва `libc`),
`notifications.rs` (кроме чтения), `spellcheck*`, `keyring*`, golden, сеть, live, commit.
`unsafe` — только в `dbus/fd.rs` (и существующих ffi-модулях), `// SAFETY:` обязательны.

## 2. Контракт

`TZ-W35a.md` §1.

## 3. Кейсы и бюджет

f01–f10, 9–11 тестов; мутации 5–7; live logind — skip-aware (`DISCORD_LAB_POWER_LIVE=1`).

## 4. DoD

red → green → мутации (`--nocapture`, SKIP=0 не-live) → отчёт → ревью → gate → push.
