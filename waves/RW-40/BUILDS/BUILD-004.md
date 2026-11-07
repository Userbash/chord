# BUILD-004 — Централизованная сборка `testing` для отдела G4 QA

- **Номер сборки:** `BUILD-004`
- **Целевая ветка интеграции:** `testing` (`/home/sanya/orca/workspaces/project/testing`)
- **Источник:** `unstable` (после слияния `PR-13`..`PR-17`)
- **Дата выпуска сборки:** `2026-10-30`
- **Статус:** `In Testing` (ожидает слота G4 QA)

## 1. Список включённых задач (прошедших PM Review + DeepSeek Tech Review + CI)

| PR | ID задачи | Ветка (удалена после merge) | Состав изменений | Статус ревью |
|---|---|---|---|---|
| `PR-13` | `task-108` | `rw40/dm-friends-spec-pass2` | `waves/RW-40/DESIGN/DM-FRIENDS-SPEC.md` | PM PASS + DeepSeek 35D PASS (`17/17 validate-pass4`) |
| `PR-14` | `task-112` | `feature/task-112-dm-friends-be` | `rust/gui-spike/src/dm_friends.rs`, `rust/gui-spike/tests/dm_friends_contract.rs` | PM PASS + DeepSeek 40B PASS (`6/6 contract, 5/5 мутаций`) |
| `PR-15` | `task-113` | `feature/task-113-dm-friends-ui` | `rust/gui-spike/ui/features/dm-friends.js`, `tests/unit/dm-friends-ui.test.mjs`, `tests/support/gui-dom-harness.mjs` | PM PASS + DeepSeek 41B PASS (`19/19 unit, 5/5 мутаций`) |
| `PR-16` | `task-107` | `rw40/int07-ui-app-mount` | `rust/gui-spike/ui/app.js`, `rust/gui-spike/ui/index.html`, `tests/unit/gui-spike-features.test.mjs` | PM PASS + DeepSeek 32C PASS (`140/140 unit, gate:worker pass, 10/10 мутаций`) |
| `PR-17` | `task-124` | `feature/task-124-voice-devices-tauri-bff` | `rust/gui-spike/src/voice_devices.rs`, `rust/gui-spike/tests/voice_devices_contract.rs` | PM PASS + DeepSeek 46B PASS (`15/15 contract, 10/10 мутаций`) |
