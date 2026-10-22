# Состояние Rust-клиента (факт на 2026-10-22)

Источник плана — `PLAN.md` (90 карточек). Статус карточки — только после независимого ревью,
зелёных тестов, гейта и записи `accepted` в `waves/db.jsonl`; «код написан» ≠ «карточка принята».

## 1. Счётчик плана

| Фаза | Принято / всего | Осталось |
|---|---|---|
| P0 — фундамент | 4 / 7 | 3 |
| P1 — транспорт и идентификация | 12 / 12 | 0 |
| P2 — данные и состояние | 11 / 11 | 0 |
| P3 — UI-каркас и чат | 1 / 16 | 15 |
| P4 — медиа и вложения | 7 / 12 | 5 |
| P4.5 — голос | 0 / 3 | 3 |
| P5 — платформа | 0 / 7 | 7 |
| P6 — настройки/безопасность | 5 / 10 | 5 |
| P7 — производительность | 3 / 7 | 4 |
| P8 — выпуск | 3 / 5 | 2 |
| **Итого** | **46 / 90** | **44** |

Принятые карточки: `P0-02`, `P0-03`, `P0-04`, `P0-07`, `P1-01`, `P1-02`, `P1-03`…`P1-09`, `P1-10`, `P1-11`, `P1-12`, `P2-01`, `P2-02`, `P2-04`, `P2-05`, `P2-06`, `P2-07`, `P2-08`, `P2-09`, `P2-10`, `P2-11`, `P3-15`, `P4-01`, `P4-02`, `P4-03`, `P4-04`, `P4-07`, `P4-11`, `P4-12`, `P2-03`, `P6-02`, `P6-03`, `P6-04`, `P6-06`, `P6-07`, `P7-01`, `P7-02`, `P7-04`, `P8-01`, `P8-03`, `P8-04`; хвосты P7-01b (ListFormat), P7-01c (RelativeTimeFormat), P7-01d (DurationFormat), P7-01e (`<t:…>`).

## 2. Что уже работает

- **ETF-кодек** (`discord-codec`): 11 тегов Discord, big-int со знаком до цифр, fuzz-lite,
  round-trip байт-в-байт; P1-01 — checked-энкодер, границы `u16/u32`/`MAX_DEPTH=64` без
  усечения, 50k fuzz + truncation.
- **zstd-stream** (`ruzstd 0.9.1`): кадры 0/2+, окно 64 КиБ, реальный поток 137 кадров;
  P1-02 — терминальные ошибки (`Poisoned`/`is_dead`), корректный `is_finished`, контракт
  удержания окна 65 536 Б.
- **Шлюз** (`discord-core`): HELLO, heartbeat 41 250 мс, ACK/RTT, zombie, reconnect,
  IDENTIFY, RESUME (op6/op7/op9), 21 тип диспатчей, WS-транспорт `ws://`/`wss://`.
- **REST v9**: 47 маршрутов, 6 auth-заголовков, 429/5xx/backoff, телеметрия-off,
  HTTP/1.1-фрейминг, TCP/TLS, X-Super-Properties (22 поля), ack/read-state.
- **READY P2-01** (`ready/`): типизированная модель A1 — 40 top-level ключей, 7 гильдий,
  272 канала, 132 роли, 363 эмодзи, 51 стикер, 17 ЛС, 146 users, 60 relationships,
  read_state 157/128949, UGS 58/734, 1328 overrides, 1 сессия, merged 7×1,
  крупная гильдия `member_count=24228`; диагностика `malformed`/`ignored`/`duplicates`,
  атом-строки, сырой индекс `merged_members`, redaction `Debug`.
