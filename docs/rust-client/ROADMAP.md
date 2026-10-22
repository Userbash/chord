# План дальнейшей разработки Rust-клиента

Основа: `PLAN.md` (90 карточек), факт — `STATUS.md`, журнал — `../../waves/STATE.md`.
Правило: один **batch** (3–6 карточек) = research → замороженный дизайн/TZ → integration-патч
общих файлов → red-first воркеры → QA/mutation → batch-гейт → независимое ревью → `accepted` →
один раз доки/журнал. Роли и WIP — `TEAM.md`; batch-план и KPI — `WAVE-PLAN.md`.

Целевой режим: **2–3 batch-волны/день, 6–8 принятых карточек/день**. 58 оставшихся карточек —
это ~16–20 batch-волн (10–13 рабочих дней после снятия блокеров), а не 58 отдельных волн.

## 1. B01 = P2-10 + P2-11 — принято 2026-10-21

Memory-only voice states и typing TTL: `READY_SUPPLEMENTAL` snapshot аддитивен,
`VOICE_STATE_UPDATE` join/move/leave last-wins, timer/status; `TYPING_START` TTL 10 000 мс,
repeat/remove/READY reset, детерминированные `received_at_ms`/`now_ms`. Targeted 33/33,
core 506, workspace 548, gate passed; мутации 18/18. Два независимых ревью: P2-10 round 2 —
P0/P1/P2=0, P3=11; P2-11 — P0/P1/P2=0, P3=10. Артефакты — `waves/B01/`, `waves/RW-19/`,
`waves/RW-20/`, ревью — `waves/reviews/review-p2-10-round2.md`, `review-p2-11.md`.

## 2. B02 = P2-08 + P1-01 + P1-02 + P0-04 + P0-07 — принято 2026-10-21

Settings-proto (structural protobuf, type 1 typed/type 2 structural), checked ETF-энкодер,
терминальный zstd-стрим, conformance loader и правовая рамка. Targeted: settings 18/18,
codec 43/43, conformance 6/6; core 524, workspace 580; fmt/clippy 0; gate
`waves/RW-21/gate-worker-b02.json`; мутации 11/11 (codec) + 13/13 (settings).
Независимые review: P2-08 round 2 accepted, P3 закрыт; P1-01/P1-02 accepted with P3 (doc),
P0-04/P0-07 round 2 accepted with P3 (env), P3 закрыт. Артефакты — `waves/RW-21/`,
ревью — `waves/RW-21/reviews/`.

### B02/B03: итог

- **B02:** P2-08 (settings-proto), P1-01 (ETF full), P1-02 (zstd strict),
  P0-04 (conformance harness), P0-07 (legal) — **принято 2026-10-21**.
- **B03:** P3-15 (i18n runtime), P4-01 (voice signaling headless), P7-04 (API completeness),
  P8-03 (CI) — **принято 2026-10-21**.
- Детали, зависимости и критический путь — `WAVE-PLAN.md` §4.

## 3. B03 = P3-15 + P4-01 + P7-04 + P8-03 — принято 2026-10-21

Строгий JSON foundation, i18n runtime (hash/locale/AST/plural/markdown), headless RTC
signaling (op0/1/2/3/4/6/8/15/16, endpoint, region), read-only D-02 API (8 GET-ресурсов),
CI/release. Targeted: JSON 11, i18n 8, voice 12, REST 6, CI 4; core 530, workspace 618;
fmt/clippy 0; gate `waves/RW-22/gate-worker-b03.json`; мутации 18/18. Review A/B нашли P1/P2,
все закрыты red-first; review C round 2 — P0/P1/P2=0. P3: `$link` URL (P7-01), UDP/DAVE
(P4-02/P4.5), query P7-04 (D-02 не подтверждает), cargo-deny/SBOM (P8-04).
Артефакты — `waves/RW-22/`, ревью — `review-A/B/C`.

## 4. B04 = P4-02 + P8-04 — принято 2026-10-21

