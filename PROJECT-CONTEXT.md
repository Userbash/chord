# discord-lab: session checkpoint (сжатый контекст)

Точка входа для нового диалога. Читать первым; детали — точечно по ссылкам.
Обновлён 2026-10-23 после RW-34.

## Состояние

- `/home/sanya/orca/project`, `main`, HEAD `c2c3f75`; RW-10…RW-33 закоммичены и запушены; RW-34 — коммит следующим шагом;
  коммиты/пуш — координатор.
- **Принято 46 / 90** карточек (хвосты P7-01b/c/d/e и P6-02b/d в 90 не входят). Последние волны:
  - **RW-34** (2026-10-23): P6-02c abstract D-Bus, P6-02d hints ext;
  - **RW-33**: P1-10 keyring через libsecret FFI (live ksecretd + secret-tool);
  - **RW-32**: P6-07 spellcheck (hunspell FFI), P6-02b hints `a{sv}`;
  - **RW-31**: P6-06 power/idle, P2-03 SQLite-store через FFI к системному libsqlite3;
  - **RW-30**: P7-01d DurationFormat, P6-02 D-Bus-уведомления (headless, без новых крейтов);
  - **RW-28**: P7-01c RelativeTimeFormat — ICU-паритет ±2, точные i64 > 2^53 (headless);
  - **RW-29**: P4-04 RTP-медиа (PT120/0xBEDE, DTX, джиттер по pcap-оракулу), P4-12
    reconnect/resume, P7-01e `<t:…>`-таймстемпы (headless).
- Факт: workspace **1057 passed**, core 531, i18n 83, media 13, reconnect 12, platform 100,
  store 14; fmt/clippy чисто; npm `gate:worker` passed; мутации 8/8…10/10, 7/7, 7/7, 6/6, 7/7,
  6/6, 6/6 (все restored по md5, SKIP=0).
- Гейт: `.gate/worker-2026-10-22T19-06-01-142Z.json`, `waves/RW-34/gate-cargo.log`.
- Ревью: RW-28 — два прохода (P0 Auto ±2 → решение TL `waves/RW-28/DECISIONS.md`; P1 точность
  → точные `plural_category_integer`/`format_integer_unsigned`); RW-29 voice — принято,
  P0/P1/P2=0, 8 P3 в бэклог (`waves/STATE.md` §7).

## Что дальше

1. **B07 = P0-01 / P1-10 / P2-03** (GUI spike, keyring, SQLite) — **заблокирован** до
   решения владельца: нет `tauri`/`iced`/`egui`/`winit`/`wry` в lock/кэше; `keyring` +
   Secret Service; `rusqlite` + системный SQLite; живой Discord — только по окну.
2. **B07 остался только GUI**: keyring и SQLite закрыты; нужен выбор GUI-тулкита
   (`tauri`/`iced`/`egui` — нет в офлайн-кэше) и окно для live-smoke. Headless-остатки:
   fd-передача D-Bus (`SCM_RIGHTS`; `libc` есть в кэше — решение владельца), P4-05/P4-06/P4-08/
   P4-09, P7-03/P7-05/P7-07 (UI).
3. Процесс: batch 3–6 карточек → red-first → targeted → mutation → один batch gate →
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

- `docs/rust-client/WAVE-PLAN.md` §«Очередь» сохранил старую нумерацию (B04/B07), тогда как
  `STATE.md`/`ROADMAP.md`/`TASKS.md` используют **B07 = GUI/keyring/SQLite**; WAVE-PLAN
  ждёт синхронизации TL. B10 (RW-28/RW-29) в WAVE-PLAN не отражён — как и прежние B06/B09.
- RTP-фикстура `fixtures/voice/rtp-pt120-oracle.json` получена из golden-pcap
  `/home/sanya/re-deep/voice/voice-udp4.pcap` генератором `waves/RW-29/gen-rtp-oracle.py`
  (md5 `798dd8ae…`); путь `/home/sanya/qwen-uncens` из запроса не существует.
- P3-бэклоги RW-26/RW-27 (SRTCP-трейлер, bounded `seen`, Exec `Name`-escaping,
  `ru-RU`-fallback) — в `waves/STATE.md` §7.
