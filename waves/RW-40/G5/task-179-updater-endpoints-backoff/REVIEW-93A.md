# Техническое ревью 93A — PR-64 / task-179 (BE-P3-07: эндпоинты апдейтера, `ScheduledUpdateBackoff`, `SplashUpdateState`)

- **Ревьюер:** Технический ревьюер, Review 93A (`deepseek/deepseek-v4.1-flash`)
- **Дата прогонов:** 2026-10-31 00:29–00:40 (+05:00), rustc/cargo 1.99.0
- **Ветка:** `feature/task-179-updater-endpoints-backoff`, HEAD `b8183f5` (`unstable` = `f331110`, впереди на 1 коммит PR-63)
- **PR:** PR-64 (`feature/task-179-updater-endpoints-backoff -> unstable`)
- **PM-приёмка:** `ACCEPT.md`, статус `Ready for Technical Review`

## Вердикт: **CHANGES REQUESTED**

- **P0 = 0**
- **P1 = 1** — `SplashUpdateState::event_name()` возвращает имена констант вместо значений: 5 из 6
  состояний расходятся с оракулом, тест закрепляет неверные литералы.
- **P2 = 4, P3 = 2** — по отдельности не блокируют, но P2-1/P2-2 рекомендуется закрыть в этом же PR.
- **Мутации: 5/5 KILLED** при зелёном baseline; после восстановления md5 `lib.rs` совпал с baseline.
- Тесты: `46/46` в `updater_contract` (12 новых), lib — 0; clippy `-D warnings` и
  `rustfmt --edition 2024 --check` чисто.

## 1. Границы изменений

`git status --porcelain` в рабочем дереве:

```
 M package-lock.json
 M rust/crates/discord-updater/src/lib.rs
 M rust/crates/discord-updater/src/manifest.rs
 M rust/crates/discord-updater/tests/updater_contract.rs
```

Три файла из write-scope ТЗ изменены ровно на `+240 / +6 / +272` строк. `package-lock.json` — унаследованный
фон: mtime `00:28:58`, то есть **до** первой правки воркера (`lib.rs` — `00:31:10`), содержимое — поля `bin`
и `engines`, которые уже есть в `package.json` на HEAD, к апдейтеру отношения не имеют (см. P2-4: файл нельзя
включать в коммит PR-64).

md5 совпадают с §файлы `REPORT.md` и не изменились после мутационных прогонов:

| файл | md5 (REPORT.md) | md5 (после ревью) |
| --- | --- | --- |
| `rust/crates/discord-updater/src/lib.rs` | `201b0e576a62032adfb331a12c6c18c2` | `201b0e576a62032adfb331a12c6c18c2` |
| `rust/crates/discord-updater/src/manifest.rs` | `17ae70c360f0ab6f5b37cfce734ba595` | `17ae70c360f0ab6f5b37cfce734ba595` |
| `rust/crates/discord-updater/tests/updater_contract.rs` | `11fda0ad490300d0660a0df91c288608` | `11fda0ad490300d0660a0df91c288608` |

Коммитов воркер не делал, `plan.rs` не тронут. `ManifestError::InvalidEndpoint` — аддитивный вариант;
исчерпывающих `match` по `ManifestError` вне крейта нет (`grep -rn ManifestError --include='*.rs' rust/`
даёт совпадения только внутри `discord-updater`), зависимых крейтов у `discord-updater` нет.

## 2. Прогоны

Команды из ТЗ на общем таргете `CARGO_TARGET_DIR=/home/user/orca/project/rust/target`:

```
cargo test   -p discord-updater --test updater_contract --lib
  -> unittests src/lib.rs: 0 tests
  -> tests/updater_contract.rs: 46 passed; 0 failed; 0 ignored   exit 0

cargo clippy -p discord-updater --test updater_contract --lib -- -D warnings
  -> Finished `dev` profile, 0 warnings                         exit 0

rustfmt --edition 2024 --check <три файла>
  -> exit 0
```

