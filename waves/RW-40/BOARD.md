# Доска управления разработкой RW-40 (PM / Tech Lead / QA / DevOps / Release)

- **Текущий оркестрационный Run:** `run_7c3a532bf84a`
- **Координатор и Продуктовый Менеджер (PM):** Team Lead (L2)
- **Технический ревьюер (Tech Reviewer):** `deepseek/deepseek-v4.1-flash` (`opencode`, независимые сессии ревью на каждую задачу/PR)
- **Дата обновления:** 2026-10-30 20:33 +05:00

---

## 1. Иерархия веток и окружений

| Ветка | Путь Worktree | Назначение и правила |
|---|---|---|
| `<type>/task-<id>-<slug>` | `/home/sanya/orca/workspaces/project/<type>-task-<id>-<slug>` | Изолированная рабочая ветка воркера (создаётся от актуального `unstable`). После слияния PR в `unstable` ветка **немедленно удаляется** (`orca worktree rm`). |
| `unstable` | `/home/sanya/orca/workspaces/project/unstable` | Ветка интеграции задач, прошедших CI, продуктовое ревью (PM) и техническое ревью (`DeepSeek`). |
| `testing` | `/home/sanya/orca/workspaces/project/testing` | Ветка централизованной сборки для QA (`BUILD-001`, `BUILD-002`, ...). Прямая передача задач в QA из веток воркеров запрещена. |
| `beta` | `/home/sanya/orca/workspaces/project/beta` | Предрелизная ветка для финальной проверки версии, прошедшей `QA Passed` в `testing` (сейчас на ней проверяется `BUILD-001`). |
| `main` | `/home/sanya/orca/project` | Стабильная продуктивная ветка. Прямые коммиты запрещены; принимает только PR из `beta` после полной предрелизной проверки. |

---

## 2. Сводка загрузки отделов (0 простаивающих отделов, 11 активных воркеров)

| Отдел | Активные воркеры | Текущие задачи и ветки |
|---|---|---|
| **G1 Backend (Rust / BFF / IPC)** | 3 параллельных воркера (`ctx_f47522db8e31`, `ctx_5ca572207cd5`, `ctx_67d715b76046`) | `task-112` (`feature/task-112-dm-friends-be`), `task-122` (`feature/task-122-settings-store-home-mode`), `task-123` (`feature/task-123-reactions-extended-be`) |
| **G2 Frontend (UI / DOM / A11y)** | 3 параллельных воркера (`ctx_4b88b798918f`, `ctx_719facef6f3f`, `ctx_d4576ebad972`) | `task-113` (`feature/task-113-dm-friends-ui`), `task-107` Pass 2 (`rw40/int07-ui-app-mount`), `task-111` Pass 2 (`rw40/m13-search-202-ui`) |
| **G3 Design (UI/UX Specs & Tokens)** | 1 воркер (`ctx_b625e6c5e7bc`) | `task-108` Pass 3 (`rw40/dm-friends-spec-pass2` — `DM-FRIENDS-SPEC.md`) |
| **G4 QA (Testing & Regression)** | 1 воркер (`ctx_783032e98b29`) | `task-120` (`qa/task-120-qa-build-002` — централизованное тестирование `BUILD-002`) |
| **G5 Voice / Platform (Rust Voice)** | 1 воркер (`ctx_88d69abb5f90`) | `task-117` (`feature/task-117-voice-stream-wiring` — `session.rs` + `session_contract.rs`) |
| **G6 Tech Writer / Parity** | 1 воркер (`ctx_a5eb5953010b`) | `task-121` (`docs/task-121-parity-wave11` — `MERGE-READINESS-15.md` + `RELEASE-NOTES-V0.2.0.md`) |
| **DevOps / Release Integrator** | 1 воркер (`ctx_b580044e1d8f`) | `task-119` (`devops/task-119-fix-release-sha256` — `scripts/release.sh`, `SHA256SUMS` и проверка `beta`) |
| **Tech Reviewer (`DeepSeek`)** | 1 ревьюер (`ctx_2519982c016f`) | `Review 38A` (`design/task-114-reactions-picker-spec` — `REACTIONS-EXTENDED-SPEC.md`) |

---

## 3. Реестр задач по 13 обязательным статусам

Допустимые статусы:
`Backlog` → `Ready for Development` → `In Progress` → `Ready for Product Review` → `Ready for Technical Review` → `Ready for Merge` → `Merged to Unstable` → `In Testing` → (`QA Failed` | `QA Passed`) → `In Beta` → `Ready for Release` → `Released`.

