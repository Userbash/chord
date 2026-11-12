# Техническое ревью (Review 87A): task-173 (BE-P2-11) — settings-proto lossless wire encode и REST GET/PATCH `/users/@me/settings-proto/{type}`

- **Ветка:** `feature/task-173-settings-proto-http`
- **Пулл-реквест:** `PR-58` (`feature/task-173-settings-proto-http -> unstable`)
- **Ревьюер:** Technical Reviewer (Review 87A)
- **Дата:** 2026-10-31
- **Вердикт:** **CHANGES REQUESTED (Not Ready for Merge)**
- **Критичность:** `P0 = 0`, `P1 = 1`, `P2 = 0`, `P3 = 3`
- **Мутации:** `5/5 (100%) KILLED`

Причина отказа ровно одна и не связана с качеством тестов: ветка отстала от `unstable` на 3 коммита, и после слияния новый код **не компилируется** (`E0063` на `RateLimitInfo`, см. §4). Тесты, линтер, скоуп и мутации в порядке.

---

## 1. Состав изменений (scope)

`git status --porcelain=v1` в worktree:

```
 M rust/crates/discord-core/src/settings/mod.rs
 M rust/crates/discord-core/src/settings/wire.rs
?? rust/crates/discord-core/tests/settings_proto_http.rs
```

Совпадает с ТЗ: изменены/созданы только три разрешённых файла. `Cargo.toml`, общие модули (`state/*`, `src/lib.rs`, `rest/routes.rs`, `rest/rate_limit.rs`) и чужие тесты не тронуты. Фикстуры `rust/crates/discord-core/tests/data/settings-*.b64` уже в репозитории, в этом PR не менялись.

| Файл | md5 |
|---|---|
| `rust/crates/discord-core/src/settings/wire.rs` | `93d9ac7e264a8d541814cf88072206bb` |
| `rust/crates/discord-core/src/settings/mod.rs` | `0ed2b411d34acc3d8627d494abdc66c8` |
| `rust/crates/discord-core/tests/settings_proto_http.rs` | `74b1c7ad48ee638a20011ca38345368c` |

md5 совпадают с §5 `REPORT.md`. `git diff --check` чистый.

---

## 2. Тесты и линтер

Команды из ТЗ, `CARGO_TARGET_DIR=/home/user/orca/project/rust/target`:

```
cargo test --manifest-path rust/Cargo.toml -p discord-core \
  --test settings_proto --test settings_proto_http --lib
```

- `lib`: **33 passed**, 0 failed
- `settings_proto`: **18 passed**, 0 failed
- `settings_proto_http`: **9 passed**, 0 failed
- Итого **60/60 passed**, exit `0`

```
cargo clippy --manifest-path rust/Cargo.toml -p discord-core \
  --test settings_proto --test settings_proto_http --lib -- -D warnings
```

- **0 warnings / 0 errors**, exit `0`

`rustfmt --check` по трём файлам воркера — чисто. Отдельно отмечу: `cargo fmt -p discord-core -- --check` ругается на `src/gateway.rs` и другие файлы, но это расхождение существует уже в `unstable` (`git show unstable:...gateway.rs | rustfmt --check` даёт тот же дифф), PR его не вносит и к скоупу не относится.

---

## 3. Мутации (5/5 KILLED)

Каждая мутация применялась к рабочему дереву, прогонялся `settings_proto_http` (+ `settings_proto` для M1), затем файл восстанавливался из побайтовой копии; после восстановления md5 совпал с исходным.

