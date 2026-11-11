# ECC 2.0 Control Plane, Testing Pyramid & Delivery Pipeline Architecture

> **Статус документа:** `Living Architecture & Process Specification (v2.0)`  
> **Проект:** `discord-lab` (Нативный клиент `discord-gpui` + BFF & Core Rust)  
> **Оркестратор:** ECC 2.0 Control Plane (`scripts/ecc`) + Orca Orchestration Engine  
> **Дата актуализации:** Ноябрь 2026 г.

---

## 1. Архитектура управления: Модель во главе с ECC 2.0 (Control Plane)

Система управления разработкой и поставкой кода строится по иерархическому принципу с единым центром координации — **ECC 2.0 Control Plane**:

```
                       ┌──────────────────────────────────────────────┐
                       │          ECC 2.0 Chief Orchestrator          │
                       │           (Profile: pm_review / PM)          │
                       └──────────────────────┬───────────────────────┘
                                              │
                     ┌────────────────────────┴────────────────────────┐
                     ▼                                                 ▼
        ┌─────────────────────────┐                       ┌─────────────────────────┐
        │   Product Manager (PM)  │                       │   Technical Reviewer    │
        │ Acceptance & Scenarios  │                       │ Architecture, Sec & QA  │
        │       (ACCEPT.md)       │                       │  (REVIEW.md + 5/5 Mut)  │
        └────────────┬────────────┘                       └────────────┬────────────┘
                     │                                                 │
  ───────────────────┴─────────────────────────────────────────────────┴───────────────────
                               РАБОЧИЕ ОТДЕЛЫ СПЕЦИАЛИСТОВ
  ┌───────────────┬───────────────┬───────────────┬───────────────┬───────────────────────┐
  │  G1 Backend   │  G2 Frontend  │   G3 Design   │    G4 QA      │  G5 Voice & Platform  │
  │  Rust Core &  │  WebView UI & │   UX/UI &     │  AT-SPI, E2E, │  DAVE E2EE, PipeWire  │
  │  Tauri/GPUI   │  DOM/A11y     │   Design-Kit  │  Fuzz, Chaos  │  Audio RingBuffer     │
  └───────────────┴───────────────┴───────────────┴───────────────┴───────────────────────┘
  ┌───────────────────────────────┬───────────────────────────────┬───────────────────────┐
  │     G6 Tech Writer / Parity   │   G7 Reverse Engineering REA  │  DevOps / Integration │
  │    Docs, Release Notes, SBOM  │   Oracle: /home/sanya/re-deep │  CI/CD, Gate, Release │
  └───────────────────────────────┴───────────────────────────────┴───────────────────────┘
```

### 1.1. Роли и непересекающиеся зоны ответственности (Write-Scopes)
Каждый отдел имеет изолированный скоуп записи, что исключает конфликты слияния при параллельной работе:

| Отдел / Роль | Профиль (`ecc2.toml`) | Зона записи (Write-Scope) | T0 Верификация |
| :--- | :--- | :--- | :--- |
| **G1 Backend** | `g1_backend` | `rust/crates/discord-{core,codec,media,store,i18n}`, `rust/gui-spike/src/*.rs`, `rust/discord-gpui/src/{net,state,storage,audio}` | `cargo test -p discord-core`, `--test <slice>_contract` |
| **G2 Frontend** | `g2_frontend` | `rust/gui-spike/ui/**/*.{js,css,html}`, `rust/discord-gpui/src/ui/**` | `node scripts/test-impact.mjs --run`, `npm run typecheck` |
| **G3 Design** | `g3_design` | `docs/specs/*.md`, `docs/specs/wireframes/`, дизайн-токены | `tests/meta/design-breakpoints.test.mjs` |
| **G4 QA** | `g4_qa` | `rust/discord-gpui/tests/`, `tests/{unit,smoke,e2e,meta}/`, `scripts/test-impact.mjs` | `cargo test --test visual_regression_contract`, `gpui_e2e_contract` |
| **G5 Voice / Platform** | `g5_voice_platform` | `rust/crates/discord-voice/**`, `rust/crates/discord-platform/**`, `voice_devices.rs`, DAVE MLS v1 | `cargo test -p discord-voice -p discord-platform` |
| **G6 Tech Writer** | `g6_tech_writer` | `docs/`, `waves/RW-*/PARITY/`, `RELEASE-NOTES-*.md` | `tests/meta/docs-sync.test.mjs` |
| **G7 RE / REA** | `g7_re_rea` | Read-only `/home/sanya/re-deep/`; записи в `waves/RW-*/RE-AUDIT/` | MCP `rea` и `understand` |
| **DevOps** | `devops` | `scripts/{ci.sh,gate.mjs,release.sh}`, `.gate/`, CI конфигурации | `bash scripts/ci.sh --dry-run` |
| **PM & Tech Reviewer** | `pm_review` | `ACCEPT.md`, `REVIEW.md` (гейт 5/5 мутаций) | Комплексная двухэтапная приёмка PR |

