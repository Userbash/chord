# ТЗ — task-278 (`DevOps / G4 QA`: `DEVOPS-P1-08` — Актуализация манифеста `BUILD-003.md` под `PR-125..PR-155`)

## Приоритет и Экономия токенов
- **Приоритет:** `P1` (замечание `P2-2` из `QA-BUILD-003-REPORT.md` и блокер из `RELEASE-NOTES-005.md`).
- **Экономия токенов:** НЕ сканируй весь репозиторий. Прочитай только `/home/sanya/orca/project/waves/RW-40/G6/task-273-release-notes-005/RELEASE-NOTES-005.md` и `/home/sanya/orca/project/waves/RW-40/QUALITY/QA-BUILD-003-REPORT.md`.

## Разрешённые файлы для записи (Write-Scope)
- `/home/sanya/orca/project/waves/RW-40/BUILDS/BUILD-003.md`
- `/home/sanya/orca/project/waves/RW-40/DEVOPS/task-278-build-003-manifest-sync/REPORT.md`

## Шаги выполнения
1. Обнови `/home/sanya/orca/project/waves/RW-40/BUILDS/BUILD-003.md`:
   - Зафиксируй актуальный HEAD ветки `testing` (`d047fe4` — синхронизированы `PR-125..PR-155`).
   - Включи полный перечень вошедших задач (`PR-125..PR-155`: 8 модулей Rust BFF включая `bff_domains_router.rs`, и 23 UI-модуля включая интеграционные мосты `shell-controllers-registry.js`, `chat-helpers-bridge.js`, `composer-popovers-bridge.js`, `app-shell-wiring.js`).
   - Укажи точные команды проверки для QA (unit-тесты 23 наборов + контрактные тесты Rust с `--manifest-path rust/Cargo.toml`) и корректные пути к оракулам в `/home/sanya/re-deep/`.
2. Запиши краткий `REPORT.md` с `md5sum` файла `waves/RW-40/BUILDS/BUILD-003.md` и вызови `orca orchestration worker-done`. Не делай `git commit`.