| # | Мутация (что сломано) | Внесённое изменение | Результат | Убивающий тест |
|---|---|---|---|---|
| **M1** | Потеря неизвестных полей при `ProtoMessage::encode` | в цикле `encode` добавлено `if field.number > 64 { continue; }` | **KILLED** (`8 passed; 1 failed`, exit `101`) | `t02_wire_encode_roundtrip_unknown_fields` (поля 9999/10000) |
| **M2** | Разрешены неподдерживаемые `proto_type` 0 и 3 | в `settings_proto_path` добавлена ветка `0 \| 3 => Ok(...)` | **KILLED** (`7 passed; 2 failed`, exit `101`) | `t05_settings_proto_path_validation`, `t06_build_get_settings_proto_request` |
| **M3** | Пропуск валидации base64/protobuf в `build_patch_settings_proto_body` | удалены `base64::decode(base64_settings)?` и `ProtoMessage::decode(&raw_bytes)?` | **KILLED** (`8 passed; 1 failed`, exit `101`) | `t07_build_patch_settings_proto_body` (кейсы 3 и 4: битый base64, `0x80`-варint) |
| **M4** | Не включается `required_data_version` при `Some(version)` | ветка `Some(_)` печатает `{"settings":"..."}` | **KILLED** (`8 passed; 1 failed`, exit `101`) | `t07_build_patch_settings_proto_body` (кейс 2) |
| **M5** | 401 классифицируется как общий клиентский отказ | удалена ветка `401 => Err(Unauthorized)` (проваливается в `400..=499 => Client`) | **KILLED** (`8 passed; 1 failed`, exit `101`) | `t09_parse_settings_proto_response_errors` |

Замечание к M1: тест на фикстурах `t04` при этой мутации остаётся зелёным, потому что топ-уровневые номера полей в четырёх фикстурах не превышают 25 (проверено независимым разбором wire-байт: `READY` — 18 полей, номера `1..14,16,19,23,25`; `UPDATE_*` — по 9 полей, `1..13`). Неизвестные поля ловит именно `t02`, и этого достаточно.

---

## 4. P1 — после слияния с `unstable` крейт не собирается

**Факт:** `HEAD` ветки = `0c70ce0` (Merge PR-53), `unstable` = `38f7b5c`; ветка отстала на 3 коммита, включая task-170 «REST Retry Policy & RateLimitInfo».

В task-170 структура `RateLimitInfo` (`rust/crates/discord-core/src/rest/rate_limit.rs`) получила два новых поля:

```rust
pub limit: Option<u32>,
pub scope: Option<String>,
```

и `impl Default for RateLimitInfo` (через `RateLimitInfo::new(Duration::ZERO)`).

Новый код `settings/mod.rs:250` строит структуру литералом:

```rust
let info = rate_limit_info(status, headers, body).unwrap_or(RateLimitInfo {
    retry_after: std::time::Duration::ZERO,
    global: false,
    remaining: None,
    reset_after: None,
    bucket: None,
});
```

`RateLimitInfo` не помечен `#[non_exhaustive]`, поэтому литерал обязан перечислять все поля.

**Проверка (воспроизведено на этом worktree):** временно подставлен `rust/crates/discord-core/src/rest/rate_limit.rs` из `unstable`, затем

```
cargo check --manifest-path rust/Cargo.toml -p discord-core --lib
```

```
error[E0063]: missing fields `limit` and `scope` in initializer of `RateLimitInfo`
   --> crates/discord-core/src/settings/mod.rs:250:73
    |
250 |             let info = rate_limit_info(status, headers, body).unwrap_or(RateLimitInfo {
    |                                                                         ^^^^^^^^^^^^^ missing `limit` and `scope`
error: could not compile `discord-core` (lib) due to 1 previous error
```

`rate_limit.rs` после проверки восстановлен (`md5 e9f0e2f4ab15c4895ec28680815bda8b`, `git status` снова показывает только три файла PR), `mod.rs` — `0ed2b411…` без изменений. То есть PR в текущем виде ломает сборку `unstable` при merge, это блокирующий дефект.

**Требуемое исправление (проверено, что компилируется против API `unstable`):**

```rust
let info = rate_limit_info(status, headers, body).unwrap_or_default();
```

Эквивалент с явными полями (`limit: None, scope: None`) тоже работает, но `unwrap_or_default()` не сломается при следующем расширении структуры.

**Проверка достаточности правки (тоже воспроизведено, затем откатано):** с этой заменой и подставленным `rate_limit.rs` из `unstable` прогнаны ровно те же команды — `cargo test … --test settings_proto --test settings_proto_http --lib` дал `33/18/9` passed (exit `0`), `cargo clippy … -- -D warnings` — чисто (exit `0`). После отката md5 файлов вернулись к значениям из §1, `git status` снова показывает только три файла PR. То есть одного однострочного исправления достаточно, других правок под `unstable` PR не требует.

