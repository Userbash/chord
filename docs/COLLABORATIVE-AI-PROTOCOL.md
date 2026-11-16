# Протокол совместной разработки Multi-Human & Multi-AI (AMCP v2.0)
## Agent Multi-Collaborator Protocol & Engineering Standard for `chord` / `discord-lab`

> **Статус документа:** Действующий производственный стандарт  
> **Версия протокола:** AMCP v2.0 (Agent-to-Agent Multi-Collaborator Protocol)  
> **Область действия:** Все инженеры-люди, ИИ-агенты (Antigravity, Claude Code, Cursor, Windsurf, Codex, Gemini CLI), автономные воркеры и боты контрибьютинга (Dependabot).

---

## 1. Введение и Архитектурные Принципы

Разработка высокопроизводительного нативного клиента (`chord` / `discord-lab`) ведётся распределённой командой: людьми-инженерами и множеством ИИ-агентов на базе различных больших языковых моделей (LLM).

Когда над одним репозиторием параллельно работают несколько ИИ-агентов и инженеров, возникают 5 критических рисков:
1. **Конфликты слияния и гонки перезаписи файлов:** два агента одновременно меняют один и тот же файл, затирая код друг друга.
2. **Сжигание контекста и токенов на рутину:** агенты тратят до 80% лимита токенов на чтение многостраничных дампов `git status`, `cargo test` и JSON-выводов.
3. **Рассинхронизация типов данных (Data Contract Drift):** бэкенд (Rust) меняет сигнатуру метода или структуры DTO, ломая фронтенд (TypeScript/GPUI/Tauri).
4. **Утечки памяти и OOM Killer:** запуск множества параллельных компиляторов `rustc`/`cargo` и серверов приводит к падению хост-машины (лимит 48 ГБ RAM).
5. **Ломающие внешние зависимости:** неконтролируемые мажорные обновления сторонних библиотек ломают скомпилированный код.

Протокол **AMCP v2.0** решает эти проблемы через **жесткую стандартизацию, локальную шину синхронизации агентов и zero-token автоматизацию**.

---

## 2. Архитектура Взаимодействия (Multi-AI Ecosystem)

```
┌────────────────────────────────────────────────────────────────────────┐
│                   HUMAN DEVELOPERS & LLM ASSISTANTS                    │
│   (Antigravity / Claude Code / Cursor / Windsurf / Codex / Gemini CLI)  │
└───────────────────────────────────┬────────────────────────────────────┘
                                    │
                                    ▼
┌────────────────────────────────────────────────────────────────────────┐
│             UNIFIED COLLABORATION CLI (scripts/collab.sh)              │
│       npm run collab status | lock | unlock | verify | sync | send     │
└──────────────┬────────────────────┬────────────────────┬───────────────┘
               │                    │                    │
               ▼                    ▼                    ▼
┌─────────────────────────┐ ┌───────────────┐ ┌──────────────────────────┐
│  DISTRIBUTED FILE LOCKS │ │  EVENT BUS    │ │     ASYNC MAILBOX        │
│  .collab/bus/locks.json │ │  events.jsonl │ │ .collab/bus/mailbox/<id> │
└─────────────────────────┘ └───────────────┘ └──────────────────────────┘
               │                    │                    │
               └────────────────────┼────────────────────┘
                                    ▼
┌────────────────────────────────────────────────────────────────────────┐
│                   PRE-FLIGHT GATES & GIT HOOKS                         │
│  • .git/hooks/pre-commit (Scope Audit + Sub-Second Syntax Linter)      │
│  • scripts/precheck.mjs (Parallel TS + Node + Meta Check in 0.7s)      │
│  • scripts/scope-audit.sh (Write-Scope Boundary Enforcement)           │
│  • scripts/tdd-runner.sh (TDD Red-Phase Failure & Green-Phase Record)  │
└───────────────────────────────────┬────────────────────────────────────┘
                                    │
                                    ▼
┌────────────────────────────────────────────────────────────────────────┐
│                       PR & MERGE CONVEYOR                              │
│  • scripts/dependabot-batch.sh (SemVer Safety Filter & Auto-Rebase)    │
│  • scripts/review-scaffold.sh (Dual Review: PM ACCEPT.md + Tech 5/5)   │
│  • scripts/worker-rotate.sh (1-to-1 Zero-Idle Slot Rotation, <=25GB)   │
│  • scripts/sync-push.sh (Atomic Dual-Remote Push to origin + public)   │
└────────────────────────────────────────────────────────────────────────┘
```

---

## 3. Ядро Протокола AMCP v2.0

### §3.1. Contract-First Разработка
- **Правило:** Любое изменение межмодульного интерфейса (Rust BFF <-> Frontend WebView / DTOs / Events) начинается с фиксации контракта.
- Запрещено менять реализацию до утверждения DTO в `rust/crates/discord-core/src/dto.rs` или сигнатур в `shared/contracts/`.
- Изменение контракта публикуется в шину событий:
  ```bash
  npm run collab publish CONTRACT_UPDATE "Updated VoiceStateDto to include deaf_reason field"
  ```

