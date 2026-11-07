# Независимое ревью — task-117 (V-08-VOICE-STREAM-SESSION-WIRING)

**Вердикт: RETURN.** P0 = 0, P1 = 2, P2 = 6.

Ревью проведено в режиме read-only по коду и тестам. Единственный записанный файл —
этот `REVIEW.md`. HEAD проверки: `4e240481e59ff4d49732ac8bd5ba08a006363748`
(совпадает с базой из `REPORT.md`).

Оракулы:

- `waves/RW-40/VOICE/STREAM-GATEWAY-CONTRACT.md` (`md5 babab27ef9a00fbb96a492e7725825ec`);
- `waves/RW-40/VOICE/stream-opcodes-fixtures.json` (`md5 1a44becfab3c6fd4170346635f80830b`);
- `waves/RW-40/VOICE/V01-SESSION-CONTRACT.md`, `DESIGN/VOICE-DEVICES-SETTINGS-SPEC.md`,
  `DESIGN/STREAM.md` §2.6.

Путь `CONTRACTS/STREAM-GATEWAY-CONTRACT.md` из ТЗ в дереве отсутствует; сверка шла по
`waves/RW-40/VOICE/STREAM-GATEWAY-CONTRACT.md` — это единственная копия контракта.

---

## 1. Сверка MD5 с `REPORT.md`

| Файл | MD5 в отчёте | Фактический MD5 | Итог |
|---|---|---|---|
| `rust/crates/discord-voice/src/session.rs` | `2a84f53056d5398cdb6d27e914780bba` | `2a84f53056d5398cdb6d27e914780bba` | совпал |
| `rust/crates/discord-voice/tests/session_contract.rs` | `fcf0410d132703eb87adeb1ba4a3945b` | `fcf0410d132703eb87adeb1ba4a3945b` | совпал |
| `waves/.../red.log` | `fada51777c1b212f43af1a889a299182` | `fada51777c1b212f43af1a889a299182` | совпал |
| `waves/.../green.log` | `2e5460d490e322ec35bf8ce8cdae265e` | `2e5460d490e322ec35bf8ce8cdae265e` | совпал |
| `waves/.../mutations.log` | `6f391d80ec058c2c501a9c536696b920` | `6f391d80ec058c2c501a9c536696b920` | совпал |

RED-лог содержит 53 ошибки компиляции ровно на отсутствующих новых символах
(`VoiceSession`, `DeviceSelection`, `AppliedDeviceSelection`, `StreamKey`, `StreamKind`,
`StreamGatewayOp`, `StreamGatewayCommand`, `StreamLifecyclePhase`, `StreamDeleteReason`,
`StreamSessionEffect`), код возврата `101`. Это правдоподобный red-first прогон.

---

## 2. Независимые прогоны

Все команды выполнены из корня репозитория с `CARGO_TARGET_DIR=/tmp/opencode/target-task117-review`,
offline.

| Команда | Результат |
|---|---|
| `cargo test --manifest-path rust/Cargo.toml --offline -p discord-voice --test session_contract` | `19 passed; 0 failed; 0 ignored`, exit `0` |
| `cargo test --manifest-path rust/Cargo.toml --offline -p discord-voice` | 13 бинарников, `165 passed; 0 failed`, exit `0` |
| `cargo clippy --manifest-path rust/Cargo.toml --offline -p discord-voice --all-targets -- -D warnings` | exit `0`, предупреждений нет |
| `rustfmt --edition 2021 --check rust/crates/discord-voice/src/session.rs rust/crates/discord-voice/tests/session_contract.rs` | exit `0` |
| `node --test tests/meta/test-policy.test.mjs` | `17 pass; 0 fail; 0 skip`, exit `0` |

Детализация полного прогона крейта: 6 unit + 16 crypto_vectors + 5 devices_contract +
13 media_contract + 10 reception_contract + 10 reconnect_contract + 10 regions_contract +
16 rtcp_contract + 19 session_contract + 11 signaling + 22 srtp_contract + 12 stats_contract +
15 udp = **165**. В `REPORT.md` §3 указано `119` и перечислено только 9 бинарников — цифра
не соответствует даже приложенному `green.log` (см. P2-4).

Секретов в выводе тестов и в `Debug`/`Display` ключа, `StreamSecret`, `SessionSnapshot` не
обнаружено; `grep` по `.only`, `#[ignore]`, `todo!`, `unimplemented!`, `skip` в проверяемых
файлах пуст.

