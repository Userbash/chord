# AGENTS.md — правила агентов и координатора в discord-lab

Для быстрого входа сначала прочитай `PROJECT-CONTEXT.md`; затем открывай полные документы
только по необходимости и для конкретной карточки. Не загружай целиком историю волн и большие
планы без прямой нужды.

Ты работаешь в репозитории `discord-lab` (локальный CLI: анализ Discord через Frida/CDP + Rust Tauri клиент `discord-gui-spike`).
Перед работой проверь HEAD и состояние назначенных файлов, затем открой только нужные выдержки:
своё ТЗ, строку карточки из `TASKS.md`, относящиеся к контракту разделы `TDD-PLAN.md`,
соответствующие правила `PROCESS.md` / `TEAM.md` / `TEST-POLICY.md` и краткое актуальное состояние из `waves/STATE.md`.
`PROCESS.md` — источник истины по процессу; `TEAM.md` — по распределению ролей, маршрутизации моделей и запуску
параллельных воркеров; `TEST-POLICY.md` (и `waves/RW-40/QUALITY/TEST-STRATEGY-V2.md`) — по тестовой стратегии.

---

## 1. Базовые правила разработки (TDD, Файлы, Гейты)

1. **Strict TDD.** Сначала тест (red), прогони и зафиксируй падение командой. Затем минимальный
   код (green). Без теста продуктовый код не пишется. Тесты сверяют контракт с независимым
   оракулом (реальный артефакт, jq/awk/md5), а не с текущей реализацией.
2. **Только свои файлы** из ТЗ. Множества файлов воркеров не пересекаются. Чужие тесты, чужие
   модули, доки и §8 без разрешения Team Lead не трогаются. Общие файлы (`state/types.rs`,
   `state/reducer.rs`, `state/mod.rs`, `src/lib.rs`, `PLAN.md`, `STATUS.md`, `ROADMAP.md`,
   `TASKS.md`, `docs/map.md`, `waves/*`) правит только Team Lead/Tech Lead.
3. **Гейт по уровню.** В batch воркер запускает только targeted-тесты своего модуля (`T0`: `node scripts/test-impact.mjs` / целевой `--test <mod>_contract`) и сохраняет
   red/green-логи. Полный `cargo fmt`/`clippy`/`test --workspace`/`npm run gate:worker` (`T1`) — один
   раз на batch в integration-лейне. Если работаешь один (вне batch) — полный `gate:worker`
   обязателен. Если упал e2e — гейт сам делает повтор и сохраняет оба результата; флаки не
   замалчиваются.
4. **Отчёт** по шаблону `PROCESS.md` §4.2: batch/карточка, сделано, red-прогон, green-прогон,
   targeted, gate, файлы+md5, риски. Задача закрывается приёмкой Team Lead + PM + Технического ревьюера, а не отчётом.
5. **Вопросы** — Team Lead (оркестрация/ask), не догадки. При неясности ТЗ — остановись и
   спроси. Если файл уже занят другим воркером — стоп, не «договаривайся» правкой.
6. **Журнал волн.** Артефакты batch (ТЗ, red/green-логи, отчёт, копии тестов) сохраняются в
   `waves/<ID>/`; после приёмки Team Lead дописывает `waves/db.jsonl` (append-only, события
   схемы v1) и обновляет `waves/STATE.md`. Журнал стережёт мета-тест `waves-sync`.
7. **Доки — после accepted.** Не обновляй `STATUS`/`ROADMAP`/`TASKS`/`map`/`STATE` в red-фазе;
   Team Lead синхронизирует их один раз после приёмки batch.

---

## 2. Организация командной разработки, Отделы и Непрерывная загрузка (Zero-Idle)

