# REPORT W33a — P1-10 Keyring через FFI к libsecret (headless)

Волна: **RW-33**. Batch: `BATCH-TZ.md`. Карточка: **P1-10** (seat SEC-1 / W33a).
Дата: 2026-10-22. Статус: **готово к приёмке** (red → green → мутации 7/7 → отчёт).

Окружение: `libsecret 0.21.8.2`, `org.freedesktop.secrets` = ksecretd (сессионная шина,
коллекция разблокирована), CLI-оракул `secret-tool`; `DISCORD_LAB_KEYRING_LIVE=1` для live.

## 1. Файлы и md5

| Файл | md5 |
|---|---|
| `rust/crates/discord-platform/src/keyring.rs` | `31793d97a750fd22facd13492db7397e` |
| `rust/crates/discord-platform/src/ffi_secret.rs` | `0c7c59c3fec6954b90c20e71bce17fc8` |
| `rust/crates/discord-platform/src/lib.rs` | `59c95a2d17060856365d7c5b47072660` |
| `rust/crates/discord-platform/tests/keyring_contract.rs` | `49e339786446173b9d71997826d4dbf4` |
| `waves/RW-33/run-mutations-w33a.py` | `df2dfe0c1afc1e3bcd48311c8a7dc4f2` |
| `waves/RW-33/workers/W33a/red.log` | `b42d25e6fbbf674deac66202ffe841f4` |
| `waves/RW-33/workers/W33a/green.log` | `b107dd4ad6157d68adbdbb4a2360a44c` |
| `waves/RW-33/mutations-w33a.log` | `d96e32e2bd8a5f9ffbe888f766a96001` |

`lib.rs`: добавлены только `pub mod keyring;`, `#[allow(unsafe_code)] mod ffi_secret;`
(3 строки). `Cargo.toml`/`Cargo.lock`, чужие модули/тесты, доки и golden не менялись.
`unsafe` — только в `ffi_secret.rs`; `unsafe_code = "deny"` действует, allow стоит на `mod`.

## 2. Red / green

- Red: `cargo test … --test keyring_contract` → **EXIT=101**, `E0432: unresolved import
  discord_platform::keyring` (`workers/W33a/red.log`). Тесты k01–k12 написаны до кода.
- Green 1/2 (`DISCORD_LAB_KEYRING_LIVE=1`, реальный ksecretd + `secret-tool`):
  **12 passed, 0 failed, SKIP=0**, **EXIT=0**.
- Green 2/2 (без live-флага): 12 passed, **EXIT=0**; k01–k08 печатают
  `SKIP: DISCORD_LAB_KEYRING_LIVE != 1`, k09–k12 выполняются.
- Live-прогон повторён 3 раза подряд на финальном коде: 12/12 зелёные (стабильность
  RSS-проверки k02 и отсутствие гонок на общей коллекции).

Команда: `cd rust && CARGO_TARGET_DIR=/tmp/opencode/target-w33a cargo test --offline
-p discord-platform --test keyring_contract`.

## 3. Кейсы (TZ §3)

| ID | Тест | Тип | Результат |
|---|---|---|---|
| k01 | `connect` к default-сервису | live | ok |
| k02 | store → lookup; серия 2000 lookup с 4 КиБ секретом без роста RSS > 4 МиБ (копия освобождается `g_free`) | live | ok |
| k03 | lookup отсутствующей записи → `Ok(None)` | live | ok |
| k04 | clear существующей → `true`, затем lookup `None` | live | ok |
| k05 | перезапись значения | live | ok |
| k06 | Unicode-секрет (кириллица/умлауты/эмодзи/Ω) дословно | live | ok |
| k07 | наш store → `secret-tool lookup key …` | live + CLI | ok |
| k08 | `secret-tool store` → наш lookup | live + CLI | ok |
| k09 | пустой ключ → `InvalidKey` (store и lookup) | без live-флага | ok |
| k10 | NUL в ключе → `InvalidKey`, NUL в секрете → `InvalidSecret`, без паники | без live-флага | ok |
| k11 | повторный clear отсутствующей → `Ok(false)` | без live-флага | ok |
| k12 | ручной `Debug`: есть имя схемы, нет секрета, нет `0x`-указателей; error Debug/Display без секрета | без live-флага (+store) | ok |

Итого 12 тестов (бюджет 10–12). Live-кейсы skip-aware; unit-кейсы от live-флага не зависят.

## 4. Мутации

`waves/RW-33/run-mutations-w33a.py`, targeted с `-- --nocapture`, baseline —
`exit=0, SKIP=0, test result=ok`. **7/7 KILLED**, md5 восстановлены, чужие файлы не тронуты:

| # | Мутация | Убита тестами |
|---|---|---|
| 01 | атрибут схемы `key` → `ke` | k07, k08 |
| 02 | `store` не пишет, возвращает `Ok(())` | k02, k04, k05, k06, k07, k11 |
| 03 | `clear` всегда `Ok(true)` | k04, k11 |
| 04 | NULL значения → `Err` вместо `Ok(None)` | k03, k04, k11 |
| 05 | убрано `g_free` копии `g_strndup` (утечка len+1 на lookup) | k02 (RSS 2000×4 КиБ) |
| 06 | `Debug` печатает адрес внутреннего объекта вместо ручной редакции | k01, k12 |
| 07 | пустой ключ больше не отвергается | k09 |

Мутация 03 (`clear` no-op) оставляет синтетические записи в коллекции; скрипт после каждого
прогона (и baseline) очищает их независимым `secret-tool clear xdg:schema
com.discordlab.Token`. После финального прогона `secret-tool search --all xdg:schema
com.discordlab.Token` пуст.

