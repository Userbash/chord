# REPORT — task-296 (`DOCS-P2-06`, G6 Tech Writer / Parity): Итоговый отчёт `MERGE-READINESS-006.md` и обновление манифеста `BUILD-003.md`

- **batch/карточка:** RW-40 / task-296, ветка `docs/task-296-merge-readiness-006`
- **воркер:** `G6 Tech Writer / Parity`, рабочее дерево `/home/user/orca/workspaces/project/docs-task-296-merge-readiness-006`
- **строгий Write-Scope:** ровно 2 файла (`waves/RW-40/G6/MERGE-READINESS-006.md`, `waves/RW-40/BUILDS/BUILD-003.md`) + локальный `REPORT.md`.

## 1. Сделано

1. **Создан итоговый отчёт готовности к слиянию `waves/RW-40/G6/MERGE-READINESS-006.md`:**
   - Зафиксирована полная сводка всех влитых PR финала волны RW-40 (`PR-137..PR-175`).
   - Детально описана ликвидация всех 3 интеграционных разрывов:
     - Единый роутер BFF `bff_domains_router.rs` (`PR-153`/`PR-164`) и регистрация команды `invoke_bff_domain` в Tauri IPC (`PR-173`).
     - Монтирование фасадов оболочки `app-shell-wiring.js` (`PR-155`/`PR-157`) и `composer-popovers-bridge.js` (`PR-154`/`PR-163`) в `app.js` (`PR-172`).
     - Полный сквозной стек WebRTC диагностики `RE-GAP-22` (`stats.rs` `PR-170`, `VOICE-DIAGNOSTICS-SPEC.md` `PR-171`, `voice-diagnostics.js` `PR-169`, `voice_diagnostics.rs` `PR-174`, `voice.js` `PR-175`).
   - Проверены и зафиксированы результаты выполнения всех гейтов (`npm run typecheck`, `ui/tsconfig.json`, `test:meta`, `dispatch_ui`, `test-impact`, targeted contracts).
   - Вынесен вердикт: **`READY FOR BETA & RELEASE (P0 = 0, P1 = 0)`** с диаграммой продвижения по веткам `unstable -> testing -> beta -> main`.
2. **Обновлён манифест централизованной сборки `waves/RW-40/BUILDS/BUILD-003.md`:**
   - Актуализирован HEAD сборки (`7f96e1b` в `testing`), счётчики модулей (36 pub mod, 53 Tauri commands, 115 unit-тестов, 98 feature-модулей).
   - Таблица включённых PR расширена до полного диапазона `PR-125..PR-175` со статусом подключения каждого модуля в рантайм.
   - Обновлены инструкции по верификации сборки для отдела G4 QA.

## 2. Верификация и метрики

| Проверка | Команда | Результат |
|---|---|---|
| Root & Agent Typecheck | `npm run typecheck` | 0 errors (PASS) |
| UI Typecheck | `npx tsc -p rust/gui-spike/ui/tsconfig.json --noEmit` | 0 errors (PASS) |
| Meta Tests | `npm run test:meta` | 102/102 PASS |
| Dispatch UI Contracts | `node --test tests/unit/dispatch_ui.test.mjs` | 58/58 PASS |
| Test Impact Rules | `node --test tests/unit/test-impact.test.mjs` | 57/57 PASS |
| App Shell & Popovers | `node --test tests/unit/app-shell-wiring.test.mjs tests/unit/composer-popovers-bridge.test.mjs` | 35/35 PASS |
| Voice Diagnostics | `node --test tests/unit/voice-diagnostics.test.mjs tests/unit/voice-ui.test.mjs` | 114/114 PASS |
| BFF Contracts | `cargo test --test bff_domains_router_contract --test voice_stage_contract` | 18/18 PASS |
| Voice Crate Lib | `cargo test -p discord-voice --lib` | 36/36 PASS |
| Platform Safe Storage | `cargo test -p discord-platform --test safe_storage_contract` | 13/13 PASS |

## 3. Файлы и md5

```
waves/RW-40/G6/MERGE-READINESS-006.md
waves/RW-40/BUILDS/BUILD-003.md
waves/RW-40/G6/task-296-merge-readiness-006/REPORT.md
```

## 4. Риски и статус

- Все блокирующие риски закрыты (`P0 = 0, P1 = 0`).
- Сборка готова к приёмке PM и Технического ревьюера с последующим переводом `testing -> beta -> main`.