1. **Независимые отделы специалистов:**
   - **G1 Backend (Rust Core & Tauri BFF):** `rust/crates/*`, `rust/gui-spike/src/*.rs`, `rust/gui-spike/tests/*_contract.rs`.
   - **G2 Frontend (WebView UI & DOM/A11y):** `rust/gui-spike/ui/**`, `tests/unit/*.test.mjs`.
   - **G3 Design (UX/UI & Specs):** `docs/specs/*.md`, `waves/RW-*/DESIGN/*.md`, CSS-токены и брейкпоинты.
   - **G4 QA (Test Automation & Verification):** централизованное тестирование сборок `testing`, `scripts/test-impact.mjs`, `tests/{unit,smoke,e2e,meta}/`, отчёты `QA-BUILD-*.md`.
   - **G5 Voice / Platform:** `discord-voice`, `discord-platform`, `voice_devices.rs`, аудио/стриминг опкоды `18..=22`.
   - **G6 Tech Writer / Parity:** `MERGE-READINESS-*.md`, `RELEASE-NOTES-*.md`, матрицы паритета и документация.
   - **G7 Reverse Engineering & Parity Discovery (`G7 RE / REA`):** непрерывный поиск и сравнение функций с эталонным дампом `/home/sanya/re-deep/` и отдельно запущенной сессией Discord через инструменты `rea` (`reverse-engineer-anything`), выявление несовпадений и недостающих функций оригинальной программы, документирование в `waves/RW-*/RE-AUDIT/RE-GAP-*.md` и передача карточек модернизации Продуктовому менеджеру (PM) в `waves/RW-*/PM/BACKLOG-RE-*.md`.
   - **DevOps / Integration:** `scripts/{release.sh,ci.sh,gate.mjs}`, `SHA256SUMS`, SBOM, сборка `unstable -> testing -> beta -> main`.
   - **Product Manager (PM) & Technical Reviewer (DeepSeek):** приёмка находок от отдела `G7 RE / REA` с формированием задач на разработку/модернизацию и двухэтапная приёмка каждого PR (`ACCEPT.md` + `REVIEW.md` с проверкой 5/5 мутаций).
2. **Строгое разделение ответственности:** Каждый воркер отвечает только за свою назначенную задачу и не берёт чужие задачи самостоятельно. Распределение задач между воркерами не меняется без согласования координатора.
3. **Запрет простоя отделов и воркеров (Zero-Idle Policy):**
   - Ни один отдел и ни один воркер не должен простаивать без задач или ждать завершения работы смежного отдела, если в пуле/бэклоге есть независимые задачи, ревью, фиксы замечаний или подготовка контрактов/тестов.
   - Как только воркер завершил задачу (`worker_done`), координатор немедленно передаёт результат на ревью, а освободившемуся воркеру/отделу делегирует следующую задачу из пула в новой рабочей ветке.
   - Не запускай несколько `cargo` одновременно на одном `target-dir` (используй общий `CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target` последовательно или изолированно).

---

## 3. Маршрутизация агентов и политика моделей (Orca Routing & Cost Policy)

1. **Запуск воркеров через Orca Orchestration:**
   - Все воркеры запускаются как видимые супервизируемые терминалы в собственных рабочих деревьях через `orca worktree create --name <name> --base-branch unstable --branch <type>/task-<id>-<slug>` и `orca orchestration worker-start --run <run_id> --worktree branch:<type>/task-<id>-<slug> ...`.
2. **Полный запрет `codex` и `deepseek`:**
   - На `codex` закончились лимиты/средства — **никогда не используй `--agent codex`**.
   - По указанию пользователя **запрещено использовать DeepSeek (`deepseek/deepseek-v4.1-flash`)**. Все ревью, QA, мутационное тестирование и рефакторинг выполняются на линейке Gemini (`antigravity`).
3. **Политика моделей линейки Gemini (`--agent antigravity`):**
   - **`gemini-4-carbon-high` (`--effort high` / `xhigh`):** исключительно для самых сложных задач (P0 ядра, атомарный zero-copy Ring Buffer, DAVE MLS v1 криптография, AppState Tokio↔GPUI).
   - **`gemini-4-argon-high` / `medium`:** для валидации, проверок, технического ревью, мутационного тестирования (QA / DevOps верификация), а также для стандартных BFF REST сервисов и WebRTC.
   - **`gemini-3.8-flash-high` / `medium` / `low`:** для UI компонентов, верстки, модалок, настроек, системного трея и документации.
4. **Политика для первичной разработки новых фич и спецификаций:**
   - Для сложнейших P0 задач ядра используй флагманскую модель `--agent antigravity --model gemini-4-carbon-high --effort high`, для обычных фич — `gemini-4-argon-high` / `gemini-3.8-flash-high`.

---

## 4. Иерархия веток, Pull Request и 13 статусов задачи

