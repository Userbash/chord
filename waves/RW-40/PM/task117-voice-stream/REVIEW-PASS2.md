# REVIEW PASS 2 — task-117 (V-08-VOICE-STREAM-SESSION-WIRING)

- **Ветка:** `feature/task-117-voice-stream-wiring`, HEAD `4e240481e59ff4d49732ac8bd5ba08a006363748`, дерево dirty (волна не закоммичена).
- **Вердикт: PASS.** P0 = 0, P1 = 0, P2 = 1, P3 = 5. Все 8 пунктов §6 `REVIEW.md` закрыты.
- **Режим:** проверка кода и прогоны; продуктовый код и тесты не изменялись. Единственный записанный файл — этот.

## 1. Прогоны (независимые, не из `green.log`)

| Команда | Результат |
|---|---|
| `CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target cargo test --offline -p discord-voice --test session_contract` | `20 passed; 0 failed; 0 ignored`, exit 0 |
| `CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target cargo clippy --offline -p discord-voice --all-targets -- -D warnings` | exit 0, предупреждений нет |
| `cargo test --offline -p discord-voice` (13 бинарников) | 6+16+5+13+10+10+10+16+20+11+22+12+15 = **166 passed**, 0 failed |
| `rustfmt --edition 2021/2024 --check session.rs session_contract.rs` | exit 0 оба |
| `grep -n "ignore\|\.only\|todo!\|unimplemented!\|skip"` по `session.rs` и `session_contract.rs` | пусто |

MD5 совпадают с `REPORT.md` §5: `session.rs` `3d0003c1ec2c29538dbc5ca487abe128`,
`session_contract.rs` `9fcecfba3e82ed33e1c5c30c97002e2b`, `green.log` `1e19916c…`,
`mutations.log` `fb0b1558…`, `red.log` `fada5177…`. Штамп гейта
`.gate/worker-2026-10-30T15-56-30-573Z.json` есть, вердикт `passed_with_skips` сходится с логом.
Полный `gate:worker` повторно не гонялся (не входит в ТЗ этой проверки).

## 2. Закрытие замечаний `REVIEW.md`

- **P1-1 (громкость входа).** `apply_device_selection` отклоняет `input_volume > MAX_INPUT_VOLUME (100)`
  (`session.rs:1142-1148`), применённые громкости берутся из возврата `set_input_volume`/`set_output_volume`
  (`devices.rs:443-453`), тест сверяет `applied.input_volume == devices().input_volume()` и отказ `125`/`101`.
  Закрыто.
- **P1-2 (оптимистичная остановка).** `stop_screen_share` (`session.rs:1414-1421`) только возвращает `op19`;
  фаза и ключ не меняются, `ping`/`pause`/повторный `stop` работают до серверного `STREAM_DELETE`.
  `t_v01_18` проверяет все три последствия. Закрыто. Остаток по старту — см. P2-1 ниже.
- **P2-1 (сверка `stream_key`).** `is_own_stream_key`/`is_active_stream_key` + `unknown_stream_event_count`;
  все четыре `STREAM_*` адресуются по ключу. `t_v01_19` прогоняет четыре чужих события, ждёт счётчик `4`
  и неизменное состояние, затем проверяет, что свои события того же ключа продолжают работать. Закрыто.
- **P2-2 (атомарность выбора устройств).** `validate_device_choice` для обоих id до первой мутации
  (`session.rs:1170-1185`); тест отдельно проверяет `WrongKind` и `NotFound` выхода с неизменным
  `selected_input`. Закрыто.
- **P2-3 (фаза без ключа).** `clear_stream_record` сбрасывает ключ, фазу, зрителей, транспорт и причину;
  вызывается из `close_terminal`, `GaveUp` и терминальных ошибок. `t_v01_20` проверяет `LeaveRequested`,
  `WsClosed{1000}` и поздний диспатч; инвариант `key.is_some() == phase.is_some()` прогоняется после
  каждого шага через `assert_stream_record_consistency`. Закрыто.
- **P2-4 (отчёт).** §1.1 теперь говорит про два поля `SessionSnapshot`, `derive(Debug)` у
  `DeviceSelection`/`AppliedDeviceSelection` и редакцию только `StreamKey`/`StreamSecret`. Счётчики
  сверены с фикстурой: 9 outbound, 21 inbound, 27 negative (13 `stream_key`), 2 канонических ключа;
  166 тестов и 13 бинарников совпали с моим прогоном. Закрыто.
- **P2-5 (21 inbound).** `t_v01_19` прогоняет все 21 id, `InboundOracle::assert_all_visited` считает
  разницу множеств и требует ровно 21; ранее непокрытые `in-update-viewers`, `in-create-paused`,
  `in-update-resumed`, `in-delete-user-requested`, `in-delete-minimal` проверяются по значениям, а не
  только фактом вызова. Закрыто.
- **P2-6 (граница типов).** `StreamLifecyclePhase::as_wire`/`StreamDeleteReason::as_wire` сверяются с
  `A-PHASES`/`A-REASONS`; `discord-core` остаётся источником UI-фазы. Формулировку в контракт вносит Lead
  (файл вне scope) — это остаётся, закрытие на уровне кода и отчёта есть.

