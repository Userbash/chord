# QA / AUTH-REPORT — offline-матрица входа и сети

Дата: 2026-10-24. QA: G4 (PM-прогон, воркеры недоступны). Окно/live не использовались.

## 1. Прогоны

| Команда | Результат |
|---|---|
| `cargo test --manifest-path rust/gui-spike/Cargo.toml --test auth_contract` | **39/0** |
| `... --test net_contract` | **6/0** |
| `... --test messages_contract` | **7/0** |
| `node --test tests/unit/gui-spike-auth.test.mjs` | **41/0** |
| `cargo test --workspace` | **1111/0** |
| `gate:release` | **passed_with_skips** (smoke 22/22; e2e 4 opt-in skip) |

## 2. Матрица

| Сценарий | Проверка | Результат |
|---|---|---|
| token ok | `auth_login_token` → keyring → ready | pass |
| token 401 | `InvalidToken`, без утечки | pass |
| password ok | `auth_login_password` → ready | pass |
| password 401 | `InvalidCredentials` | pass |
| 429 | `RateLimited{retry_after_ms}` | pass |
| captcha | `LoginOutcome::Captcha`, поля session/rqdata, adapter | pass |
| MFA totp/sms/backup/password | `auth_mfa`, коды 60006/60008 | pass |
| logout | stop gateway → keyring clear; idempotent | pass |
| restore | keyring → `/users/@me` | pass |
| secure defaults | token/password/2FA не в Debug/логах/DOM | pass |
| close 4004 | Offline + очистка токена | pass |
| close 4013/4014 | Error/stop, код сохраняется | pass |
| send message | optimistic, 401/429/сеть, retry | pass |
| live READY/send | — | **не выполнено** (нужен тестовый токен + окно) |

## 3. Утечки

- В отчётах/логах нет реальных токенов; найденные совпадения — **фейковые** тестовые значения
  (`DO-NOT-LEAK`) в mutation-логах.
- `grep` по `tests/unit/gui-spike-auth.test.mjs`, `net_contract.rs`, `messages_contract.rs`:
  `.skip/.only/todo` — **0**.

## 4. Дефекты

| Уровень | Что | Статус |
|---|---|---|
| P0 | — | нет |
| P1 | — | нет |
| P2 | нет регресс-тестов на clear-error и logout-reject | карточка A6-P2 |

## 5. Вердикт

**Offline-приёмка auth/net/send — PASS** (P0/P1=0). Live-часть (READY + send) остаётся
за согласованным окном и тестовым аккаунтом; без неё релизный gate не считается полным.
