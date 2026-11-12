# Отчёт воркера — `TASK-1010` (Автоматический перевод в AFK-канал при простое)

- **Batch / Карточка:** `RW-47` / `TASK-1010` (`TASK-1010A`, `TASK-1010B`) — `G5 Voice / Platform` + `G1 Backend`
- **Воркер / Терминал:** `term_ddfd4621-79cf-40f2-b04e-3e70a75fd58a` (`task_d1d97346cc2a`, `ctx_8b8d906564af`)
- **Ветка:** `feature/sprints-1-5-build-004`

---

## 1. Сделано

1. **Монитор бездействия и автоматического перевода в AFK-канал (`rust/discord-gpui/src/voice/afk_monitor.rs`):**
   - Выбор системного бэкенда бездействия (`select_idle_backend`) с каскадным приоритетом Wayland `ext-idle-notifier-v1` -> D-Bus `org.freedesktop.ScreenSaver` (`GetSessionIdleTime`, `UnknownMethod`/`UnknownInterface` -> `Ok(None)`) -> X11 `XScreenSaverQueryInfo` -> монотонный fallback.
   - Фильтрация ложных срабатываний (`compute_effective_idle_ms`): подавление простоя (`0` мс) при активном воспроизведении медиа, активных стримах Go Live (`active_stream_count > 0`) и речи пользователя (`SPEAKING`), а также обработка сигналов `PrepareForSleep` (`suspend`/`resume`) и блокировки экрана.
   - Сверка с настройкой гильдии `afk_timeout` (`60, 300, 900, 1800, 3600` секунд, модуль `185255` RE-дампа) и формирование кадра Gateway Opcode 4 (`VOICE_STATE_UPDATE`, модуль `576991` / `926456`) на смену `channel_id` на `guild.afk_channel_id` с принудительным выставлением `self_mute: true`, `self_video: false` и флагом `bypass_idle_update: true`.
   - Защита от повторного спама Opcode 4 при нахождении в AFK-канале, пропуск трибун (`Stage Voice`), детекция 5-часового жёсткого отключения (`18_000_000` мс) и откат оптимистичного перевода (`rollback_afk_move`) при обрыве соединения со шлюзом.
   - Полная валидация 9 измерений `TEST-POLICY.md`: Snowflake (`0`, `u64::MAX`), лимиты сообщения (`2000`), вложений (`1..=10`, `25 МБ`), `offset <= 9975`, громкость `0..=200%`, бейдж `99+`, права доступа (`VIEW_CHANNEL`, `CONNECT`, `AGE_GATED_SPACES`) и маскирование секретов (`Bearer`, `mfa.`, `session_id=`).

2. **Автономный CLI-валидатор и RE-оракул (`scripts/verify-afk.mjs`):**
   - Сверка карточек `TASK-1010`, `TASK-1010A`, `TASK-1010B` в `waves/kanban-board.json`, фикстуры `fixtures/dbus/power.json` (`GetSessionIdleTime -> 1234`), `docs/re/gateway-protocol.json` (`VOICE_STATE_UPDATE = 8`), `docs/re/gateway-schema.json`, 7 живых гильдий из `reference-oracle/gw-proto/msgs-run1-57992_5112.jsonl` и модулей RE-дампа (`885576`, `576991`, `926456`, `185255`, `260509`).
   - Поддержка аргументов `--help`, `--oracle`, `--out-json`, `--out-md` и кодов выхода `0 / 1 / 2`.

3. **Контрактный и мутационный тест (`tests/unit/voice-afk-monitor-1010.test.mjs`):**
   - Покрытие `SUB-1010-0` .. `SUB-1010-5`, включая `5/5` убитых мутаций в `afk_monitor.rs` (`MUT-1010-RS-01` .. `MUT-1010-RS-05`) и `5/5` убитых мутаций в `verify-afk.mjs` (`MUT-1010-JS-01` .. `MUT-1010-JS-05`, `100%` mutation kill rate).

---

## 2. Red-прогон (до реализации модулей)

Команда:
```sh
node --test tests/unit/voice-afk-monitor-1010.test.mjs
```
Результат (`waves/RW-47/VOICE/task-1010-voice-afk-monitor/red.log`):
- `tests 6 | pass 0 | fail 6` (`Модуль rust/discord-gpui/src/voice/afk_monitor.rs должен существовать`).

---

## 3. Green-прогон и Мутационный гейт (после реализации)

Команда:
```sh
node --test tests/unit/voice-afk-monitor-1010.test.mjs
```
Результат (`waves/RW-47/VOICE/task-1010-voice-afk-monitor/green.log` и `mutation.log`):
- `tests 6 | suites 1 | pass 6 | fail 0` (`771.2 ms`).
- Мутации `afk_monitor.rs`: `5/5` убито (`MUT-1010-RS-01` .. `MUT-1010-RS-05`).
- Мутации `verify-afk.mjs`: `5/5` убито (`MUT-1010-JS-01` .. `MUT-1010-JS-05`).

---

## 4. Проверки качества (`T0`)

- `#![forbid(unsafe_code)]` в `rust/discord-gpui/src/voice/afk_monitor.rs`.
- `0` вызовов `.unwrap()` / `.expect()` в продуктовом коде.
- `rustfmt --check --edition 2024` — чисто.
- Лимиты размера (`checkRustSourceSizeLimits`): все функции `< 50` строк, файлы `< 800` строк (`798`, `780`, `766` строк).
- Секреты (`scanTextForSecrets` из `scripts/secret-scan.mjs`): `0` находок.
- TypeScript (`npx tsc -p tsconfig.json --noEmit`): `0` ошибок в `scripts/verify-afk.mjs` и `tests/unit/voice-afk-monitor-1010.test.mjs`.

---

## 5. Файлы и MD5

| Файл | Строк | MD5 |
|---|---:|---|
| `rust/discord-gpui/src/voice/afk_monitor.rs` | 798 | `b64a8660868e2cb3c152e014c247c3c0` |
| `scripts/verify-afk.mjs` | 780 | `4c5ca6550aa64324b5dc15a1bd622eed` |
| `tests/unit/voice-afk-monitor-1010.test.mjs` | 766 | `2cfc5f662c65c8cb660190d0c3347df8` |
| `waves/RW-47/VOICE/task-1010-voice-afk-monitor/red.log` | 127 | `f1a31d8008f76f8c40a98cdc22ea816c` |
| `waves/RW-47/VOICE/task-1010-voice-afk-monitor/green.log` | 16 | `7f05c909887319ebf3a3006289139de3` |
| `waves/RW-47/VOICE/task-1010-voice-afk-monitor/mutation.log` | 16 | `7f05c909887319ebf3a3006289139de3` |

---

## 6. Риски и примечания

- Общие индексные файлы не модифицировались в соответствии с правилом `AGENTS.md` §1.2.