Из-за известной проблемы общего таргета (одинаковые `name+version` в разных worktree дают один unit hash,
см. REVIEW-88A §6.1) прогон повторён на изолированном таргете `rust/.target-review93a` (удалён после
проверки): те же `46 passed`, clippy `exit 0`, rustfmt `exit 0`, md5 файлов не изменились. Это не чужой
артефакт.

Red-first: `red.log` содержит `E0432` на 12 новых импортов и `E0599` на `ManifestError::InvalidEndpoint`,
суммарно 5 ошибок, `exit 101` — падение из-за отсутствия реализации. Замечание к составу лога — P3-1.

## 3. Мутационное тестирование: 5/5 KILLED

Каждая мутация вносилась в `lib.rs` точечной заменой, прогонялся
`cargo test -p discord-updater --test updater_contract`, файл восстанавливался из бэкапа.
Baseline и restore: md5 `201b0e576a62032adfb331a12c6c18c2`; после restore — `46 passed`, clippy/fmt чисто.

| # | Мутация | Итог | Убита тестом |
| --- | --- | --- | --- |
| M1 | первый шаг расписания `2_000` → `2_500` | KILLED, 4 failed | `updater_endpoint_constants_k`, `scheduled_backoff_sequence_k`, `scheduled_backoff_retry_seconds_k`, `scheduled_backoff_clamps_to_max_k` |
| M2 | убран `disable_flag \|\|` | KILLED, 1 failed | `should_disable_updater_k` |
| M3 | убран `.min(self.max_ms)` | KILLED, 1 failed | `scheduled_backoff_clamps_to_max_k` |
| M4 | `update-manually` → `UPDATE_MANUALLY` | KILLED, 1 failed | `splash_update_state_event_names_k` |
| M5 | убран `trim_end_matches('/')` | KILLED, 3 failed | `linux_feed_url_k`, `modules_base_url_k`, `url_builders_reject_bad_endpoint_k` |

Выживших нет. Важно: 5/5 подтверждает чувствительность тестов, но **не** правильность литералов — M4
убивается и после исправления P1, потому что `update-manually` останется значением одного из состояний.

## 4. Сверка с оракулом

Источник: `reference-oracle/app-asar/bundle.js`, `splashScreenPreload.js`, `main-arch.md` §5.1–§5.4.

| Контракт | Оракул | Код | Итог |
| --- | --- | --- | --- |
| `NEW_UPDATE_ENDPOINT` | `https://updates.discord.com/` | то же | OK |
| `UPDATE_ENDPOINT` | `https://discord.com/api` | `DEFAULT_UPDATE_ENDPOINT` | OK |
| каналы | `stable` / `ptb` / `canary` / `development` | то же | OK |
| `disableUpdater` | `disableUpdater \|\| ("development"===releaseChannel && (standaloneModules \|\| localModulesRoot))` | та же формула, два флага слиты в один bool | OK |
| Linux download | `https://discord.com/api/download/<channel>?platform=linux&format=<deb\|tar.gz>` | то же | OK |
| feed | `${_endpoint}/updates/${releaseChannel}?platform=linux&version=${_buildInfo.version}` (`bundle.js@62493`) | версия только `major.minor.patch` | **P2-3** |
| modules | `<endpoint>/modules/<channel>` | то же | OK |
| расписание | `[2e3,3e3,3e3,4e3,5e3,7e3,1e4,12e3,15e3]` (`@97361`) | тот же массив, шаг с clamp на последнем | OK |
| jitter | `new ScheduledBackoff([...], !0)` → `jitter=true`, `Math.round(step*(0.85+0.3*Math.random()))` (`@108995`, `@109525`) | джиттера нет | **P2-1** |
| предел задержки | `updateBackoffMaxMs` уходит в `Backoff(1e3, max)`; у `ScheduledBackoff` конструктор только `(steps, jitter)` | `ScheduledUpdateBackoff` усекает по `max_ms` | **P2-2** |
| статусы сплэша | `checking-for-updates`, `downloading-updates`, `installing-updates`, `update-failure`, `update-manually`, `launching` | имена констант в верхнем регистре | **P1-1** |

