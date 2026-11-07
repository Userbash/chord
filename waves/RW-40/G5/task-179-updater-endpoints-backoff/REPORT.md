# Отчёт воркера: task-179 / BE-P3-07 — эндпоинты апдейтера, scheduled backoff, состояния сплэша

## batch/карточка

- task-179 (`BE-P3-07`), волна RW-40, отдел G5 Platform/Updater.
- Оракул: `/home/sanya/re-deep/main-arch.md` §5.1–§5.4 (строки 525–595 фактического файла):
  `app #19192 @3231` (`NEW_UPDATE_ENDPOINT`/`UPDATE_ENDPOINT`), `app #55405 @26169`
  (`disableUpdater`), `app #53006 @97270..@99200` (`updateUntilCurrent`, `ScheduledBackoff`,
  `Math.round(delayMs / 1e3)`), `app #2278` (Linux `update-manually`),
  `app #47924 @61321/@62552` (`modules/{channel}`, feed) и `app/splashScreenPreload.js`
  (`LINUX_DOWNLOAD_URL_BASE`, `deb`/`tar.gz`).

## сделано

- `rust/crates/discord-updater/src/lib.rs` (+240 строк):
  - константы `NEW_UPDATE_ENDPOINT`, `DEFAULT_UPDATE_ENDPOINT`, `DEFAULT_UPDATE_BACKOFF_MAX_MS`,
    `SCHEDULED_UPDATE_BACKOFF_MS` (9 значений `2e3..15e3`);
  - `ReleaseChannel` (`as_str`) и `should_disable_updater(channel, disable_flag, standalone_or_local_modules)
    = disable_flag || (Development && standalone_or_local_modules)`;
  - `LinuxPackageFormat` (`as_query_value`: `deb`, `tar.gz`);
  - `build_linux_download_url` → `https://discord.com/api/download/{channel}?platform=linux&format=…`;
  - `build_linux_feed_url(endpoint, channel, &Version)` → `{base}/updates/{channel}?platform=linux&version={major}.{minor}.{patch}`
    (pre-release суффикс отбрасывается, хвостовые `/` и пробелы нормализуются);
  - `build_modules_base_url(endpoint, channel)` → `{base}/modules/{channel}`;
  - приватный `endpoint_base`: пустой эндпоинт, отсутствие схемы `http(s)://`, пробелы внутри,
    `?`/`#` и «голый» `https://` ⇒ `Err(ManifestError::InvalidEndpoint)`;
  - `ScheduledUpdateBackoff` (`new`/`default` с лимитом `30_000`, `with_max_ms`, `failures`,
    `next_delay_ms` — шаг по массиву с clamp на последнем значении и по `max_ms`,
    `retry_seconds` — `Math.round(delayMs / 1e3)`, `succeed` — сброс счётчика);
  - `SplashUpdateState` с `event_name()`: `CHECKING_FOR_UPDATES`, `DOWNLOADING_UPDATES`,
    `INSTALLING_UPDATES`, `UPDATE_FAILURE`, `update-manually`, `LAUNCHING`.
- `rust/crates/discord-updater/src/manifest.rs` (+6 строк): вариант
  `ManifestError::InvalidEndpoint { value: String }` — единственное расширение API вне `lib.rs`,
  файл входит в разрешённый write-scope; других совпадений по `ManifestError` в воркспейсе нет,
  крейт не имеет зависимых, поэтому аддитивное изменение ничего не ломает.
- `rust/crates/discord-updater/tests/updater_contract.rs` (+272 строки): 12 новых контрактных
  тестов на константы, каналы, флаг отключения, формат пакета, три URL-билдера (включая
  негативные эндпоинты), расписание backoff, `max_ms`-clamp, округление секунд и имена событий,
  плюс проверка сохранения payload в `UpdateFailure`/`UpdateManually`.

## red

```
CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target cargo test --manifest-path rust/Cargo.toml \
  -p discord-updater --test updater_contract --lib
```
→ `E0432: no \`NEW_UPDATE_ENDPOINT\`/\`ReleaseChannel\`/\`ScheduledUpdateBackoff\`/… in the root`,
`E0599: no variant named \`InvalidEndpoint\` found for enum \`ManifestError\``,
`error: could not compile \`discord-updater\` (test "updater_contract") due to 5 previous errors`,
**exit 101**. Лог: `red.log` (снят до реализации, при уже добавленных тестах).

## green

```
CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target cargo test --manifest-path rust/Cargo.toml \
  -p discord-updater --test updater_contract --lib
```
→ `running 46 tests`, `test result: ok. 46 passed; 0 failed; 0 ignored`, **exit 0**. Лог: `green.log`
(там же `--all-targets` и финальный прогон после восстановления мутантов).

```
CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target cargo clippy --manifest-path rust/Cargo.toml \
  -p discord-updater --test updater_contract --lib -- -D warnings
```
→ `Finished \`dev\` profile`, **exit 0**. Лог: `clippy.log`.

