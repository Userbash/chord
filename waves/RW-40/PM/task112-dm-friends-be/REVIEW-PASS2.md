# DeepSeek Review 40B — task-112, повторная проверка P2-1 и P2-2

**Вердикт: PASS.**
Оба замечания Review 40A закрыты, регрессий не нашёл. Ниже — независимый прогон,
аудит контрольных сумм и то, чем именно подтверждено каждое закрытие.
Не блокирующие замечания вынесены в §6 (P3), как и в первом ревью.

Ветка: `feature/task-112-dm-friends-be`, HEAD `4e24048`. Рабочее дерево то же, что в
Review 40A: `dm_friends.rs` и `dm_friends_contract.rs` не закоммичены, `src/lib.rs` — modified
(в диффе по-прежнему только `pub mod dm_friends;` и `pub use dm_friends::*;` по этой карточке,
остальное — чужие карточки общего worktree).

## 1. Независимый прогон (не авторский)

| Команда | Результат | Exit |
|---|---|---|
| `CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target cargo test --manifest-path rust/gui-spike/Cargo.toml --test dm_friends_contract` | `6 passed; 0 failed; 0 ignored` | 0 |
| `... cargo clippy --manifest-path rust/gui-spike/Cargo.toml -p discord-gui-spike --test dm_friends_contract -- -D warnings` | `Finished`, предупреждений нет | 0 |
| `... cargo test --manifest-path rust/gui-spike/Cargo.toml --test ipc_wiring_contract` | `8 passed; 0 failed` | 0 |
| `rustfmt --edition 2021 --check rust/gui-spike/src/dm_friends.rs rust/gui-spike/tests/dm_friends_contract.rs` | чисто | 0 |
| `node --test tests/meta/test-policy.test.mjs` | `pass 17 / fail 0 / skipped 0` | 0 |

Область clippy та же, что в Review 40A: `--all-targets` в этом worktree падает на
`clippy::type_complexity` в `tests/net_contract.rs` и `src/main.rs` (файлы других карточек,
`dm_friends` там не используется). Расширение области не требуется.

## 2. MD5-аудит против REPORT.md §6

| Файл | В отчёте | Фактически | Итог |
|---|---|---|---|
| `rust/gui-spike/src/dm_friends.rs` | `6e74f81b47b9579bb0f863889c453479` | `6e74f81b47b9579bb0f863889c453479` | совпадает |
| `rust/gui-spike/tests/dm_friends_contract.rs` | `920b11eef5ef5de7bebf715a9846f5ad` | `920b11eef5ef5de7bebf715a9846f5ad` | совпадает |
| `red.log` | `9e2c6644d794bcccb14a6718756d4495` | `9e2c6644d794bcccb14a6718756d4495` | не переписан |
| `red-final.log` | `ce944aa49c6016a07d3a97ad3a86cda4` | `ce944aa49c6016a07d3a97ad3a86cda4` | совпадает |
| `green.log` | `06927be94491d833af50f179934214be` | `06927be94491d833af50f179934214be` | совпадает |
| `green-final.log` | `1ab68c45acdfc7113687f471cbd88354` | `1ab68c45acdfc7113687f471cbd88354` | совпадает |
| `mutations.log` | `9d8e7168eb796c2e16f2fb89de057819` | `9d8e7168eb796c2e16f2fb89de057819` | совпадает |
| `rust/gui-spike/src/lib.rs` | `dd547303373928de27f6c18ce52c41a9` | `dd547303373928de27f6c18ce52c41a9` | совпадает |

## 3. P2-1 — RED-лог для финальной ревизии тестов: закрыто

Что требует Review 40A: новый лог финального тест-файла против заглушки или отката
реализации, с кодом выхода; старый `red.log` не переписывать.

Что проверил:

- `red.log` не тронут (`9e2c6644…`), `red-final.log` — отдельный файл (`ce944aa4…`).
- В шапке `red-final.log` указаны MD5 финального тест-файла `920b11ee…` и редакции
  `dm_friends.rs` до фикса P2-2 `d1820fe3…`; обе величины совпадают с фактическими.
- Лог содержательный: `running 6 tests`, `5 passed; 1 failed`, `EXIT=101`, паника
  `tests/dm_friends_contract.rs:1127:5`. Строка 1127 финального файла — это `assert_eq!(` в §6.1
  (`403 + 40002`); проверил по `sed -n '1120,1135p'`, смещений нет.
- Аутентичность лога подтвердил собственным мутантом: откатил в `map_error_response` условие
  до редакции до фикса (`status != 401 && status != 403 && status != 404`), прогнал тест —
  получил **тот же** сигнал, что в `red-final.log`: та же строка 1127, то же сообщение
  `403 обязан сохранять код Discord 40002 (EMAIL_VERIFICATION_REQUIRED)`,
  `left: Forbidden` / `right: DiscordApiCode { status: 403, code: 40002 }`,
  `5 passed; 1 failed`, exit 101. После проверки файл восстановлен из побайтовой копии,
  MD5 снова `6e74f81b47b9579bb0f863889c453479`, повторный `cargo test` — `6 passed`.
