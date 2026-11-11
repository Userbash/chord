# G-01b-CONTRACT (P1B) — `lastGuildId` в `UiSettings` + restore-контракт

- batch/карточка: `RW-40 / G-01b-CONTRACT` (P1B, G1 Backend writer);
- роль/место: G1 backend, файлы настроек `rust/gui-spike/**`; UI — не мой файл (G2 отдельно);
- дата: 2026-10-24 (UTC). Первый прогон шёл на HEAD `0239543` (RW-35); повторная
  проверка (`G-01b-CONTRACT-RETRY`) — на HEAD `4e24048` (RW-40), куда результат уже
  вошёл коммитом координатора.

## Сделано

1. **Поле.** `UiSettings.last_guild_id: Option<String>` — аддитивное, дефолт `None`.
   - в файле и IPC — camelCase `lastGuildId` (`#[serde(rename = "lastGuildId")]`);
   - `skip_serializing_if = "Option::is_none"`: без выбора файл остаётся байт-в-байт прежним
     (канонический JSON дефолтов не изменился, существующий тест не пришлось ослаблять);
   - `default` + собственный десериализатор: файл прежней схемы без ключа читается как `None`
     (аддитивная миграция), `SETTINGS_VERSION` **не** поднят — иначе все существующие файлы
     (включая реальный `.local/settings.json`, 5 полей) стали бы `Invalid`.
2. **Валидация/нормализация.**
   - `normalize_last_guild_id()` — только непустая десятичная строка, влезающая в `u64`
     (то же правило, что `guilds::is_snowflake` для enter/leave guild);
   - пустое/чужое (пробелы, буквы, `-1`, `+1`, `1.5`, 21 цифра, не-ASCII цифры, число,
     массив, объект, `true`, `null`) → `None`; ошибкой не становится:
     - при **чтении** — битая подсказка не рушит загрузку остальных настроек;
     - при **записи** — `SettingsStore::save` нормализует копию до `validate()`, на диск
       попадает только snowflake;
   - `UiSettings::validate()` остаётся строгим для напрямую собранных структур:
     `Some("не snowflake")` → `Invalid { field: "lastGuildId" }`;
   - **источник не выдумывается**: backend не подставляет первую гильдию из READY/списка,
     дефолт — `None`, хранится ровно то, что записал UI.
3. **UI не тронут** (`ui/**` — 0 изменений), live/attach/spawn не запускались, сеть не
   использовалась, golden (`logs/`, `functions/`, `evidence/`) только читался.

## Red

```
cd rust/gui-spike && cargo test --offline --test settings_contract
```
→ exit **101**, 11 ошибок компиляции: `E0560`/`E0609` — «struct `UiSettings` has no field
named `last_guild_id`», available fields: `version, toggles, density, theme, locale`.
Лог: `PM/g01b-contract/logs/red.log` (3996 B, 11 `error[E…]`).

## Green

```
cd rust/gui-spike && cargo test --offline --test settings_contract
```
→ exit **0**, `test result: ok. 23 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`.
Лог: `PM/g01b-contract/logs/green.log`.
Новые тесты (6 из 23):

| Тест | Что фиксирует |
|---|---|
| `last_guild_id_defaults_to_none_and_stays_out_of_canonical_file` | дефолт `None`; нет ключа `lastGuildId`/`last_guild_id` в каноническом JSON |
| `last_guild_id_roundtrips_as_camel_case_ipc_field` | save→load без потерь; ключ ровно `lastGuildId`; density/theme/locale/toggles целы |
| `legacy_file_without_last_guild_id_migrates_to_none` | файл прежней схемы (5 полей) читается, `None`, прочие значения сохранены |
| `non_snowflake_last_guild_id_normalizes_to_none` | 14 чужих значений при чтении → `None`, загрузка не падает, настройки целы |
| `save_drops_non_snowflake_last_guild_id_without_failing` | мусор на записи → ключа нет; верхняя граница `u64` (20 цифр) сохраняется |
| `validate_rejects_non_snowflake_last_guild_id` | прямой `validate()` → `Invalid { field: "lastGuildId" }` |

Дополнительно (не гейт): `rustfmt --check` по двум файлам — exit 0;
`cargo clippy --offline --lib --test settings_contract` — exit 0, предупреждений 0
(`logs/clippy-targeted.log`).

## Оракул

- Реальный legacy-артефакт: `.local/settings.json` (5 полей, md5 `c14195f6…`, **не изменялся**,
  только чтение) — источник кейса миграции;
- правило snowflake — независимое от этой реализации: `guilds::is_snowflake`
  (`guilds.rs:422-426`: непустая строка, только ASCII-цифры, `parse::<u64>()`);
- имя поля `lastGuildId` — из карточки G-01b / `G6-REPARITY-04 §3` (camelCase IPC), не из кода;
- дефолты UI-настроек по-прежнему сверяются с `ui/index.html` в существующем тесте.

## Остаток / риски (передать дальше)