---

## 3. Честность мутаций

`mutations.log` утверждает 5/5 KILLED. Проверка:

1. Каждая из пяти записей содержит MD5 мутанта, `EXIT_CODE=101` и падение ровно того теста,
   который заявлен в таблице `REPORT.md` §4 (M1/M2 → `t_v01_17`, M3/M4 → `t_v01_18`,
   M5 → `t_v01_19`). Тексты паник соответствуют заявленным нарушениям.
2. MD5 мутантов независимо пересчитаны: я применил к текущему baseline ровно описанные в
   отчёте текстовые правки на копии файла в памяти и получил все пять хешей побайтово:

   | ID | Правка | MD5 совпал с логом |
   |---|---|---|
   | M1 | `selection.input_volume > MAX_OUTPUT_VOLUME` → `>=` | да, `976829c1…` |
   | M2 | `effective_mute = self.devices.effective_mute()` → `selection.user_mute` | да, `a911c18f…` |
   | M3 | `self.active_stream_key = Some(key.clone())` → `None` (первое вхождение) | да, `02c7ad58…` |
   | M4 | `self.active_stream_key.take()` → `.clone()` | да, `3deb5c9f…` |
   | M5 | `if unavailable` → `if false && unavailable` | да, `68d64851…` |

3. После восстановления baseline MD5 снова `2a84f53056d5398cdb6d27e914780bba`, контрольный
   прогон `19 passed`, `RESTORED_TEST_EXIT=0`. Текущий файл в дереве побайтово равен baseline.

Вывод: журнал мутаций честный, подделки хешей или «зеленения» не видно. Ограничение:
набор мутаций не покрывает обязательную мутацию контракта §7.2 «`StopRequested` сразу ставит
`Ended`» — она выжила бы, потому что тест закрепляет именно такое поведение (см. P1-2).

---

## 4. Соответствие контракту и фикстурам

Что совпадает:

- опкоды `18..=22` ↔ имена, `from_code(17/23) == None`, изоляция от RTC `VoiceOp` — совпадает;
- все **9/9** outbound-векторов фикстуры: `Create{guild, guild без region, call}`, `Delete`,
  `Delete{call}`, `Watch`, `Ping`, `SetPaused{true,false}`; `d` совпадает с `payload`
  семантически, `guild_id: null` для `call` есть;
- все **13/13** негативных векторов `stream_key` дают точный `StreamKeyError`;
- `StreamSecret` не утекает: `Debug` → `"<redacted>"`, `Display` → `<redacted>`;
- `StreamKey::Debug/Display` не печатают `guild_id` и полную строку;
- `STREAM_SERVER_UPDATE` не меняет фазу, выставляет `transport_ready` и эмитит `OpenStreamRtc`;
- приоритет `STREAM_DELETE` для проверенных ветвей: `unavailable` → `Reconnecting` с сохранением
  ключа, `unauthorized` → `Failed`, `safety_guild_rate_limited` → `Ended` +
  `ShowSafetyRateLimit`, `stream_full` → `Ended` + `NotifyViewerFull`, неизвестный `reason` →
  `Ended` + `Unknown(raw)`, `user_requested` после `Failed` сохраняет `Failed`;
- `H-VIEWERS` (`None` ≠ `Some([])`) и `H-PAUSE` (отсутствие `paused` сохраняет паузу)
  реализованы;
- терминальное закрытие сессии очищает `active_stream_key` и `stream_transport_ready`.

Чего не хватает по покрытию: использованы **13 из 21** inbound-вектора. Не задействованы
`in-create-stale`, `in-create-paused`, `in-create-call`, `in-server-update-call`,
`in-update-viewers`, `in-update-resumed`, `in-delete-user-requested`, `in-delete-minimal`.
В частности, не проверены замена списка зрителей (`in-update-viewers`) и `CREATE{paused:true}`,
хотя код эти ветви содержит. Утверждение отчёта «сверяет все 9 inbound-фикстур» неверно
(см. P2-4/P2-5).

---

## 5. Замечания

### P1-1. Входная громкость принимается до 200, но модель устройств клампит её до 100