UDP media primitives (74-байтный discovery, 8-байтный keepalive/5 с, rotation 60 080 мс,
8-байтный RTCP RR + 28-байтный SRTCP wrapper, loopback, peer/SSRC checks, `rotate_if_due`)
и offline reproducible SPDX 2.3 SBOM (85 locked-пакетов, strict allowlist/AND/OR/WITH,
CI hook, release checksums). Targeted UDP 15/15, SBOM 5/5; core 530, workspace 638;
fmt/clippy 0; gate `waves/RW-23/gate-worker-b04-final.log`; мутации 21 caught + 1 inconclusive.
Review A/B: P0/P1/P2=0 после red-first фиксов. Артефакты — `waves/RW-23/`.

## 4.1. B05 = P7-01 — принято 2026-10-21

Полный i18n: 36 локалей (31 включена), CLDR cardinal/ordinal, точный `=N` до offset,
числа/валюты/проценты, 4 стиля даты/времени UTC, `th` +543, localized names/timezone,
skeleton, 23 markdown-правила с safe URL и bounded depth, fallback current → default → en-US.
`discord-i18n` 33/33, workspace 663, core 530; fmt/clippy 0; gate
`.gate/worker-2026-10-21T08-58-08-129Z.json`; мутации 20/20. Review A/B: P0/P1/P2=0
после red-first фиксов; P3 — backlog (`$link`, полные переводы, list/relative/duration,
`<t:R>`-таймеры). Артефакты — `waves/RW-24/`.

## 4.1.1. B06 = P4-03 + P7-02 + P8-01 — принято 2026-10-22

Первая **headless**-волна: офлайн, без новых внешних зависимостей (только офлайн-кэш
cargo), три независимых лейна с разными наборами файлов.

- **P4-03 SRTP-крипта** (`discord-voice`): AES-256, AES-256-GCM, ChaCha20/XChaCha20-Poly1305,
  HKDF-SHA1 — без зависимостей; S-box алгебраический (`affine(x^254)`), таблицы убраны
  (side-channel через кэш). Раскладка `[clear][ciphertext][tag 16][counter 4 LE]` подтверждена
  capture'ом и функциями клиента (`HeaderLength`/`SpaceRequiredToEncrypt`). Исходящий счётчик
  `0x80000000 | n`, общий для RTP/SRTCP; AAD = открытая часть в режимах `*_rtpsize`.
  38 тестов (KAT NIST/RFC). **Совместимость с wire Discord не заявлена.**
- **P7-02 перф-харнесс** (`discord-perf`, новый крейт): детерминированная нагрузка
  24 229 участников + 100 000 сообщений на реальном редьюсере, гейт регрессии >10 %
  (T-HARN-04). Холодный старт ~47 мс при бюджете 1500 мс. 17 тестов.
- **P8-01 updater** (`discord-updater`, новый крейт): манифест + подпись Ed25519 над
  исходными байтами тела, проверка SHA-256 до перемещений, staged-замена, откат с проверкой
  backup, запрет downgrade, защита от path traversal. 34 теста + 1 doctest. Сети нет.
- **Интеграция I1**: устранён O(n²) в `upsert_presence` (полный скан `state.presences`);
  red — 3.17 с на 10 000 upserts, green — 0.03 с (тест `r19`).

Ревью: независимый codex — 2 × P0 (side-channel S-box, path traversal + неподписанный
манифест) и 2 × P1 (откат на Windows, переполнение счётчика ChaCha20); Team Lead — 2 × P1
(повтор nonce на приёме, перемотка счётчика). Все закрыты red-first. Мутации **18/18**.
Workspace **785**, gate passed. Артефакты — `waves/RW-25/`.

## 4.1.2. RW-26 = P4-07 + P4-11 — принято 2026-10-22

