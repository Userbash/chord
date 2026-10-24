# REPORT W39e2 — 4×P1 Codex в BFF (`data.rs`)

Владелец: W39e2 (Backend, продолжение W39e). PM: DeepSeek. Итерация 2.
ТЗ: `waves/RW-39/PM/BRIEF-W39e2.md`. Основание: `waves/RW-39/PM/w39e/REVIEW-codex.md` (FAIL, P0=0, P1=4).

**Статус: DONE.** Все 4 P1 закрыты red→green, существующие 17 тестов не тронуты,
clippy `-D warnings` и `fmt --check` — чисто. Окно/probe/gate/сеть/commit не запускались.

## Изменённые файлы

| файл | md5 |
|---|---|
| `rust/gui-spike/src/data.rs` | `58051842200ce0eb483e6f8698744dbf` |
| `rust/gui-spike/tests/data_contract.rs` | `c55681f298856a35eb860fa1baa9aeb0` |

`Cargo.toml`, `ui/**`, `fixtures/**`, `ipc_contract.rs`, `launch.rs`, `lib.rs` — не менялись.

## R1 — лимит размера фикстуры (P1 data.rs:231)

Что сделано: `MAX_FIXTURE_BYTES = 64 * 1024 * 1024` (u64, pub). В `read_json`
перед чтением берётся `fs::metadata(path)?.len()`; `> MAX_FIXTURE_BYTES` → `DataError::Parse`
с путём и лимитом, файл в память не читается. `NotFound`/`Io` по-прежнему через `map_io`.

Тест: `oversized_fixture_is_rejected_before_reading` — валидный READY + разреженный
(`File::set_len`) `001-nil.json` на `64 MiB + 1` во **временном** каталоге теста
(`fixtures/` не трогается). Проверяется `kind == "Parse"`, путь в сообщении и число лимита.

Red (`cargo test --test data_contract`, лог `red-data_contract.log`):
```
---- oversized_fixture_is_rejected_before_reading stdout ----
panicked at tests/data_contract.rs:563:
лимит в ошибке: Parse (.../001-nil.json): некорректный JSON: expected value at line 1 column 1
```
То есть до правки файл читался целиком (64 MiB нулей) и падал на JSON, а не на лимите.

Green: тест проходит, в сообщении `фикстура больше лимита 67108864 байт (67108865 байт): чтение отклонено`.

## R2 — лимит входа id/cursor (P1 data.rs:461, 638, 822)

Что сделано: `MAX_INPUT_LEN = 256` (pub), `ERROR_VALUE_PREVIEW = 32`. Хелперы:
`preview()` — усечение значения до 32 символов + `…`; `ensure_input_len()` — `InvalidArg`
при длине > 256 символов, в тексте только усечённое значение и фактическая длина.
Применено к `channelId` (`messages_page`), `guildId` (`members_page`), `cursor`
(`cursor_start`, до поиска по выборке). Сообщение о «чужом» курсоре тоже печатает
`preview(cursor)`, чтобы длина ошибки не зависела от входа. Пустые строки — прежний
`InvalidArg`; ровно 256 символов — допустимо (граница включительно).

Тест: `oversized_ids_and_cursor_are_invalid_arg_with_truncated_message` — id/cursor по
10 000 символов на реальных фикстурах: `InvalidArg`, вся строка в сообщение не попадает,
есть `…`, длина сообщения `< 256`; плюс граница: id из 256 символов → `NotFound`, не `InvalidArg`.

Red:
```
---- oversized_ids_and_cursor_are_invalid_arg_with_truncated_message stdout ----
panicked at tests/data_contract.rs:580:
assertion `left == right` failed: NotFound [канал]: канал 7777…(10 000 символов)… не найден
  left: "NotFound"   right: "InvalidArg"
```
(10 000 символов целиком уходили в текст `NotFound`.)

Green: `InvalidArg: channelId длиннее 256 символов: 77777777777777777777777777777777… (всего 10000)`.

## R3 — строгая валидация кадра by-type (P1 data.rs:258)

