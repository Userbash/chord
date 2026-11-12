# B2a-ATTACH-BACKEND — отчёт воркера

- **batch/карточка:** RW-40 / B2a-ATTACH-BACKEND, отдел G1 Rust Backend (единственный владелец cargo).
- **сделано:** новый модуль `rust/gui-spike/src/attachments.rs` — слоты загрузки вложений
  (`POST /channels/{id}/attachments`), контрактный тест `rust/gui-spike/tests/attachments_contract.rs`,
  экспорт `pub mod attachments;` в `rust/gui-spike/src/lib.rs`. Чужие файлы и `ui/**` не тронуты.

## Что реализовано

| Элемент | Контракт (RE) |
|---|---|
| `AttachmentFileSpec { filename, file_size, id, is_clip }` | reference-oracle-REA §1.3 `getUploadPayload` |
| `AttachmentUploadSlot { id, upload_url, upload_filename }` | §1.2 ответ `attachments[0]` |
| `UploadedAttachment { id, filename, uploaded_filename }` | §1.7 `uploaded_filename` в сообщении |
| `request_upload_slots(&transport, channel_id, files)` + `DiscordAttachments::request_upload_slots` | §1.1–§1.2 |
| `build_message_attachments(slots, specs)` — пары по локальному `id`, порядок `specs` | §1.2, §1.7 |
| `MAX_UPLOAD_FILE_BYTES = 10 * 1024 * 1024` | §1.5 (`0xa00000`) |
| `MAX_ATTACHMENTS_PER_MESSAGE = 10` | §1.5 (`n4=10`) |

Валидация до I/O: allowlist `channel_id` (ASCII-цифры/буквы, `_`, `-`, 1..=64 байта — отсекает
`../..`, слэш, `%`, `?`, `#`, CR/LF), непустой список файлов, непустые `filename`/`id`, ненулевой
`file_size`, отсутствие дубликатов `id`, не больше 10 файлов, размер ≤ 10 МиБ. Тело — ровно
`{"files":[{"filename","file_size","id","is_clip"}]}` без лишних ключей. Ответ разбирается в три
поля; пустой/битый `attachments[]` — `InvalidResponse`. `429` берёт `retry-after` из заголовка
(секунды, дробные тоже) и отдаёт `RateLimited { retry_after_ms }`; `413` и `400` с `body.code=40005`
сводятся в `EntityTooLarge` (§1.6); прочие статусы — через общий `rest_failure::RestFailure`.
PUT на `upload_url`, докачка `308` и превью в объём карточки не входят (B2b/следующий шаг).

## red

```
cargo test --offline --test attachments_contract   (заглушка с сигнатурами)
→ test result: FAILED. 1 passed; 14 failed; finished in 0.00s
→ код возврата cargo 101
```

Лог: `waves/RW-40/PM/b2a-attach/red.log`. Прошёл только `oracle_constants_match_re`
(сверка констант с RE, поведения не касается).

## green

```
cargo test --offline --test attachments_contract        → ok. 15 passed; 0 failed  (exit 0)
cargo test --offline --lib attachments                  → ok. 2 passed; 0 failed   (exit 0)
cargo fmt --all -- --check                              → exit 0
cargo clippy --offline --all-targets -- -D warnings     → exit 0
```

Лог: `waves/RW-40/PM/b2a-attach/green.log` (снят после восстановления мутаций, на финальных байтах).

## targeted

`tests/attachments_contract.rs` — 15 тестов; плюс 2 unit-теста в модуле (`retry_after`,
`entity_too_large_code`). Каждое утверждение контракта привязано к тесту:

| Test ID | Утверждение |
|---|---|
| `oracle_constants_match_re` | 10 МиБ, 10 вложений, шаблон пути, ключи `files[]`/`attachments[]` |
| `rejects_unsafe_channel_id_before_any_io` | `../..`, пустой id, слэши, `%`, CR/LF, >64 байт — отказ без вызова транспорта |
| `channel_id_allowlist_matches_messages_module` | тот же вердикт, что у `messages::send_message` |
| `rejects_empty_or_broken_file_lists_before_io` | пустой список, пустые `filename`/`id`, `file_size=0`, дубликаты, >10 файлов |
| `rejects_oversized_file_locally_with_entity_too_large` | `10 МиБ + 1` → `EntityTooLarge` без сети, ровно `10 МиБ` — валидно |
| `posts_exact_upload_slot_request` | target `/channels/{id}/attachments`, тело ровно из 4 полей |
| `parses_slots_and_keeps_only_oracle_fields` | разбор ответа, лишние поля игнорируются |
| `rejects_malformed_upload_slot_response` | пустое тело, не JSON, `[]`, нет `attachments`, пустой `upload_url`/`upload_filename` |
| `maps_429_with_retry_after_header` | `retry-after: 3` → 3000 мс, `Retry-After: 1.5` → 1500 мс, без заголовка → 0 |
| `maps_413_and_40005_to_entity_too_large` | `413` и `400 {code:40005}` → `EntityTooLarge` |
| `maps_other_transport_and_http_failures` | 401/403/404/400/500/502 и обрыв транспорта |
| `builds_message_attachments_in_spec_order` | пары по `id`, порядок `specs`, `uploaded_filename` |
| `builder_rejects_mismatched_slots_and_specs` | нет/лишний/повторный слот, пустой `upload_url` |
| `real_client_requires_token_and_sends_nothing` | пустой токен → `Unauthorized` |
| `api_impl_matches_free_function` | метод и свободная функция дают один результат |