Headless голосовой транспорт: compound-парсер RTCP (SR/RR/SDES/BYE/APP, SRTCP-трейлер
16/20), RFC 3550 loss/jitter по открытым RTP-заголовкам, `RttEstimator`/`StatsWindow`;
регионы — парсер `regions-rtc.json` (5×5 endpoint'ов), ранжирование измеренные→geo→лексика,
`plan_probes`. Ревью TL-1: P0/P1/P2 = 0 после red-first (timestamp wrap, `#[path]`-копия
теста, граница compound-лимита). Voice **114**, workspace 869; мутации **19/19**.
Артефакты — `waves/RW-26/`.

## 4.1.3. RW-27 = P7-01b + P6-04 + P6-03 — принято 2026-10-22

Headless i18n/platform: ListFormat на 31 локали (1674 кейса оракула ICU), deep links
`discord://` с RE-маршрутами и redaction токена в `Debug`, XDG autostart с Exec-quoting
и Unity badge. Ревью REV-2: 2×P2 (Exec reserved-символы, расхождение доказательной базы)
закрыты red-first/актуализацией; P0/P1/P2 = 0. Targeted 32, workspace **902**, npm gate 214;
мутации list 6/6 (+2 эквивалентных), platform **14/14**. Артефакты — `waves/RW-27/`.

## 4.2. RW-17 = P2-07 — принято 2026-10-20

Memory-only read-state: `MessageAck`, ленивое создание записи, non-channel/stale guard,
manual/non-manual mentions, глобальная version, селекторы mentions/бейджей/ack-точки.
40 тестов, core 418, workspace 460, gate passed. Ревью — два раунда: P2-1 (channel
fallback в `last_acked_id`) закрыт red-first; итог P0/P1/P2 = 0. Артефакты —
`waves/RW-17/`, ревью — `waves/reviews/review-p2-07.md`, уроки — `CODE-STANDARD.md` §9.

### RW-16 = P2-05 — принято 2026-10-20

Memory-only `GUILD_MEMBERS_CHUNK`: 43 теста, core 378, workspace 420, gate passed; ревью —
два раунда (P2-1 duplicate-index закрыт red-first), P0/P1/P2 = 0. Артефакты — `waves/RW-16/`,
ревью — `waves/reviews/review-p2-05.md`.

### RW-15 = P2-04 — принято 2026-10-20

Memory-only member list: 40 тестов, core 335, workspace 377, gate passed; ревью — два раунда,
P0/P1/P2 = 0. Артефакты — `waves/RW-15/`, ревью — `waves/reviews/review-p2-04.md`.

## 5. Следующие batch-волны

Детально фиксируются B01–B03; B04+ — rolling plan, чтобы не переписывать план после каждого
решения владельца. Полная матрица — `WAVE-PLAN.md`.

| Batch | Карточки | Содержание | Зависит от |
|---|---|---|---|
| **B01** | P2-10, P2-11 | voice states, typing TTL — **принято 2026-10-21** | P2-02/P2-04; P2-09 |
| **B02** | P2-08, P1-01, P1-02, P0-04, P0-07 | settings-proto, ETF full, zstd strict, conformance harness, legal — **принято 2026-10-21** | P0-02/P0-03 |
| **B03** | P3-15, P4-01, P7-04, P8-03 | i18n runtime, voice signaling headless, API, CI — **принято 2026-10-21** | P1-06/P1-07 |
| **B04** | P4-02, P8-04 | UDP media, reproducible SBOM/licenses — **принято 2026-10-21** | P4-01/P8-03 |
| **B05** | P7-01 | полный i18n: 36 локалей, CLDR plural, даты/время/числа, 23 markdown-правила — **принято 2026-10-21** | P3-15 |
| **B06** | P4-03, P7-02, P8-01 | SRTP-крипта, перф-харнесс, updater (headless, офлайн) — **принято 2026-10-22** | P4-02; P3-15; P7-01 |
| **RW-26** | P4-07, P4-11 | RTCP/RTP-статистика, регионы/latency (headless) — **принято 2026-10-22** | P4-02/P4-03 |
| **RW-27** | P7-01b, P6-04, P6-03 | ListFormat, deep links, autostart/badge (headless) — **принято 2026-10-22** | P7-01; P3-01 |
| **B07** | P0-01, P1-10, P2-03 | GUI spike, keyring, SQLite — **только после решения владельца** | зависимости/окно |
| **B08+** | P3/P4/P5/P6/P7/P8 | UI, voice/media, platform, perf, release | предыдущие batch |

B06, RW-26 и RW-27 (headless) приняты 2026-10-22; принято 39/90. Следующий batch
**B07** (GUI/keyring/SQLite) остаётся заблокированным до решения владельца.
Разблокированные P3/P7-остатки можно вести headless-дорожками без GUI.

Параллельные дорожки:

- **A. State/data** — P2-02 → P2-04 → P2-05/P2-07/P2-09/P2-10/P2-11/P2-08.
- **B. История/сеть** — P2-06 (память) после P2-02; P2-03 (SQLite) — после разрешения.
- **C. UI headless** — модели дерева каналов, выбора канала, composer state в `discord-ui`
  без оконного тулкита; i18n; тесты. Это можно делать до GUI.
- **D. Платформа** — P1-10 keyring и P2-03 SQLite отдельными решениями владельца.
- **E. Качество** — P3-7 backlog, покрытие chunked/TLS, strict-TE, doc-md5, тесты Host/TE.

## 6. Вехи

| Веха | Что готово | Критерий |
|---|---|---|
| M1. Headless core | P2-01 + P2-02 + P2-06 | READY/события → состояние → история; детерминизм на фикстурах |
| M2. Headless chat | P3-03 без окна + REST send/edit/delete | отправка/правка/удаление через loopback/стаб, без UI |
| M3. UI shell | P0-01/P3-01/P3-12 | окно, гильд-рейл, дерево каналов, чат-вью на фикстурах |
| M4. Media/voice | P3-04/P4/P4.5 | вложения, Opus/VP8, voice-статусы; crypto — по объёму |
| M5. Release | P5–P8 | платформа, настройки, безопасность, производительность, упаковка |

## 7. Решения владельца (до соответствующих волн)

1. **GUI:** разрешить загрузку `tauri`/`iced`/`egui` (выбрать один) **или** согласовать
   временный TUI/headless-путь. Без этого P0-01/P3-* заблокированы.
2. **SQLite:** разрешить `rusqlite`/`libsqlite3-sys` + системный SQLite. До этого P2-03/P2-06
   работают на памяти.
3. **keyring:** разрешить `keyring` + Secret Service/KWallet. До этого токен — только
   in-memory/явный конфиг, без хранения.
4. **Живой Discord:** окно для live-smoke. Без него — фикстуры и loopback-стабы.
5. **Сеть:** любые загрузки зависимостей — только после явного разрешения; тесты офлайн.

## 8. С чего начать прямо сейчас

1. Прочитать `STATUS.md`, `waves/STATE.md`, `TEAM.md`, `WAVE-PLAN.md`.
2. RW-26/RW-27 приняты; журнал/доки синхронизированы (39/90). Следующий batch — **B07**
   (GUI/SQLite/keyring) — после решения владельца; headless-остатки P4/P6/i18n можно
   брать отдельными волнами.
3. Batch собирать по 3–6 независимых карточек, не более 2 code workers, контракты
   заморозить до кода. B06 показал, что 3 независимых лейна с раздельными
   target-dir работают параллельно без гонок за файлы.
4. После green — QA/mutation, один batch gate, независимое ревью, accepted.
5. Один раз обновить `waves/db.jsonl`, `STATE.md`, `STATUS.md`, `ROADMAP.md`, `TASKS.md`, `map`.

Не начинать с UI: без reducer и истории UI будет рисовать заглушки. Не начинать с
SQLite/keyring/GUI до решения владельца по зависимостям.