```
rustfmt --edition 2021 --check rust/crates/discord-updater/src/lib.rs \
  rust/crates/discord-updater/src/manifest.rs rust/crates/discord-updater/tests/updater_contract.rs
```
→ **exit 1**, но не из-за форматирования: парсер 2021 не понимает let-chain
`if let Some(pre) = pre && pre.is_empty()` в `manifest.rs:58` (код карточки P8-01, edition 2024 из
`rust/Cargo.toml`). Канонический для крейта прогон `rustfmt --edition 2024 --check` по трём файлам
и `cargo fmt -p discord-updater -- --check` → **exit 0**. Лог: `fmt.log` (все три команды).

## targeted

- `updater_contract`: 46 тестов, из них 12 новых по task-179; 0 skip, 0 ignored.
- Новые тесты: `updater_endpoint_constants_k`, `release_channel_as_str_k`,
  `should_disable_updater_k`, `linux_package_format_query_value_k`, `linux_download_url_k`,
  `linux_feed_url_k`, `url_builders_reject_bad_endpoint_k`, `modules_base_url_k`,
  `scheduled_backoff_sequence_k`, `scheduled_backoff_clamps_to_max_k`,
  `scheduled_backoff_retry_seconds_k`, `splash_update_state_event_names_k`.

## oracle

- `/home/sanya/re-deep/main-arch.md` §5.1–§5.4 — единственный источник значений: эндпоинты,
  точный массив `ScheduledBackoff`, формула `disableUpdater`, шаблоны Linux-URL, имя
  `update-manually` в нижнем регистре (единственное такое среди событий).
- Формула секунд сверена с JS-семантикой: `Math.round(2500 / 1000) = 3` зафиксировано тестом на
  `with_max_ms(2_500)`, чтобы отличать округление от усечения.
- Тесты сверяют литералы, а не реализацию: константы и строки выписаны из документа дословно.

## генерация

Не применимо: артефактов-генераторов нет, входа два (документ + версия в тесте), хеши не сравниваются.

## мутации

5/5 KILLED при зелёном baseline (`mutation.log`):

| Мутант | Изменение | Убит тестом |
| --- | --- | --- |
| M1 | первая задержка `2_000` → `2_500` | `updater_endpoint_constants_k`, `scheduled_backoff_sequence_k`, `scheduled_backoff_retry_seconds_k`, `scheduled_backoff_clamps_to_max_k` |
| M2 | убран `disable_flag ||` из `should_disable_updater` | `should_disable_updater_k` |
| M3 | убран clamp `.min(self.max_ms)` | `scheduled_backoff_clamps_to_max_k` |
| M4 | `update-manually` → `UPDATE_MANUALLY` | `splash_update_state_event_names_k` |
| M5 | убран `trim_end_matches('/')` у эндпоинта | `linux_feed_url_k`, `modules_base_url_k`, `url_builders_reject_bad_endpoint_k` |

## baseline/restore

- Baseline: `cargo test -p discord-updater --test updater_contract` → 46 passed, exit 0;
  `lib.rs` md5 `201b0e576a62032adfb331a12c6c18c2`.
- После каждого мутанта файл восстанавливался из копии; финальный `lib.rs` md5
  `201b0e576a62032adfb331a12c6c18c2` совпал с baseline, повторный прогон после restore —
  46 passed, clippy и rustfmt(2024) чисто.

## стабильность

Не требовалось: тестов с сетью, потоками, таймерами и файловыми гонками в этой карточке нет
(новые тесты — чистые функции).

## gate:worker

Не запускался: по процессу RW-40 воркер в batch делает только targeted-прогон, полный
`gate:worker` — один раз на batch в integration-лейне. Targeted: тесты + clippy + rustfmt(2024).

## файлы

| Файл | md5 |
| --- | --- |
| `rust/crates/discord-updater/src/lib.rs` | `201b0e576a62032adfb331a12c6c18c2` |
| `rust/crates/discord-updater/src/manifest.rs` | `17ae70c360f0ab6f5b37cfce734ba595` |
| `rust/crates/discord-updater/tests/updater_contract.rs` | `11fda0ad490300d0660a0df91c288608` |

`plan.rs` не тронут (`2088cc72b4e46aa83f3adf437a1e9113`). Вне write-scope изменений нет;
`package-lock.json` был изменён до начала работы (унаследован от ветки, не мной).
Git-коммиты не делались.

## риски/вопросы

- `ManifestError::InvalidEndpoint` — новый публичный вариант ошибки. Если у ревьюера есть
  предпочтение держать URL-валидацию вне `manifest.rs` (например, отдельный `UpdateEndpointError`),
  это меняется одной правкой; сейчас выбран минимальный путь в разрешённом write-scope.
- `retry_seconds` и `next_delay_ms` оба регистрируют неудачу (шаг расписания), как `fail()` в JS:
  вызывающий код обязан использовать что-то одно на попытку, иначе расписание «убежит» вперёд.
- `endpoint_base` принимает только `http://`/`https://` в нижнем регистре и отвергает `?`/`#`;
  если понадобится путь с query (например, прокси-эндпоинт), правило придётся расширять осознанно.
- `rustfmt --edition 2021` из ТЗ неприменим к крейту на edition 2024 — это ограничение парсера,
  а не дефект форматирования (детали в `fmt.log`).

## следующий шаг

Передать Team Lead: ревью границ файлов (3 файла), приёмка контракта по `main-arch.md` §5,
затем QA/Technical Review (мутации 5/5 уже приложены).
