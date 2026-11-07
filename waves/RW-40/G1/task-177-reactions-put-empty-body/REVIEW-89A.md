# Техническое ревью 89A — PR-60 / task-177 (BE-P3-01)

**Вердикт:** `APPROVED` → **Ready for Merge**
**P0 = 0, P1 = 0, мутации 5/5 killed.**
**Ревьюер:** Technical Reviewer (DeepSeek), Review 89A
**Дата:** 2026-10-31
**Ветка:** `feature/task-177-reactions-put-empty-body` → `unstable`
**Worktree:** `/home/sanya/orca/workspaces/project/feature-task-177-reactions-put-empty-body`
**HEAD:** `38f7b5c` (Merge PR-57 into unstable, task-172)

---

## 1. Границы изменений (write-scope)

`git status --porcelain` до начала ревью:

```
 M package-lock.json
 M rust/gui-spike/src/reactions.rs
 M rust/gui-spike/tests/reactions_contract.rs
```

| Файл | В скоупе ТЗ | Действие ревьюера |
|---|---|---|
| `rust/gui-spike/src/reactions.rs` | да | проверен полностью |
| `rust/gui-spike/tests/reactions_contract.rs` | да | проверен полностью |
| `package-lock.json` (+6 строк: `bin`, `engines` в корневой пакет) | нет | **не трогал**: изменение появилось до старта задачи (синхронизация lock с `package.json`), к PR-60 отношения не имеет — исключить из коммита/мержа |
| `rust/gui-spike/Cargo.lock` | нет | **откатил**: перегенерация от моих `cargo test`/`clippy` (`ruzstd` → `zstd` в `discord-codec`, +`jobserver`). Дрейф pre-existing (в `Cargo.toml` уже `zstd`), это отдельная задача синхронизации lock |

После моих прогонов (Cargo.lock откатан) — изменены только два файла в скоупе.
`git commit` не выполнялся.

## 2. Сверка с оракулом `/home/sanya/re-deep/rest-d02.json`

Проверено напрямую по `endpoints[38]` (read-only):

```
method = PUT
path = /api/v9/channels/{id}/messages/{id}/reactions/%F0%9F%91%8B/%40me
statuses = [204]            statusCounts = {"204": 1}
reqBodyKeys = []            reqBodyRequests = 0            requestsWithContentType = 0
```

Совпадает с утверждениями REPORT.md. Doc-комментарий константы и тестов ссылается на тот же
endpoint; в тестах используется другой эмодзи (👍), но маршрут строится из данных, поэтому
расхождение эмодзи контракт не меняет.

**204 без `Content-Type`.** Проверено по коду клиента, не по догадке:
`rust/crates/discord-core/src/rest/client.rs:593`
`if !prepared.body.is_empty() && !has_header(&prepared.headers, "content-type")` — заголовок
добавляется только при непустом теле. Пустое тело `b""` через `ApiClient::put` даёт PUT без
`Content-Type` и с нулевым `Content-Length`. Файл вне write-scope, не менялся.

## 3. Прогоны

Команды из ТЗ, общий target-dir:

```
CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target cargo test \
  --manifest-path rust/gui-spike/Cargo.toml --test reactions_contract --lib
```

| Набор | Результат |
|---|---|
| `lib` | **17 passed, 0 failed** |
| `reactions_contract` | **16 passed, 0 failed** |
| `cargo clippy ... --test reactions_contract --lib -- -D warnings` | exit=0, **0 warnings** |
| `rustfmt --edition 2021 --check` (оба файла) | exit=0 |

Пять новых тестов действительно исполняются (видны в выводе поимённо), ни один не skip.

## 4. Мутационное тестирование (5/5 killed)

Каждая мутация вносилась в `reactions.rs`, прогонялся `--test reactions_contract`, файл
восстанавливался из бэкапа, итоговый md5 сверен после каждой серии. Все пять — `exit=101`,
без ошибок компиляции (то есть убиты тестами, а не «не собралось»).

| # | Мутация | Убийца | Результат |
|---|---|---|---|
| M1 | в `add_reaction_empty_body` отправляется `put(&path, b"{}")` вместо `put_empty_body` | `add_reaction_empty_body_sends_zero_byte_put_and_accepts_204`, `add_reaction_empty_body_encodes_custom_emoji_and_validates_before_transport` | killed (14 passed / 2 failed) |
| M2 | валидация `channel_id`/`message_id`/`emoji` обходится до `put_empty_body` (прямой `format!` + `encode_reaction_emoji(...).unwrap_or_else(невалидный → сырой emoji)`) | `add_reaction_empty_body_encodes_custom_emoji_and_validates_before_transport` (8 невалидных наборов ожидают `InvalidArg` и 0 вызовов транспорта) | killed (15/1) |
| M3 | legacy `add_reaction` шлёт `REACTION_PUT_EMPTY_BODY` вместо `b"{}"` | `legacy_add_reaction_still_sends_json_body_for_backward_compatibility` + `discord_reaction_api_transport_handles_204_no_content_and_put_delete_routing` + `idempotent_toggle_helper_adds_when_inactive_and_removes_when_active` | killed (13/3) |
| M4 | `encode_reaction_emoji` не делает percent-encoding (возвращает нормализованную строку) | `add_reaction_empty_body_encodes_custom_emoji_and_validates_before_transport`, `custom_and_unicode_emoji_encoding_and_route_construction`, `add_reaction_empty_body_sends_zero_byte_put_and_accepts_204`, `discord_reaction_api_transport_handles_204_no_content_and_put_delete_routing` | killed (12/4) |
| M5 | в `add_reaction_empty_body` собственный `decode_response`: `403 → NotFound`, `404 → Forbidden`, `429 → Client{429}` | `add_reaction_empty_body_handles_204_and_maps_429_403_404_401_5xx_network` | killed (15/1) |