## 5. Находки

### P0 — нет

### P1-1. `SplashUpdateState::event_name()` возвращает имена констант, а не их значения

`lib.rs:242-254`:

```rust
Self::CheckingForUpdates => "CHECKING_FOR_UPDATES",
Self::DownloadingUpdates => "DOWNLOADING_UPDATES",
Self::InstallingUpdates => "INSTALLING_UPDATES",
Self::UpdateFailure { .. } => "UPDATE_FAILURE",
Self::UpdateManually { .. } => "update-manually",
Self::Launching => "LAUNCHING",
```

В бандле имя константы и её значение — разные вещи, и в IPC уходит **значение**:

- `bundle.js@95704` и `@95794..@96164`: `CHECKING_FOR_UPDATES="checking-for-updates"`,
  `UPDATE_FAILURE="update-failure"`, `LAUNCHING="launching"`, `DOWNLOADING_UPDATES="downloading-updates"`,
  `INSTALLING_UPDATES="installing-updates"`; `@66218`: `exports.UPDATE_MANUALLY="update-manually"`.
- `bundle.js@100581`: `webContentsSend(splashWindow,"SPLASH_UPDATE_STATE",{status:event,...splashState})` —
  в `status` попадает значение без преобразования.
- `splashScreenPreload.js`: `switch` по `state.status` с ветками `case"downloading-updates"` (`@2279`),
  `case"update-failure"` (`@2395`), `case"launching"` (`@2495`); остальное уходит в
  `default: return "Checking for updates…"`.

Практическое следствие: сплэш получит `"UPDATE_FAILURE"`/`"LAUNCHING"` и не покажет ни таймер повтора, ни
«Starting…»; работает только `update-manually`. Контракт IPC неверен для 5 состояний из 6, а тест
`splash_update_state_event_names_k` (`updater_contract.rs:1134-1160`) закрепляет ошибку литералами.
Комментарий `lib.rs:243-244` («`update-manually` — единственное имя в нижнем регистре») тоже неверен:
в нижнем регистре через дефис записаны **все** значения.

Точный фикс (в пределах write-scope):

```rust
Self::CheckingForUpdates => "checking-for-updates",
Self::DownloadingUpdates => "downloading-updates",
Self::InstallingUpdates => "installing-updates",
Self::UpdateFailure { .. } => "update-failure",
Self::UpdateManually { .. } => "update-manually",
Self::Launching => "launching",
```

и те же шесть литералов в `splash_update_state_event_names_k`; doc-комментарий переписать без ложного
утверждения про «единственное имя».

### P2-1. Нет джиттера у расписания

Реальный вызов — `new ScheduledBackoff([...], !0)` (`bundle.js@97361`), то есть `jitter=true`; `fail()`
делает `delay = Math.round(delay*(0.85+0.3*Math.random()))` (`@109525`). Rust-модель детерминирована:
при шаге 15 с реальные паузы 13–17 с, а `retry_seconds()` всегда вернёт 15. Для апдейтера это механизм,
который разводит клиентов по времени. Рекомендация: инъецируемый джиттер (`next_delay_ms_with(rand: f64)`
или `with_jitter`) и тест на границы `[0.85, 1.15]`; точные проверки оставить для режима без джиттера.

### P2-2. `max_ms` приписан `ScheduledBackoff`, хотя в оракуле он относится к `Backoff`

