# Pull Request #06: `qa/task-116-qa-build-001` → `unstable`

- **ID задачи:** `task-116` (`QA-BUILD-001-TESTING`)
- **Отдел / Воркер:** G4 QA (`codex`)
- **Исходная ветка:** `qa/task-116-qa-build-001`
- **Целевая ветка:** `unstable`
- **Статус:** `QA Passed` / `Merged to Unstable` (ветка `qa/task-116-qa-build-001` удалена после слияния)

## 1. Результаты централизованного тестирования `BUILD-001`

- Мета-политики (`tests/meta/*.test.mjs`): `102/102 pass` (`exit 0`)
- Проверка типов (`tsc` root + `rust/gui-spike/ui/tsconfig.json`): `0 errors` (`exit 0`)
- Целевые unit-наборы (`notifications-ui`, `voice-devices-settings`, `attachments-render`, `gui-spike-dom`, `gui-spike-features`): `183/183 pass` (`exit 0`)
- Контрактные тесты Rust (`notifications_contract`, `ipc_wiring_contract`, `attachments_contract`): `35/35 pass` (`exit 0`)
- **Вердикт QA:** **QA Passed (`P0=0, P1=0, P2=3`)** — сборка `BUILD-001` одобрена к переносу из `testing` в предрелизную ветку `beta`.