M3 убит тремя тестами (новым регрессионным и двумя существующими) — обратная совместимость
защищена не только новым тестом, но и старым контрактом, который не ослаблялся.

Сырые логи: `mutations-89A.log` в этой же директории.

## 5. Разбор реализации

1. **`REACTION_PUT_EMPTY_BODY: &'static [u8] = b""`** — ноль байт, `is_empty() == true`;
   тип совпадает с параметром `ReactionTransport::put` (`&'a [u8]`), лишних аллокаций нет.
2. **`ReactionTransport::put_empty_body` — метод с дефолтной реализацией** (`self.put(target,
   REACTION_PUT_EMPTY_BODY)`). Трейт остаётся object-safe (возврат `Pin<Box<dyn Future + Send>>`,
   без generics — компилятор это подтверждает: `Arc<dyn ReactionTransport>` работает).
   Все существующие реализации (`ApiReactionTransport`, `DiscordReactionApi`) не правились —
   дефолт подхватывается, регрессии для них нет.
3. **`DiscordReactionApi::add_reaction_empty_body`** — сначала `build_reaction_path` (тот же
   контракт numeric-id + emoji-нормализация + percent-encoding), только потом транспорт;
   ответ разбирается общим `decode_response`, поэтому 204→`Ok`, 401/403/404/429/5xx/Network
   классифицируются ровно как у `add_reaction`. Дублирования логики нет.
4. **Обратная совместимость.** `toggle`/`add_reaction`/`remove_reaction`/`toggle_reaction_from_active`
   не тронуты: PUT по-прежнему с `b"{}"`, DELETE без тела. `ReactionApi` (trait для Tauri-слоя)
   не расширялся — `main.rs` в write-scope не входит, поведение UI не изменилось.
5. **Тесты** независимы от реализации: проверяют записанные вызовы транспорта (`method`, `target`,
   `body`), а не внутренние функции; ожидаемый путь зафиксирован строкой с оракульными
   percent-encoded сегментами (`%F0%9F%91%8D`, `%E2%9D%A4%EF%B8%8F`, `a%3Aparty_blob%3A...`).
6. Существующие тесты не ослаблялись и не переписывались: в диффе только добавление импорта
   константы и новых кейсов (219 добавленных строк, 1 изменённая — строка `use`).

## 6. Замечания

**P0 — нет. P1 — нет.**

P2 (не блокирует мерж, вынести в бэклог):
1. **Нет теста на отсутствие `Content-Type` на уровне `ApiClient`.** Утверждение «204 без
   Content-Type» подтверждено чтением `client.rs:593` и оракулом, но не тестом в
   `discord-core` (`rust/crates/discord-core/tests/`) — эти файлы вне write-scope задачи.
   Предложение: карточка QA/Backend на контрактный тест «пустое тело → нет `Content-Type`,
   `Content-Length: 0`» в `discord-core`.
2. **`add_reaction_empty_body` пока не вызывается из продакшн-кода.** UI/команда
   `toggle_reaction` остаётся на старом пути с `{}` (так и задумано по ТЗ: переключение —
   отдельное решение PM/UI). Метод публичный, поэтому `dead_code` не срабатывает, но
   ценность фичи появится только после подключения — нужна карточка на UI-интеграцию.
3. **Дрейф `rust/gui-spike/Cargo.lock`** (`ruzstd` → `zstd`, `+jobserver`): любой прогон
   `cargo test`/`clippy` по этому манифесту его перегенерирует. Нужна отдельная
   задача DevOps на синхронизацию lock, иначе он будет попадать в чужие диффы.
4. **`package-lock.json`** в рабочем дереве изменён до начала задачи (`bin`/`engines`) —
   перед мержем убедиться, что он не попал в коммит PR-60.

P3 (косметика): doc-комментарий константы ссылается на эмодзи 👋 из оракула, тесты — на 👍;
можно унифицировать, на контракт не влияет.

## 7. Контрольные суммы (на момент вердикта)

| Файл | md5 |
|---|---|
| `rust/gui-spike/src/reactions.rs` | `1cb0952e95d5c237add7b4b16332bcb2` |
| `rust/gui-spike/tests/reactions_contract.rs` | `d85a352be0332ee4a2f6a4d53c7b41a1` |

Совпадают с REPORT.md §5 — файлы после мутационных прогонов восстановлены побайтово.

## 8. Итог

`APPROVED (Ready for Merge)` — `P0=0`, `P1=0`, `5/5` мутаций убиты, `17/17 lib` +
`16/16 contract`, `clippy -D warnings` чисто, `rustfmt --check` чисто, границы файлов
соблюдены, оракул подтверждён, обратная совместимость защищена тестами.
`git commit` не выполнялся — коммит и мерж за координатором/интегратором.