Что сделано: в `read_frames` кадр обязан иметь непустой строковый `t` и присутствующий `d`.
`null` в `d` отвергается: по всем `fixtures/gateway/by-type/*.json` (199 файлов) `d`
всегда присутствует, nil-кадры несут `"d": "nil"`, `"d": null` не встречается ни разу —
обоснование записано в doc-комментарии `read_frames`. Ошибка — `Parse` с путём и
индексом кадра (`кадр by-type #N …`), вместо прежнего `unwrap_or_default()`/`Value::Null`.

Тест: `frame_without_type_or_data_is_parse_error` — 5 кейсов в отдельных временных корнях:
`{}`, `{"t":""}`, `{"t":"MESSAGE_CREATE"}` (нет `d`), `{"t":42,"d":{}}`, `{"t":"MESSAGE_CREATE","d":null}`.

Red:
```
---- frame_without_type_or_data_is_parse_error stdout ----
panicked at tests/data_contract.rs:638:
empty-object: кадр без валидных t/d принят, строк: 0
```

Green: все 5 → `Parse` (`кадр by-type #0: поле t должно быть непустой строкой`,
`кадр by-type #0 (MESSAGE_CREATE): нет поля d`, `… поле d = null`), путь `001-broken.json` в тексте.

## R4 — битый item `GUILD_MEMBER_LIST_UPDATE` (P1 data.rs:747)

Что сделано: `collect_members` теперь `Result<Vec<MemberRow>, DataError>`. Элемент
`ops[].items[]` — либо группа (`{"group": …}`, валидный не-участник, пропускается),
либо участник (`{"member": …}`). Элемент без `member`/`group` и участник без
`member.user.id` → `Parse` с путём и индексом `ops[i].items[j]`. Тихих `continue` на
повреждённых данных больше нет.

Тест: `member_item_without_user_id_is_parse_error` — `items: [{"group":{"id":"online"}}, {"member":{"user":{}}}]`
→ `Parse`, в сообщении `items[1]` и имя файла; контрольная часть того же теста: снимок
только из групп → `Ok`, `total == 0` (группы не считаются участниками и не ломают страницу).

Red:
```
---- member_item_without_user_id_is_parse_error stdout ----
panicked at tests/data_contract.rs:677:
битый item принят молча, участников: 0
```

Green: `Parse (…/001-member-list.json): GUILD_MEMBER_LIST_UPDATE: элемент ops[0].items[1] без member.user.id`.

## Тесты

- `tests/data_contract.rs`: **было 17 → стало 22** (+4 по R1–R4, +1 guard `public_limits_match_specification`:
  `MAX_FIXTURE_BYTES == 64 MiB`, `MAX_INPUT_LEN == 256` — ловит дрейф констант, сами R1/R2
  сверяются с литералами спецификации, а не с константами кода).
- Весь крейт: было 38 (17 data_contract + 20 ipc_contract + 1 launch) → стало **43** (22 + 20 + 1), doc-tests 0.
- Существующие 17 тестов не менялись; `DL_STRICT`-логика и skip-aware макрос не тронуты.

Логи (`waves/RW-39/PM/w39e2/`):
- `red-data_contract.log` — `17 passed; 4 failed` (ровно 4 новых теста, все падения по существу);
- `green-data_contract.log` — `22 passed; 0 failed`;
- `green-full-test.log` — `22 / 20 / 1` passed, 0 failed;
- `green-clippy.log` — `cargo clippy --all-targets -- -D warnings`, exit 0, без замечаний;
- `green-fmt.log` — `cargo fmt -- --check`, `fmt-exit=0` (перед этим rustfmt применён к двум
  разрешённым файлам; чужие файлы крейта не переформатировались).

## Что осталось / вне scope

- `settings(section?)`: строка `section` не ограничена по длине — в брифе R2 перечислены
  только `channel_id`/`guild_id`/`cursor`, поэтому не трогал. Кандидат в отдельную карточку
  (тот же `NotFound` с полной строкой в тексте).
- `ready()`/`settings()` читают `002-READY.json` напрямую (не через `read_frames`), поэтому
  R3 их не касается; лимит размера (R1) на них действует через общий `read_json`.
- Политика `d: null` = `Parse` опирается на текущий дамп; при импорте новых дампов с
  `"d": null` решение нужно пересмотреть (место помечено в doc-комментарии `read_frames`).
- `cargo test`/`clippy`/`fmt` — только для `rust/gui-spike`; полный `gate:worker`, окно,
  probe и commit — за координатором.
