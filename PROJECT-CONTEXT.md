# discord-lab: session checkpoint (сжатый контекст)

Точка входа для нового диалога. Читать первым; детали — точечно по ссылкам.
Обновлён 2026-10-23 после RW-37.

## ПАУЗА (2026-10-24)

Разработка приостановлена по команде владельца. Активных процессов нет.
B3b (UI реакций) — **red: frontend 176/5**, gui-spike **121/0**, core **573/0**.
Приняты: A5, B3a, B4a, G3 Design-MSG. Возобновление — `waves/RW-40/PAUSE.md`:
завершить B3b → gate/workspace → очередь B4b/B2/B5/…

## Текущая работа (2026-10-24, auth)

- Auth-контур собран: A1 окно входа (41/41, tsc 0), A2 captcha-поля, A3b core events,
  A3c state machine, A3d production driver, A4 logout/events.
- Проверки: core **573/0**, gui-spike **105/0**, workspace **1111/0**, `gate:release` **passed_with_skips**
  (e2e 4 opt-in skip). Живой frame-smoke отключает auth-гейт через `SPIKE_AUTH_DISABLED=1`.
- В работе: A6 security-review; далее A5 QA-AUTH (offline), A2b UI captcha-поля, B1 отправка.
- Live READY требует тестового токена и согласованного окна.


- **RW-40** — план продукта: `waves/RW-40/{TZ,TASKS,COMMS}.md` (инвентарь функций, отделы,
  доска, коммуникация). Предусловие: закрыть auth (W5 41/41 + tsc 0, NET-F1b, NET-E2).
- **Auth W5**: окно входа **40/41**, tsc 52 ошибки — не принято; критический путь.
- **NET**: RE login/captcha/2FA готов (D-38), дизайн готов (D-36), backend password/MFA готов
  (D-35); connect/READY (NET-E2) и captcha-поля (NET-F1b) — очередь.
- **Гейт**: `gate:release` passed_with_skips; workspace 1106/0; e2e 4 skip; QA-матрица не прогнана.

## Текущая работа (2026-10-23)

- **RW-38** (усиление тестов GUI): **принят** — REV-2 PASS (P0=0, P1=0), md5 7/7, окно за
  ревью 0/1. DOM 21 + model 26 (0 skip, ~120 мс), Rust IPC 20 + launch 1, AT-SPI/X11-проба
  с PID-привязкой, GBM/ранний exit → fail, `metrics.json` с FPS. Детали — `waves/RW-38/`.
- **RW-39** (дизайн-паритет): **E1 закрыт** (токены приняты, D-06; `ui/tokens.css`
  перевендорен), **E3a принят** (канонический `index.html`, D-07). В работе: W39d (frontend,
  D-04 высоты 26/48/+22) и W39e (backend BFF на `fixtures/`) — параллельно. Далее QA/REV
  и один приёмочный gate. Доска — `waves/RW-39/PM/STAGE.md`.
- **Анти-петля**: `waves/RW-39/PM/WINDOW-BUDGET.md` — итерации только DOM/model/geometry,
  живое окно 1 раз на приёмку (кэш `window-budget.mjs`), лимит попыток — `loop-guard.mjs`.

## Состояние

- `/home/sanya/orca/project`, `main`, HEAD `0239543`; RW-35 принят, **RW-36/B07 (GUI-спайк
  Tauri) принят условно**. Локальные изменения RW-35/RW-36 требуют отдельного ревью и не
  входят в принятый baseline; коммиты/пуш — координатор.
- **Принято 48 / 90** карточек (хвосты P7-01b/c/d/e и расширения P6-02b/c/d, P6-06c в 90
  не входят). Последние волны:
  - **RW-37** (2026-10-23): оконная модель данных (ADR-1 follow-up) — `PageStore` ≤500 строк,
    LRU/`inFlight`; **PSS 393,9 → 276,2 МБ** (≤300), 60 fps p95 18 мс; RE-контракт раскладки;
  - **RW-36** (2026-10-23): P0-01/P0-06 GUI-спайк Tauri 2 + статический фронтенд: 4 экрана
    (100k/24 229/10/38), 60 fps p95 18 мс, cold start 102–137 мс, cold build 29,56 с;
    **ADR-1 — Tauri принят, PSS 394 МБ > 300 МБ → follow-up оконная модель**;
  - **RW-35** (2026-10-23): P6-06c fd-передача D-Bus (SCM_RIGHTS) + logind sleep-inhibitor;
  - **RW-34** (2026-10-23): P6-02c abstract D-Bus, P6-02d hints ext;
  - **RW-33**: P1-10 keyring через libsecret FFI (live ksecretd + secret-tool);
  - **RW-32**: P6-07 spellcheck (hunspell FFI), P6-02b hints `a{sv}`;
  - **RW-31**: P6-06 power/idle, P2-03 SQLite-store через FFI к системному libsqlite3;
  - **RW-30**: P7-01d DurationFormat, P6-02 D-Bus-уведомления (headless, без новых крейтов);
  - **RW-28**: P7-01c RelativeTimeFormat — ICU-паритет ±2, точные i64 > 2^53 (headless);
  - **RW-29**: P4-04 RTP-медиа (PT120/0xBEDE, DTX, джиттер по pcap-оракулу), P4-12
    reconnect/resume, P7-01e `<t:…>`-таймстемпы (headless).
