# RW-21 / B02 — отчёт batch

Дата: 2026-10-21. Batch: **B02**. Карточки: **P2-08** (settings-proto), **P1-01** (ETF full),
**P1-02** (zstd strict), **P0-04** (conformance skeleton), **P0-07** (legal).
Статус: **green, gate passed; независимые review P0/P1/P2=0; P3 закрыты**.
Осталась финальная синхронизация доков/журнала.

## 1. Сделано

- **P2-08**: protobuf/base64 structural decoder, type 1 typed sections, type 2 structural
  `FrecencyUserSettings`, unknown preservation, лимиты/ошибки, redacted Debug. Review
  round 1 нашёл P2 (Debug секций), fixed red-first; round 2 — accepted with P3, P3 закрыт
  boundary-тестом `MAX_FIELDS`.
- **P1-01**: `EtfEncodeError` + `encode_checked`/`encode_term_checked`; проверки `u32`/`u16`/
  `MAX_DEPTH` без молчаливого усечения; infallible `encode` оставлен для доверенных
  значений и документирован; 50k fuzz + все однобайтовые + усечённые префиксы.
- **P1-02**: терминальные ошибки `ZstdStream` (`is_dead`, `Poisoned`), корректный
  `is_finished` у свежего потока, контракт удержания окна 64 КиБ; 4 новых regression-теста.
- **P0-04**: `discord-conformance` loader repo→lab, `RE_FIXTURES_ROOT`/`RE_LAB_ROOT`,
  типизированные `InvalidName`/`NotFound`/`Io`/`Utf8`, 6 тестов T-HARN-01;
  `RE_REQUIRE_LAB=1` запрещает засчитывать отсутствие lab как pass.
- **P0-07**: `docs/rust-client/LEGAL.md` (ToS, telemetry off, честный отпечаток, секреты,
  ассеты, approval) + ссылки в README клиента и `rust/README.md`.
- **Метод/ускорение**: `docs/rust-client/CODEX-WORKERS.md` (роли, sandbox, промты, DAG,
  mutation feedback loop); 5 уроков B02 добавлены в `CODE-STANDARD.md` §9.

## 2. Red / green

| Фаза | Команда/лог | Результат |
|---|---|---|
| P2-08 red | `settings_proto` | `E0583`, exit 101 — `logs/p08-red-compile.log` |
| P2-08 P2 red | `p08t17` | id/locale в Debug — `logs/p08-p2-red.log` |
| P2-08 green | `settings_proto` | **18 passed** — `logs/p08-targeted-final3.log` |
| P1-01 red | `etf_full` на старом продукте | compile-red, exit 101 — `logs/etf-full-red.log` |
| P1-01 green | `discord-codec` | **43 passed** — `logs/b02-codec-targeted.log` |
| P1-02 red | `zstd_stream` на старом продукте | compile-red, exit 101 — `logs/zstd-strict-red.log` |
| P1-02 green | `discord-codec` | **43 passed** — `logs/b02-codec-targeted.log` |
| P0-04 red | `fixtures` на старом API | compile-red, exit 101 — `logs/conformance-red.log` |
| P0-04 green | `discord-conformance` | **6 passed** — `logs/b02-conformance-require-green3.log` |
| core | `cargo test -p discord-core` | **524 passed, 0 failed** — `logs/b02-core-final.log` |
| workspace | `cargo test --workspace` | **580 passed, 0 failed** — `logs/b02-workspace-final2.log` |
| fmt | `cargo fmt --all -- --check` | exit 0 — `logs/b02-fmt-final2.log` |
| clippy | `cargo clippy ... -D warnings` | exit 0 — `logs/b02-clippy-final2.log` |
| gate | `npm run gate:worker` | **passed** — `gate-worker-b02.json` |

## 3. Мутации

- B02 codec: **11/11 caught** — `P101-M1…M6` (глубина, atom/binary length, tuple threshold,
  big-int sign), `P102-M1…M5` (dead state, poison, fresh finished, trailing/decode).
- P2-08: **13/13 caught** — `M1…M13` (лимиты, last-wins, wrapper, signed timezone, Debug,
  fixed64, MAX_FIELDS off-by-one). Логи: `mutations/`, сводки `summary-b02.log`,
  `summary-p208-extra.log`.
- Продукт восстановлен; md5 после restore совпали. `mutations/` — намеренные падения,
  не текущий green.

## 4. Проверка по RE-дампам

- `/home/sanya/re-deep/gw-proto/zstd-frames-95974_1389.hexl`: 137 кадров, 104 066 сжатых,
  выход 759 280 Б; удержание окна 65 536 = 824 816 − 759 280; HELLO 138 Б совпал.
- `/home/sanya/re-deep/gw-proto/msgs-run2-95974_1389.jsonl`: 44 полных терма, смещение
  758 576, остаток 704 Б — тест `lab_stream_1389_matches_oracle`.