### §3.2. Изоляция Отделов и Write-Scope Границы
Каждый ИИ-воркер и разработчик работает строго в границах своего отдела (Department), определенных в `AGENTS.md` §2:
- **G0 (Routine & Adapters):** Документация, утилиты, тесты мета-синхронизации, тривиальные DTO.
- **G1 (Backend Core & BFF):** `rust/crates/discord-{core,codec,media,store,i18n}`, `rust/gui-spike/src/*.rs`.
- **G2 (Frontend & UI):** `rust/gui-spike/ui/**/*.{js,css,html}`, `tests/unit/*.test.mjs`.
- **G3 (Design & Specs):** `docs/specs/*.md`, `waves/RW-*/DESIGN/*.md`.
- **G4 (QA Automation):** `tests/{meta,unit,smoke,e2e}/`, `scripts/test-impact.mjs`.
- **G5 (Voice & Platform):** `rust/crates/discord-voice/**`, `rust/crates/discord-platform/**`, WebRTC/DAVE.
- **G7 (Clean-Room RE):** Изучение оракулов `docs/re/`, генерация спецификаций без изменения прод-кода.

Перед коммитом запускается проверка:
```bash
npm run audit:scope
```
Если воркер отдела G2 изменил файл бэкенда G1, коммит блокируется.

### §3.3. Распределённые Блокировки Файлов (Distributed File Locks)
Перед модификацией общего или критического файла агент ОБЯЗАН захватить лок:
```bash
npm run collab lock rust/crates/discord-core/src/dto.rs TASK-123
```
- Лок сохраняется в реестре `.collab/bus/locks.json`.
- Если другой агент попытается захватить тот же файл, он получит отказ с указанием имени владельца и номера задачи:
  ```
  ❌ CONFLICT BLOCKED: File rust/crates/discord-core/src/dto.rs is already locked by @g1_backend for task TASK-110!
  ```
- После завершения работы и прогона тестов агент освобождает лок:
  ```bash
  npm run collab unlock rust/crates/discord-core/src/dto.rs
  ```

### §3.4. Асинхронный Обмен Сообщениями (Direct Mailbox)
Для взаимодействия между агентами и инженерами (например, уведомление фронтенда о готовности новой команды бэкенда) используется почтовый ящик:
```bash
# Отправка сообщения
npm run collab send g2_frontend "IPC Ready" "Command voice_connect is implemented and tested"

# Чтение входящих
npm run collab inbox g2_frontend
```

### §3.5. Жесткий Лимит Памяти (`WORKER_RAM_LIMIT <= 25 GiB`) и Zero-Idle
- Суммарная память всех активных ИИ-агентов, серверов и сборщиков `cargo`/`node` не должна превышать **25.00 GiB** (на хосте 48 GiB).
- Максимум **5 параллельных рабочих слотов** (3–4 код/ревью + 1–2 RE).
- Ротация 1-в-1: завершивший задачу воркер мгновенно освобождает терминал (`worker:rotate`), а освободившийся слот получает следующую задачу из очереди без простоя команды.

---

## 4. Конвейер Проверки, Контрибьютинга и Слияния (Merge Conveyor)

### 4.1. Локальные Pre-Flight Проверки (Zero-Token)
В репозитории установлены нативные хуки (`npm run hooks:install`):
1. **Pre-commit:**
   - Проверяет границы write-scope (`scripts/scope-audit.sh`).
   - Проверяет корректность синтаксиса JS и TypeScript типов (`scripts/precheck.mjs`).
   - Время выполнения: **0.7 секунды** (не сжигает токены LLM).
2. **Commit-msg:**
   - Валидирует формат Conventional Commits (`feat:`, `fix:`, `refactor:`, `test:`, `docs:`, `chore:`, `perf:`, `ci:`).

### 4.2. TDD Цикл (Test-Driven Development)
1. **Красная фаза (RED):** Агент пишет изолированный тест и запускает:
   ```bash
   npm run tdd:red -- --test my_contract
   ```
   Скрипт проверяет, что тест упал именно с ожидаемой ошибкой и фиксирует лог в wave journal.
2. **Зеленая фаза (GREEN):** Агент пишет минимально необходимый код и запускает:
   ```bash
   npm run tdd:green -- --test my_contract
   ```
   Скрипт проверяет прохождение теста и прогоняет 5/5 мутационный фильтр.

### 4.3. Двойное Ревью (Dual Review Gate)
Ни один PR не может быть слит в `unstable` без двух подтверждений:
1. **Product Review (`ACCEPT.md`):** Продакт-менеджер валидирует выполнение критериев приемки (DoD), работу UI и отсутствие регрессий пользовательского опыта.
2. **Technical Review (`REVIEW.md`):** Техлид проверяет чистоту архитектуры, потокобезопасность, запрет `unsafe_code` и **100% уничтожение мутаций (5/5)**.
Генерация шаблонов ревью автоматизирована:
```bash
npm run review:scaffold TASK-123
```

