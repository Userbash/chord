# task-139 (BE-RE-03 / F-GW-02): исходящие опкоды 3/4/8/37

- **batch/карточка:** RW-40 / G1, task-139; находка F-GW-02, карточка бэклога BE-P1-01
  (`PM/BACKLOG-RE-BACKEND.md:35`). Файл — только `rust/crates/discord-core/src/gateway.rs`.
- **сделано:** в `gateway.rs` добавлены константы опкодов (`PRESENCE_UPDATE_OP=3`,
  `VOICE_STATE_UPDATE_OP=4`, `REQUEST_GUILD_MEMBERS_OP=8`, `GUILD_SUBSCRIPTIONS_BULK_OP=37`),
  типизированные тела `OutboundPresenceUpdate`, `OutboundVoiceStateUpdate`,
  `OutboundGuildMembersRequest`, `OutboundGuildSubscription`, ошибка `OutboundError` (без паник)
  и кодировщики `encode_presence_update`, `encode_voice_state_update`,
  `encode_request_guild_members`, `encode_guild_subscriptions`, плюс `chunk_user_ids` (границы
  100/101). Формы взяты дословно из клиента 1.0.162
  (`/home/sanya/re-deep/out/bundles/discord.com_assets_web.4d4b2f75fdea5470.js`):
  `presenceUpdate` @4 998 360, `voiceStateUpdate` там же, `requestGuildMembers` @4 998 360,
  `updateGuildSubscriptions` @4 998 666, `.chunk(100)` @4 954 008 (`GUILD_MEMBERS_REQUEST`),
  `SubscriptionManager.get` @9 112 505. Кодирование — wetf-раскладка клиента
  (`string:"binary", key:"binary", array:"list", null:"atom", undefined:"null"`): ключи и
  строки — `BINARY_EXT`, массивы — `LIST_EXT`, отсутствующие значения — `NIL_EXT`, булевы —
  атомы `true`/`false`.
- **red:** `CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target cargo test --offline
  --manifest-path rust/Cargo.toml -p discord-core` → exit **101**, `E0432`: девять имён
  (`Outbound*`, `encode_*`, `chunk_user_ids`) отсутствуют в `gateway`; лог —
  `RED.log`.
- **green:** та же команда → exit **0**, все тесты `discord-core` зелёные, суммарно
  **583 passed, 0 failed**; лог — `GREEN.log`.
- **targeted:** модуль `gateway::tests` — 7 тестов, из них 6 новых:
  `presence_update_frame_bytes_match_wetf_layout`,
  `presence_update_frame_passes_activity_terms_and_afk`,
  `voice_state_update_frame_matches_client_shape`,
  `guild_members_request_covers_query_and_user_ids_paths`,
  `user_ids_chunk_at_one_hundred`, `guild_subscriptions_frame_matches_client_shape`.
  Плюс `cargo clippy --offline -p discord-core --all-targets -- -D warnings` → exit 0
  (`CLIPPY.log`) и `cargo fmt --check` → exit 0 (`FMT.log`).
- **oracle:** исходящие кадры в захвате маскированы — `gateway-protocol.json`
  `opcodes.sentGateway` прямо говорит `decodable:false` (sha256+len), а `capture-run2.jsonl`
  хранит только длину и хеш. Поэтому оракул двухуровневый: (1) исходный код клиента — точные
  литералы тел и порядок ключей; (2) независимая байтовая раскладка ETF в тесте op3: полный
  hex-кадр `83 74…` собран вручную по тегам (`0x6d` binary, `0x61` small int, `0x6c` list,
  `0x6a` nil, `0x64` atom), а не снят с текущей реализации. `gateway-schema.md` подтверждает
  словарь значений (атомы `online`/`idle`, `true`/`false`) и отсутствие `nonce` в наблюдавшихся
  кадрах.