- Панику на `1127:5` воспроизвёл и мутант, и `red-final.log` — значит лог снят с текущего
  тест-файла, а не с промежуточной редакции. Именно этого не хватало в Review 40A.

Замечание по формулировке отчёта, не блокирует: утверждение «финальная редакция тестов
отличается от исходной только блоком §6» буквально не доказуемо — номера строк в `red.log`
сдвинуты на +2…+28, и часть сдвига даёт `rustfmt`, а не только новые строки. Для P2-1 это
неважно: каждая паника из `red.log` имеет в финальном файле тот же текст утверждения
(`146 пользователей`, `60 записей`, маршрут `PUT /users/@me/relationships/…`,
`Debug DiscordDmFriends` с токеном), то есть red-first исходных пяти тестов подтверждён,
а новые утверждения закрыты `red-final.log`.

## 4. P2-2 — код Discord на 403/404: закрыто

`rust/gui-spike/src/dm_friends.rs:914-940`, `map_error_response`:

- `429` обрабатывается первым (`RateLimited { retry_after_ms }`), затем для любого `4xx`, кроме
  `401`, из тела извлекается числовой `code` до статусного fallback; `code == 0` кодом не
  считается. Порядок веток: `80013` → `StrangerConfirmationRequired` (при известном `user_id`),
  иначе `DiscordApiCode { status, code }`, иначе `RestFailure::from_status`.
- Коды совпадают со SPEC §1.4: `40002` — `EMAIL_VERIFICATION_REQUIRED` (HTTP 403), `10013` —
  `UNKNOWN_USER` (строка 150 и 153 `DM-FRIENDS-SPEC.md`).
- `Display` варианта `DiscordApiCode` печатает `HTTP {status}, код {code}`, `discord_error_code()`
  отдаёт `Some(code)` — UI отличает «подтвердите почту» от произвольного 403.

Тест `rest_error_responses_preserve_discord_code_on_403_and_404` (`dm_friends_contract.rs`,
§6) чёрный, через публичный API и `FakeDmFriendsTransport`, покрывает:

- `403 {"code":40002}` → `DiscordApiCode { 403, 40002 }` + `discord_error_code() == Some(40002)`
  + код в `Display`;
- `404 {"code":10013}` → `DiscordApiCode { 404, 10013 }`;
- `403` без `code` → `Forbidden`, `404` с `"code":0` → `NotFound`, `discord_error_code() == None`;
- `401` с `"code":0` → `Unauthorized`.

Регрессий не нашёл: для `400` извлечение кода было и до фикса (исключались только `401/403/404`),
`429` и `80013` не затронуты; в тестах нет `#[ignore]`, `.only`, `todo!`, `unimplemented!`,
`dbg!`, `#[allow(...)]`, `unsafe`; сеть не используется.

## 5. Границы и запреты

- Изменены только файлы карточки: `rust/gui-spike/src/dm_friends.rs`,
  `rust/gui-spike/tests/dm_friends_contract.rs` (новые), `src/lib.rs` (2 строки экспорта),
  `REPORT.md` и логи в `waves/RW-40/PM/task112-dm-friends-be/`.
- `fixtures/`, `logs/`, `functions/`, `evidence/` не изменены (в `git status` нет правок).
- Ветка не закоммичена — это ожидаемо, коммит делает координатор.

## 6. P3 (backlog, PASS не блокирует)

1. `mutations.log` по-прежнему содержит 5 мутантов, а REPORT §5 заявляет `6/6 KILLED`;
   MUT-6 живёт в `red-final.log`. Стоит либо дописать MUT-6 в `mutations.log`, либо явно
   назвать его в шапке файла — сейчас связка «мутация → лог» держится на тексте отчёта.
2. Вакуумная проверка `transport.calls().is_empty()` в
   `ready_private_channels_and_rest_static_oracle_parity` (P3-1 из Review 40A) не исправлена —
   вне объёма P2, «0 GET» подтверждается только `rest-static.json`.
3. Патч MUT-1 побайтно не воспроизводится (P3-2 из Review 40A) — для аудита полезен
   mutation-runner или диффы рядом с логом.
4. Приоритет `Retry-After` над телом `retry_after` (P3-3 из Review 40A) по-прежнему не описан
   в SPEC; поведение корректное.

## 7. Что нужно от координатора

Ничего по этой карточке: P2-1 и P2-2 закрыты, гейт по модулю зелёный, MD5 в отчёте
актуальны. Можно коммитить и переводить карточку в accepted; P3-пункты — в общий backlog волны.
