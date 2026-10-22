# REPORT W31a — P6-06 Питание и idle (headless)

Батч: RW-31, карточка P6-06, seat PLATFORM-1 / W31a. Дата: 2026-10-22.
Репозиторий: `discord-lab`, крейт `discord-platform`. Сеть не использовалась, `unsafe` нет,
новых зависимостей нет (только `std` + уже принятый `crate::dbus` из P6-02).

## 1. Что сделано

- `power::{ScreenSaver, PowerMonitor, PowerEvent, PresenceStatus, IdlePolicy}` по
  замороженному контракту `TZ-W31a.md` §1.
  - `ScreenSaver`: `connect`, `inhibit(app, reason) -> cookie`, `uninhibit(cookie)`,
    `is_active`, `idle_seconds` (`GetSessionIdleTime`, KDE).
  - `PowerMonitor`: `connect`, `subscribe` (`AddMatch PrepareForSleep`), `next_event(timeout)`.
  - `IdlePolicy::DEFAULT` = 600/1800 с; `status` — только сравнения `u64`, без арифметики.
- `src/lib.rs` — ровно одна строка `pub mod power;` после `pub mod notifications;`
  (файл не переформатировался).
- `tests/power_contract.rs` — 12 тестов p01–p12 на приватном `dbus-daemon` + libdbus-моке.
- `waves/RW-31/power/mock_power.py` — мок на `python3-dbus`/libdbus: два well-known имени
  (`org.freedesktop.ScreenSaver`, `org.freedesktop.login1`), журнал вызовов TSV и
  управляющий интерфейс `lab.Mock.Power`.
- `fixtures/dbus/power.json` — оракульные адреса/сигнатуры/формы ответов libdbus-сервиса;
  используется в p02 (ScreenSaver) и p06 (login1).
- `waves/RW-31/run-mutations-w31a.py` — 7 мутаций с restore по md5.

## 2. Оракул

Независимый от Rust: сервисы и транспорт — python3-dbus (libdbus) на приватном
`dbus-daemon --session --fork`. Мок журналирует каждый вызов, умеет:

- `Inhibit` → инкрементный cookie (старт 41), `UnInhibit` и алиас `Uninhibit`;
- неизвестный cookie → D-Bus-ошибка `org.freedesktop.ScreenSaver.Error.InvalidCookie`;
- `--force-uninhibit-alias` → `UnInhibit` отвечает `UnknownMethod` (проверка fallback);
- `IsActive` c переключением через `SetActive`;
- `GetSessionIdleTime` в режимах `value` / `unknown-method` / `unknown-interface` / `error`;
- `PrepareForSleep(b)` и «чужие» сигналы (другой интерфейс + `PrepareForSleep` с телом `s`).

Мок и фикстура ничего не пишут в golden-каталоги; live Discord не запускался.

## 3. Red / green

Red (до реализации, модуля `power` нет):

```
cd rust && CARGO_TARGET_DIR=/tmp/opencode/target-w31a \
  cargo test --offline -p discord-platform --test power_contract
error[E0432]: unresolved import `discord_platform::power`
...
# EXIT=101
```

Лог: `waves/RW-31/workers/W31a/red.log` (md5 `b737cdb1d1b0868cac59f4549bb82778`).

Green (предписанный target-dir, предупреждений нет):

```
running 12 tests
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
# EXIT=0
```

Лог: `waves/RW-31/workers/W31a/green.log` (md5 `6f0e733068faf08f58751a13f020d707`).

## 4. Тесты