- READY/UPDATE settings: base64 4400/17 436, raw 3300/13 077, sha256 зафиксированы в
  `REPORT-P2-08.md`; type 2 versions 7545/7546/7547.
- ETF golden-векторы RW-01 и `gateway-protocol.md` §2 остаются независимым оракулом.
- Никаких выдуманных Discord API: type 2 favorite/frecency semantics не типизирована.

## 5. Файлы и md5

| Файл | md5 |
|---|---|
| `rust/crates/discord-codec/src/etf.rs` | `ae550fcc506082db0a2e86f74042001a` |
| `rust/crates/discord-codec/src/zstd.rs` | `bc5c9ae4bf481a6592c2fd13522fbfb1` |
| `rust/crates/discord-codec/src/lib.rs` | `ee292d8bb636d33975375c9012f8c9d9` |
| `rust/crates/discord-codec/tests/etf_full.rs` | `079ba256e725435be3f795ee6a08fee8` |
| `rust/crates/discord-codec/tests/zstd_stream.rs` | `6891626df861267c68205e495a4648f9` |
| `rust/crates/discord-conformance/src/lib.rs` | `ae6dd3a4facfe9a12aca4dc9d0fafd64` |
| `rust/crates/discord-conformance/tests/fixtures.rs` | `9be1baafd24eb22011c57b380c8859ba` |
| `rust/crates/discord-core/src/settings/preloaded.rs` | `01539d24d6ba64d730502e2bbf041593` |
| `rust/crates/discord-core/tests/settings_proto.rs` | `617010189be41bff41e244a211951e82` |
| `docs/rust-client/LEGAL.md` | `c4055cbc6b315d6b552d13299704a675` |
| `docs/rust-client/CODEX-WORKERS.md` | `209feba769c4300cf68355971bdeb1ed` |
| `docs/rust-client/README.md` | `19f52dcf1bd0499a17375f05efa8fd14` |
| `docs/rust-client/CODE-STANDARD.md` | `7f27feb06762ae1840c74d33df59b43d` |
| `rust/README.md` | `519d28f9fc7eb6c230942535244bac2e` |
| `waves/RW-21/gate-worker-b02.json` | `d46ae40c32229b4a8eb12b28c0f666bc` |

P2-08 product md5 (wire/frecency/mod/lib) — в `REPORT-P2-08.md` §5; после review менялся
только `preloaded.rs` и тест.

## 6. Независимые review и закрытие находок

- **P2-08**: round 1 — P2 (Debug секций печатал fixed64 id/locale), закрыт red-first;
  round 2 — accepted with P3 (`MAX_FIELDS` boundary), P3 закрыт `p08t18` + mutation M13.
- **P1-01/P1-02** (Codex CLI, read-only): accepted with P3 — устаревший статус zstd в
  `lib.rs`; исправлен, P0/P1/P2=0 (`reviews/review-codec-codex.md`).
- **P0-04/P0-07**: round 1 — 3 P2. (1) «манифест не там» — **retracted** после `realpath`:
  путь `/home/sanya/orca/project/fixtures/MANIFEST.json`, тест 6/6; (2) lab-skip —
  добавлен `RE_REQUIRE_LAB=1` (red 101 без lab, green 6/6 с lab); (3) LEGAL — гарантии
  keyring/скана/SBOM переведены в policy/backlog, чек-лист `[x]/[~]/[ ]`. Round 2 —
  accepted with P3 (`RE_REQUIRE_LAB=0` считался включённым), P3 закрыт: проверяется
  только `"1"`; red/green в `logs/b02-conformance-require*.log`.
- Итог: **P0/P1/P2=0**, P3 закрыты или в backlog; авторы код не ревьюили.

## 7. Риски / backlog

- P1-02: последнее окно незавершённого кадра не отдаётся — ограничение `ruzstd` streaming;
  контракт зафиксирован, не потеря данных. Для «дочитать до конца» нужен другой декодер.
- P0-04: реализован скелет T-HARN-01; полный replay T-HARN-02/03 — отдельная карточка
  (нужен JSON-компаратор, чтобы не тащить зависимость в скелет).
- P0-07: юридическая оценка ToS/распространения — за владельцем; документ не заменяет юриста.
- P2-08: `status_expires_at_ms`/custom_status остаются structural/raw; type 2 semantics —
  backlog. Это не блокирует P2-08.
- Нет сети, live Discord, SQLite, keyring, GUI, новых зависимостей, коммитов.
- P1-01/P1-02/P0-04/P0-07 review-артефакты: `reviews/review-codec-codex.md`,
  `reviews/review-harness-legal-round2.md`; P2-08 — `reviews/review-p2-08-round2.md`.