- Зафиксированный gate-baseline: workspace **1069 passed**, core 531, i18n 83, media 13,
  reconnect 12, platform 110, store 14; GUI-спайк Rust **16**, Node targeted **66**, live
  smoke **1**; fmt/clippy чисто; npm `gate:worker` `passed_with_skips` (1 opt-in GUI smoke);
  мутации RW-36 Rust **9/9**, RW-37 model **12 caught** (2 эквивалентных), restore md5.
- Гейт: `.gate/worker-2026-10-23T14-57-20-838Z.json`; метрики — `waves/RW-37/METRICS.md`
  (PSS 276,2 МБ), baseline RW-36 — `waves/RW-36/METRICS.md` (393,9 МБ).
- Ревью: RW-36 — два прохода codex: pass1 FAIL (P0=0, P1=4) → pass2 **PASS (P0=0, P1=0)**,
  13 P2 (критичные закрыты); RW-28 — два прохода; RW-29 voice — принято, 8 P3 в бэклог.

## Что дальше

1. **P3-01 (каркас окна и навигация)** — следующий шаг; RE-контракт готов
   (`waves/RW-37/RE/layout.json`): rail 64, tree 311, header 49, composer 56,
   строки сообщений 26/48, участник 42. Оконная модель (`PageStore`) уже принята.
2. **P3-02 (чат-вью)** после P3-01; затем P3-03…P3-16 по `PLAN.md`; live-smoke — по
   согласованному окну.
3. Остаток P0 — `P0-05` (спайк хранения секретов; keyring фактически закрыт RW-33, статус
   сверить). Headless-остатки — P4/P6/P7 по `PLAN.md`.
4. Процесс: batch 3–6 карточек → red-first → targeted → mutation → один batch gate →
   независимое ревью → accepted → доки один раз. Роли — `docs/rust-client/TEAM.md`.

## Правила (кратко)

- Strict TDD; только назначенные файлы; shared-файлы и `waves/*` — Team Lead/Tech Lead.
- Golden `logs/`, `functions/`, `evidence/`, `/home/sanya/re-deep` — только чтение.
- Без сети, установок, `package.json`-правок, live Discord и commit/push без разрешения.
- Один `cargo` за раз; в batch полный gate — один раз в integration-лейне.

## Команды

- `npm run gate:worker` — быстрый гейт (typecheck, meta, unit, smoke).
- `cd rust && cargo test --offline --workspace` — Rust (npm-гейт его не запускает).
- Восстановление: `git log --oneline -3` → `git status --short` → `tail -n 5 waves/db.jsonl`
  → верх `waves/STATE.md`.

## Скиллы

`token-efficiency` (фильтровать чтение, bash вместо Read), `discord-lab-context`
(навигация по репо), `context-engineering-collection`; `session-compression` избыточен.
MCP: rea, socraticode, github, basic-memory, context7.

## Известные расхождения

- Нумерация rolling batches отличается между `WAVE-PLAN.md`, `ROADMAP.md` и журналом;
  перед постановкой следующего batch сверяй карточки с `TASKS.md` и `waves/STATE.md`.
- RTP-фикстура `fixtures/voice/rtp-pt120-oracle.json` получена из golden-pcap
  `/home/sanya/re-deep/voice/voice-udp4.pcap` генератором `waves/RW-29/gen-rtp-oracle.py`
  (md5 `798dd8ae…`); путь `/home/sanya/qwen-uncens` из запроса не существует.
- P3-бэклоги RW-26/RW-27 (SRTCP-трейлер, bounded `seen`, Exec `Name`-escaping,
  `ru-RU`-fallback) — в `waves/STATE.md` §7.
- RW-36: GUI-спайк на X11 требует `WEBKIT_DISABLE_DMABUF_RENDERER=1` +
  `LIBGL_ALWAYS_SOFTWARE=1`, иначе GBM-буферы падают и rAF не тикает (`BENCH` не появляется).
  RAM-бюджет §9 сверяется по PSS дерева (`waves/RW-36/pss.json`), не по RSS родителя.
- RW-36: `rust/gui-spike/ui/tsconfig.json` не подключён к `npm run typecheck` (правка
  `package.json` требует разрешения владельца); гоняется вручную в отчёте волны.
