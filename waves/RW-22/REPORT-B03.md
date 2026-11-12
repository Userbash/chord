# RW-22 / B03 — отчёт batch

Дата: 2026-10-21. Batch: **B03**. Карточки: **P3-15** (i18n runtime),
**P4-01** (headless RTC signaling), **P7-04** (read-only API surface),
**P8-03** (CI/release). Foundation: `discord-json`.
Статус: **green, gate passed; независимые review P0/P1/P2=0; P3 — backlog**.
Изменения не закоммичены: коммит/push — координатор.

## 1. Сделано

- **Foundation `discord-json`**: строгий RFC 8259 parser/writer без внешних зависимостей и
  `unsafe`; raw-числа, `BTreeMap`, duplicate-key reject, Unicode/surrogate, depth/node limits,
  ошибки с offset без payload, writer round-trip. 11 тестов.
- **P3-15**: `xxh64` + `message_key` (6 base64-символов), 36/31/5 локалей, `resolve_locale`,
  AST-каталог, `I18n` с fallback, CLDR plural (`=N`, offset, `#`, ru/en, дроби → other),
  rich-теги `$b/$i/$del/$code/$p/$link`, безопасный markdown с балансировкой скобок,
  XSS-фильтр схем и 10k truncation. 8 тестов; реальные ru/en каталоги 23 836/23 903.
- **P4-01**: `VoiceOp` 0/1/2/3/4/6/8/15/16, `Hello`, `Ready` (streams/ssrc/rtx/rid/quality),
  `SessionDescription` (32-byte secret, DAVE/secure, codecs), `Identify`/`SelectProtocol`
  wire, base64, endpoint `wss://…/?v=9`, region ranking. 12 тестов (1 unit + 11 integration);
  реальные `rtc1/rtc2` кадры; секреты и `Other.raw` не попадают в `Debug`.
- **P7-04**: 8 подтверждённых D-02 GET-ресурсов, точные относительные пути, валидация
  application id (1..=20 ASCII-цифр), лишний id — ошибка, `SavedMessages` 404 сохранён.
  6 тестов; независимый oracle читает `docs/re/rest-d02.json` (path/method/statuses).
- **P8-03**: `scripts/ci.sh` (fmt → clippy → workspace test → ETF bench → optional
  cargo-deny → release), `scripts/release.sh` (offline build, `dist/discord-rs`,
  `SHA256SUMS`), workflow с двумя jobs, `deny.toml`, `etf_bench` example. 4 контрактных
  теста; реальный `scripts/ci.sh` — `ci: OK`, release-артефакт 445 040 B.

## 2. Red / green

| Фаза | Команда/лог | Результат |
|---|---|---|
| JSON red | `red-json.log`, `red-json-writer.log` | compile/behavior-red, exit 101 |
| P3-15 red | `red-w22a.log` | compile/behavior-red, exit 101 |
| P3-15 review fixes | `red-w22a-fixes.log` → `green-w22a-fixes.log` | plural/link/fallback закрыты |
| P4-01 red | `red-w22b.log` | compile/behavior-red, exit 101 |
| P4-01 review fixes | `red-w22b-review-fixes.log` → `green-w22b-review-fixes.log` | 3 red → green |
| P7-04 red | `red-w22c.log` | compile/behavior-red, exit 101 |
| P7-04 oracle | `green-w22c-oracle.log` | 6/6 |
| P8-03 contract | `targeted-b03-b.log` | 4/4 |
| targeted final | `targeted-b03-final-a.log`, `targeted-b03-final-b.log` | JSON 11, i18n 8, voice 12, REST 6, CI 4 |
| core | `test-core-b03-final.log` | **530 passed / 0 failed** |
| workspace | `test-workspace-b03-final.log` | **618 passed / 0 failed** |
| fmt/clippy | `clippy-b03-final.log`, `ci-actual.log` | чисто, `-D warnings` |
| CI actual | `ci-actual.log` | `ci: OK`; bench 3 170 793 ops/s; release + checksum |
| gate | `gate-worker-b03-final.log`, `gate-worker-b03.json` | **passed** (final 07:33 UTC) |

## 3. Мутации

**18/18 caught** — подробная таблица в `MUTATIONS-B03.md`, логи в `mutations/`.
Покрыты: JSON depth/duplicate/writer, i18n hash/plural/fallback/XSS, voice secret/tie-break/
Debug/endpoint/op8, REST id/404/extra-id, CI deny/release/checksum. Продукт восстановлен;
`m-voice-01b.log` — слабая первая версия теста, не считается (усилена до `m-voice-01`).

## 4. Проверка по RE-дампам

- `reference-oracle/ru-catalog.json` — **23 836** ключей; `hPBScv` = «Закрыть всплывающее
  окно»; `323362` содержит `$link`. `reference-oracle/en-catalog.json` — **23 903**.
- `reference-oracle/voice/rtc1-frames.jsonl` и `rtc2-frames.jsonl`: Hello v8/13 750 мс,
  Ready ssrc 7142/port 19 334/streams 7143/7144/rid 100/quality 100, Session Description
  mode `aead_aes256_gcm_rtpsize`, DAVE 1, secure frames 1, op15/16.
- `reference-oracle/rest-d02.json`: 8 путей, GET, статусы; `saved-messages` — 404.
- `rtc1-summary.json`/`rtc2-summary.json` подтверждают набор op и top-level `address`/
  `port`/`mode` в Select Protocol; сериализатор теперь повторяет эту форму.