`rust/crates/discord-voice/src/session.rs:1062-1067` проверяет `input_volume` через
`MAX_OUTPUT_VOLUME` (200). В `devices.rs:17` действует `MAX_INPUT_VOLUME = 100`, а
`set_input_volume` (`devices.rs:443-447`) молча клампит значение. В результате
`apply_device_selection` возвращает `AppliedDeviceSelection { input_volume: 125, … }`, а
`session.devices().input_volume()` равно `100`. Тест `t_v01_17` закрепляет это расхождение:
строка 1471 задаёт `input_volume: 125`, строка 1485 ожидает `125` в применённой структуре,
строка 1496 ожидает `100` в состоянии устройств.

Это противоречит `VOICE-DEVICES-SETTINGS-SPEC.md` §379 и таблице слайдеров (`OX2Bnr`
`0..=100`, `aUJ062` `0..=200`): «применённое» значение врёт, UI покажет 125 % при реальных
100 %. Заявление `REPORT.md` §1.1 о диапазоне `input_volume` и `output_volume` «0..=200»
тоже неверно для входа. Нужно либо отклонять `>100` с `VolumeOutOfRange { max: 100 }`, либо
возвращать клампированное значение из `set_input_volume`, а тест — привести к контракту
V-07.

### P1-2. «Оптимистичный» стоп: `stop_screen_share` сразу ставит `Ended` и отвязывает ключ

`session.rs:1259-1267`: до отправки/подтверждения `STREAM_DELETE` локально выполняется
`active_stream_key.take()`, `stream_phase = Ended`, `stream_transport_ready = false`.
`STREAM-GATEWAY-CONTRACT.md` §4.2 прямо требует: `StopRequested` фазу **не меняет**,
«офф» наступает только по серверному `STREAM_DELETE`; §7.2 включает мутацию
«`StopRequested` сразу ставит `Ended`» в обязательный список — она должна краснеть.
`DESIGN/STREAM.md` §2.6 запрещает оптимистичный стоп. Тест `t_v01_18` (строки 1850-1864)
наоборот требует немедленных `Ended` и `None`, поэтому контрактная мутация выживает.

Последствия: UI показывает «выключено» до подтверждения; при отказе сервера
(`unauthorized`/`stream_full`) состояние уже потеряно, `ping_stream`/`set_stream_paused`
возвращают `NoActiveStream`, хотя стрим на сервере ещё жив. Связанный, но менее тяжёлый
случай — `start_screen_share` сразу создаёт запись с `Connecting` (строки 1213-1216), тогда
как §4.2 для `StartRequested` требует «ничего не создаёт». Нужно хранить ключ как
`pending_*` до `STREAM_CREATE`/`STREAM_DELETE` либо явно согласовать исключение для
session-слоя с Team Lead и добавить контрактную мутацию.

### P2-1. `STREAM_CREATE`/`STREAM_DELETE` не сверяют ключ события с активным ключом

`session.rs:1333` (CREATE) и `1375`, `1408-1414` (UPDATE/DELETE) меняют единственный
`active_stream_key` независимо от того, совпадает ли `stream_key` события с текущим.
Контракт §4.1 для удаления несуществующей записи требует «ничего», только
`unknown_streams += 1` (и `NotifyViewerFull` при `StreamFull`). В сессии с чужим стримом в
том же канале `STREAM_CREATE` перезапишет собственный ключ, а `STREAM_DELETE` чужого стрима
отвяжет свой — следующий `stop_screen_share` отправит `op19` с чужим ключом. Фикстуры этого
не ловят: во всех векторах один и тот же ключ. Нужна либо карта по `StreamKey`, как в
`discord-core/src/stream.rs`, либо фильтр `key == active && own`.

### P2-2. `apply_device_selection` не атомарен при ошибке выбора устройства

`session.rs:1083-1102`: сначала мутирует `devices.select_input(...)?`, затем
`devices.select_output(...)?`. Если вход валиден, а выход даёт `WrongKind`/`NotFound`,
возвращается ошибка, но выбранный вход уже изменён, а `applied_devices` остаётся прежним.
Тест `t_v01_17` (строки 1624-1642) проверяет только ошибку, не состояние. Нужно сначала
валидировать оба id, потом мутировать.

### P2-3. После терминального закрытия остаётся `stream_phase` без ключа