- **Состояние P2-02** (`state/`): чистый reducer `(State, Event) -> (State, Effects)`,
  READY/SUPPLEMENTAL/RESUMED, `SESSIONS_REPLACE`, `THREAD_LIST_SYNC`, `CHANNEL_INFO`,
  `MESSAGE_CREATE`/`MESSAGE_UPDATE`; `BTreeMap`-индексы, stale-политика, детерминированные
  эффекты `SubscribeMembers`, селекторы и redaction. Без I/O, SQLite, UI и сети.
- **История P2-06** (`history/`): memory-only bounded history по каналам, `latest`/`before`/
  `after`/`around`, generation-токены и stale-защита, `Live > Page`, дедуп/last-wins,
  tri-state `has_older`/`has_newer`, селекторы. Без REST-JSON, SQLite, сети и UI.
- **Участники P2-04** (`state/member_list.rs`): memory-only `GUILD_MEMBER_LIST_UPDATE`,
  raw `MemberListId`, плоский `group|member`, `SYNC`/`INSERT`/`UPDATE`/`DELETE`, сдвиги и
  reset, snapshot groups/counts, duplicate/out-of-range/unknown-op диагностика, минимальный
  presence, селекторы. Wire op37/op14 не подтверждён capture — остаётся
  `Effect::SubscribeMembers`; chunks — P2-05 (принято).
- **Чанки P2-05** (`state/member_chunk.rs`): memory-only `GUILD_MEMBERS_CHUNK` с
  `guild_id`/`chunk_index`/`chunk_count`/`members`/`not_found`, optional `nonce`; сборка по
  `(guild, nonce)`, first-wins count, duplicate last-wins, out-of-order, `is_complete`,
  аддитивный merge `Guild.members`/`State.users`, `not_found` без удаления; селекторы и
  декларация `Effect::RequestPresence` для видимого диапазона. `presences[]` внутри chunk
  не разбирается до P2-09.
- **Read-state P2-07** (`state/read_state*`): memory-only `MESSAGE_ACK` с обязательными
  `channel_id`/`message_id` и optional `flags`/`last_viewed`/`mention_count`/`manual`/
  `version`; ленивое создание записи, non-channel/stale guard, manual/non-manual mentions,
  глобальная версия без max-guard, равенство message-id как норма; селекторы mentions,
  guild/private, badge, ack-точки и `is_message_read`. Channel использует только
  `last_message_id`, non-channel — `last_acked_id`; без unread-count, REST ACK, UI и сети.
- **Presence P2-09** (`state/presence*`): memory-only `PRESENCE_UPDATE`, `READY_SUPPLEMENTAL`,
  member-list и best-effort `GUILD_MEMBERS_CHUNK.presences[]`; scopes `Guild(u64)`/`Private`,
  выбор лучшего видимого online/idle/dnd, offline+hidden, activities и диагностика. 54 теста
  (18+18+18) + unit; мутации 13/13; **принят 2026-10-21**, review round 2: P0/P1/P2=0, P3=8
  backlog.
- **Voice P2-10** (`state/voice/*`): memory-only `READY_SUPPLEMENTAL` snapshot (аддитивный),
  `VOICE_STATE_UPDATE` join/move/leave last-wins, `(guild,user)` store, timer/status
  `(guild,channel)`, member upsert только при совпадении id, stale/duplicates диагностика.
  18 тестов; 18/18 B01-мутаций; **принят 2026-10-21**, review round 2: P0/P1/P2=0, P3=11.
- **Typing P2-11** (`state/typing/*`): memory-only `TYPING_START` TTL `10_000` мс, повтор
  перезапускает TTL, `MESSAGE_CREATE` автора снимает запись, READY сбрасывает, время только
  из `received_at_ms`/`now_ms`. 15 тестов; 8/8 typing-мутаций; **принят 2026-10-21**,
  review: P0/P1/P2=0, P3=10.
- **Settings P2-08** (`settings/`): protobuf/base64 structural decoder, type 1
  `PreloadedUserSettings` (versions/inbox/appearance/voice/text/notifications/status/
  localization), type 2 `FrecencyUserSettings` structural, unknown preservation, лимиты,
  redacted Debug. 18 тестов; **принят 2026-10-21**.