### 1.2. Политика моделей (Model Routing)
1. **Строгий запрет `codex` и `deepseek`:** Использование `codex` и `deepseek/deepseek-v4.1-flash` запрещено регламентом.
2. **Линейка Gemini (`antigravity`):**
   - **`gemini-4-carbon-high` (`--effort high`):** Применяется исключительно для задач уровня P0 (ядро, криптография DAVE MLS v1, Ring Buffer с нулевым копированием, акторы Tokio ↔ GPUI).
   - **`gemini-4-argon-high` (`--effort high`):** Стандарт для большинства задач разработки, BFF REST, WebRTC, QA валидации, мутационного тестирования и визуальной регрессии.
   - **`gemini-3.8-flash-high` / `medium`:** Верстка UI компонентов, модальные окна, настройки, документация и вспомогательные чек-листы.

### 1.3. Контроль ресурсов: Защита от OOM (`WORKER_RAM_LIMIT <= 25 GB`)
* **Лимит оперативной памяти:** На хосте 48 GB RAM без файла подкачки (`Swap: 0B`). Суммарное потребление всех процессов воркеров (`agy`, `cargo`, `rustc`, `node`) **никогда не должно превышать 25 GB RAM**, гарантируя минимум 20–23 GB свободной памяти ОС.
* **Политика нулевого простоя (Zero-Idle Policy):** При завершении задачи (`worker_done`) воркер немедленно сдает отчёт, его терминал освобождается (`worker-release`), а освободившийся слот сразу же назначается на следующую задачу из пула.

---

## 2. Пирамида тестирования и Конвейер проверки кода (T0–T4)

Конвейер верификации кода разбит на 5 уровней (от микросекундного теста воркера до релизной квалификации):

```
       ▲
      / \       T4: Qualification Gate (Live probes, Keyring, Packaging)
     /---\      
    /     \     T3: Release & Mutation Gate (5/5 mutations killed, ASan/TSan, SBOM)
   /-------\    
  /         \   T2: Integration & E2E (GPUI Headless Smoke, 50 E2E Journeys, SSIM >0.99)
 /-----------\  
/             \ T1: Worker Gate (Clippy -D warnings, Typecheck, Unit, Contract)
───────────────
T0: Targeted    (Targeted test impact: node scripts/test-impact.mjs --run, <500ms)
```

### 2.1. Уровни верификации
* **T0 (Targeted):** Запуск целевого теста только для изменяемого модуля (`node scripts/test-impact.mjs --run` или `cargo test --test <module>_contract`). Время: `< 1 сек`.
* **T1 (Worker Gate):** Проверка перед подачей PR:
  * `npm run typecheck` + `npx tsc -p rust/gui-spike/ui/tsconfig.json`.
  * `cargo fmt --all -- --check`.
  * `cargo clippy --workspace --all-targets --all-features -- -D warnings`.
  * `node scripts/gate.mjs --scope worker`. Время: `< 30 сек`.
* **T2 (Integration & E2E Gate):**
  * `cargo run --manifest-path rust/discord-gpui/Cargo.toml -- --headless-smoke` (проверка cold boot, SQLite WAL, 120 FPS frame budget, VmRSS < 60 MB).
  * `cargo test --manifest-path rust/discord-gpui/Cargo.toml --test gpui_e2e_contract` (50 сквозных пользовательских сценариев без внешнего интернета).
  * `cargo test --manifest-path rust/discord-gpui/Cargo.toml --test visual_regression_contract test_sub_904` (SSIM > 0.995, проверка маскирования динамических зон).
* **T3 (Release & Mutation Gate):**
  * Проверка мутационного покрытия: **5 из 5 намеренных мутаций должны быть убиты тестами**.
  * Санитайзеры памяти: сборка с AddressSanitizer и ThreadSanitizer для поиска гонок в многопоточном рантайме.
  * Генерация и проверка SBOM (`node scripts/sbom.mjs --check`).
  * Сверка контрольных сумм в `rust/dist/SHA256SUMS`.
* **T4 (Qualification Gate):**
  * Запуск `node scripts/gate.mjs --scope qualify`.
  * Проверка аппаратного хранилища ключей (`DISCORD_LAB_KEYRING_LIVE=1`).
  * Сборка и верификация финального tarball-пакета.

---

## 3. Процесс разработки, ветвления и работа с GitHub

### 3.1. Иерархия веток
Разработка ведётся строго через иерархию веток. Прямые коммиты в защищённые ветки запрещены:

```
 worker branch (<type>/task-<id>-<slug>)
       │  (локальные тесты T0 + T1, PR в unstable)
       ▼
    unstable  ◄─── Двойное ревью (PM: ACCEPT.md + Tech: REVIEW.md)
       │  (сборка проверенных задач в манифест BUILD-00X)
       ▼
    testing   ◄─── Централизованное тестирование отделом G4 QA
       │  (регрессионные тесты, e2e, smoke)
       ▼
      beta    ◄─── Предрелизная проверка (SHA256SUMS, SBOM, откат)
       │
       ▼
      main    ◄─── Стабильный релиз, Git-тег vX.Y.Z, синхронизация в unstable
```