| ID задачи | Отдел / Роль | Рабочая ветка (от `unstable`) | Описание и артефакт | PR / Ревью | Текущий статус (из 13) |
|---|---|---|---|---|---|
| `task-101..103` | Все отделы (`G1–G6`) | (слиты и удалены) | Базовый пакет 35 принятых срезов RW-40 (`L-02`, `STUB-27`, `C-02`, `M-07`, `M-08`, `P-03`, `B2a/B2b`, `AT-03`, `M-10/STUB-10`, `STUB-05`, `STUB-16`, `MB-03`, `V-01/V-06/V-07`, `N-05-BE`, `INT-05`) | [`BUILD-001.md`](file:///home/sanya/orca/project/waves/RW-40/BUILDS/BUILD-001.md), [`QA-BUILD-001-REPORT.md`](file:///home/sanya/orca/project/waves/RW-40/QUALITY/QA-BUILD-001-REPORT.md) | `In Beta` (`BUILD-001` перенесён в `beta`) |
| `task-104` | G2 Frontend | `rw40/review-n05-ui-pass2` (удалена после merge) | `N-05-UI-NOTIFICATIONS` Pass 2: закрытие P1 (`NodeList.filter`) и 7 P2 в `notifications.js` + `17/17` unit, `5/5` мутаций | [`PR-03`](file:///home/sanya/orca/project/waves/RW-40/PR/PR-03-task-104-n05-ui-notifications.md), [`REVIEW-PASS2.md`](file:///home/sanya/orca/project/waves/RW-40/PM/n05-ui-notifications/REVIEW-PASS2.md) (`PASS`) | `In Beta` (`BUILD-001` → `beta`) |
| `task-105` | G3 Design | `rw40/search-202-nsfw-spec-pass2` (удалена после merge) | `SEARCH-202-NSFW-SPEC.md` Pass 2: спецификация поиска `HTTP 202` и `NSFW` | [`PR-01`](file:///home/sanya/orca/project/waves/RW-40/PR/PR-01-search-202-nsfw-spec.md), [`REVIEW-PASS2.md`](file:///home/sanya/orca/project/waves/RW-40/DESIGN/SEARCH-202-NSFW-SPEC-REVIEW-PASS2.md) (`PASS`) | `In Beta` (`BUILD-001` → `beta`) |
| `task-106` | G1 Backend | `feature/task-106-notif-ipc` (удалена после merge) | `INT-06-NOTIFICATIONS-IPC-WIRING`: регистрация Tauri IPC-команд уведомлений в `main.rs` (`F-5`) | [`PR-02`](file:///home/sanya/orca/project/waves/RW-40/PR/PR-02-task-106-notif-ipc.md), [`REVIEW.md`](file:///home/sanya/orca/project/waves/RW-40/PM/int06-notif-ipc/REVIEW.md) (`PASS`) | `In Beta` (`BUILD-001` → `beta`) |
| `task-116` | G4 QA | `qa/task-116-qa-build-001` (удалена после merge) | Централизованное тестирование `BUILD-001` (`meta 102/102`, `tsc 0`, `unit 183/183`, `cargo 35/35`) | [`PR-06`](file:///home/sanya/orca/project/waves/RW-40/PR/PR-06-task-116-qa-build-001.md), [`QA-BUILD-001-REPORT.md`](file:///home/sanya/orca/project/waves/RW-40/QUALITY/QA-BUILD-001-REPORT.md) | `QA Passed` → `In Beta` |
| `task-118` | G6 Tech Writer | `docs/task-118-parity-wave10` (удалена после merge) | Релизная матрица паритета Wave 10 (`MERGE-READINESS-14.md`) | [`PR-07`](file:///home/sanya/orca/project/waves/RW-40/PR/PR-07-task-118-parity-wave10.md), [`MERGE-READINESS-14.md`](file:///home/sanya/orca/project/waves/RW-40/PARITY/MERGE-READINESS-14.md) | `In Testing` (`BUILD-002`) |
| `task-109` | G1 Backend | `bugfix/task-109-ipc-oracle-strict` (удалена после merge) | Строгий оракул `38/38` UI IPC-команд, отказ от тихого fallback на чужой канал и поле `override` | [`PR-05`](file:///home/sanya/orca/project/waves/RW-40/PR/PR-05-task-109-ipc-oracle-strict.md), [`REVIEW.md`](file:///home/sanya/orca/project/waves/RW-40/PM/task109-ipc-oracle/REVIEW.md) (`PASS, P0=0, P1=0, P2=0`) | `In Testing` (`BUILD-002`) |
| `task-110` | G1 Backend | `rw40/m13-search-202-be` (удалена после merge) | `M-13-SEARCH-202-NSFW-BACKEND` Pass 2: неблокирующий IPC-конверт `HTTP 202` (`indexing: true, retry_after_ms`) + `execute_with_retry` (`23/23` `search_contract`) | [`PR-04`](file:///home/sanya/orca/project/waves/RW-40/PR/PR-04-task-110-search-202-be.md), [`REVIEW-PASS2.md`](file:///home/sanya/orca/project/waves/RW-40/PM/m13-search-be/REVIEW-PASS2.md) (`PASS, P0=0, P1=0`) | `In Testing` (`BUILD-002`) |
| `task-114` | G3 Design | `design/task-114-reactions-picker-spec` | `REACTIONS-EXTENDED-SPEC.md`: спецификация поповера выбора реакций и быстрых реакций сообщения (84/84 токена) | На ревью у `DeepSeek Review 38A` (`ctx_2519982c016f`) | `Ready for Technical Review` |
| `task-107` | G2 Frontend | `rw40/int07-ui-app-mount` | `INT-07-UI-APP-MOUNT` Pass 2: устранение 2 P1 (`aria-disabled` guard) и 8 P2 из [`REVIEW.md`](file:///home/sanya/orca/project/waves/RW-40/PM/int07-ui-mount/REVIEW.md) | В работе у G2 Frontend (`ctx_719facef6f3f`) | `In Progress` |
| `task-111` | G2 Frontend | `rw40/m13-search-202-ui` | `M-13-SEARCH-202-NSFW-UI` Pass 2: устранение 3 P1 (4 CSS-токена, контраст `18+`, `localStorage` accessor) и 8 P2 из [`REVIEW.md`](file:///home/sanya/orca/project/waves/RW-40/PM/m13-search-ui/REVIEW.md) | В работе у G2 Frontend (`ctx_d4576ebad972`) | `In Progress` |
| `task-108` | G3 Design | `rw40/dm-friends-spec-pass2` | `DM-FRIENDS-SPEC.md` Pass 3: устранение 1 P1 (персистентность Home) и 11 P2 из [`DM-FRIENDS-SPEC-REVIEW-PASS2.md`](file:///home/sanya/orca/project/waves/RW-40/DESIGN/DM-FRIENDS-SPEC-REVIEW-PASS2.md) | В работе у G3 Design (`ctx_b625e6c5e7bc`) | `In Progress` |
| `task-112` | G1 Backend | `feature/task-112-dm-friends-be` | `N-107-DM-FRIENDS-BACKEND`: `rust/gui-spike/src/dm_friends.rs` + `tests/dm_friends_contract.rs` | В работе у G1 Backend (`ctx_f47522db8e31`) | `In Progress` |
| `task-113` | G2 Frontend | `feature/task-113-dm-friends-ui` | `N-107-DM-FRIENDS-UI`: `rust/gui-spike/ui/features/dm-friends.js` + `tests/unit/dm-friends-ui.test.mjs` | В работе у G2 Frontend (`ctx_4b88b798918f`) | `In Progress` |
| `task-117` | G5 Voice | `feature/task-117-voice-stream-wiring` | `V-08-VOICE-STREAM-SESSION-WIRING`: интеграция выбора устройств и `stream_key` в `discord-voice/src/session.rs` | В работе у G5 Voice (`ctx_88d69abb5f90`) | `In Progress` |
| `task-119` | DevOps | `devops/task-119-fix-release-sha256` | Исправление `scripts/release.sh` (`CARGO_TARGET_DIR`), пересборка `rust/dist` + `SHA256SUMS` и предрелизная проверка `beta` | В работе у DevOps (`ctx_b580044e1d8f`) | `In Beta` / `In Progress` |
| `task-120` | G4 QA | `qa/task-120-qa-build-002` | Централизованное тестирование `BUILD-002` (`PR-04` + `PR-05` + полная регрессия) → `QA-BUILD-002-REPORT.md` | В работе у G4 QA (`ctx_783032e98b29`) | `In Testing` |
| `task-121` | G6 Tech Writer | `docs/task-121-parity-wave11` | Подготовка `MERGE-READINESS-15.md` и `RELEASE-NOTES-V0.2.0.md` | В работе у G6 (`ctx_a5eb5953010b`) | `In Progress` |
| `task-122` | G1 Backend | `feature/task-122-settings-store-home-mode` | Поддержка `lastGuildId: "@me"` и `homeMode` в `settings_store.rs` + `settings_contract.rs` (закрытие P1 `DM-FRIENDS-SPEC`) | В работе у G1 Backend (`ctx_5ca572207cd5`) | `In Progress` |
| `task-123` | G1 Backend | `feature/task-123-reactions-extended-be` | Расширенный REST/429/custom-emoji контракт реакций в `reactions.rs` + `reactions_contract.rs` | В работе у G1 Backend (`ctx_67d715b76046`) | `In Progress` |