| ID | Тест | Итог |
|---|---|---|
| p01 | `p01_idle_policy_boundaries` — 0/599/600/601/1799/1800, нулевые и равные пороги, `u64::MAX` | ok |
| p02 | `p02_inhibit_returns_cookie_and_mock_sees_fields` — cookie 41, app/reason в журнале, сверка с `power.json` | ok |
| p03 | `p03_uninhibit_reaches_mock_and_unknown_cookie_is_remote_error` — `UnInhibit`, `InvalidCookie` → `Remote`, fallback `Uninhibit` | ok |
| p04 | `p04_is_active_reflects_mock_state` — true → `SetActive(false)` → false | ok |
| p05 | `p05_idle_seconds_handles_unsupported_and_failing_services` — `Some(1234)`, `UnknownMethod` → `None`, `UnknownInterface` → `None`, прочая ошибка → `Err` | ok |
| p06 | `p06_prepare_for_sleep_maps_to_events` — true/false, сверка сигнала с `power.json` | ok |
| p07 | `p07_foreign_signals_are_skipped` — чужой интерфейс и тело `s` пропущены, затем WakingUp/GoingToSleep | ok |
| p08 | `p08_next_event_timeout_is_none` — 150 мс → `Ok(None)` | ok |
| p09 | `p09_inhibit_requires_app_name_and_reason` — пустые поля → `Protocol`, на шину ничего не уходит | ok |
| p10 | `p10_closed_connection_returns_error_without_panic` — убитый демон → `DbusError`/`None`, без паники | ok |
| p11 | `p11_live_service_is_flag_gated_and_read_only` | skip: `live-питание выключено (DISCORD_LAB_LIVE_POWER != 1)` |
| p12 | `p12_missing_environment_is_skip_with_reason` | ok (окружение готово; ветка skip проверена пустым PATH) |

В этом окружении `dbus-daemon` и `python3-dbus`/`gi` есть, поэтому skip только у p11 —
она гейтится флагом и в live-режиме тоже только читает `IsActive`/`GetSessionIdleTime`,
не вызывая `Inhibit` и не гася экран. Отсутствие `dbus-daemon`/`python3-dbus` даёт
`SKIP: <причина>` через макрос `harness!`/`environment_ready` (не падение).

## 5. Мутации

`CARGO_TARGET_DIR=/tmp/opencode/target-w31a python3 waves/RW-31/run-mutations-w31a.py`
→ exit 0. Baseline: exit=0, SKIP=0. Лог: `waves/RW-31/mutations-w31a.log`
(md5 `7a089b0dbc687069b288ae9f3b0bca83`).

| # | Мутация | Убита тестом |
|---|---|---|
| 01 | граница `idle_after` (`>=` → `>`) | p01 |
| 02 | флаг sleep перевёрнут | p06, p07 |
| 03 | таймаут → `Err` вместо `Ok(None)` | p08 |
| 04 | `UnknownMethod`/`UnknownInterface` больше не `Ok(None)` | p03, p05 |
| 05 | cookie +1 | p02, p03 |
| 06 | `PrepareForSleep` с чужим телом принимается за событие | p07 |
| 07 | удалён fallback `UnInhibit` → `Uninhibit` | p03 |

Все 7 мутаций `KILLED` при `test result: FAILED` и `SKIP=0`; после каждой restore,
md5 совпал. Итоговый md5 `src/power.rs` — `fe15c0d192234de71dea7c0715f58469`.

## 6. Файлы и md5

| Файл | md5 |
|---|---|
| `rust/crates/discord-platform/src/power.rs` | `fe15c0d192234de71dea7c0715f58469` |
| `rust/crates/discord-platform/src/lib.rs` | `28f7cf155af907ce64dc9eb381b413ea` |
| `rust/crates/discord-platform/tests/power_contract.rs` | `0a28edfaef3254eea2fe46577b9de68a` |
| `waves/RW-31/power/mock_power.py` | `b5764c2b0ca0266f148b06f45559a192` |
| `fixtures/dbus/power.json` | `eb49fc2b08429b75bb9a9513cba847a9` |
| `waves/RW-31/run-mutations-w31a.py` | `2bf5b35d2be766a05c5621f4c43610bb` |
| `waves/RW-31/workers/W31a/red.log` | `b737cdb1d1b0868cac59f4549bb82778` |
| `waves/RW-31/workers/W31a/green.log` | `6f0e733068faf08f58751a13f020d707` |
| `waves/RW-31/mutations-w31a.log` | `73e73f386a24f4636eb0e569d895b09c` |