#### Правила именования рабочих веток:
* `feature/task-<id>-<short-slug>` — новая функциональность (например, `feature/task-901-headless-harness`).
* `bugfix/task-<id>-<short-slug>` — исправление дефекта (например, `bugfix/task-909-fix-render-clip`).
* `qa/task-<id>-<short-slug>` — тестовые контракты и тестовая автоматизация.
* `design/task-<id>-<short-slug>` — спецификации интерфейса, токены, макеты.
* `devops/task-<id>-<short-slug>` — скрипты сборки, CI/CD, релизный конвейер.
* `docs/task-<id>-<short-slug>` — документация, матрицы паритета, релизные заметки.

### 3.2. 13 обязательных статусов жизненного цикла задачи
Ни одна задача не может попасть в релиз, минуя обязательные стадии:

1. `Backlog` — задача описана с критериями приемки и декомпозицией на подзадачи.
2. `Ready for Development` — проверены зависимости, назначен отдел и выделен исполнитель.
3. `In Progress` — создана рабочая ветка от `unstable`, пишется RED-тест.
4. `Ready for Product Review` — код написан, T0/T1 пройдены, сформирован PR в `unstable`.
5. `Ready for Technical Review` — PM проверил бизнес-логику (`ACCEPT.md`), передано техревьюеру.
6. `Ready for Merge` — Технический ревьюер проверил код (`REVIEW.md`), убиты 5/5 мутаций, CI зелёный.
7. `Merged to Unstable` — PR слит в ветку `unstable`, рабочая ветка удалена.
8. `In Testing` — сборка `BUILD-00X` собрана интегратором в ветку `testing`, запущено тестирование QA.
9. `QA Failed` *(если найден баг)* — возврат с баг-репортом, создание ветки `bugfix/` от `unstable`.
10. `QA Passed` — все тесты уровня T2/T3 пройдены без единого регресса.
11. `In Beta` — сборка перенесена в ветку `beta`, проверка в боевом окружении.
12. `Ready for Release` — проверены `SHA256SUMS`, SBOM, LTO/strip и готов план отката.
13. `Released` — слияние `beta -> main`, выставлен Git-тег `vX.Y.Z`, артефакт опубликован.

### 3.3. Стандарт сообщений коммитов (Conventional Commits)
Формат сообщений коммита:
```
<type>(<scope>): <краткое описание в повелительном наклонении>

<подробное описание: зачем сделано, какие контракты изменены>

Closes: #<task-id>
```
* `type`: `feat`, `fix`, `refactor`, `perf`, `test`, `docs`, `chore`, `ci`.
* `scope`: `gpui`, `voice`, `codec`, `net`, `a11y`, `visual`, `e2e`.

---

## 4. Карта автоматизации: Что внедрено и что автоматизируется

### 4.1. Внедрённые автоматизированные механизмы:
1. **`scripts/ci.sh`**:
   * Автоматическое обнаружение браузера и системного графического окружения.
   * Проверка форматирования (`cargo fmt --check`) и статического анализа (`clippy -D warnings`).
   * Интегрированный прогон нативного GPUI: `headless-smoke`, 50 E2E контрактов и SSIM регрессия.
   * Контроль лицензий и генерация SBOM через `scripts/sbom.mjs`.
2. **`scripts/gate.mjs`**:
   * Фазовый раннер с тремя профилями (`worker`, `release`, `qualify`).
   * Сохранение JSON-артефактов прогона в `.gate/`.
   * Автоматический повтор упавших e2e-тестов для выявления нестабильных (flaky) проверок.
   * Строгий принцип: `skip` никогда не считается успешным прохождением.
3. **`scripts/test-impact.mjs`**:
   * Анализ карты зависимостей между изменёнными файлами и тестовыми наборами для ультрабыстрого T0 прогона.
4. **Канбан-доска (`waves/kanban-board.json`, `.html`)**:
   * Автоматическая синхронизация статусов карточек с оркестратором Orca.

### 4.2. План следующих автоматизаций (Roadmap):
1. **Автоматический запуск виртуального Wayland-композитора:**
   * Скрипт `scripts/run-headless-compositor.sh` для запуска `cage -s -- discord-gpui` на виртуальном сокете `wayland-99` в CI контейнерах без физического X-сервера.
2. **Автоматический дамп видео при падении E2E (Failure Video Recording):**
   * Запись фреймбуфера через протокол `wlr-screencopy` в сжатый WebM файл при возникновении `panic!` в E2E-тесте.
3. **Автоматический пересчёт SSIM эталонов при смене темы:**
   * Флаг `--update-goldens` для автоматической перегенерации 15 эталонных снимков экрана при изменении глобальных дизайн-токенов.
4. **Интеграция CI бота GitHub Actions:**
   * Автоматический комментарий в Pull Request с матрицей покрытия, временем кадра (FPS Budget) и интерактивным отчётом `visual_diff_report.html`.