- **JSON foundation** (`discord-json`): строгий RFC 8259 parser/writer без внешних
  зависимостей, duplicate-key reject, Unicode/surrogate, depth/node limits, raw-числа,
  ошибки с offset без payload; 11 тестов на реальных каталогах/кадрах.
- **I18N P3-15/P7-01** (`discord-i18n`): `xxh64`/6-символьный `message_key`,
  36/31/5 локалей, AST-каталог, fallback current → default → en-US, CLDR cardinal/ordinal
  (`=N` до offset, `#`, дроби, 56 значений на локаль), числа/валюты/проценты, 4 стиля
  даты/времени UTC, `th` +543, localized names/timezone, skeleton, 23 markdown-правила
  с безопасными URL и bounded depth; реальные ru/en каталоги 23 836/23 903;
  **33 теста** (8 lib + 25 новых); **принят 2026-10-21**.
- **Voice signaling P4-01** (`discord-voice`): op0/1/2/3/4/6/8/15/16, Hello/Ready/
  Session Description/Identify/SelectProtocol, endpoint `wss://…/?v=9`, region ranking,
  redaction секретов; реальные `rtc1/rtc2`; 12 тестов; **принят 2026-10-21**.
- **UDP media P4-02** (`discord-voice`): discovery 74 Б, keepalive 8 Б/5 с с LE-счётчиком,
  rotation 60 080 мс, 8 Б RTCP empty RR + 28 Б SRTCP wrapper, loopback socket, `encode`
  fallible, peer/SSRC validation, `rotate_if_due`; 15 тестов; **принят 2026-10-21**.
  Без live/DNS/TLS/DAVE; SRTCP-крипто — P4-03/P4.5.
- **SBOM P8-04** (`scripts/sbom.mjs`): offline SPDX 2.3 на 85 `Cargo.lock`-пакетов,
  host metadata + version-pinned platform fallback, strict AND/OR/WITH, invalid-token reject,
  CDLA allowlist, LLVM exception только `WITH`; 5 тестов; CI mandatory hook, release
  `-ef` + SHA256SUMS; **принят 2026-10-21**.
- **Read-only API P7-04** (`rest::resources`): 8 подтверждённых D-02 GET-ресурсов, точные
  относительные пути, валидация application id, сохранённый 404 saved messages; 6 тестов,
  независимый oracle `docs/re/rest-d02.json`; **принят 2026-10-21**.
- **CI P8-03**: `scripts/ci.sh` (fmt/clippy/test/bench/optional deny/release),
  `scripts/release.sh` (`dist/discord-rs` + SHA256SUMS), workflow, `deny.toml`, ETF bench;
  4 контрактных теста, реальный `ci: OK`; **принят 2026-10-21**.
- **Conformance P0-04** (`discord-conformance`): repo→lab загрузчик фикстур, env-overrides,
  типизированные ошибки, `RE_REQUIRE_LAB=1`; 6 тестов.
- **SRTP-крипта P4-03** (`discord-voice::crypto`, `discord-voice::srtp`): AES-256, AES-256-GCM,
  ChaCha20-Poly1305, XChaCha20-Poly1305, HKDF-SHA1 — **без внешних зависимостей**; S-box
  считается алгебраически (`affine(x^254)`), без таблиц — табличный вариант давал side-channel
  через кэш. Раскладка кадра `[clear][ciphertext][tag 16][counter 4 LE]` подтверждена capture'ом
  (`docs/re/voice-deep.md` §2.3–2.5, `waves/RW-25/research/RE-SRTP-LAYOUT.md`) и функциями
  самого клиента (`PacketCrypter::HeaderLength` / `SpaceRequiredToEncrypt`,
  `waves/RW-25/research/RE-NATIVE-SRTP.md`): исходящий счётчик `0x80000000 | n`, общий для RTP и
  SRTCP; AAD = открытая часть в режимах `*_rtpsize`. 38 тестов, KAT по NIST/RFC.
  **Совместимость с wire Discord не заявлена** — ключевого материала в capture нет.