1. **G2 `G-01b-RESTORE`** — потребитель: `ui/app.js`/`ui/features/settings.js` должны
   (а) сохранять выбранный `guildId` через `settings_save` как `lastGuildId`,
   (б) при загрузке rail выбирать сохранённый сервер, только если он есть в READY,
   иначе — текущее поведение (первый/пустой). Сейчас UI `settings_save` не вызывает
   (`app.js` — только allow-list invoke), поэтому поле пока никем не пишется.
2. **Форма IPC при `None`**: ключ отсутствует (не `null`). Потребителю читать как
   `settings.lastGuildId ?? null`. Если G2 нужен явный `null` — скажите, это одна строка
   (`skip_serializing_if`), но тогда канонический файл дефолтов меняется.
3. **Кросс-версия**: старый читатель с `deny_unknown_fields` не прочитает файл, где уже есть
   `lastGuildId` (новое поле). Откат на старый билд после выбора сервера = `Parse` на всём
   файле. Приемлемо для спайка; при необходимости — обсудить до релиза.
4. `save()` намеренно **не** возвращает ошибку за мусорный `lastGuildId` (нормализует в `None`),
   чтобы битая подсказка не блокировала сохранение всех настроек. Прямой `validate()` — строгий.
   Если политика «явный отказ на записи» нужна, это меняет один тест.
5. Полный `gate:worker`/`tsc`/e2e воркер не запускал (по ТЗ — integration-лейн batch).

## Файлы + md5 (финальные байты)

| Файл | md5 | Δ |
|---|---|---|
| `rust/gui-spike/src/settings_store.rs` | `ac58e876a6553e0d00cd881f30622b0b` | +поле/нормализация/десериализатор/save |
| `rust/gui-spike/tests/settings_contract.rs` | `0e943bb9a487d77718cb37b3338e4ea6` | +6 тестов G-01b, `sample()` с `None` |
| `PM/g01b-contract/logs/red.log` | red-прогон (exit 101) | новый |
| `PM/g01b-contract/logs/green.log` | green-прогон (23/23, exit 0) | новый |
| `PM/g01b-contract/logs/green-verify.log` | повторный green на HEAD `4e24048` (23/23, exit 0) | новый (retry) |
| `PM/g01b-contract/logs/clippy-targeted.log` | clippy clean, exit 0 | новый |
| `rust/gui-spike/ui/**` | без изменений | — |
| `.local/settings.json` | `c14195f667c064e14daa899d5999d4fd` | только чтение |

`rust/gui-spike/` вошёл в integration-коммит `4e24048` (RW-40); на момент retry
`git status --short -- rust/gui-spike .local` пуст — рабочее дерево совпадает с HEAD,
локального дрейфа нет. md5 файлов совпадают с зафиксированными ниже
(`ac58e876…` / `0e943bb9…`), т.е. после приёмки код не менялся.

## Пост-проверка `G-01b-CONTRACT-RETRY` (2026-10-24, HEAD `4e24048`)

Предыдущий запуск отчёт **оставил** (`PM/g01b-contract/REPORT.md` уже в коммите
`4e24048`), но он ссылался на старый HEAD и на «untracked» статус — это исправлено
выше. Содержательно работа подтверждена без правок product-кода:

1. **Поле на месте:** `UiSettings.last_guild_id: Option<String>` — `#[serde(default,
   rename = "lastGuildId", skip_serializing_if = "Option::is_none",
   deserialize_with = "deserialize_last_guild_id")]`
   (`rust/gui-spike/src/settings_store.rs:158-166`); `Default` — `None`;
   `SETTINGS_VERSION` остался `1`.
2. **Валидация:** `is_snowflake` — непустая ASCII-цифровая строка, влезающая в `u64`
   (то же правило, что `guilds::is_snowflake`, `guilds.rs:421-426`);
   `normalize_last_guild_id` → `None` для пустого/чужого; `load` не падает на битой
   подсказке; `save` нормализует до `validate()`; прямой `validate()` строгий
   (`Invalid { field: "lastGuildId" }`); гильдия из READY/списка не подставляется.
3. **Повторный targeted-прогон** (единственный `cargo` в retry):
   `cd rust/gui-spike && cargo test --offline --test settings_contract`
   → exit **0**, `23 passed; 0 failed; 0 ignored`
   (`logs/green-verify.log`, md5 `8aec8a7ad5af229b678c51149806534e`).
   Red-доказательство прежнего запуска действительно: `logs/red.log` (exit 101,
   12 строк `error`) ссылается на строки 518/540/582/620/631 текущего
   `settings_contract.rs`, т.е. тесты были написаны до поля.
4. **UI не менялся:** `git status` по `rust/gui-spike` пуст; `ui/app.js:332`
   по-прежнему помечает restore как G-01b (потребитель — G2).
5. Live/attach/spawn, сеть, полный `gate:worker`/`tsc`/e2e не запускались; golden
   (`logs/`, `functions/`, `evidence/`) только читался.

**Итог retry:** контракт G-01b принят к ревью, дефектов не найдено, новых правок не
потребовалось. Остаток — `G-01b-UI` (G2, restore рейла из READY).

## Следующий шаг

Приёмка Team Lead (review + gate на batch) → `G-01b-RESTORE` (G2: UI-потребитель).
