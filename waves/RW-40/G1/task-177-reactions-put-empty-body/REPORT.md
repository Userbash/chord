# Отчёт воркера — task-177 (BE-P3-01): PUT реакции без тела, 204 без Content-Type, обратная совместимость

## 1. Batch / карточка

- **Волна**: `RW-40`, отдел `G1` (Backend).
- **Карточка**: `task-177` / `BE-P3-01` (PUT реакции без тела, `RE/rest-d02.json` `reqBodyKeys: []`).
- **Файлы воркера (единственные изменённые)**:
  - `rust/gui-spike/src/reactions.rs`
  - `rust/gui-spike/tests/reactions_contract.rs`
- **Ветка/worktree**: `feature-task-177-reactions-put-empty-body`, `CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target`.
- **Оракул** (`/home/sanya/re-deep/rest-d02.json`, `endpoints[38]`, read-only):
  - `method: "PUT"`, `path: "/api/v9/channels/{id}/messages/{id}/reactions/%F0%9F%91%8B/%40me"`;
  - `statuses: [204]`, `statusCounts: {"204": 1}`;
  - `reqBodyKeys: []`, `reqBodySample: null`, `reqBodyRequests: 0`;
  - `requestsWithContentType: 0` — Content-Type в живом запросе не отправлялся.
- **Существующий контракт, который нельзя ломать**: `reactions_contract.rs` проверяет
  `body == b"{}"` для `add_reaction`/`toggle_reaction` (строки 403, 415, 624 исходного файла).
  `RE/rest-d02.json` фиксирует живой клиент без тела — расхождение закрывается **новым**
  методом, а не изменением старого.

## 2. Что сделано

1. **`reactions.rs`**:
   - Добавлена константа `pub const REACTION_PUT_EMPTY_BODY: &[u8] = b"";` с ссылкой на оракул
     в doc-комментарии (`reqBodyKeys: []`, `reqBodyRequests: 0`, `requestsWithContentType: 0`, `204`).
   - В трейт `ReactionTransport` добавлен метод по умолчанию
     `fn put_empty_body<'a>(&'a self, target: &'a str) -> Pin<Box<dyn Future<...> + Send + 'a>>`,
     который делегирует в `self.put(target, REACTION_PUT_EMPTY_BODY)`. Реализации транспорта
     (включая `ApiReactionTransport` и `DiscordReactionApi`) не менялись — дефолт подхватывается
     автоматически, трейт остаётся object-safe.
   - Добавлен `pub async fn DiscordReactionApi::add_reaction_empty_body(&self, channel_id, message_id, emoji) -> Result<(), ReactionError>`:
     валидирует вход через `build_reaction_path` (тот же numeric-id/emoji-контракт), вызывает
     `self.transport.put_empty_body(&path)` и разбирает ответ через общий `decode_response`.
   - `add_reaction`/`remove_reaction`/`toggle` **не тронуты**: PUT по-прежнему шлёт `b"{}"`,
     DELETE — без тела.
2. **Отсутствие `Content-Type` на 204**: в `discord_core::rest::client::ApiClient::prepare`
   заголовок `Content-Type: application/json` добавляется только при непустом теле
   (`if !prepared.body.is_empty() ...`, `client.rs:593`). Пустое тело `b""` даёт PUT без
   Content-Type и `Content-Length: 0`; тест на уровне реакций фиксирует 0-байтовое тело,
   контракт клиента подтверждён чтением кода (файл вне write-scope, не менялся).