### 4.1. Иерархия веток (`worker branch -> unstable -> testing -> beta -> main`)
1. **Рабочая ветка воркера:** создаётся строго от актуальной ветки `unstable` для каждой новой задачи. Имя ветки: `<type>/task-<id>-<short-slug>`, например:
   - `feature/task-123-reactions-extended-be`
   - `bugfix/task-109-ipc-oracle-strict`
   - `design/task-114-reactions-picker-spec`
   - `qa/task-125-test-impact-v2`
   - `devops/task-119-fix-release-sha256`
   - `docs/task-121-parity-wave11`
2. **Локальные проверки и PR в `unstable`:** после завершения воркер выполняет локальные тесты, сборку, линтер и `typecheck`, оформляет отчёт и подаёт PR в `unstable`. Воркер **не имеет права** сам сливать код в общие/защищённые ветки.
3. **Двойное ревью (PM + Technical Reviewer):**
   - **Продуктовый менеджер (PM)** сверяет результат с ТЗ, пользовательскими сценариями, макетами и DoD. При несоответствии возвращает PR с конкретными ошибками, ожидаемым результатом и способом проверки.
   - **Технический ревьюер (DeepSeek)** проверяет архитектуру, безопасность, качество кода, тесты, обработку ошибок, совместимость, отсутствие лишних правок и убивает `5/5` мутаций.
   - Слияние в `unstable` разрешено **только после** зелёного CI, одобрения PM, одобрения Технического ревьюера, закрытия блокирующих замечаний (`P0=0, P1=0`) и разрешения конфликтов.
4. **Удаление ветки после слияния:** сразу после merge в `unstable` рабочая ветка воркера удаляется. Продолжать разработку новой задачи в старой ветке запрещено.
5. **Ветка `testing` (Централизованная сборка для QA):**
   - Изменения отдельных воркеров **запрещено** передавать в QA напрямую из рабочих веток. Прямые коммиты в `testing` запрещены.
   - Ответственный за интеграцию (DevOps/Coordinator) собирает проверенные задачи из `unstable` в `testing` и выпускает манифест сборки `BUILD-00X` (список задач, номер сборки, инструкции по проверке, тестовые данные, известные ограничения).
   - QA фиксирует версию сборки, окружение, шаги воспроизведения, фактический/ожидаемый результат, логи, скриншоты и критичность (`P0–P3`). При дефекте создаётся отдельная `bugfix/task-*` ветка от `unstable` -> PR -> `unstable` -> следующая сборка `testing`.
6. **Ветка `beta` (Предрелизная проверка):** после `QA Passed` и регрессии в `testing` сборка переносится в `beta` для финальной проверки окружения, критических сценариев, производительности (`perf_contract`), `SHA256SUMS`/SBOM и плана отката.
7. **Ветка `main` (Стабильный релиз) и `hotfix/*`:**
   - Прямые коммиты в `main` запрещены. После успешной проверки `beta` создаётся PR `beta -> main`, после релиза выставляется версия и Git-тег, а изменения синхронизируются обратно в `unstable`.
   - Срочные исправления выполняются через `hotfix/task-<id>-<slug>` с обязательными тестами, ревью, проверкой QA, выкладкой в `main` и немедленной обратной синхронизацией в `unstable`, `testing`, `beta`.

### 4.2. Обязательные 13 статусов задачи
Каждая задача проходит строго по цепочке статусов без пропуска обязательных этапов:
`Backlog` → `Ready for Development` → `In Progress` → `Ready for Product Review` → `Ready for Technical Review` → `Ready for Merge` → `Merged to Unstable` → `In Testing` → (`QA Failed` | `QA Passed`) → `In Beta` → `Ready for Release` → `Released`.

---

## 5. Тестовая стратегия (12 уровней, `P0–P3`, `T0–T4`, 9 измерений)