- **Перф-харнесс P7-02** (`discord-perf`): детерминированная нагрузка (SplitMix64) — 24 229
  участников + 100 000 сообщений, замеры на реальном редьюсере, гейт регрессии >10 % (T-HARN-04).
  Холодный старт на референсной машине ~47 мс при бюджете 1500 мс; 17 тестов.
- **Свой updater P8-01** (`discord-updater`): манифест релиза, подпись Ed25519 (ring) над
  исходными байтами тела, проверка SHA-256 **до** перемещений, staged-замена, откат с
  проверкой целостности backup, запрет downgrade, защита от path traversal; 34 теста + 1 doctest.
  Сети в карточке нет — только проверка и план локальной замены.
- **LEGAL P0-07**: правовая рамка ToS/telemetry/off/keyring/ассеты/live approval; политика
  отделена от фактически реализованного.
- **I1 REST-фасад**: auth + транспорт + 429/5xx + телеметрия, 16 кейсов; принят.
- Legacy `ready_data` удалён из сборки и публичного API (P-4).

## 3. Тесты и гейты

| Проверка | Результат |
|---|---|
| `cargo test --offline --workspace` | **1067 passed, 0 failed** |
| `cargo test --offline -p discord-core` | **531 passed, 0 failed** |
| RW-35 `discord-platform` | **110 passed, 0 failed** (+ fd 6, power +4) |
| RW-34 `discord-platform` | **100 passed, 0 failed** (+ abstract 15, hints 25) |
| RW-33 `discord-platform` | **86 passed, 0 failed** (+ keyring 12) |
| RW-32 `discord-platform` | **74 passed, 0 failed** (dbus 8, notifications 18, power 12, spellcheck 13) |
| RW-31 `discord-platform` | **53 passed, 0 failed** (41 + power 12) |
| RW-31 `discord-store` | **14 passed, 0 failed** (SQLite-store P2-03) |
| RW-30 `discord-platform` | **41 passed, 0 failed** (1 lib + 9 + dbus 7 + 13 + notifications 11) |
| RW-30 `discord-i18n` | duration **13**, 0 failed (P7-01d) |
| RW-29 `discord-voice` | media **13**, reconnect **10 + 2 unit**, 0 failed (P4-04/P4-12) |
| RW-28/29 `discord-i18n` | **70 passed, 0 failed** (relative 12 + timestamp 14 + прежние 44) |
| RW-28 мутации | **8/8 caught**, продукт восстановлен по md5 |
| RW-29…RW-35 мутации | W29a 8/8, W29b 9/9, W29c 6/6, W30a 8/8, W30b 10/10, W31a 7/7, W31b 7/7, W32a 8/8, W32b 6/6, W33a 7/7, W34a 6/6, W34b 6/6, W35a 6/6 (SKIP=0) |
| RW-35 гейт | passed, `waves/RW-35/gate-cargo.log` / `.gate/worker-2026-10-22T19-41-26-571Z.json` |
| RW-26 `discord-voice` | **114 passed, 0 failed** (P4-07 статистика 38 + P4-11 регионы 10 + прежние) |
| RW-27 `discord-platform` | **23 passed, 0 failed** (1 lib + deep links 13 + autostart 9) |
| RW-27 `discord-i18n` | **10 passed, 0 failed** (`list_contract`, 1674 кейса) |
| B06 `discord-voice` | **65 passed, 0 failed** (SRTP-крипта P4-03: 16 KAT + 22 контрактных + 27 прежних) |
| B06 `discord-perf` | **17 passed, 0 failed** (перф-харнесс P7-02) |
| B06 `discord-updater` | **34 passed, 0 failed** + 1 doctest (updater P8-01) |
| B06 мутации | **18 / 18 поймано, 0 пропущено**, продукт восстановлен по md5 |
| B05 `discord-i18n` | **33 passed, 0 failed** (8 lib + locale 2 + plural 2 + format 3 + i18n 6 + markdown 12) |
| B04 UDP / SBOM | **15 / 5 passed, 0 failed** |
| B03 `discord-json` | **11 passed, 0 failed** (parser/writer, real catalogs/frames, truncation) |
| B03 `discord-i18n` | **8 passed, 0 failed** (hash/locale/AST/plural/markdown/real catalogs) |
| B03 `discord-voice` | **12 passed, 0 failed** (1 unit + 11 signaling) |
| B03 `discord-core` `rest_resources` | **6 passed, 0 failed** (D-02 oracle) |
| B03 `discord-conformance` `ci_contract` | **4 passed, 0 failed** |
| P2-08 settings tests | **18 passed, 0 failed** (`settings_proto`) |
| P1-01/P1-02 codec tests | **43 passed, 0 failed** (`discord-codec`) |
| P0-04 conformance | **6 passed, 0 failed** (`RE_REQUIRE_LAB=1`) |
| P2-10 voice tests | **18 passed, 0 failed** (types 6, reducer 8, selectors 4) |
| P2-11 typing tests | **15 passed, 0 failed** (types 5, reducer 5, selectors 5) |
| P2-09 presence tests | **54 passed, 0 failed** (types 18, reducer 18, selectors 18; +1 unit) |
| P2-07 read-state tests | **40 passed, 0 failed** (types 12, reducer 17, selectors 11) |
| P2-05 chunk tests | **43 passed, 0 failed** (types 19, reducer 13, selectors 11) |
| P2-04 member list tests | **40 passed, 0 failed** (types 14, reducer 17, selectors 9) |
| P2-06 history tests | **20 passed, 0 failed** (types 7, pagination 5, selectors 8) |
| P2-06 red | compile-red W14a/b + behavior-red W14c + root/counts, все `EXIT=101` |
| P2-02 state tests | **42 passed, 0 failed** (types 15, reducer 19, selectors 8) |
| P2-01 `ready_review` | 7/7 green; red-final 0/7, `# EXIT=101` |
| `cargo fmt --all -- --check` | чисто |
| `cargo clippy --offline --workspace --all-targets -- -D warnings` | чисто |
| `npm run gate:worker` | passed, `waves/RW-29/gate-worker.log` / `.gate/worker-2026-10-22T17-12-46-556Z.json` (RW-28/29; B05 — `waves/RW-24/…`, B04 — `waves/RW-23/…`, B03 — `waves/RW-22/…`) |
| `waves-sync` | 3/3 (в составе gate meta) |