`dbus/`, `notifications.rs`, `discord-core`, `discord-voice`, `discord-i18n`, чужие тесты,
`Cargo.toml`/`Cargo.lock`, доки и golden не изменялись.

## 7. Ограничения и заметки

- **Пороги idle** — конфигурация (`IdlePolicy`), не RE-факт: точные значения нативного
  клиента в RE не зафиксированы; DEFAULT 600/1800 помечен как продуктовое значение.
- **fd-ингибитор сна** (`Inhibit("sleep", …)` → fd через SCM_RIGHTS) не реализован —
  клиент `dbus` не поддерживает unix-fd; вынесено в бэклог. В этой карточке только
  `PrepareForSleep` как push-событие.
- **Live не гасит экран**: p11 только читает `IsActive`/`GetSessionIdleTime` по флагу
  `DISCORD_LAB_LIVE_POWER=1`; `Inhibit` на живой сессии не вызывается.
- **Имя метода**: каноническое `UnInhibit` (freedesktop/KDE); мок дополнительно отдаёт
  `Uninhibit`, клиент пробует его только при `UnknownMethod`/`UnknownInterface`.
  Расхождение зафиксировано в коде и фикстуре.
- **Типы**: `GetSessionIdleTime` отдаёт `u` (секунды); контракт возвращает `u64`,
  расширение без потерь.
- Wayland `ext-idle-notify-v1` и X11 `XScreenSaver` не реализуются (нет клиентских
  библиотек); `GetSessionIdleTime` — единственный headless-путь к idle.
- Соединение не переиспользуется между `ScreenSaver` и `PowerMonitor`: у них разные шины
  (сессионная/системная) в реальном окружении.

## После ревью codex (2026-10-22)

Codex не смог независимо подтвердить прогон: его sandbox не даёт `dbus-daemon` забиндить
приватный unix-сокет, а без `--nocapture` строки `SKIP:` скрывались — из-за этого раннер мог
ложно засчитать прогон. Исправлено:

- мутационный раннер запускает тесты с `-- --nocapture` и считает `SKIP:` (кроме намеренных
  live-skip с «live» в тексте); baseline обязан быть зелёным и без harness-skip;
- прогон переснят: **7/7 KILLED, SKIP=0**, restore по md5;
- независимая проверка на обычной ФС — отдельным агентом (codex sandbox ограничен).

## После независимого ревью (2026-10-22)

Вердикт: «доработка» (P0/P1 по коду нет; P1 — мутационный якорь и md5 после fmt, P2 — лог,
дедлайн `next_event`, red-лог). Закрыто:

- **P1-1** якорь мутации 04 приведён к post-fmt виду; прогон переснят на текущих байтах:
  **7/7 KILLED, SKIP=0**, restore по md5.
- **P1-2** md5 в отчёте пересчитаны по финальным файлам (таблица выше).
- **P2-1** лог мутаций переснят одним прогоном без перенаправления stdout в тот же файл
  (именно это давало перекрывающиеся записи); в логе ровно один `## Итог` и 7 блоков.
- **P2-2** `next_event` теперь считает общий дедлайн: остаток времени передаётся в
  `next_signal`, поток «чужих» сигналов не продлевает ожидание.
- **P2-3** red/green перегенерены на финальных байтах теста (`E0599` на отсутствующем
  `IdlePolicy::status`, `# EXIT=101` / `# EXIT=0`, 12 passed).
- **P3-2** при ошибке старта мока демон и tempdir убираются (`abort()`), skip-путь без утечки.
- **P3-1/P3-3** ужесточение p10 и точный текст live-skip — бэклог (`STATE.md` §7).