- **генерация:** не применимо — артефакты не генерируются, функции детерминированы.
- **мутации:** не запускались: в ТЗ task-139 мутационный прогон не входил, его делает отдельный
  QA-агент на batch.
- **baseline/restore:** baseline до правки — `discord-core` зелёный (прогон перед red-фазой).
  Для фиксации red-кода реализация временно снята (baseline + новые тесты), получен exit 101;
  тогдашние байты восстановлены точно (md5 `b51f27b2…` до и после). После уточнения doc-комментария
  про `nonce` финальные байты — `958a3708…`, на них пересняты green/clippy/fmt.
  `waves/window-runs.jsonl`, куда `spike-rust` дописывает запись пробы окна, возвращён к HEAD.
  `package-lock.json` был изменён до старта задачи (виден в `git status` на входе) — не трогал.
- **стабильность:** не требовалось — чистые функции без гонок, ресурсов и времени.
- **gate:worker:** запущен (один раз, `GATE-WORKER.log`, отчёт
  `.gate/worker-2026-10-30T16-55-51-106Z.json`). Rust-фазы зелёные: `spike-rust` 53.8 с,
  `workspace-rust` 1124 теста (1112 pass, 12 ожидаемых ignored, 0 fail), `unit` 702/702,
  `smoke` 21 pass + 1 skip. Вердикт `failed` из-за двух дефектов, которые уже есть в HEAD и к
  моему файлу отношения не имеют: `typecheck` падает на `tests/unit/dispatch_ui.test.mjs`
  (TS7006/TS2339, файл не изменён), `meta` — `plan-sync` требует строк §8 для 10 тестовых
  файлов (`tests/e2e/m2-*.e2e.test.mjs`, `tests/meta/e2e-required.test.mjs`,
  `tests/unit/dispatch_ui.test.mjs`, `tests/unit/search-remote-real-index.test.mjs`).
- **файлы:** `rust/crates/discord-core/src/gateway.rs` —
  `958a3708bc00f4caf6126686e7d91f4f`; отчёт `REPORT.md`; логи `RED.log`, `GREEN.log`,
  `CLIPPY.log`, `FMT.log`, `GATE-WORKER.log`.
- **риски/вопросы:**
  1. Условные поля op4 `preferred_region(s)` и `tracks[{type,rid,quality}]` не реализованы:
     клиент шлёт их только при наличии, тип `quality` в захвате не подтверждён, стриминга у
     продукта нет. Форма зафиксирована в комментарии к `OutboundVoiceStateUpdate`.
  2. `nonce` в op8 — требование ТЗ; веб-клиент его не отправляет, эхо в `GUILD_MEMBERS_CHUNK`
     в захвате не наблюдалось. Поле необязательное: `None` → `null`.
  3. Объект подписки op37 собран по `SubscriptionManager.get`: `members` — ключи LRU,
     `member_updates` — bool; `thread_member_lists` не включён (нет в списке задачи,
     RE-P1-01a остаётся открытой). Чанкование op37 по 15 360 Б не реализовано — в задаче
     только карта подписок; лимит задокументирован в бэклоге.
  4. Отправки в сокет нет: `ws.rs`, `supervisor.rs` и `tests/gateway_opcodes_out.rs` вне
     разрешённого файла. Билдеры готовы к подключению в следующей карточке.
  5. Падающие `typecheck` и `plan-sync` — отдельный дефект HEAD, нужен свой пункт бэклога;
     без него `gate:worker` не станет зелёным ни у одного воркера.
  6. Новые имена доступны как `discord_core::gateway::*`; в корневой re-export `lib.rs` они не
     добавлены — файл общий, правит Team Lead. Если нужен `pub use`, это одна строка в его
     integration-патче.
- **следующий шаг:** приёмка Team Lead; интеграция отправки (READY/смена статуса → op3,
  join/leave voice → op4, запрос участников → op8 с чанками, открытие канала → op37) —
  следующим ТЗ с правом на `ws.rs`/`supervisor.rs`.