### 4.4. SemVer Автоматизация и Работа с Ботами (Dependabot Conveyor)
Все входящие PR от ботов (`dependabot[bot]`) и внешних контрибьюторов обрабатываются пакетным скриптом `scripts/dependabot-batch.sh`:
- **PATCH-версии (0.14.3 -> 0.14.4):** Автоматически одобряются и вливаются squash-мерджем при зеленых тестах CI.
- **MINOR-версии (0.15.0 -> 0.16.0):** Проходят `scripts/precheck.mjs` и вливаются полуавтоматически.
- **MAJOR-версии (0.15 -> 0.18):** Изолируются, требуют подтверждения от техлида и прогона `npm run test:smoke`.
- **Коллизии `Cargo.lock`:** Автоматически устраняются отправкой `@dependabot rebase`.

### 4.5. Протокол Карантина Дефектных PR (Quarantine Protocol)
Если сторонний PR содержит ломающие регрессии (как PR #16 с повреждением аудио-пайплайна):
- PR переводится в статус **Changes Requested** с детальным логом регрессии (`npm run pr:request-changes`).
- Ветка PR изолируется и **НЕ блокирует** конвейер остальных задач.
- Полезные части кода могут быть спасены через cherry-pick (`npm run pr:salvage`).

---

## 5. Полный Реестр Автоматизаций и CLI Команд

| Команда | Скрипт | Назначение | Экономия токенов / выгоды |
|---|---|---|---|
| `npm run collab status` | `scripts/collab.sh status` | Сводный дашборд: локи, события, воркеры, RAM | Заменяет 6 ручных команд в один клик |
| `npm run collab lock` | `scripts/collab.sh lock` | Захват эксклюзивного лока на файл | Предотвращает конфликты правок между ИИ |
| `npm run collab unlock` | `scripts/collab.sh unlock` | Освобождение лока на файл | Чистота реестра локов |
| `npm run collab verify` | `scripts/collab.sh verify` | Комплексная pre-flight проверка | 100% защита от коммита поломок |
| `npm run collab sync` | `scripts/sync-push.sh` | Двойной push в origin и public репозитории | Исключает рассинхронизацию зеркал |
| `npm run precheck` | `scripts/precheck.mjs` | Параллельный TS + Node + Meta чекер | 0.7 сек вместо 12 сек `npm test` |
| `npm run audit:scope` | `scripts/scope-audit.sh` | Линтер границ отделов AGENTS.md §2 | Защита от случайных правок чужих файлов |
| `npm run report:worker` | `scripts/worker-report.sh` | Генерация отчета по PROCESS.md §4.2 с SHA256 | -1500 токенов на ручное составление Markdown |
| `npm run hooks:install` | `scripts/install-git-hooks.sh` | Установка git pre-commit и commit-msg хуков | Автоматический контроль на уровне git |
| `npm run wave:accept` | `scripts/wave-accept.sh` | Внесение задачи в waves/db.jsonl | Соблюдение схемы v1 wave journal |
| `npm run review:scaffold`| `scripts/review-scaffold.sh` | Генерация ACCEPT.md и REVIEW.md (5/5 мутаций) | Стандартизация двойного ревью |
| `npm run tdd:red` | `scripts/tdd-runner.sh --red` | Автоматическая фиксация падения теста | Строгий TDD без подгонки тестов под код |
| `npm run tdd:green` | `scripts/tdd-runner.sh --green`| Проверка прохождения теста и мутаций | Автоматизация зеленой фазы TDD |
| `npm run orca:status` | `scripts/orca-slim.sh` | Компактная сводка воркеров Orca | Сжатие вывода с 8000 до 30 токенов (-99.5%) |
| `npm run gc` | `scripts/worker-gc.mjs` | Очистка target cache и stale воркдеревьев | Освобождает до 105 ГБ диска за 1 секунду |
| `npm run dependabot:batch`| `scripts/dependabot-batch.sh` | SemVer фильтр и авто-мердж PR ботов | Полная автономия обновления зависимостей |

---

## 6. Памятка Разработчику и ИИ-Агенту

1. **Перед началом работы:**
   ```bash
   npm run collab status
   npm run collab lock <файл> <номер_задачи>
   ```
2. **В процессе разработки:**
   - Писать тест перед кодом (`npm run tdd:red`).
   - Соблюдать правила своего отдела (`AGENTS.md`).
   - Использовать только проверенные контракты и чистые оракулы (`docs/re/`).
3. **Перед коммитом:**
   ```bash
   npm run collab verify
   npm run collab unlock <файл>
   npm run report:worker
   ```
4. **Синхронизация с командой:**
   ```bash
   npm run collab publish TASK_DONE "Completed TASK-XXX: Description"
   npm run collab sync
   ```

Следуя этому протоколу, любая комбинация разработчиков-людей и ИИ-агентов может параллельно развивать проект без риска сломать архитектуру или вызвать деградацию системы.
