# RW-40 / SEC-02 — keyring-ошибка не маскируется под `Network` (G1 Backend)

- **batch/карточка:** RW-40 / SEC-02 (`QUALITY/PLATFORM-AUDIT.md` §3, P1).
- **роль:** воркер G1 Backend; PM — DeepSeek.
- **границы:** только файлы ТЗ. Остальной UI/src, golden (`logs/`, `functions/`,
  `evidence/`), общие доки и `waves/*` (кроме этого каталога) не тронуты.
  Окно/live/сеть/спавн Discord/коммиты не выполнялись.
- **HEAD при работе:** `0239543` (RW-35 принят). `rust/gui-spike/` — untracked
  в git (новый проект), поэтому границы сверялись по списку файлов и md5.

## Сделано

1. **Типизированный `Storage` в доменных enum'ах** (`serde(tag = "kind",
   rename_all = "camelCase")` → `"storage"`, аддитивно): `MessageError`,
   `ReactionError`, `SearchError`, `EnterGuildError`, `LeaveGuildError`.
   `Display` — «хранилище токена недоступно»; причина keyring (`AuthError::Keyring
   { message }`) в доменную ошибку **не копируется**.
2. **`token_or` (`main.rs`)** — параметр `unavailable` переименован в `storage`;
   keyring-ошибка и незавершившийся `spawn_blocking` → `Storage` у всех 8 сетевых
   команд; «токена нет» → прежние `Unauthorized` (messages/reactions) и
   `MissingToken` (search/guild). Классификатор `RestFailure` **не** переиспользует
   `Storage`: 401/403/404/429/4xx/5xx/Network остаются своими кодами.
3. **`From<EnterGuildError> for LeaveGuildError`** — добавлено единственное
   необходимое плечо `Storage → Storage` (исчерпывающий match).
4. **UI `ui/features/search-overlay.js`**: `"storage"` в `SEARCH_ERROR_KINDS`,
   текст «хранилище токена недоступно»; `isSearchError` теперь истинен для
   `storage`, поэтому локальный fallback для него выключен.
5. **Тесты (red-first):**
   - `src/main.rs::wiring_tests::token_or_preserves_domain_error_codes` — оракул по
     serde-тегу `kind` (не по имени варианта): fake keyring → `storage` во всех
     5 доменах, отсутствие текста keyring в `Display`/`Debug`, no token → прежние
     варианты, OK-path без изменений, `Network` остаётся `network`.
   - `tests/{messages,reactions}_contract.rs` — `Storage` проходит через
     action-обёртки, сериализуется как `kind: "storage"`.
   - `tests/search_contract.rs` — новый `storage_failure_is_typed_for_ui`
     (kind + отсутствие причины), `Storage` в наборе «ошибки не раскрывают токен».
   - `tests/guilds_contract.rs` — `(EnterGuildError::Storage, "storage")`,
     `(LeaveGuildError::Storage, "storage")` в таблицах camelCase-kind.
   - `tests/unit/gui-spike-features.test.mjs` — аддитивный тест SEC-02: `{kind:
     "storage"}` → `data-state="error"`, текст «хранилище токена недоступно»,
     **не** «сеть недоступна», 0 результатов, `matchesCount = 0`, `mode = "remote"`.

## Red (до реализации)

| # | команда | результат | лог |
|---|---|---|---|
| 1 | `cargo test --offline --manifest-path rust/gui-spike/Cargo.toml` | exit 101: `E0599 no variant ... Storage` в 5 контрактных крейтах + bin (API ещё нет) | `01-red-api.log` |
| 2 | `node --test tests/unit/gui-spike-features.test.mjs` | exit 1: `SEC-02: ... storage не показан ошибкой` (`'ok' !== 'error'`) — неизвестный kind уходил в локальный fallback | `02-red-ui.log` |
| 3 | `cargo test ...` после добавления только вариантов enum/Display (без проводки) | exit 101: `token_or_preserves_domain_error_codes` — `left: "network"`, `right: "storage"` (`main.rs:737`) | `03-red-behavior.log` |

## Green (после реализации)

| проверка | результат | лог |
|---|---|---|
| `cargo test --offline --manifest-path rust/gui-spike/Cargo.toml` | **191 passed**, 0 failed (было 190, +1 тест) | `04-green-cargo.log` |
| `cargo test --offline -p discord-core` (cwd `rust/`) | **574 passed**, 0 failed | `06-core.log` |
| `node --test <6 файлов из ТЗ>` | **245 passed**, 0 failed (было 244; +1 SEC-02) | `05-green-ui.log` |
| `cargo clippy --offline --manifest-path rust/gui-spike/Cargo.toml --all-targets -- -D warnings` | exit 0, без предупреждений | `07-clippy.log` |
| `cargo fmt --manifest-path rust/gui-spike/Cargo.toml -- --check` | exit 0 | `08-fmt.log` |
| `npx tsc -p rust/gui-spike/ui/tsconfig.json` | exit 0 | `09-tsc.log` |

Замечание по счётчикам: в ТЗ указано «228 frontend / 190 gui / 574 core». По факту
указанная команда по 6 файлам даёт **245** (набор шире, чем 228; +1 от SEC-02),
gui — **191** (было 190 ровно, +1), core — **574** (совпадает). Расхождение 228
не связано с этой карточкой — стоит сверить методику подсчёта у PM.

## Оракул

- Rust: serde-тег `kind` сериализованной ошибки (контракт с фронтендом),
  независимый от имени Rust-варианта; `Display`/`Debug` проверяются на отсутствие
  строки keyring-ошибки (`Secret Service`) и токена.
- UI: DOM-шим (`loadApp`), observable-контракт `.search-status[data-state]`,
  тексты, отсутствие `.search-result`, `api.search.matchesCount/mode` —
  fallback доказывается наблюдаемым состоянием, а не текстом кода.
- Non-regression: прежние таблицы kind'ов (401/403/404/429/4xx/5xx/network) не
  менялись; `Network` в классификаторе по-прежнему `network`
  (`error_kind(&EnterGuildError::Network) == "network"`).

## Файлы + md5

```
6ea7941df82e0b4bab7a2fa6b7a2e26d  rust/gui-spike/src/main.rs
d0556da9f3203879831a29d9dd6f7a6a  rust/gui-spike/src/messages.rs
f6473a4ef7fd4dbdfc020dbdd83e361a  rust/gui-spike/src/reactions.rs
ef35f8b8cf3c8b4f8b5d5d1bd524cf26  rust/gui-spike/src/search.rs
45cc55f727863bdf8780b044d3bc881f  rust/gui-spike/src/guilds.rs
1ee81cd20629e00ef3928a0fcdbf786f  rust/gui-spike/tests/messages_contract.rs
c86ce15d7cb232ddb6071cd1b420fd06  rust/gui-spike/tests/reactions_contract.rs
ffedbf800bd96f15bad04700885b9062  rust/gui-spike/tests/search_contract.rs
e79c84bbf53fe919da4428c94c5eb66b  rust/gui-spike/tests/guilds_contract.rs
1b71bcc0ae3f2e2074e6d04794d7fac9  rust/gui-spike/ui/features/search-overlay.js
9908772ddde16ef772af86211ecfe467  tests/unit/gui-spike-features.test.mjs
```

## Остаток / риски

1. **Другие домены UI показывают storage как сеть.** `ui/features/chat.js:437-438`,
   `reactions.js:121-123`, `actions.js:168-193` маппят неизвестный `kind` в «сеть
   недоступна». Это READ-ONLY UI вне файлов карточки, поэтому не менялось; по ТЗ
   «остальные домены могут показывать общий message», но фактический текст —
   сетевой и может вводить в заблуждение. Предложение PM: отдельная карточка на
   `storage`-текст в chat/reactions/join-leave (или общий маппер `kind → текст`).
2. **Blocking join error** (паника/незавершение `spawn_blocking`) тоже отдаёт
   `Storage` — по ТЗ; семантика «не удалось получить токен» шире, чем keyring.
   Текст ошибки хранилища наружу не идёт, диагностика ограничена видом `storage`.
3. **Причина не логируется** (по ТЗ и SEC-02 «без секрета»): после типизации
   `Storage` единственный диагностический след keyring-сбоя исчезает из IPC.
   Если нужен счётчик/категория отказа (audit предлагал вариантом), это отдельное
   решение PM — в этой карточке намеренно не добавлено.
4. Публичные enum'ы расширены аддитивно (`Storage`); существующие потребители
   (`From<RestFailure>`, `From<EnterGuildError> for LeaveGuildError`) обновлены,
   ломающих изменений нет. Требование п.5 (вернуть дизайн-предложение) не
   потребовалось.
5. `rust/gui-spike/**` не в git — коммит/границы фиксируются координатором (L1);
   md5 выше снят с финальных байтов после всех правок и прогонов.