## 4. Крейты: что реализовано

| Крейт | Состояние |
|---|---|
| `discord-codec` | рабочий: ETF + zstd |
| `discord-core` | рабочий, частично: gateway/rest/ready/state/history/dispatch/resume/resources |
| `discord-json` | рабочий foundation: строгий parser/writer |
| `discord-i18n` | рабочий: P3-15 runtime + P7-01 locale/CLDR/числа/даты/markdown + ListFormat/RelativeTimeFormat/DurationFormat/`<t:…>` (36 локалей, 31 включена) |
| `discord-voice` | рабочий, частично: signaling + UDP discovery/keepalive/RR + SRTP-крипта P4-03 + RTP PT120/DTX/джиттер P4-04 + reconnect P4-12; DAVE/MLS — P4.5 |
| `discord-conformance` | рабочий тестовый помощник |
| `discord-perf` | рабочий: перф-харнесс T-HARN-04 (P7-02) |
| `discord-updater` | рабочий: манифест/подпись/план обновления (P8-01) |
| `discord-app` | каркас (точка входа, GUI нет) |
| `discord-ui` | каркас (контракты, окна нет) |
| `discord-store` | рабочий: SQLite через FFI (P2-03), миграции/CRUD/kv |
| `discord-media` | каркас (кодеки/пайплайн не реализованы) |
| `discord-platform` | частично: deep links, autostart, D-Bus (уведомления/power/abstract/fd), spellcheck, keyring (трей/PTT нет) |