Подробная спецификация: `TEST-POLICY.md` (§11–§15) и `waves/RW-40/QUALITY/TEST-STRATEGY-V2.md`.
- **Запрет понижения приоритета:** ни один существующий тест (`102 meta`, `879+ unit`, `1142+ workspace`, `24 gui-spike contract`, `22 smoke`, `9 e2e`) никогда не понижается в критичности (`P0/P1`).
- **12 уровней:** `Unit`, `Integration`, `Contract` (`38/38 IPC`, `Stream Opcodes 18..22`, `002-READY.json`), `API` (REST mock `200/202/204/400/401/403/404/413/429/5xx`), `E2E`, `UI`, `Visual`, `Accessibility` (WCAG/ARIA/Keyboard), `Performance`, `Security` (Keyring, XSS, SRTP, SBOM/`SHA256SUMS`), `Regression`, `Smoke`.
- **9 обязательных измерений на каждый срез:** позитивные, негативные, граничные (Snowflake `0`/`u64::MAX`, 2000 символов, 10 файлов/25 МБ, `offset=9975`, громкость `0..200`, бейдж `99+`), ошибки (`401/403/404/413/429 retry_after_ms/5xx`), пустые (`empty`), загрузочные (`skeleton`/`HTTP 202`), права доступа (`capabilities`/`AGE_GATED_SPACES`), восстановление (`reconnect`/`fallback`), совместимость и наблюдаемость (`.gate/*.json`, `window-runs.jsonl`, маскирование токенов).
- **Пороги CI:** `Pass P0/P1 = 100%`, `lines >= 90%`, `branches >= 90%`, `functions >= 80%`, `mutations = 5/5 (100%)`, `flaky_rate = 0%` на `beta`/`main` (`failOnSkips: true`).

---

## 6. Запреты и Стиль

### Запреты
- `logs/`, `functions/`, `evidence/` — golden: **только чтение**. Никаких записей/переименований.
- Никаких `git commit/push` из воркеров в общие ветки — слияние и релизные коммиты делает координатор/интегратор по правилам пайплайна.
- Никаких сетевых вызовов, установок пакетов и изменений `package.json` без разрешения.
- Не спавнить Discord (`device.spawn`) и не делать attach/walk на живом Discord без окна, согласованного координатором.
- Не «зеленить» тесты правкой тестов под код. Ослаблять тесты, отключать проверки и принимать решения мимо тестов/гейта — запрещено (`TEST-POLICY.md`, нарушение = `P0` и возврат работы).

### Стиль
- Node ≥ 24, ESM, `// @ts-check` + JSDoc в каждом модуле; `npx tsc -p tsconfig.json` — чисто.
- Сообщения CLI и доки — на русском; коды выхода: `0` успех, `1` ошибка аргументов, `2` среда недоступна.
- Тесты skip-aware: отсутствие внешней среды → skip с причиной (в `gate:release` — `failOnSkips: true`).
- Ошибки не глотать молча: либо последствие (код/счётчик/сообщение), либо явный комментарий, почему это best-effort.


---

## 7. Обязательная сверка с эталонным RE-дампом `/home/sanya/re-deep` и инструментами `rea`

1. **Запрет догадок при реализации и специфицировании:**
   - Если неизвестна или непонятна точная логика функции, формат payload/ошибки, константа, лимит, i18n-ключ, опкод шлюза/голоса или поведение UI-компонента — **строго запрещено придумывать поведение наугад**.
   - Сначала выполни поиск и сверку по эталонному RE-дампу **`/home/sanya/re-deep/`** (read-only оракул) или примени инструменты обратного инжиниринга **`rea`** (скилл `reverse-engineer-anything`) к артефактам дампа либо к отдельно запущенному экземпляру Discord.
2. **Карта оракулов в `/home/sanya/re-deep/`:**
   - `decoded/` и `decoded-index.json` (759 КБ), `app-asar/`, `core-asar/` — декодированные JS/Webpack-чанки и модули Electron.
   - `gateway-protocol.md`/`.json`, `gateway-schema.md`/`.json`, `gw-proto/`, `capture-ready.jsonl`, `capture-run2.jsonl` — реальные трассы шлюза, ETF big-int, zstd-стримы и `READY`.
   - `rest-d02.md`/`.json`, `rest-static.md`/`.json`, `d02-snapshot/` — маршруты REST v9, заголовки, `X-Super-Properties`, коды ошибок и ответы.
   - `frontend-d03.md`/`.json`, `ui/`..`ui8/`, `voice/ui7/` — контракты 12 UI-компонентов, DOM-селекторы, поиск, эмодзи-пикер, настройки, попапы и голосовая панель.
   - `main-arch.md`/`.json`, `backend-ipc.md`/`.json`, `native-re.md`/`.json`, `voice-deep.md`/`.json`, `voice/` — IPC-каналы `DISCORD_*`, `DiscordNative`, нативные модули (`discord_voice.node`), сигнализация RTC, RTP/RTCP и крипто-режимы.
   - `i18n-deep.md`/`.json`, `ru-catalog.json`, `en-catalog.json` — хеши ключей локализации (`xxh64 -> base64[0..6]`) и русские/английские строки.
   - `rea-evidence.json`, `rea-web-observations.json`, `rea-web-digest.json` — собранные Evidence-бандлы REA.