## 5. Решения и ограничения

- **Схема.** `secret_schema_newv("com.discordlab.Token", SECRET_SCHEMA_DONT_MATCH_NAME,
  {key: STRING})`; коллекция `NULL` (default), метка `discord-lab token`. `DONT_MATCH_NAME`
  выбран осознанно: без него libsecret добавляет в поиск `xdg:schema`, и записи `secret-tool`
  (схема `org.freedesktop.Secret.Generic`) не находятся — k08 из TZ §3 требовал бы провала.
  Имя схемы при этом сохраняется в записи (`xdg:schema = com.discordlab.Token`).
- **FFI.** Не-variadic `secret_service_store_sync`/`lookup_sync`/`clear_sync`,
  `secret_schema_newv`; `GError` читается и `g_error_free` на всех путях; значение копируется
  `g_strndup` → `String`/`Vec<u8>` → `g_free` до `secret_value_unref`; `secret_value_new` +
  `secret_value_unref` для записи; `g_hash_table_new_full` с `g_str_hash`/`g_str_equal`.
  `gio-2.0` линкуется по контракту (прямых вызовов gio нет, декларация помечена `allow(dead_code)`).
- **Владение.** `Service` (`g_object_unref`) и `Schema` (`secret_schema_unref`) освобождаются
  ровно один раз в `Drop` полей `Keyring`; сам `Keyring` `Drop` не реализует, чтобы не было
  двойного unref. `!Send + !Sync` — `PhantomData<*mut ()>`.
- **Ошибки.** Пустой/NUL-ключ и NUL-секрет отсекаются до C. `clear` отсутствующей записи у
  ksecretd — `FALSE` **без** `GError` → `Ok(false)`; `GError` → `Err(Secret { code, message })`.
  Невалидный UTF-8 из сервиса → `Encoding`. Секрет не попадает в `Debug`/`Display`/сообщения.
- **Unit-кейсы и сервис.** Замороженный API не содержит конструктора без сервиса, поэтому
  k09–k12 подключаются к ksecretd (без live-флага) и печатают `SKIP`, только если сервиса в
  окружении нет; это единственное расхождение с буквой TZ §2 «без шины» и оно зафиксировано.
- **Блокировка коллекции.** `*_sync` может ждать prompt разблокировки; live-кейсы запускаются
  только по `DISCORD_LAB_KEYRING_LIVE=1`. В текущем окружении ksecretd разблокирован, prompt не
  появлялся. На заблокированной коллекции возможен блокирующий вызов — это известный риск
  (headless без GUI-агента), в коде не обходится.
- **Секреты.** Только синтетические (`test-secret-…`, 4 КиБ `x` для RSS-кейса), ключи
  уникальны (`w33a-<tag>-<pid>-<seq>`), после кейсов выполняется `clear` (+ CLI-очистка).
- **Совместимость.** Wire-совместимость с крейтом `keyring` и миграция чужих записей не
  заявляются: свой атрибут `key`/схема; пароля/токена в файлах нет.
- **Порог RSS** (4 МиБ на 2000 lookup, замеренный baseline ~72 КиБ) зависит от аллокатора;
  при регрессе тест печатает before/after. Мутация 05 стабильно даёт ~8 МиБ.

## 6. Проверки

- `cargo fmt --check -p discord-platform` — чисто.
- `cargo clippy --offline -p discord-platform --all-targets -- -D warnings` — 0 предупреждений.
- `cargo test --offline -p discord-platform --no-run` — все тесты крейта компилируются.
- Полный `gate:worker`/workspace-тесты не запускались: batch-воркер делает только targeted,
  общий гейт — один раз в integration-лейне (BATCH-TZ §4).

## После codex-ревью (2026-10-22)

- **P2** (`put_string`): при ошибке второго `g_strdup` ключ освобождается `g_free` — утечки
  на пути ошибки больше нет.
- **P1** (live не подтверждён в sandbox codex): live-прогон на реальной шине выполнен
  (`DISCORD_LAB_KEYRING_LIVE=1`, `ksecretd` + `secret-tool`) — **12 passed, SKIP=0**;
  независимая проверка — отдельным агентом.
- Cleanup в мутационном прогоне: `secret-tool clear` с exit 1 трактуется как «нет записей»
  (нормальная очистка); лог переснят — **7/7, SKIP=0**, restore по md5.
- red/green и md5 пересняты на финальных байтах.

## После независимого ревью (P1-1/P1-2/P2-1)

- **P1-1** (обрезка значения на встроенном NUL): lookup копирует ровно `length` байт из
  `secret_value_get` (`slice::from_raw_parts`), без `g_strndup`; бинарные значения не портятся.
- **P1-2** (коллизия чужих записей): схема создаётся без `SECRET_SCHEMA_DONT_MATCH_NAME`,
  поэтому поиск/удаление совпадают по `xdg:schema` + `key`; CLI-оракул k07/k08 передаёт
  `xdg:schema com.discordlab.Token` явно. Чужая запись без схемы больше не находится и не
  удаляется нашим `clear`.
- **P2-1** (тихий skip при `LIVE=1`): при явном live-флаге ошибка `connect` теперь падает
  тестом, а не уходит в `SKIP`; проверено с битым `DBUS_SESSION_BUS_ADDRESS`.
- Мутация 05 переориентирована на новую копию (`length-1`); прогон **7/7, baseline SKIP=0**,
  red — `E0432` (модуль скрыт), green — 12 passed, `SKIP: 0`.
- md5 обновлены (таблица выше).