Итого: **7 рабочих/частично рабочих крейтов + 1 тестовый + 5 каркасов**. Это не
«приложение готово», а работающее ядро протокола, данных, сигнализации, крипто-контура,
перф-гейта и контура обновления.

## 5. Чего ещё нет

- B02 закрыт 2026-10-21: settings-proto (`P2-08`), ETF full (`P1-01`), zstd strict
  (`P1-02`), conformance skeleton (`P0-04`), legal (`P0-07`).
- B03 закрыт 2026-10-21: i18n runtime (`P3-15`), voice signaling (`P4-01`), read-only API
  (`P7-04`), CI/release (`P8-03`), foundation `discord-json`.
- B04 закрыт 2026-10-21: UDP media (`P4-02`), reproducible SBOM/licenses (`P8-04`).
- B05 принят 2026-10-21: полный i18n (`P7-01`) — 36 локалей/CLDR cardinal+ordinal,
  числа/валюты, даты/время UTC, 23 markdown-правила, fallback и AST-интеграция.
- B06 принят 2026-10-22: SRTP-крипта (`P4-03`), перф-харнесс (`P7-02`), updater (`P8-01`) —
  первая **headless**-волна: офлайн, без новых внешних зависимостей (кроме офлайн-кэша).
  Следующий batch — **B07** (GUI/SQLite/keyring) заблокирован зависимостями.
- SQLite (`P2-03`) и keyring (`P1-10`) — нет зависимостей в офлайн-кэше;
- UI-окна, чата, composer, компонентов (`P0-01`, `P3-*`) — GUI-крейтов нет;
- DAVE/MLS (`P45-01`…`P45-03`), платформенные интеграции (P5/P6), упаковка (`P8-02`);
- совместимость SRTP с wire Discord не заявлена (в capture нет ключевого материала) — свой
  формат самосогласован, раскладка байт подтверждена;
- в updater нет сети: загрузка артефакта — следующий подэтап P8-01.

Порядок и batch-план — `WAVE-PLAN.md`; роли и WIP — `TEAM.md`.

## 6. Блокеры владельца

| Блокер | Что нужно |
|---|---|
| GUI: `tauri`/`iced`/`egui`/`winit`/`wry`/`tao` отсутствуют в `Cargo.lock`/кэше | разрешение на загрузку конкретного набора **или** временный headless/TUI-путь |
| `keyring` (P1-10) | разрешение на зависимость + Secret Service/KWallet |
| `rusqlite`/`libsqlite3-sys` (P2-03) | разрешение на зависимость + системный SQLite |
| Живой Discord (PID 95828) | окно владельца для live-smoke; без него — только фикстуры/loopback |

Статус процесса Discord на 2026-10-22: прежний PID 95828 уже не существует, запущен
**1.0.165** (PID 16632). RE-разбор велся **статически** по установленному модулю
`discord_voice.node` (без attach к живому процессу) — см.
`../../waves/RW-25/research/RE-NATIVE-SRTP.md`.

## 7. Граф проекта

`../../.ua/knowledge-graph.json` — снимок Understand-Anything до появления `state/`:
97 файлов, **232 узла / 432 ребра**, 8 слоёв, 12-шаговый tour. Он **устарел** относительно
P2-02…P2-11, B03, B04 и B05; полная пересборка требует LLM-анализа (`autoUpdate=false`) и
недоступна в текущем offline-прогоне. Актуальная карта верхнего уровня и поток команд/данных —
`../map.md`; журнал — `../../waves/STATE.md`.

