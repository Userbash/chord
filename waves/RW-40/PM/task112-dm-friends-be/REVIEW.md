# DeepSeek Review 40A — task-112 (N-107-DM-FRIENDS-BACKEND)

**Вердикт: RETURN (P2 ×2; P0 — 0, P1 — 0).**
Функциональная часть соответствует заявленному контракту: фикстуры, маршруты, тела запросов,
`429 retry_after_ms` и защита токена подтверждены независимым прогоном. Возврат — из-за
evidence-дефекта RED-лога и потери Discord-кода ошибки на HTTP 401/403/404.

Ветка: `feature/task-112-dm-friends-be`, HEAD `4e24048`. Изменения не закоммичены
(`dm_friends.rs` и `dm_friends_contract.rs` — untracked, `src/lib.rs` — modified).

## 1. MD5-аудит против REPORT.md §6

| Файл | В отчёте | Фактически | Итог |
|---|---|---|---|
| `rust/gui-spike/src/dm_friends.rs` | `d1820fe304816de3e4def848d992d3b9` | `d1820fe304816de3e4def848d992d3b9` | совпадает |
| `rust/gui-spike/tests/dm_friends_contract.rs` | `05fd4df3ba4d0145f3705f1177750a81` | `05fd4df3ba4d0145f3705f1177750a81` | совпадает |
| `waves/.../red.log` | `9e2c6644d794bcccb14a6718756d4495` | `9e2c6644d794bcccb14a6718756d4495` | совпадает |
| `waves/.../green.log` | `06927be94491d833af50f179934214be` | `06927be94491d833af50f179934214be` | совпадает |
| `waves/.../mutations.log` | `9d8e7168eb796c2e16f2fb89de057819` | `9d8e7168eb796c2e16f2fb89de057819` | совпадает |
| `rust/gui-spike/src/lib.rs` | `dd547303373928de27f6c18ce52c41a9` | `dd547303373928de27f6c18ce52c41a9` | совпадает |

`git diff HEAD -- rust/gui-spike/src/lib.rs` содержит только две строки по этой карточке:
`pub mod dm_friends;` и `pub use dm_friends::*;`. Остальные 136 строк диффа — чужие изменения
общего worktree (`attachments`, `notifications`, `pins_threads`, IPC-обёртки), к task-112 не относятся.

## 2. Независимый прогон (не авторский)

| Команда | Результат | Exit |
|---|---|---|
| `cargo test --offline --test dm_friends_contract` (в `rust/gui-spike`) | `5 passed; 0 failed; 0 ignored` | 0 |
| `cargo clippy --offline --test dm_friends_contract -- -D warnings` | `Finished`, предупреждений нет | 0 |
| `rustfmt --edition 2021 --check rust/gui-spike/src/dm_friends.rs rust/gui-spike/tests/dm_friends_contract.rs` | чисто | 0 |
| `node --test tests/meta/test-policy.test.mjs` | `pass 17 / fail 0 / skipped 0 / todo 0` | 0 |

В тестах нет `#[ignore]`, `.only`, `todo!`, `unimplemented!`, `dbg!`; `#[allow(...)]` отсутствует
в обоих файлах. Сеть не используется: транспорт — `FakeDmFriendsTransport`.

## 3. Сверка с оракулами

Независимо от кода посчитано по `fixtures/gateway/by-type/002-READY.json`:

- `d.users` — **146**; `d.private_channels` — **17**, все `type=1`, ровно один `recipient_ids`,
  0 нерезолвленных id;
- `d.relationships` — **60**: `type=1` × **18**, `type=2` × **42**; `id != user_id` — **0**;
  `nickname == "nil"` у всех; ровно одна запись с `user_ignored == "true"` —
  `1373418368463278182` (`type=2`);
- `003-READY_SUPPLEMENTAL.json`: 2 друга, `idle` (`275258435199827968`) и `online` (`634667869254189065`).

Совпадение с тестами и реализацией:

- **Сортировка** — числовая `u64` по `(last_message_snowflake, channel_snowflake)` по убыванию.
  Порядок фикстуры: 0-й `1213176899493306439` (`1561431148641521707`, `.kirumi`),
  15-й `914570058590736436` (`1000134165657825371`, `zay_alabay`),
  16-й `1346114403149549638` (`nil` → `None`/0, `unix2096`). 19-значный snowflake
  `1000134165657825371` идёт раньше 18-значного `999999999999999999` — лексикографический
  баг поймал бы тест.
- **Маршруты** — `USER_CHANNELS` в `rest-static.json` = `POST /users/@me/channels`
  (`callSiteCount 2`, тела `{recipients}`); `GET` не вызывается ни разу; `group-dm-shell` —
  единственный маршрут с `{recipient_id}`; `PUT`/`DELETE` — `/users/@me/relationships/{user_id}`;
  `POST` — `/users/@me/relationships`.
- **Тела** — принятие заявки: `{"type":1,"confirm_stranger_request":false|true}`; блок:
  `{"type":2}`; Pomelo — `"discriminator": null`; legacy `alexvance#0421` → `421` (число);
  `from_friend_suggestion` передаётся только при `Some`.