`close_terminal` (`session.rs:1482-1499`) очищает `active_stream_key` и
`stream_transport_ready`, но не `stream_phase`. После `LeaveRequested` или `WsClosed{1000}`
снимок отдаёт `active_stream_key = None` и `stream_phase = Some(Active/Paused)`. Это
противоречивая пара для UI и расходится с `H-DELETE-END`/§4.1 (запись живёт до `Close`).
Нужно сбрасывать фазу вместе с ключом либо сохранять оба поля согласованно.

### P2-4. `REPORT.md` содержит неверные утверждения о коде и счётчиках

- §1.1 заявляет 10 новых полей `SessionSnapshot` (`selected_input_device_id`,
  `selected_output_device_id`, `selected_video_device_id`, `input_volume`, `output_volume`,
  `user_mute`, `self_deaf`, `effective_mute`, `self_video`, `stream_transport_ready`).
  Фактически `SessionSnapshot` (`session.rs:751-766`) содержит только `active_stream_key` и
  `stream_phase`; `stream_transport_ready` доступен лишь методом.
- §1.1 заявляет «полное редактирование чувствительных идентификаторов в `Debug` и `Display`»
  для выбора устройств. `DeviceSelection` и `AppliedDeviceSelection` (`session.rs:481-504`)
  используют `derive(Debug)` и печатают `*_device_id` как есть; `Display` у них нет. Тест это
  не проверяет.
- Строка 8: «`11` outbound + `9` inbound + `10` stream_keys». В фикстуре 9 outbound,
  21 inbound, `keys` — 2 канонических ключа; негативных ключей 13. Строки 36-37 «все 11
  outbound» и «все 9 inbound» также неверны (outbound 9/9 покрыты, inbound 13/21).
- §3: «119 passed» и список из 9 бинарников. `green.log` содержит 13 бинарников и 165 тестов;
  пропущены `reception_contract` (10), `reconnect_contract` (10), `regions_contract` (10),
  `rtcp_contract` (16).

### P2-5. Покрытие inbound-фикстур неполное (13/21)

См. §4. Контракт T-STREAM-08 требует все 21 вектор. Без них не проверены как минимум
`in-update-viewers` (H-VIEWERS: замена списка), `in-create-paused` (фаза `Paused`),
`in-update-resumed` (снятие паузы), `in-delete-user-requested` и `in-delete-minimal`
(default → `Ended`). Код эти ветви содержит, но зелёный прогон их не подтверждает.

### P2-6. Дублирование модели стрима в двух крейтах

`discord-core/src/stream.rs` уже реализует `StreamKey`, `StreamPhase`, `StreamDeleteReason`,
`StreamSecret` и редьюсер строго по контракту, включая запрет optimistic start/stop
(`stream.rs:1457-1481`). `discord-voice/src/session.rs` заводит одноимённые типы заново и
с другой семантикой фаз. Два источника истины для одной сущности: интегратор может
случайно смешать типы или получить разные фазы из core-редьюсера и из `SessionSnapshot`.
Нужно либо реэкспортировать типы из `discord-core`, либо явно зафиксировать в контракте,
что `Session` — отдельный транспортный слой без UI-фазы стрима.

---

## 6. Что нужно для PASS

1. Исправить границу `input_volume` (0..=100, клампированное значение в
   `AppliedDeviceSelection`) и привести `t_v01_17` к спецификации V-07. (P1-1)
2. Убрать optimistic stop: не менять фазу и не отвязывать ключ до `STREAM_DELETE`; добавить
   мутацию «`StopRequested` сразу ставит `Ended`» и убедиться, что она KILLED. (P1-2)
3. Сверять `stream_key` события с активным ключом или перейти на карту ключей. (P2-1)
4. Сделать `apply_device_selection` атомарным. (P2-2)
5. Согласованно сбрасывать `stream_phase` при терминальном закрытии. (P2-3)
6. Привести `REPORT.md` к фактам: поля `SessionSnapshot`, редактирование ID, счётчики
   фикстур, 165 тестов и 13 бинарников. (P2-4)
7. Добавить недостающие 8 inbound-векторов. (P2-5)
8. Зафиксировать границу типов `discord-core` ↔ `discord-voice`. (P2-6)

---

## 7. Ограничения

- Живой Discord, сеть и второй RTC-сокет не запускались.
- Полный `gate:worker`/workspace не гонялся: выполнены только команды из ТЗ.
- P2-2 и P2-3 — статический разбор кода, отдельным тестом не воспроизводились.
- Продуктовый код, тесты и логи не изменялись; записан только этот файл.