## oracle

- `waves/RW-40/reference-oracle-REA.md` §1.1–§1.7 (строки дампа `4d2a64cb5df5fc4e_0@269215`, `@2573535`,
  `@2572444`, `@2572800`, `@322893`, `@2361900`, `@667084`);
- `waves/RW-40/ATTACH-RE.md` §2 (сводная таблица, те же факты + лимиты tier NONE/T1 = 10 МиБ);
- независимый контроль allowlist — публичный `messages::send_message` (parity-тест), а не копия
  текущей реализации.

## генерация

Не применимо: генераторов/фикстур карточка не добавляет, вход — константы RE и фейковый транспорт.

## мутации

6/6 KILLED, все — на зелёном baseline (exit 0) и с падением ожидаемого теста (exit 101):

| Мутант | Убит тестом |
|---|---|
| M1 allowlist пропускает точку (`a.b`) | `channel_id_allowlist_matches_messages_module` |
| M2 `>=` вместо `>` на границе 10 МиБ | `rejects_oversized_file_locally_with_entity_too_large` |
| M3 `429` без `retry-after` | `maps_429_with_retry_after_header` |
| M4 ключ тела `file_size` → `size` | `posts_exact_upload_slot_request` |
| M5 `40005` не маппится в `EntityTooLarge` | `maps_413_and_40005_to_entity_too_large` |
| M6 пары слот/файл по индексу | `builds_message_attachments_in_spec_order`, `builder_rejects_mismatched_slots_and_specs` |

Лог: `waves/RW-40/PM/b2a-attach/mutations.log` (md5 мутанта, падающие тесты, exit, restore-md5).

## baseline/restore

- baseline: `src/attachments.rs` = `c2e1062287d334a397901a7ebe90b3a3`,
  `tests/attachments_contract.rs` = `cdc8cbd9dd91323ad97a84ef4acfa962`, targeted зелёный, exit 0.
- после каждого мутанта — `restore md5 = c2e1062287d334a397901a7ebe90b3a3` (совпал), тест-файл
  не менялся; финальное состояние совпадает с baseline по обоим файлам.
- отдельно: `copy2` возвращает старый mtime, поэтому после restore файл дополнительно
  `touch`-ится — иначе cargo переиспользует сборку мутанта (поймано один раз, в логе отражено).

## gate:worker

Воркер targeted-гейт выполнил: `attachments_contract` + unit-тесты модуля, `cargo fmt --all -- --check`,
`cargo clippy --offline --all-targets -- -D warnings` — всё зелёное. Полный batch-гейт
(`cargo test --workspace`, `npm run gate:worker`) — за Integration/QA, воркер его не запускает.

## файлы

| Файл | md5 |
|---|---|
| `rust/gui-spike/src/attachments.rs` (новый) | `c2e1062287d334a397901a7ebe90b3a3` |
| `rust/gui-spike/src/lib.rs` (одна строка экспорта) | `f8b5af6132cba13213d3a79031b8f976` |
| `rust/gui-spike/tests/attachments_contract.rs` (новый) | `cdc8cbd9dd91323ad97a84ef4acfa962` |
| `waves/RW-40/PM/b2a-attach/red.log` | снят до реализации |
| `waves/RW-40/PM/b2a-attach/green.log` | снят после restore |
| `waves/RW-40/PM/b2a-attach/mutations.log` | 6 KILLED, restore подтверждён md5 |

`git status` по моей области: только `M rust/gui-spike/src/lib.rs`, `?? rust/gui-spike/src/attachments.rs`,
`?? rust/gui-spike/tests/attachments_contract.rs`. `git commit/push` не выполнялся.

## риски/вопросы

1. **`messages::validate_channel_id` приватна.** ТЗ требовало валидировать «через
   `messages::validate_channel_id`», но функция в `messages.rs` объявлена без `pub` (её добавила
   PLAT-11-B), а `messages.rs` не входит в мои файлы. По решению Team Lead allowlist продублирован
   в `attachments.rs` и связан parity-тестом с публичным `messages::send_message`. Если валидатор
   станет `pub(crate)`, дубль можно убрать одной правкой — контракт теста не изменится.
2. **PUT файла не реализован** — это следующий шаг (B2b): `upload_url` отдаётся наружу как есть,
   `308`/`Content-Range` не трогаются.
3. **`apiRetryPolicy::default()`** вместо 12 попыток из §1.2: политику повторов держит ядро,
   локально её не дублирую; при необходимости отдельная карточка на конфиг ретраев upload.
4. **Лимит 10 вложений** добавлен из §1.5 (`n4=10`), хотя ТЗ перечисляло только размер файла.
   Если продуктовый лимит другой — это одна константа и один тест.
5. **`40005` в теле `400`** трактуется как `EntityTooLarge` (§1.6). Если сервер начнёт отдавать
   этот код на нерелевантные ошибки, ветка станет слишком широкой — тогда сузить по `message`.

## следующий шаг

Передать Team Lead на приёмку B2a; после приёмки batch — Integration/QA на полный гейт, затем
B2b (PUT `upload_url`, докачка, прогресс, drag&drop в UI).