- **Ошибки** — `80013` → `StrangerConfirmationRequired`; `30004` → `discord_error_code() == 30004`;
  `429` → `RateLimited { retry_after_ms }`, заголовок `Retry-After: 2` даёт `2000`.
- **Токен** — `Debug` клиента печатает `<redacted>`; `Display`/`Debug` всех 16 вариантов
  `DmFriendsError` не содержат токен, даже когда сервер отражает его в теле 401/500/429.
  Тело ответа ни в один вариант ошибки не попадает.
- **Валидация до сети** — пустой токен, `0`, 10 получателей, дубликаты, `../@me`, `123/456`,
  битые теги отклоняются с `0` вызовов транспорта.

## 4. Честность мутаций

`mutations.log`: baseline `d1820fe304816de3e4def848d992d3b9` совпадает с текущим файлом,
восстановление после всех пяти мутантов — тот же MD5, повторный прогон `5 passed`.

Реконструкция патчей в `/tmp` (read-only для репозитория) дала точное совпадение MD5 для 4 из 5:

| ID | MD5 в логе | Реконструкция | Итог |
|---|---|---|---|
| MUT-1 | `638606a5…` | другой вариант лексикографической сортировки | не воспроизведён побайтно |
| MUT-2 | `dee493d8…` | `pending_incoming_count = incoming_total` | **совпал** |
| MUT-3 | `b830f4d8…` | граница `MIN..=10` | **совпал** |
| MUT-4 | `a8f53100…` | `if false && code == 80013` | **совпал** |
| MUT-5 | `14fd47a8…` | `Value::String("0")` вместо `Null` | **совпал** |

Номера строк в `mutations.log` (320, 482, 580, 829, 949) точно соответствуют финальному
тест-файлу и именно тем проверкам, которые должны падать. Подделки не обнаружено.

## 5. Находки

### P2-1 — `red.log` снят с другой ревизии тестов

Номера строк паники в `red.log` не совпадают с финальным файлом:
`194 → 196`, `358 → 367`, `762 → 782`, `993 → 1021`. Rust рапортует строку вызова
`assert_eq!`/`assert!` (проверено отдельным примером: паника указывает на строку макроса, не
на строку сообщения). `red.log` создан в 20:28:34, тест-файл изменён в 20:37:09; смещение
растёт монотонно (+2, +9, +20, +28), то есть после RED-прогона в тесты дописано ~28 строк
утверждений.

Сами пять тестов и их сообщения в RED-логе присутствуют, поэтому red-first для исходного набора
подтверждён. Но финальные утверждения (в том числе часть проверок фикстур и утечки токена)
красными не наблюдались. По `TEST-POLICY.md` §4 повтор после правки тестов — **новый файл**,
а не старый лог. Нужен `red-final.log`: финальный тест-файл против заглушки (или отката
реализации), с exit-кодом. Старый `red.log` не переписывать.

### P2-2 — `map_error_response` теряет Discord-код на 401/403/404

`rust/gui-spike/src/dm_friends.rs`, `map_error_response`: ветка разбора `body.code` выполняется
только при `status != 401 && status != 403 && status != 404`. Для `403 {"code":40002}`
(`EMAIL_VERIFICATION_REQUIRED`, SPEC §1.4) результат — `DmFriendsError::Forbidden`,
`discord_error_code() == None`; для `404 {"code":10013}` — `NotFound` без кода. UI не может
отличить «подтвердите почту» от произвольного 403, то есть ошибка теряет контекст
(`PROCESS.md` §3.5, «Ввод и ошибки»).

Фикс: извлекать `code` до status-fallback для всех 4xx (как минимум для 403) и покрыть
red-тестом `403 + 40002` и `404 + 10013`.

### P3 (backlog, не блокирует)

1. В `ready_private_channels_and_rest_static_oracle_parity` проверка
   `transport.calls().is_empty()` использует fake, который никуда не передаётся: утверждение
   вакуумное. Реальное «0 GET» подтверждается только `method: POST` в `rest-static.json`.
2. Точный патч MUT-1 из описания не воспроизводится побайтно; для аудита стоит хранить
   mutation-runner или диффы мутантов рядом с логом.
3. Приоритет `Retry-After` над телом `retry_after` не зафиксирован в SPEC; обе величины в
   секундах, поведение корректное, но выбор стоит описать комментарием.
4. `PUT /channels/{id}/recipients/{user_id}` и поля `safety_warnings` /
   `blocked_user_warning_dismissed` из SPEC §1.5 / `T-N107-GDM-SHELL-01` в эту карточку не
   входят (минимальный объём N1-07 — 6 маршрутов) — остаются за core-карточкой.

## 6. Применимость остальных пунктов

- Live/AT-SPI, golden, FFI/unsafe, таймеры, сокеты — **не применимо**: модуль чистый,
  без сети и системных ресурсов.
- Границы файлов: `dm_friends.rs` и тест — новые файлы карточки; `lib.rs` — общий файл,
  правка минимальна (2 строки) и необходима для экспорта. Замечаний нет.

## 7. Что нужно для PASS

1. Новый `red-final.log` для финальной ревизии `dm_friends_contract.rs` (P2-1).
2. Типизированный код для 403/404 + red-тест (P2-2).
3. Перепрогон `cargo test` / `clippy` / `rustfmt` после фиксов и обновление MD5 в отчёте.
