# TASK-300 — Продвижение `testing -> beta` (`Build-003`) и предрелизная проверка `beta` (`L1 / P1`, сжатый контекст)

- **Деревья:** `/home/sanya/orca/workspaces/project/unstable`, `/home/sanya/orca/workspaces/project/testing`, `/home/sanya/orca/workspaces/project/beta`. Не читай другие файлы репозитория.
- **Шаги (выполнить строго по порядку):**
  1. Скопируй `/home/sanya/orca/project/waves/RW-40/QUALITY/QA-BUILD-003-REPORT.md` и `/home/sanya/orca/project/waves/RW-40/PM/MODEL-ROUTING-CRITERIA.md` в `unstable`, закоммить и запушь `origin unstable`, затем слей в `testing` (`git merge --no-ff -X theirs unstable`) и запушь `origin testing`.
  2. В `/home/sanya/orca/workspaces/project/beta` слей проверенную ветку `testing`:
     ```bash
     cd /home/sanya/orca/workspaces/project/beta && git merge --no-ff -X theirs testing -m "Promote Build-003 (PR-137..PR-178) from testing to beta (QA Passed)"
     ```
  3. Прогони предрелизную проверку в `/home/sanya/orca/workspaces/project/beta`:
     ```bash
     npm run typecheck && npm run test:meta
     CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target cargo test --manifest-path rust/gui-spike/Cargo.toml --test perf_contract --test ipc_wiring_contract --test voice_diagnostics_contract
     git push origin beta
     ```
  4. Запиши компактный отчёт (10 строк: коммит `beta`, результаты `perf_contract` / `test:meta` / `typecheck`, вердикт `Beta Passed — Ready for Release`) в `/home/sanya/orca/project/waves/RW-40/DEVOPS/task-300-promote-testing-to-beta/REPORT.md`.