Больше семантических конфликтов с `unstable` нет: остальные 7 изменённых там файлов (`routes.rs`, `transport.rs` не менялся, `base64.rs` не менялся, `discord-json` не менялся, `deep_link.rs`, `attachments.rs` и их тесты) в диффе `HEAD..unstable` содержат только добавления публичных сущностей, без изменения сигнатур. `HttpMethod::{Get,Patch}`, `HttpRequest::new/header/body`, `base64::{encode,decode}`, `rate_limit_info` сохранили сигнатуры.

---

## 5. Замечания P3 (не блокируют)

1. **`build_patch_settings_proto_request` не покрыт прямым тестом.** Функция добавлена сверх ТЗ (удобная обёртка), но ни один тест не проверяет её `method == Patch`, `target`, `content-type: application/json` и тело. Обе составные части (`settings_proto_path` + `build_patch_settings_proto_body`) покрыты, так что риск низкий; стоит добавить один assert в `t06`/`t07` при следующем касании файла.
2. **`parse_settings_proto_response` для прочих статусов** (например, 201/204/3xx) возвращает `Client { status }`, то есть 204 попадёт в «клиентскую ошибку». Для контракта Discord (200/401/429/4xx/5xx) это недостижимо, но лучше либо сузить до `400..=499`, либо явно описать поведение в doc-комментарии.
3. **Ветка `unwrap_or(...)` в 429 недостижима:** `rate_limit_info` возвращает `None` только при `status != 429`, поэтому внутри `429 =>` результат всегда `Some`. После правки P1 это остаётся безвредным защитным дефолтом — достаточно оставить комментарий «best-effort, если заголовки не разобрались», чтобы читатель не искал причину.

Проверено отдельно и претензий нет: в новом коде нет `unwrap()`/`expect()`/`panic!`; JSON для PATCH собирается `format!`, но перед подстановкой вызывается `base64::decode`, чей алфавит `A-Za-z0-9+/=` не содержит `"`/`\`, поэтому инъекция в JSON-строку невозможна; `ProtoField::encode_into` корректен для wire types 0/1/2/5, `write_varint` корректен на `u64::MAX`.

---

## 6. Сверка с RE-оракулом

- `rest-static.md:705` — `GET|PATCH /users/@me/settings-proto/{arg}` (`Endpoints.USER_SETTINGS_PROTO`), `rest-static.md:55` — `GET /users/@me/settings-proto/{type}` (загрузка настроек).
- `rest-d02.md:95` — GET `{num}` ×3 со статусом 200; `rest-d02.md:115` — PATCH `{num}` ×1, 200; `rest-d02.md:252-267` — тело `{"settings":"<base64 protobuf>"}` (в снимке обрезано на 4001 символе).
- `rest-static.json:424` / `rest-d02.json:3207` — `staticBodyKeys: "{settings,required_data_version}"`, то есть ключ `required_data_version` не выдуман.
- База запроса — относительный путь без `/api/v9`, как принято в `rest/routes.rs` (`«шаблон пути (относительный, без базы /api/v9)»`); `routes.rs` уже содержит обе записи `/users/@me/settings-proto/{num}` (GET и PATCH), поэтому путь билдера согласован с таблицей маршрутов.

Lossless-часть подтверждена независимо: четыре реальные фикстуры (`READY`, `UPDATE_8269`, `UPDATE_8608`, `UPDATE_13072`) декодируются и кодируются байт-в-байт, `t04` сравнивает и байты, и base64-строку, а число топ-уровневых полей `READY` (18) сходится с разбором wire-байт сторонним скриптом.

---

## 7. Итог

- **P0 = 0.**
- **P1 = 1** — `RateLimitInfo { … }` в `settings/mod.rs:250` после слияния с `unstable` даёт `E0063` (нет `limit`, `scope`).
- **P2 = 0.** **P3 = 3** (см. §5).
- **Мутации: 5/5 убито (100%).**
- **Вердикт: CHANGES REQUESTED (Not Ready for Merge).**

Воркеру (Pass 2): заменить литерал `RateLimitInfo { … }` на `unwrap_or_default()`, пересобрать на базе с влитым `unstable` (`cargo check -p discord-core --lib` + полный гейт), обновить `REPORT.md` (новый md5 `mod.rs`) и вернуть PR на повторное техническое ревью. После этого единственное блокирующее замечание снимается, и PR-58 можно переводить в `Ready for Merge`.