3. **Применение всех инструментов `rea` (`reverse-engineer-anything`):**
   - Для статического и графового анализа JS/Electron/ASAR: `analyze_javascript_application`, `trace_javascript_semantics`, `trace_application_feature`, `trace_web_module_imports`, `recover_javascript_sources`, `compare_source_to_bundle`, `build_reconstruction_obligation_ledger`, `evaluate_reconstruction_coverage`.
   - Для анализа нативных бинарников и `.node`-модулей: `open_binary`, `binary_overview`, `list_procedures`, `search_procedures`, `procedure_pseudo_code`, `procedure_assembly`, `analyze_function`, `search_strings`, `xrefs`, `trace_call_path`, `correlate_static_and_runtime`.
   - Для динамического анализа отдельно запущенного приложения Discord (Electron/CDP/Frida/pcap): `list_electron_targets`, `inspect_electron_page`, `capture_electron_scenario`, `observe_javascript_runtime`, `reconcile_javascript_runtime`, `start_frida_session`, `instrument_with_frida`, `inspect_pcap_capture`, `inspect_web_network_capture`.
   - Каждая найденная функция, константа или контракт фиксируется в спецификации/отчёте с точной ссылкой на чанк/смещение в `/home/sanya/re-deep/` или Evidence-запись `rea`.


---

## 8. Предел памяти воркеров до 25 GB RAM (`WORKER_RAM_LIMIT <= 25 GB`), защита от OOM Killer и фиксированный пул (`3–4 Dev/Review + 1–2 G7 RE = max 5–6`)

1. **Жёсткий предел ОЗУ на всех воркеров — до 25 GB RAM (`WORKER_RAM_LIMIT = 25 GB`):**
   - На хосте 48 GB RAM (`Swap: 0B`). **Суммарное потребление памяти всеми воркерами и их дочерними процессами (`opencode`, `agy`, `cargo`, `rustc`, `node`, `rea`) никогда не должно превышать `25 GB RAM`**, оставляя не менее 20–23 GB системе и базовому окружению.
   - Чтобы гарантированно держаться в пределах **до 25 GB RAM** и исключить срабатывание OOM Killer, используется фиксированный пул из **5–6 активных слотов** (`3–4` слота разработки/ревью/QA + `1–2` выделенных read-only слота отдела `G7 RE / REA`).
   - Запрещено создавать воркеров сверх лимита. Zero-Idle обеспечивается **непрерывной ротацией внутри этих слотов (1-в-1)**: как только воркер завершил задачу (`worker_done`), его терминал освобождается (`worker-release`), и на этот освободившийся слот сразу назначается следующая задача из очереди.
2. **Непрерывный конвейер обратной разработки (`G7 RE / REA -> PM -> Разработка`):**
   - Выделенные `1–2` воркера отдела **`G7 RE / REA`** непрерывно сравнивают наш код (`rust/crates/*`, `rust/gui-spike/*`) с дампом `/home/sanya/re-deep/` и живой сессией Discord (CDP `9223` / Electron / ASAR) через инструменты `rea`, документируют недостающие части кода и несовпадения в `waves/RW-*/RE-AUDIT/RE-GAP-*.md` и передают их PM (`waves/RW-*/PM/BACKLOG-RE-*.md`) для постановки задач на разработку, улучшение и модернизацию.
3. **Обязательное освобождение терминалов (`worker-release`) и замер RSS воркеров (`<= 25 GB`):**
   - Перед любым запуском нового воркера сначала выполни:
     `for ctx in $(orca orchestration worker-list --run <run_id> | awk '/terminal=reclaimable/ {print $1}'); do orca orchestration worker-release --dispatch "$ctx"; done`
   - Проверь суммарный RSS процессов воркеров (`ps -eo rss,comm | awk '/opencode|agy|cargo|rustc/ {s+=$1} END {printf "%.2f GiB\n", s/1024/1024}'` — должен быть строго `< 25 GiB`) и `free -h`.
   - Не более 1 процесса `cargo` одновременно на общем `CARGO_TARGET_DIR`.
