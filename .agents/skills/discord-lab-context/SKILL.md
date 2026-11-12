---
name: discord-lab-context
description: Карта репозитория discord-lab (анализ Discord через Frida/CDP + Rust-клиент с оболочкой Tauri) — что читать первым, где что лежит, команды (гейты, тесты, Rust-воркспейс), каталоги только для чтения, где журнал волн и оракулы RE. Использовать при старте работы в этом репозитории, когда нужен быстрый контекст проекта. Регламент ролей — в скиллах discord-lab-process / discord-lab-worker / discord-lab-review.
---

# discord-lab — компас по репозиторию

Проект: локальный CLI для анализа Discord (Frida/CDP) и Rust-клиент Discord (оболочка —
Tauri). Разработка идёт волнами по hard TDD. Источник истины по процессу — `PROCESS.md`.

## Что читать первым (just-in-time)

1. `PROJECT-CONTEXT.md` — краткий статус, ограничения и навигация.
2. `git rev-parse --short HEAD`; проверяй dirty-файлы только для назначенной области (`git status --short -- <paths>`).
3. Перед кодом точечно прочитай TZ текущего batch, запись карточки в `TASKS.md`, нужный контракт `TDD-PLAN.md` и применимые разделы `PROCESS.md`/`TEAM.md`.
4. `waves/STATE.md` используй как указатель текущего состояния; читай только верхний статус и релевантный раздел. Историю `waves/db.jsonl` проверяй через `tail -n 5`.
5. Полные планы `PLAN.md`/`TEST-PLAN.md` и архивные волны открывай только если контракт/зависимость неясны.

## Карта

- `rust/` — Cargo-воркспейс (10 крейтов): `discord-core` (протокол/ws/dispatch), `discord-codec`,
  `discord-store`, `discord-platform`, `discord-ui`, `discord-voice`, `discord-media`,
  `discord-i18n`, `discord-app`, `discord-conformance`.
- `src/`, `bin/`, `scripts/` — Node ≥ 24 (ESM, JSDoc): CLI, CDP-скрипты, `scripts/gate.mjs`.
- `tests/` — `unit/`, `meta/` (форматы: §8, журнал волн), `smoke/`, `e2e/`, `helpers/`.
- `waves/` — журнал волн: `STATE.md` (точка восстановления), `db.jsonl` (append-only),
  `RW-XX/` (ТЗ, red/green-логи, отчёты), `reviews/`; правила — `waves/README.md`.
- `fixtures/` — стабы/данные (в т.ч. `gw-proto/`, `rest/`); `docs/re/` — RE-отчёты (оракулы).
- `docs/` — `map.md`, `feature-backlog.md`, `rust-client/`, `re/`.
- Скиллы: `.opencode/skills/` (процесс/ревью/воркер) и `.agents/skills/` (официальные Orca +
  этот компас); `skills-lock.json` — пины установленных скиллов.

## Только для чтения

- `logs/`, `functions/`, `evidence/` — golden (не перезаписывать, не переименовывать).
- `reference cleanroom oracle ()` — внешние оракулы RE (`gw-proto/`, `rest-static.json`).

## Команды

- `npm run gate:worker` — обязательный быстрый гейт (typecheck, meta, unit, smoke).
- `npm run gate:release` — полный (+ e2e с автоповтором, coverage, golden-сверка);
  `gate:qualify` — выпускной; отчёты гейтов — `.gate/*.json`.
- `npm run check` — typecheck + все тесты; `cd rust && cargo test --offline` — Rust
  (npm-гейт cargo пока не запускает — прогонять вручную).
- `npm run golden:freeze` — md5-манифест golden перед серией правок.
- После сбоя/сна: `git log --oneline -3` + `git status` → `tail -n 5 waves/db.jsonl` →
  `waves/STATE.md`.

## Запреты (кратко; полный список — `AGENTS.md` / `PROCESS.md`)

- Живой Discord не трогать без согласованного окна (`device.spawn`/attach — по команде).
- Сеть, новые зависимости, установка пакетов — только с разрешения владельца.
- Коммиты и пуш делает координатор; воркер не коммитит и не правит чужие файлы.
- Golden не перезаписывать; тесты под код не «зеленить»; ошибки не глотать молча.
