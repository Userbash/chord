# BATCH RW-33 — P1-10 keyring через libsecret (headless)

Дата: 2026-10-22. Team Lead: TL-1. Статус: **frozen**.

## 1. Состав

| Карточка | Seat | Файлы | Зависит от | Оракул |
|---|---|---|---|---|
| P1-10 Keyring (libsecret FFI) | SEC-1 / W33a | `discord-platform/src/keyring.rs`, `src/ffi_secret.rs`, `src/lib.rs` (+2 строки), `tests/keyring_contract.rs`, `waves/RW-33/keyring/*` | системный `libsecret 0.21` + `org.freedesktop.secrets` | `secret-tool` CLI + реальный ksecretd на сессионной шине |

Запрещено: чужие модули/тесты, `Cargo.toml`/workspace-манифесты (кроме одной строки lint в
`discord-platform/Cargo.toml`, если понадобится), golden, сеть, live Discord, commit.
`unsafe` — только в `ffi_secret.rs` с `// SAFETY:`; `unsafe_code = "deny"` уже стоит.

## 2. Контракт

`TZ-W33a.md` §1.

## 3. Кейсы и бюджет

k01–k12, 10–12 тестов; live-кейсы skip-aware (`DISCORD_LAB_KEYRING_LIVE=1`), мутации 5–7.

## 4. Гейт и DoD

`CARGO_TARGET_DIR=/tmp/opencode/target-w33a`, targeted-тесты; полный gate — один раз в
integration-лейне. DoD: red → green → мутации (SKIP=0 для не-live) → отчёт → ревью → accepted.

## 5. Ограничения

Секреты в тестах — синтетические (`test-secret-…`), после теста `clear`; реальные токены
не используются. Если ksecretd требует разблокировки — live-кейсы skip с причиной.
