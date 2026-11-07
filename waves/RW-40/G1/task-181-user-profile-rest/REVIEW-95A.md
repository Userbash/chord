# Техническое ревью 95A: task-181 (BE-P2-10) — PR-66

- **PR:** PR-66 (`feature/task-181-user-profile-rest` → `unstable`), база `b8183f5`
- **Карточка:** BE-P2-10 — `GET /users/{id}/profile`
- **Ревьюер:** Technical Reviewer (DeepSeek), Review 95A
- **Дата:** 2026-10-31
- **Вердикт:** **APPROVED — Ready for Merge** (P0 = 0, P1 = 0, мутации 5/5)

## 1. Write-scope

Состав изменений в рабочем дереве совпадает с ТЗ; посторонних правок нет.

| Файл | Статус | md5 | md5 в REPORT.md | Совпадает |
|---|---|---|---|---|
| `rust/gui-spike/src/profile.rs` | новый | `c29689e4daf174f62ba2e0e3e993e4be` | `c29689e4daf174f62ba2e0e3e993e4be` | да |
| `rust/gui-spike/src/lib.rs` | изменён (+3 строки: doc-комментарий и `pub mod profile;`) | `331615ce51071b3bc7e789db24aca5a2` | `331615ce51071b3bc7e789db24aca5a2` | да |
| `rust/gui-spike/tests/profile_contract.rs` | новый | `2bb3aebf3d5098fbf17a3c7632e0aecc` | `2bb3aebf3d5098fbf17a3c7632e0aecc` | да |

Дифф `lib.rs` — только вставка `pub mod profile;` с доком в алфавитном месте
(между `pins_threads` и `reactions`), посторонних строк нет.

`git status` в рабочем дереве дополнительно показывает ` M package-lock.json`
(дрейф `bin`/`engines`, вне write-scope и не относится к задаче — см. P2-1).
`rust/gui-spike/Cargo.lock` при сборке дрейфует (следствие `b8183f5`); после
прогонов откачен командой `git checkout -- rust/gui-spike/Cargo.lock`,
в итоговом `git status` его нет.

## 2. Гейты (CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target)

| Команда | Результат |
|---|---|
| `cargo test --manifest-path rust/gui-spike/Cargo.toml --test profile_contract --lib` | exit 0: `profile_contract` **26/26**, lib **17/17** |
| `cargo clippy --manifest-path rust/gui-spike/Cargo.toml --test profile_contract --lib -- -D warnings` | exit 0, предупреждений нет |
| `rustfmt --edition 2021 --check src/profile.rs tests/profile_contract.rs` | exit 0 |
| `rustfmt --edition 2021 --check --config skip_children=true src/lib.rs` | exit 0 |

Логи: `mutations-95a/gate-test.log`, `mutations-95a/gate-clippy.log`.

## 3. Сверка с оракулом `/home/sanya/re-deep/rest-static.json`

Проверено независимо от отчёта воркера: `endpoints[5]` — `key: USER_PROFILE`,
`method: GET`, `path: /users/{arg}/profile`, `callSiteCount: 3`,
`confirmed: false`; `cacheEvidence.example`:

```
https://discord.com/api/v9/users/1168176063113875531/profile?type=popout&with_mutual_guilds=true&
with_mutual_friends=true&with_mutual_friends_count=false&guild_id=1014537851418980372
```

`DEFAULT_POPOUT_QUERY` и порядок параметров в `build_user_profile_path` совпадают
с захватом дословно, `guild_id` дописывается последним и только при `Some`.
Догадок в коде нет; поля тела помечены как неподтверждённые и разбираются
терпимо (`#[serde(default)]`, лишние ключи игнорируются).

## 4. Мутационное тестирование: 5/5 KILLED

Каждая мутация применялась к чистому файлу, прогонялся
`cargo test ... --test profile_contract --lib`, затем файл восстанавливался
из копии с проверкой md5 (`c29689e4daf174f62ba2e0e3e993e4be` после каждого шага).

| # | Мутация | md5 мутанта | exit | Итог | Убившие тесты |
|---|---|---|---|---|---|
| M1 | убрана проверка `user_id` как снежинки | `122bf509e6a07c06b2ddaaf38d7062a5` | 101 | **KILLED** | `invalid_ids_are_rejected_before_any_transport_call`, `user_id_must_be_a_non_empty_ascii_digit_snowflake` |
| M2 | не дописывается `guild_id` в query | `683b6cac457a5a13833906d15afddd97` | 101 | **KILLED** | `fetch_forwards_the_guild_scoped_path`, `guild_id_is_appended_last_exactly_as_in_the_capture` |
| M3 | допускается пустой `user.id` в `parse_user_profile` | `bbe2a39861cdbcde41e2a968999dadd7` | 101 | **KILLED** | `empty_or_missing_user_identity_is_invalid_response` |
| M4 | на 429 возвращается `retry_after_ms = 0` | `af29752f6bd28991d5195fcbe0d5a0e5` | 101 | **KILLED** | `fetch_429_extracts_retry_after_from_headers`, `fetch_429_extracts_retry_after_from_json_body` |
| M5 | `404` мапится в `Unauthorized` | `355ee810d35a2b6c1e82e412e95446d8` | 101 | **KILLED** | `http_statuses_map_to_typed_errors` |

