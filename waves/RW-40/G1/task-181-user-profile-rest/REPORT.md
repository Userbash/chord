# task-181 — BE-P2-10: профиль пользователя `GET /users/{id}/profile`

- **Воркер:** G1 Backend, task-181 (`feature/task-181-user-profile-rest`)
- **Карточка:** BE-P2-10 (чтение профиля пользователя из `rest-static.json`)
- **Write-scope:** `rust/gui-spike/src/profile.rs` (new), `rust/gui-spike/src/lib.rs`
  (+`pub mod profile;`), `rust/gui-spike/tests/profile_contract.rs` (new) — других
  файлов не касался.

## Оракул

`/home/sanya/re-deep/rest-static.json`, endpoint `USER_PROFILE`:
`method: "GET"`, `path: "/users/{arg}/profile"`, `rest: true`, `callSiteCount: 3`,
`confirmed: false`. Живой захват (`cacheEvidence.example`, 9 вызовов, последний
`18 Oct 2026 18:42:23`):

```
https://discord.com/api/v9/users/1168176063113875531/profile?type=popout&with_mutual_guilds=true&with_mutual_friends=true&with_mutual_friends_count=false&guild_id=1014537851418980372
```

Отсюда дословно взяты шаблон, состав/порядок query-параметров и значения по
умолчанию (`DEFAULT_POPOUT_QUERY`, `ProfileQuery::default`), а `guild_id`
дописывается последним и только если задан. `confirmed: false` означает, что
схема тела ответа в дампе не подтверждена: структуры описывают поля, которые
реально приходят в профиле, и терпимы к отсутствующим ключам (`#[serde(default)]`),
лишние ключи игнорируются.

## Сделано

`rust/gui-spike/src/profile.rs` (451 строка):

- Константы `USER_PROFILE_PATH_TEMPLATE = "/users/{id}/profile"` и
  `DEFAULT_POPOUT_QUERY` (ровно строка из захвата без `guild_id`).
- `ProfileQuery` (`profile_type: Option<String>`, три bool-флага, `guild_id`)
  с `Default` = popout / mutual guilds+friens включены / count выключен / без guild.
- `build_user_profile_path(user_id, query)`: снежинка-валидация `user_id` и
  `guild_id` (непустые ASCII-цифры, `u64`), `type` проверяется на query-инъекцию
  (`&`, `#`, `?`, `/`, пробелы, control), сборка
  `/users/{id}/profile?type=...&with_mutual_guilds=...&with_mutual_friends=...&with_mutual_friends_count=...[&guild_id=...]`.
- Типизированный ответ: `ProfileUser`, `ProfileBadge`, `ConnectedAccount`
  (`#[serde(rename = "type")]`), `GuildMemberProfile`, `MutualGuild`,
  `UserProfileResponse`; все с `Debug/Clone/PartialEq/Eq/Default/Serialize/Deserialize`
  и `#[serde(default)]`. `parse_user_profile` возвращает
  `ProfileError::InvalidResponse`, если `user.id` или `user.username` пустые.
- `ProfileError` (`InvalidArg`, `Unauthorized`, `Forbidden`, `NotFound`,
  `RateLimited { retry_after_ms }`, `Client { status }`, `Server { status }`,
  `Network`, `Storage`, `InvalidResponse`) с `Display`, `Error`, `From<RestFailure>`
  и разбором `retry_after` из JSON/заголовков (тот же алгоритм, что в `reactions.rs`).
- `ProfileTransport` (async `get`) + `ApiProfileTransport` поверх `ApiClient` ядра
  + `DiscordProfileApi<T>::fetch_user_profile` + свободная функция
  `fetch_user_profile(&transport, ...)`. 401/403/404/429/4xx/5xx классифицируются
  через `rest_failure::RestFailure`.

`rust/gui-spike/src/lib.rs`: строка `pub mod profile;` и doc-комментарий (3 строки
диффа), порядок модулей сохранён.

`rust/gui-spike/tests/profile_contract.rs` (689 строк, 26 тестов): путь/query
против захвата (включая точный guild-вариант), отсутствие `type` при `None`,
снежинки (пусто/нецифры/`u64::MAX+1`/`u64::MAX`), отказ до вызова транспорта,
полный и минимальный разбор тела, `guild_member: null`, пустая идентичность,
битые тела, rename `type`, ровно один `GET` на построенный путь, коды
401/403/404/429/400/405/500/503, `retry_after` из JSON-числа/строки/заголовков
(включая `x-ratelimit-reset-after`), непригодные значения → 0, `u64::MAX` при
переполнении, проброс ошибок транспорта, полный маппинг `RestFailure`, `Display`.

## TDD

- **Red:** `red.log` — `cargo test ... --test profile_contract --lib` падает на
  компиляции: `E0432 unresolved import discord_gui_spike::profile` (модуля ещё нет).
  Первый прогон дополнительно поймал мои ошибки в тесте (non-ASCII в byte-string,
  идентификатор с цифры) — они исправлены до фиксации финального red-лога.
- **Green:** `green.log` — 26/26 `profile_contract`, 17/17 lib-тестов;
  `cargo clippy ... -- -D warnings` exit 0; `rustfmt --check` по трём файлам exit 0.

## Команды

```sh
CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target \
  cargo test --manifest-path rust/gui-spike/Cargo.toml --test profile_contract --lib
CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target \
  cargo clippy --manifest-path rust/gui-spike/Cargo.toml --test profile_contract --lib -- -D warnings
rustfmt --edition 2021 --check \
  rust/gui-spike/src/profile.rs rust/gui-spike/tests/profile_contract.rs
rustfmt --edition 2021 --check --config skip_children=true rust/gui-spike/src/lib.rs
```

## Файлы и md5

| Файл | md5 |
|---|---|
| `rust/gui-spike/src/profile.rs` | `c29689e4daf174f62ba2e0e3e993e4be` |
| `rust/gui-spike/src/lib.rs` | `331615ce51071b3bc7e789db24aca5a2` |
| `rust/gui-spike/tests/profile_contract.rs` | `2bb3aebf3d5098fbf17a3c7632e0aecc` |

## Риски и замечания

- **`confirmed: false` в оракуле.** Подтверждён только маршрут и query (живой
  захват); схема тела не подтверждена дампом. Поэтому разбор терпимый
  (`#[serde(default)]`, неизвестные ключи игнорируются), а обязательны только
  `user.id`/`user.username` — при появлении RE-подтверждения полей структуры надо
  сверить, а не догадываться.
- **Cargo.lock.** `rust/gui-spike/Cargo.lock` при сборке обновился: в нём не было
  `zstd`-замены `ruzstd` у `discord-codec` (следствие мержа b8183f5, не этой
  задачи). Дрейф откачен (`git checkout -- rust/gui-spike/Cargo.lock`), в
  write-scope он не входит; на следующей сборке cargo обновит его снова, пока
  интегратор не закоммитит актуальный lock.
- **`package-lock.json`** был изменён до начала работы (`git status` на входе:
  ` M package-lock.json`), я его не трогал.
- **`rustfmt` по `lib.rs`** рекурсивно проверяет дочерние модули и показывает
  прежний дрейф в `src/net.rs` (пять сигнатур/`write!`), к task-181 не относящийся;
  `net.rs` не правил. Мои три файла проходят `rustfmt --check` (для `lib.rs` —
  `--config skip_children=true`).
- IPC/Tauri-команда и UI-проводка не добавлялись: карточка BE-P2-10 — только
  REST-слой, `pub mod profile;` даёт фронтенд-слою точку входа.