`updateBackoffMaxMs` читается из `DESKTOP_TTI_UPDATE_BACKOFF_MAX_MS` и передаётся в `Backoff(1e3, max)`
(ветка `ALGO=0`, `main-arch.md` §5.2). У `ScheduledBackoff` конструктор только `(steps, jitter)`
(`bundle.js@108995`) — верхнего предела у него нет. Rust-структура объединяет оба алгоритма: усекает шаг
по `max_ms`, а тест `with_max_ms(0) → 0` закрепляет поведение, которого в бандле нет. При дефолте
`30_000` усечение инертно (максимальный шаг 15 000), но при настройке `max < 15 000` поведение разойдётся.
Рекомендация: убрать clamp и `max_ms` из `ScheduledUpdateBackoff` либо явно описать это как расширение,
не ссылаясь на `updateBackoffMaxMs`.

### P2-3. Feed-URL теряет pre-release суффикс версии

Оракул подставляет `${_buildInfo.version}` как есть (`bundle.js@62493`). `build_linux_feed_url` печатает
только `major.minor.patch` (`lib.rs:143-145`), а тест `linux_feed_url_k` фиксирует
`1.2.3-beta.1 → version=1.2.3`. Для stable/ptb/canary версии обычно числовые, поэтому практический эффект
низкий, но `manifest::Version` хранит `pre`, и молчаливая потеря поля — расхождение с оракулом.
Рекомендация: форматировать версию целиком (Display/`to_string` с `pre`) либо явно задокументировать
допущение и убрать «догадочный» тест-кейс.

### P2-4. `package-lock.json` вне write-scope

Файл изменён в рабочем дереве, но не воркером (mtime раньше правок, содержимое — `bin`/`engines` из
`package.json` на HEAD). В коммит PR-64 его включать нельзя: интегратору нужно `git checkout -- package-lock.json`
или отдельный коммит. На код апдейтера не влияет.

### P3-1. `red.log` содержит две синтаксические ошибки теста

Помимо `E0432`/`E0599` лог начинается с `error: struct literals are not allowed here`
(`updater_contract.rs:1160`, `:1164`) — тест пришлось чинить по пути. Red остаётся настоящим (5 ошибок,
`exit 101`), но `REPORT.md` §red перечисляет только ошибки отсутствующей реализации. Рекомендация:
перезаписать red после исправления синтаксиса теста и до реализации, чтобы артефакт показывал ровно
отсутствие символов.

### P3-2. Два мутирующих аксессора backoff

`next_delay_ms()` и `retry_seconds()` оба увеличивают `failures`; вызов обоих на одну попытку перескочит
шаг расписания. В отчёте это отмечено, тест на сброс есть, но API провоцирует ошибку. Рекомендация:
разделить `record_failure()` и `current_delay_ms()` либо явно зафиксировать контракт в doc-комментарии.

## 6. Что проверено и признано корректным

- Константы эндпоинтов, каналы, формула `disableUpdater`, шаблоны download/feed/modules, массив
  расписания и `Math.round(delayMs/1e3)` совпадают с §5 оракула (кроме пунктов выше).
- `ManifestError::InvalidEndpoint` аддитивен и не ломает исчерпывающие `match` — зависимых крейтов нет.
- `endpoint_base` корректно режет хвостовые слэши, отвергает пустой хост, пробелы, `?`, `#`; срез по
  байтам безопасен, потому что отрезаются только ASCII-слэши.
- Тесты сверяют литералы, а не реализацию; 12 новых тестов покрывают позитивные, негативные и граничные
  случаи (пустой эндпоинт, `max_ms=0`, pre-release версия).

## 7. Что осталось

1. Исправить P1-1 (6 значений `event_name` + литералы теста + doc) — обязательное условие merge.
2. По возможности закрыть P2-1/P2-2 в этом же PR; P2-3 — либо фикс, либо явное допущение в доке.
3. Перепрогнать targeted (`updater_contract` + clippy + rustfmt 2024), прислать Review 93A-fix.
4. `package-lock.json` не включать в коммит PR-64.
5. Merge в `unstable` делает координатор/интегратор после повторного ревью; ветка отстаёт от `unstable`
   на PR-63 (`f331110`) — перебазирование на интеграторе.

Полный `gate:worker` не запускался: по ТЗ ревью ограничено targeted-прогоном `discord-updater` + clippy.