- Ничего не выдумано: UDP/медиа/DAVE (P4-02/P4.5), query-параметры P7-04 и даты/время
  i18n (P7-01) остаются за границей карточки.

## 5. Файлы и md5

| Файл | md5 |
|---|---|
| `rust/crates/discord-json/src/lib.rs` | `be180b17d461616a7e1404714baf83bf` |
| `rust/crates/discord-json/tests/json_contract.rs` | `f595c41babf50f137db553c4ce71ede7` |
| `rust/crates/discord-i18n/src/lib.rs` | `9dda42f6bfb56b23d76c35460b70c21f` |
| `rust/crates/discord-voice/src/lib.rs` | `95fb2d0ac9ae6dd43d6e21d6c20b7b7b` |
| `rust/crates/discord-voice/tests/signaling.rs` | `413f0ce1a4b8ea2817590f69d72cee95` |
| `rust/crates/discord-core/src/rest/resources.rs` | `46f9202bfdcdf11b8f4dac5392ae7bb3` |
| `rust/crates/discord-core/tests/rest_resources.rs` | `e3de4f1d64c34d7a70bfa2f6a6fecd25` |
| `rust/crates/discord-core/Cargo.toml` | `a4e26163180ffbfdae7a17ebe7499175` |
| `rust/Cargo.toml` | `5ae43a65f92bd9f83a644d2dc49351ef` |
| `rust/Cargo.lock` | `edf3ebe0ed0cef5772d5dad992d45176` |
| `scripts/ci.sh` | `98eda13a7754dde7f40f3a2e8e3a8f4e` |
| `scripts/release.sh` | `4373eb1feaca0ff62ba7464de98fad13` |
| `.github/workflows/ci.yml` | `b8ca0eee07c6466cf65bcf00ac534531` |
| `rust/deny.toml` | `e8eee809d905d4af18cd8d4f6dd06a2e` |
| `rust/crates/discord-codec/examples/etf_bench.rs` | `6c8fc60cdfb35156df857c02388669fd` |
| `waves/RW-22/gate-worker-b03.json` | `21a97b187650c4956e2bacf9dd7358a4` |

## 6. Независимые review

- **Review A (i18n + JSON)**: P1 про панику `parse_fence` на `"```\n"` **не подтвердился**
  при репродукции (срез `4..` валиден; тест добавлен); P2 plural exact/дроби и P2
  link-delimiter подтверждены и закрыты red-first. JSON P0/P1/P2 не найдено.
- **Review B (voice/REST/CI)**: P1 `Other.raw` в `Debug` — закрыт; P1 endpoint с путём
  `/other` не воспроизвёлся (валидация уже отклоняла), но найден реальный дефект дубля
  `/?v=9` — закрыт; P2 отсутствие независимого REST-oracle — закрыт D-02 JSON-тестом.
- **Review C (round 2, read-only)**: «остаточные P0/P1/P2 не обнаружены»; подтверждены
  все семь исправлений.
- Авторы свой код не ревьюили; review — отдельные Codex CLI read-only агенты.

## 7. Риски / backlog (P3)

- P3-15: `$link` с URL из control/аргумента не типизирован (подтверждённого URL в capture
  нет) — сейчас рендерятся только children; даты/время/продвинутые number skeletons — P7-01.
- P4-01: UDP discovery/keepalive/ротация и DAVE/MLS — P4-02/P4.5; сеть в этой карточке
  отсутствует. `parse_identify`/`parse_select_protocol` проверяют capture-значения
  (max_dave=1, video=true, AV1); audio-only Select Protocol — уточнить в P4-02.
- P7-04: `ResourceQuery` не вводился: D-02 не подтверждает query у этих восьми маршрутов;
  пути относительные, без `/api/v9`, как в `routes::ROUTES` (уточнение к WAVE-PLAN §6).
- P8-03: `cargo-deny` локально skip, если инструмент не установлен (P8-04/SBOM); bench —
  smoke, порог регрессии 10 % остаётся P7-02/T-HARN-04.
- Реальные каталоги/кадры в тестах skip-aware: при отсутствии `reference-oracle/` они
  не падают, но и не доказывают RE-контракт; в этом прогоне артефакты были доступны.
- `VoicePayload::Other.raw` сохранён для диагностики; `Debug` его скрывает, но вызывающий
  код не должен логировать `raw` вручную.

## 8. Уроки метода

1. **Мутация должна проверять заявленный контракт, а не побочную ошибку.** `m-voice-01b`
   падал из-за отсутствующих DAVE-полей; после усиления payload мутация стала ловить
   именно длину secret key.
2. **Fallback-тест должен иметь разные каталоги.** Первая версия держала ключ в обоих
   locale, поэтому мутация fallback не ловилась; тест переписан на en-only ключ.
3. **Оракул — внешний JSON, а не копия констант.** P7-04 теперь читает D-02 JSON; voice
   wire проверяется полями JSON, а не только round-trip тем же парсером.
4. **Сначала воспроизведи finding.** P1 про fence и P1 про `/other` не подтвердились;
   вместо «исправления по отчёту» добавлены точные regression-кейсы и найден реальный
   дефект `/?v=9`.
5. **Лимиты и redaction — часть контракта.** JSON depth/nodes/duplicate и `Debug`
   неизвестного voice payload закрыты тестами и мутациями, а не комментариями.

## 9. Приёмка

B03 считается принятым после записи `accepted` в `waves/db.jsonl` и синхронизации
`STATE/STATUS/ROADMAP/TASKS/map`. Код, тесты и артефакты оставлены в рабочем дереве без
коммита; коммит/push выполняет координатор.
