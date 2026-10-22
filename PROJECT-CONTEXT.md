# discord-lab: session checkpoint (сжатый контекст)

Точка входа для нового диалога. Читать первым; детали — точечно по ссылкам.
Обновлён 2026-10-22 после RW-30.

## Состояние

- `/home/sanya/orca/project`, `main`, HEAD `ebf5efd`; принятые волны RW-10…RW-29 закоммичены и запушены; RW-30 — коммит
  следующим шагом; коммиты/пуш — координатор.
- **Принято 42 / 90** карточек (хвосты P7-01b/c/d/e в 90 не входят). Последние волны 2026-10-22:
  - **RW-30**: P7-01d DurationFormat, P6-02 D-Bus-уведомления (headless, без новых крейтов);
  - **RW-28**: P7-01c RelativeTimeFormat — ICU-паритет ±2, точные i64 > 2^53 (headless);
  - **RW-29**: P4-04 RTP-медиа (PT120/0xBEDE, DTX, джиттер по pcap-оракулу), P4-12
    reconnect/resume, P7-01e `<t:…>`-таймстемпы (headless).
- Факт: workspace **984 passed**, core 531, i18n 83 (relative 12 + timestamp 14 + duration 13),
  media 13, reconnect 12, platform 41 (dbus 7 + notifications 11); fmt/clippy чисто;
  npm `gate:worker` passed; мутации 8/8, 8/8, 9/9, 6/6, 8/8, 10/10 (все restored по md5).
- Гейт: `.gate/worker-2026-10-22T17-54-26-464Z.json`, `waves/RW-30/gate-cargo.log`.
- Ревью: RW-28 — два прохода (P0 Auto ±2 → решение TL `waves/RW-28/DECISIONS.md`; P1 точность
  → точные `plural_category_integer`/`format_integer_unsigned`); RW-29 voice — принято,
  P0/P1/P2=0, 8 P3 в бэклог (`waves/STATE.md` §7).

## Что дальше

1. **B07 = P0-01 / P1-10 / P2-03** (GUI spike, keyring, SQLite) — **заблокирован** до
   решения владельца: нет `tauri`/`iced`/`egui`/`winit`/`wry` в lock/кэше; `keyring` +
   Secret Service; `rusqlite` + системный SQLite; живой Discord — только по окну.
2. Headless-остатки: **P6-06** power/idle (контракт `waves/RW-31/TZ-W31a.md`, на общем
   D-Bus-клиенте), **P2-03** SQLite через FFI к системному `libsqlite3` (решение —
   `waves/RW-31/SQLITE-DECISION.md`, нужен «ок» владельца на `unsafe` в модуле `ffi`);
   P4-05/P4-06/P4-08/P4-09 требуют GUI/зависимостей.
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