3. **`reactions_contract.rs`** — 5 новых тестов (+1 регрессионный), 16 всего:
   - `empty_body_constant_is_zero_bytes_and_default_transport_method_puts_it` — `REACTION_PUT_EMPTY_BODY == b""`, дефолтный `put_empty_body` на моке, реализующем только `put`, даёт PUT с 0 байт на `.../reactions/%F0%9F%91%8D/@me` и принимает 204.
   - `add_reaction_empty_body_sends_zero_byte_put_and_accepts_204` — 204 + `content-length: 0`, тело `is_empty()` и `!= b"{}"`.
   - `add_reaction_empty_body_encodes_custom_emoji_and_validates_before_transport` — кастомный `<a:name:id>` и `❤️` кодируются в путь, пустое тело; 8 невалидных наборов (id/emoji/слэши/control/битый custom) → `InvalidArg` до HTTP, счётчик вызовов 0.
   - `add_reaction_empty_body_handles_204_and_maps_429_403_404_401_5xx_network` — 204 → `Ok`, 429 JSON `1.5` → 1500 мс, 429 `Retry-After: 2.5` → 2500 мс, 403 → `Forbidden`, 404 → `NotFound`, 401 → `Unauthorized`, 503 → `Server { 503 }`, транспортная ошибка → `Network`.
   - `legacy_add_reaction_still_sends_json_body_for_backward_compatibility` — старый `add_reaction` шлёт ровно `b"{}"`.
4. **Совместимость**: все 16 контрактных тестов и 17 lib-тестов зелёные, существующие
   проверки `body == b"{}"` не менялись.

## 3. RED-прогон

`red.log` — до реализации (14 ошибок компиляции):
```
error[E0432]: unresolved import `discord_gui_spike::reactions::REACTION_PUT_EMPTY_BODY`
error[E0599]: no method named `add_reaction_empty_body` found for struct `DiscordReactionApi`
error[E0782]: expected a type, found a trait  // вызов дефолтного put_empty_body у trait-объекта
error: could not compile `discord-gui-spike` (test "reactions_contract") due to 14 previous errors
```

## 4. GREEN-прогон

`green.log` (команда из ТЗ):
```
CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target \
  cargo test --manifest-path rust/gui-spike/Cargo.toml --test reactions_contract --lib
lib:                 17 passed; 0 failed
reactions_contract:  16 passed; 0 failed
exit=0
```

`clippy.log`:
```
cargo clippy --manifest-path rust/gui-spike/Cargo.toml --test reactions_contract --lib -- -D warnings
Finished `dev` profile ... exit=0 (0 warnings)
rustfmt --edition 2021 --check rust/gui-spike/src/reactions.rs rust/gui-spike/tests/reactions_contract.rs
exit=0
```

## 5. Файлы + md5

| Файл | md5 |
|---|---|
| `rust/gui-spike/src/reactions.rs` | `1cb0952e95d5c237add7b4b16332bcb2` |
| `rust/gui-spike/tests/reactions_contract.rs` | `d85a352be0332ee4a2f6a4d53c7b41a1` |
| `waves/RW-40/G1/task-177-reactions-put-empty-body/red.log` | `ee97dc3b32886b20e2fee4bbaf7b53a7` |
| `waves/RW-40/G1/task-177-reactions-put-empty-body/green.log` | `e3548839e8300bbb51896795ed638f43` |
| `waves/RW-40/G1/task-177-reactions-put-empty-body/clippy.log` | `be0fdf0ac755aad88f04ac0ee664f98c` |

## 6. Риски и замечания

- **Обратная совместимость**: сохранена. `add_reaction_empty_body` — отдельный метод; UI/Tauri-команда
  `toggle_reaction` продолжает работать через старый путь с `{}`. Переключение UI на пустое тело —
  отдельное решение PM/UI (вне write-scope, `main.rs` не менялся).
- **`Content-Type` не проверен на живом запросе из нашего клиента**: тест фиксирует 0-байтовое тело,
  а отсутствие заголовка следует из `ApiClient::prepare` (`client.rs:593`) и оракула
  (`requestsWithContentType: 0`). Live-окно с реальным Discord не открывалось (не требуется ТЗ).
- **Дрейф `rust/gui-spike/Cargo.lock`**: `cargo test`/`clippy` по этому манифесту перегенерируют
  закоммиченный lock (`ruzstd` → `zstd` в `discord-codec`, +`jobserver`), потому что `Cargo.toml`
  уже содержит `zstd`. Я откатил lock (`git checkout -- rust/gui-spike/Cargo.lock`), чтобы не выходить
  за write-scope; Team Lead/DevOps стоит синхронизировать lock отдельной задачей.
- `package-lock.json` был изменён до начала работы (не мной), не трогал.
- `git commit` не выполнялся; ветка не мержилась.