Ни одна мутация не выжила; ни одна не падала на компиляции
(`compile_errors=0`), то есть тесты ловят именно поведение, а не сборку.
Сводка и логи: `mutations-95a/SUMMARY.md`, `mutations-95a/M1..M5.log`.

После прогона исходник восстановлен байт-в-байт (md5 `c29689e4daf174f62ba2e0e3e993e4be`
совпадает с REPORT.md), `Cargo.lock` откачен.

## 5. Ревью кода

**Безопасность.** Токен живёт в `ApiClient`; `Debug` у `DiscordProfileApi`
намеренно неполный (`finish_non_exhaustive`), в `ProfileError`/`Display` токен и
тело ответа не попадают. `percent-encoding` в query нет, но `type` проходит
`is_safe_query_token` (отвергаются `&`, `#`, `?`, `/`, пробелы, control), а
`user_id`/`guild_id` — только ASCII-цифры и `u64`: подменить остальные параметры
нельзя. Сетевой вызов не выполняется, пока аргументы не прошли валидацию
(проверено тестом `invalid_ids_are_rejected_before_any_transport_call`).

**Ошибки.** `429` разбирается до `RestFailure::from_status`, `retry_after`
читается из JSON-числа, JSON-строки и заголовков `Retry-After` /
`x-ratelimit-reset-after`, переполнение насыщается в `u64::MAX`, непригодные
значения дают `0` (задержка не выдумывается). `4xx`/`5xx` идут через общий
`rest_failure::RestFailure`, ни один вариант не теряется. Паник в продуктовом
коде нет: `unwrap`/`expect`/`unwrap_or`-злоупотреблений и `unsafe` нет,
`unwrap()` встречается только в тестах.

**Типы.** Все структуры ответа `#[serde(default)]` и `PartialEq/Eq`, что
соответствует `confirmed: false` у оракула; `type` у `ConnectedAccount`
переименован и обратимо сериализуется (отдельный тест). Обязательны только
`user.id` и `user.username`.

**Тесты.** 26 контрактных тестов сверяются с захватом (точные строки путей
заданы константами из оракула, а не вычисляются из реализации), покрывают
негативные и граничные случаи: пустые/нецифровые идентификаторы, `u64::MAX`,
`u64::MAX + 1`, пустая идентичность, битые тела, `guild_member: null`, пустые
коллекции, все коды ошибок и оба источника `retry_after`. Фейковый транспорт
фиксирует ровно один `GET` по построенному пути.

## 6. Замечания

**P0 — 0. P1 — 0.** Блокирующих замечаний нет.

**P2 (не блокирует merge этой задачи, требует внимания интегратора):**

1. `package-lock.json` изменён в рабочем дереве (`bin`/`engines` у корневого
   пакета). Это не следствие task-181 и не входит в write-scope: в PR-66 файл
   попасть не должен — при сборке PR проверить состав диффа.
2. `rust/gui-spike/Cargo.lock` дрейфует при каждой сборке (в нём нет
   `ruzstd`-замены `zstd` у `discord-codec` после `b8183f5`). Воркер откатывал
   его, я откатил повторно; пока актуальный lock не закоммичен отдельной
   задачей, любой прогон оставляет грязный worktree.

**P3 (наблюдения, можно отложить):**

1. `ProfileQuery` выводит `Serialize`/`Deserialize`, но поле `profile_type`
   сериализуется как `profile_type`, тогда как на проводе параметр называется
   `type`. Сейчас структура не используется как wire-DTO (путь собирается
   вручную в `build_user_profile_path`), поэтому дефекта нет, но при
   переиспользовании получится неверный ключ. Вариант: `#[serde(rename = "type")]`
   либо убрать derive.
2. `ApiProfileTransport::new` отображает отказ `ApiAuth::from_token`/identity в
   `ProfileError::InvalidArg`, чей `Display` говорит про «некорректный id
   пользователя или сервера». Семантически это разные вещи (проблема
   аутентификации ≠ неверный аргумент); сообщение вводит в заблуждение при
   диагностике, хотя на поведение не влияет.
3. `parse_user_profile` проверяет непустоту `user.id`, но не проверяет, что это
   снежинка. Для доверенного ответа Discord это допустимо; входной `user_id`
   уже валидируется в `build_user_profile_path`.

## 7. Решение

**APPROVED — Ready for Merge в `unstable`.**

Условия соблюдены: P0 = 0, P1 = 0, мутации 5/5 убиты, гейты зелёные,
write-scope не нарушен, md5 совпадают с REPORT.md, `Cargo.lock` откачен.
Коммит и merge выполняет координатор/интегратор; из диффа PR-66 исключить
`package-lock.json`.