## 8. Доказательства

- B06: `../../waves/RW-25/` (WAVE-PLAN, TZ-W26a/b/c, REVIEW-CHECKLIST, REPORT-B06,
  TEST-MATRIX-B06, MUTATIONS-B06, research/RE-SRTP-LAYOUT + RE-NATIVE-SRTP + llm-review,
  red/green по лейнам и по фиксам ревью, codex-review, `gate-worker-b06.log`,
  `mutations/`); мутации **18/18 caught, 0 missed**; независимое ревью codex — 4 находки
  (2 × P0, 2 × P1) + 2 находки Team Lead (nonce reuse, перемотка счётчика) — все закрыты
  red-first.
- B05: `../../waves/RW-24/` (WAVE-PLAN, TZ-W25a/b, REPORT-B05, REVIEW-RESOLUTION-B05,
  TEST-MATRIX-B05, MUTATIONS-B05, MUTATION-REPORT-B05, red/green, review A/B, `gate-worker-b05-final.log`);
  мутации **20/20 caught, 0 missed**.
- B03: `../../waves/RW-22/` (WAVE-PLAN, DESIGN-FOUNDATION, REPORT-B03, TEST-MATRIX-B03,
  MUTATIONS-B03, red/green, `gate-worker-b03.json`, `review-A/B/C`); 18/18 мутаций.
- B02: `../../waves/RW-21/` (DESIGN, REPORT-B02, TEST-MATRIX-B02, red/green,
  `gate-worker-b02.json`, mutations, reviews).
- B01: `../../waves/B01/` (INTEGRATION, MUTATIONS, gate-worker), `../../waves/RW-19/`
  (P2-10: DESIGN/TZ/REPORT/TEST-CORRECTIONS, red/green, mutation addendum),
  `../../waves/RW-20/` (P2-11), ревью — `review-p2-10-round2.md`, `review-p2-11.md`.
- P2-09: `../../waves/RW-18/` (RESEARCH/DESIGN/TZ/REPORT/TEST-CORRECTIONS, red/green,
  mutation 13/13, gate; review round 1 — `../../waves/reviews/review-p2-09.md`; round 2 —
  `../../waves/reviews/review-p2-09-round2.md`).
- P2-06: `../../waves/RW-14/` (DESIGN/TZ/RESEARCH/TEST-CORRECTIONS, red/green/gate),
  ревью — `../../waves/reviews/review-p2-06.md`.
- P2-07: `../../waves/RW-17/` (RESEARCH/DESIGN/TZ/TEST-CORRECTIONS, red/green/gate,
  mutation 7/7, corrections P2-1), ревью — `../../waves/reviews/review-p2-07.md`.
- P2-05: `../../waves/RW-16/` (RESEARCH/DESIGN/TZ/TEST-CORRECTIONS, red/green/gate,
  mutation 3/3), ревью — `../../waves/reviews/review-p2-05.md`.
- P2-04: `../../waves/RW-15/` (RESEARCH/DESIGN/TZ/TEST-CORRECTIONS, red/green/gate),
  ревью — `../../waves/reviews/review-p2-04.md`.
- P2-02: `../../waves/RW-13/` (DESIGN/TZ/RESEARCH/TEST-CORRECTIONS, red/green/gate),
  ревью — `../../waves/reviews/review-p2-02.md`.
- P2-01: `../../waves/RW-12/reports/w12a.md`, `w12b.md`, `w12c.md`,
  `p2-01-review-fixes.md`; ревью — `../../waves/reviews/review-p2-01-round3.md`.
- RW-10/RW-11: `../../waves/reviews/review-rw-10-11-round2.md`.
- I1: `../../waves/reviews/review-i1.md`.
- Гейт B02: `../../waves/RW-21/gate-worker-b02.json`; предыдущий B01 — `../../waves/B01/gate-worker.json`.
- Журнал: `../../waves/db.jsonl`.
