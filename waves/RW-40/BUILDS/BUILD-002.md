# Централизованная сборка QA: `BUILD-002` (ветка `testing`)

- **Номер сборки:** `BUILD-002`
- **Ветка сборки:** `testing` (`/home/user/orca/workspaces/project/testing`)
- **Источник интеграции:** `unstable` (`/home/user/orca/workspaces/project/unstable`)
- **Предыдущая сборка:** [`BUILD-001.md`](file:///home/user/orca/project/waves/RW-40/BUILDS/BUILD-001.md) — статус **QA Passed** (`QA-BUILD-001-REPORT.md`), перенесена в предрелизную ветку `beta` (`/home/user/orca/workspaces/project/beta`).
- **Дата сборки:** 2026-10-30

---

## 1. Список новых задач, вошедших в `BUILD-002` (поверх `BUILD-001`)

| ID задачи | Срез / Компонент | Pull Request | Продуктовое ревью (PM) | Техническое ревью (DeepSeek) | Ключевые файлы и MD5 |
|---|---|---|---|---|---|
| `task-110` | `M-13-SEARCH-202-NSFW-BACKEND` (Pass 2): неблокирующий IPC-конверт `HTTP 202` (`indexing: true, retry_after_ms`), `execute_with_retry` с `&attempts=1..5`, `documents_indexed: u64` default `0`, `include_nsfw=true` только при `Some(true)` | [`PR-04`](file:///home/user/orca/project/waves/RW-40/PR/PR-04-task-110-search-202-be.md) | **APPROVED** (`ACCEPT.md`) | **PASS (`P0=0, P1=0, P2=2`)** (`DeepSeek 34B` — `REVIEW-PASS2.md`) | `rust/gui-spike/src/search.rs` (`3ec29ae3219be411465ce3b2cc749287`), `rust/gui-spike/tests/search_contract.rs` (`f113b75de5727b82fe1ca5dc0f57eb3b`) |
| `task-109` | `BUGFIX-IPC-ORACLE-STRICT`: строгий оракул `38/38` UI IPC-команд (`AUTH_COMMANDS`, `JOIN_COMMANDS`, `NET_COMMANDS`, `READY_COMMAND`), строгий `Err(NotificationError::InvalidChannelId)` без fallback на чужой канал, поддержка поля `override` (`r#override`) | [`PR-05`](file:///home/user/orca/project/waves/RW-40/PR/PR-05-task-109-ipc-oracle-strict.md) | **APPROVED** (`ACCEPT.md`) | **PASS (`P0=0, P1=0, P2=0`)** (`DeepSeek 36A` — `REVIEW.md`) | `rust/gui-spike/src/main.rs` (`24bb37a8f233213ba7c59dc7b9fb1b22`), `rust/gui-spike/tests/ipc_wiring_contract.rs` (`01929cf0e635a2e4ef946e416a61801f`) |
| `task-116` | Отчёт централизованного тестирования `BUILD-001` (`QA Passed`) | [`PR-06`](file:///home/user/orca/project/waves/RW-40/PR/PR-06-task-116-qa-build-001.md) | **APPROVED** | **QA Passed** | `waves/RW-40/QUALITY/QA-BUILD-001-REPORT.md` |
| `task-118` | Релизная матрица паритета Wave 10 (`MERGE-READINESS-14.md`) | [`PR-07`](file:///home/user/orca/project/waves/RW-40/PR/PR-07-task-118-parity-wave10.md) | **APPROVED** | **Verified** | `waves/RW-40/PARITY/MERGE-READINESS-14.md` (`6f80719b979a9cc6f168bdcc31dc3087`) |

---

## 2. Инструкции по проверке для QA (`qa/task-120-qa-build-002`)

1. **Контрактные тесты новых задач `PR-04` и `PR-05` (Rust):**
   ```bash
   cargo test --manifest-path rust/gui-spike/Cargo.toml --offline --test search_contract --test ipc_wiring_contract --test notifications_contract --test attachments_contract
   cargo test --manifest-path rust/gui-spike/Cargo.toml --offline --bin discord-gui-spike
   cargo test --manifest-path rust/gui-spike/Cargo.toml --offline --lib search
   ```
   Ожидается: `search_contract` (`23/23`), `ipc_wiring_contract` (`9/9`), `notifications_contract` (`23/23`), `attachments_contract` (`4/4`), `--bin discord-gui-spike` (`12/12`), `--lib search` (`2/2`) — `0 failed`.
2. **Мета-политики и TypeScript:**
   ```bash
   node --test tests/meta/*.test.mjs
   npx tsc -p tsconfig.json
   npx tsc -p rust/gui-spike/ui/tsconfig.json
   ```
   Ожидается: `102/102` meta pass, `0` ошибок `tsc`.
3. **Регрессионный прогон unit-наборов:**
   ```bash
   npm run test:unit
   ```
   Ожидается: `879/879 pass` (`0 fail`).
4. **Оформление отчёта:**
   Зафиксировать версию сборки (`BUILD-002`), окружение, MD5 проверенных файлов (`search.rs`, `search_contract.rs`, `main.rs`, `ipc_wiring_contract.rs`), шаги воспроизведения, фактический и ожидаемый результат, логи и вердикт (`QA Passed` или `QA Failed`) в `waves/RW-40/QUALITY/QA-BUILD-002-REPORT.md`.

---

## 3. Тестовые данные и известные ограничения

- **Тестовые данные (read-only):**
  - `fixtures/gateway/by-type/002-READY.json`
  - `waves/RW-40/SEARCH-202-NSFW-RE.md` и `waves/RW-40/DESIGN/SEARCH-202-NSFW-SPEC.md`
- **Известные ограничения:**
  - Задачи `task-107` (`INT-07-UI-APP-MOUNT` Pass 2) и `task-111` (`M-13-SEARCH-202-NSFW-UI` Pass 2) находятся в доработке (`In Progress`) в изолированных ветках воркеров и войдут в `BUILD-003` после прохождения Product Review и Technical Review.
  - Актуализация `rust/dist/SHA256SUMS` и `scripts/release.sh` (`CARGO_TARGET_DIR`) по находкам `DEVOPS-BUILD-001-REPORT.md` выполняется параллельно в задаче `devops/task-119-fix-release-sha256-and-beta`.
