# Матрица критериев выбора моделей, `effort`, приоритетов и сжатия контекста (PM Token-Economy Policy)

## 1. Главный принцип экономии лимитов и токенов
Сильные модели (`gemini-4-argon-high`, `claude-opus-4-6-thinking`) и высокий `effort` (`--effort high` / `xhigh` / `max`) **строго запрещено** назначать на простые, рутинные, документационные, проверочные или декомпозированные задачи.
Любая крупная задача перед выдачей воркеру **декомпозируется PM-координатором на атомарные подзадачи** (`L1`/`L2`, 1–3 файла в write-scope, с точным указанием путей, номеров строк и команд проверки в `TASK.md`), после чего распределяется между **лёгкими моделями Antigravity с минимальным `effort` (`--agent antigravity --model gemini-3.8-flash-low --effort low` / `gemini-4-argon-low --effort low`)** и **DeepSeek (`--agent opencode --model deepseek/deepseek-v4.1-flash`)**.

---

## 2. Классификация задач по сложности (`L1`–`L3`) и выбор модели / `effort`

| Уровень сложности | Критерии отбора задачи | Назначаемый агент и модель (балансировка пула) | Уровень `effort` | Лимит контекста / ТЗ |
|---|---|---|---|---|
| **`L1 — Простая / Рутинная / Атомарная` (75% задач)** | - Правки типов JSDoc / `tsconfig.json` (`tsc`) по известному списку ошибок<br>- Точечные багфиксы и подключение модулей (1–2 файла)<br>- Обновление манифестов сборки (`BUILD-*.md`), `RELEASE-NOTES`, `MERGE-READINESS`, `TDD-PLAN.md`<br>- Дизайн-спеки (`docs/specs/*.md`), токены CSS<br>- Прогон готовых unit/contract тестов (`QA`) | 1) **Antigravity Flash Low:**<br>`--agent antigravity --model gemini-3.8-flash-low --effort low`<br>2) **DeepSeek Flash:**<br>`--agent opencode --model deepseek/deepseek-v4.1-flash` | **Минимальный:**<br>`--effort low` для `antigravity`;<br>`null` для `opencode` | `TASK.md` ≤ 25 строк: точные пути, строки (`StartLine..EndLine`) и 1–2 команды `T0`; **запрещён** поиск по репозиторию |
| **`L2 — Средняя / Модульная / Ревью с мутациями` (20% задач)** | - Техническое ревью 1 PR + прогон `5/5` мутаций (`REVIEW-*A.md`)<br>- Реализация одного изолированного UI-контроллера или BFF-маршрута по готовому контракту + 1 тестовый файл (2–4 файла)<br>- Мост-адаптер между 2–3 готовыми модулями | 1) **DeepSeek Flash:**<br>`--agent opencode --model deepseek/deepseek-v4.1-flash`<br>2) **Antigravity Low / Flash Medium:**<br>`--agent antigravity --model gemini-3.8-flash-medium --effort low` или `--model gemini-4-argon-low --effort low` | **Низкий:**<br>`--effort low` (максимум `medium` для сложных модулей) | Только целевой модуль + его тест (`T0`), без запуска полного `gate:worker` |
| **`L3 — Сложная архитектурная / Критическая` (≤ 5% задач)** | - Сквозная интеграция нескольких крейтов Rust (`discord-voice` + `discord-gateway` + `gui-spike`) или сложные состояния гонок/протоколов без готового шаблона | Сначала обязательная декомпозиция до `L1/L2` (`gemini-3.8-flash-low --effort low` / `deepseek-v4.1-flash`); только для неделимого сложного ядра — `--agent antigravity --model gemini-4-argon-medium --effort medium` | `--effort medium` (и только в исключительных `P0` случаях `--effort high`) | Строго после предварительной декомпозиции PM |

---

## 3. Матрица приоритизации (`P0` → `P1` → `P2` → `P3`)

1. **`P0 (Blocker / Критический путь гейта)` — 1-я очередь слотов:**
   - Падения компиляции, `tsc` (`npm run typecheck`), `clippy -D warnings`, падающие unit/contract/meta тесты или централизованная верификация сборки `testing` (`QA-BUILD-*.md`).
   - Выдаются **немедленно** свободным слотам, разбитыми на узкие батчи по 1–3 файла на `gemini-3.8-flash-low --effort low` и `deepseek/deepseek-v4.1-flash`.
2. **`P1 (Интеграция и закрытие контракта сборки)` — 2-я очередь слотов:**
   - Техническое ревью готовых PR (`TR-*A`, `5/5` мутаций), склейка готовых модулей в `app.js` / `lib.rs` / `main.rs`, синхронизация `test-impact.mjs`.
   - Выдаются на `deepseek/deepseek-v4.1-flash` и `gemini-3.8-flash-low --effort low` / `gemini-4-argon-low --effort low`.
3. **`P2 (Вторичные улучшения, Pass 2 hardening, спеки, готовность релиза)` — 3-я очередь слотов:**
   - Нефункциональные улучшения из ревью (`P2`), дизайн-спецификации, `MERGE-READINESS-*.md` и `BUILD-00X.md`.
   - Запускаются **строго на минимальных моделях (`--agent antigravity --model gemini-3.8-flash-low --effort low` или `deepseek/deepseek-v4.1-flash`)**.
4. **`P3 (Косметика и документация)` — 4-я очередь:**
   - Группируются в один короткий пакет в конце волны на `--agent antigravity --model gemini-3.8-flash-low --effort low`.

---

## 4. Сжатие контекста для воркеров и использование `basic-memory`, `socraticode`, `understand` MCP

1. **Поиск символов и зависимостей через MCP до запуска воркера:**
   - Вместо чтения больших файлов или поручения поиска воркеру координатор использует:
     - **`socraticode` MCP** (`codebase_symbol`, `codebase_search`, `codebase_impact`, `codebase_flow` — коллекция `codebase_eb958c6ee842`, `24 551` чанк, `912` файлов) для мгновенного получения точных номеров строк и сигнатур.
     - **`understand` MCP** (`understand_search`, `understand_node`, `understand_neighbors`, `understand_path` — `.ua/knowledge-graph.json`) для архитектурных связей между модулями и тестами.
     - **`basic-memory` MCP** (`bm_read`, `bm_write`, `bm_edit` в проекте `discord-lab`, заметка `context/rw40-build003-compact-digest.md`) для хранения сжатого межагентного дайджеста состояния волны, якорей файлов и результатов проверок.
2. **Жёсткий формат сжатого ТЗ (`TASK.md` ≤ 25 строк):**
   - Воркеру передаётся **только дельта, необходимая для его задачи**:
     1. Write-Scope: 1–3 конкретных пути файла с диапазонами строк (`#L100-L140`).
     2. Готовый фрагмент кода / сигнатура.
     3. Ровно 1–2 целевые команды проверки (`T0`).
   - **Запрещено воркерам:** читать общие документы (`PLAN.md`, `ROADMAP.md`, `TASKS.md`, `waves/*`), делать `grep`/`find` по репозиторию или запускать тяжёлый `npm run gate:worker` на промежуточных ветках (`gate:worker` запускается централизованно один раз отделом `G4 QA` на `testing`).