## 3. Независимая проверка мутаций

Семь мутаций применил сам к текущему baseline, прогнал `session_contract`, восстановил файл.
`RESTORED_MD5_MATCH=True`, итоговый md5 файла — исходный `3d0003c1…`.

| Мутант | MD5 (мой) | Совпал с `mutations.log` | Убит |
|---|---|---|---|
| граница входа по `MAX_OUTPUT_VOLUME` | `8a0b3a05…` | да | `t_v01_17` |
| валидация выхода отключена | `07554e28…` | нет (иная правка) | `t_v01_17` |
| `stop` сразу ставит `Ended` | `9171dcd8…` | да | `t_v01_18` |
| `DELETE` без сверки ключа | `4fda292f…` | нет (иная правка) | `t_v01_19` |
| ветка `unavailable` отключена | `b3845f57…` | да | `t_v01_19` |
| `clear_stream_record` без фазы | `2c90da20…` | да | `t_v01_20` |
| `effective_mute = user_mute` | `7ecb6cb9…` | да | `t_v01_17` |

Пять из семи хешей воспроизвелись побайтово; два расходятся только потому, что я правил иначе
(удалял блок целиком вместо точечной подмены), но оба мутанта краснеют на заявленных тестах.
Подделки журнала не вижу. Обязательная мутация §7.2 «`StopRequested` сразу ставит `Ended`» теперь
в наборе есть и KILLED.

## 4. Что осталось (не блокирует PASS)

**P2-1. Оптимистичный старт не согласован с Lead.** `start_screen_share`/`start_call_screen_share`
(`session.rs:1333-1408`) привязывают ключ и ставят `Connecting` до серверного `STREAM_CREATE`, тогда как
§4.2 требует «ничего не создаёт». В `REPORT.md` §6.2 это названо осознанным отличием с просьбой
подтвердить, но записи решения (`D-27-x` или строка в контракте §4.2) нигде нет. Останавливаться и
возвращать карточку из-за этого не стал: стоп — то, что требовало исправления, — исправлен, а UI-фаза
принадлежит редьюсеру `discord-core`. Но **до мержа исключение должно быть зафиксировано письменно**,
иначе следующая ревизия контракта снова прочитает код как нарушение.

**P3-1. Вектор `in-create-stale` проверяет не то, что заявлено.** В фикстуре `note` говорит «должен быть
отброшен политикой stale», а `expect.variant` — `created`. `t_v01_19` (строки 2200-2205) применяет вектор
и ждёт замену записи с комментарием «валидная замена записи (§4.5)». §4.5 разрешает замену только при
другом `seq`, а `on_stream_dispatch` принимает `event_name` и `d_json` без `seq`, то есть stale-защиты
на этом слое нет. Либо поправить `note`/`expect` фикстуры, либо принимать `seq` в session-слое и
проверять его как в `discord-core`.

**P3-2. Пустой список устройств даёт расхождение applied ↔ модель.** При `devices().is_empty()`
`select_input` не вызывается, но `AppliedDeviceSelection.input_device_id` всё равно заполняется входным
id (`session.rs:1187-1197`). Получается то же расхождение, что в P1-1, только для id. Инвариант стоит
распространить на идентификаторы или явно запрещать выбор до первой энумерации.

**P3-3. `lib.rs` изменён, но в отчёте не указан.** В `rust/crates/discord-voice/src/lib.rs` добавлены
`pub mod devices;` и `pub mod session;`. Файл общий (по `AGENTS.md` его правит Lead), в таблице §5
`REPORT.md` его нет. При мерже проверить, что правка согласована и не потеряется.

**P3-4. RED раунда исправлений не сохранён в карточке.** Лог лежит в `/tmp/opencode/red1.log` и умрёт
вместе с машиной; `red.log` в карточке — от исходного прогона. Артефакт стоит положить рядом
(`red-pass2.log`), раз `green.log` и `mutations.log` туда уже пишутся.

**P3-5. Мелкие отличия session-слоя от редьюсера не все названы.** `STREAM_UPDATE` по неизвестному ключу
не создаёт запись (§4.1 допускает создание), `STREAM_DELETE` по неизвестному ключу при
`reason==stream_full` не эмитит `NotifyViewerFull`, а `is_own_stream_key` при отсутствии записи принимает
`CREATE` любого ключа того же владельца. Отчёт упоминает только расхождение по `STREAM_SERVER_UPDATE`.
Стоит перечислить остальные в §7, чтобы интегратор не ждал от сессии поведения редьюсера.

## 5. Ограничения

- Живой Discord, сеть и второй RTC-сокет не запускались; `OpenStreamRtc`/`CloseStreamRtc` проверены
  только как эффекты.
- P3-2 и P3-5 — разбор кода, отдельными тестами не воспроизводились.
- Полный `gate:worker` не повторялся: взяты артефакт гейта и `green.log`, плюс собственные targeted-прогоны.
