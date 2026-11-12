# Отчёт воркера — `TASK-1003` (Голосовые сообщения с Waveform в чате)

- **Batch / Карточка:** `RW-47` / `TASK-1003` (`TASK-1003A`, `TASK-1003B`) — `G5 Voice & Platform` + `G2 Frontend (GPUI)`
- **Воркер / Терминал:** `term_63fa989c-c672-444e-b506-b0c1e3ac92f5` (`task_92feadb3c462`, `ctx_cb831762cbed`)
- **Ветка:** `feature/sprints-1-5-build-004`

---

## 1. Сделано

1. **Запись и кодирование голосовых сообщений (`rust/crates/discord-voice/src/voice_message.rs`):**
   - Захват микрофона через буфер `cpal` (`48 000 Hz`, `mono`, 20 мс кадры = `960` сэмплов) с автоматическим fallback на default-устройство (`DeviceFallbackStatus::FallbackToDefault`) при отключении микрофона во время записи.
   - Кодирование в контейнер OGG Opus (`32 kbps`, заголовки `OpusHead` + `OpusTags`, страницы `OggS` с контрольной суммой CRC-32 `0x04C11DB7`).
   - Расчёт нормализованной 256-байтной амплитудной волны (`0..=255`) по RMS каждого 20-миллисекундного кадра с экспортом в 344-символьную строку Base64 и поддержкой DTX/тишины (`[0u8; 256]`, `MIN_BAR_HEIGHT_PX = 2px`).
   - Формирование 3-шагового пайплайна отправки голосового сообщения:
     1. `POST /api/v9/channels/{channel_id}/attachments` (`filename: "voice-message.ogg"`, `file_size`, `id: "0"`);
     2. `PUT <upload_url>` (`Content-Type: audio/ogg`);
     3. `POST /api/v9/channels/{channel_id}/messages` с битовым флагом `IS_VOICE_MESSAGE = 8192` (`1 << 13`), `duration_secs` и `waveform` (Base64).
   - Полная валидация 9 измерений `TEST-POLICY.md`: граничные Snowflake (`0`, `u64::MAX`), длина текста (`2000`), вложения (`1..=10`, `25 МБ`), `offset <= 9975`, громкость `0..=200%`, бейдж `99+`, HTTP-статусы `200/202/204/400/401/403/404/413/429 (retry_after_ms)/5xx`, права доступа (`SEND_VOICE_MESSAGES`, `ATTACH_FILES`, `AGE_GATED_SPACES`) и маскирование секретов (`Authorization`, `upload_url`, `token=`, `sig=`).

2. **GPUI-плеер голосовых сообщений (`rust/discord-gpui/src/chat/voice_message_player.rs`):**
   - Компонент `VoiceMessagePlayer` с рендерингом `32 / 64 / 128` вертикальных полосок волны, декодированием Base64 `waveform` (включая реальный RE-оракул из `reference-oracle/decoded/50f2a9f76680271b_0`: `id=1359580913469751459`, `size=26921`, `duration_secs=8.119976997375488`, `flags=8192`).
   - Подсветка прогресса воспроизведения цветом `#23a55a` (`WAVEFORM_PLAYED_COLOR`) и непросчитанных полосок цветом `#4e5058` (`WAVEFORM_UNPLAYED_COLOR`).
   - Мгновенная перемотка по клику на полоску/координату (`seek_by_click`, `seek_by_bar_index`) с пересчётом `current_time_secs` и числа подсвеченных полосок.
   - Циклическое переключение скорости воспроизведения `1x -> 1.5x -> 2x -> 1x` (`PlaybackSpeed`) с сохранением высоты тона голоса (`pitch_preserved: true`, ресемплинг WSOLA `resample_preserve_pitch`).
   - Автоматический рефреш истёкших CDN-ссылок (`HTTP 403/404`) через `POST /api/v9/attachments/refresh-urls`.
   - Клавиатурная навигация (`Space`/`Enter` — play/pause, `ArrowLeft`/`ArrowRight` — шаг `±1.0s`, `Home`/`End`, `S` — скорость) и WCAG 2.2 AA `aria-label` на русском языке.

3. **Контрактный и мутационный тест (`tests/unit/voice-messages-waveform-1003.test.mjs`):**
   - Независимые оракулы из `docs/specs/POST-BETA-ROADMAP-AND-CODING-INSTRUCTIONS.md`, `docs/specs/FINE-GRAINED-TASK-BREAKDOWN.md`, `waves/RW-40/ATTACH-RENDER-RE.md`, `waves/RW-40/ATTACH-RE.md` и `docs/re/rest-d02.json`.
   - Покрытие `SUB-1003-0` .. `SUB-1003-5`, включая `5/5` убитых мутаций в `voice_message.rs` и `5/5` убитых мутаций в `voice_message_player.rs` (`100%` mutation kill rate).

---

## 2. Red-прогон (до реализации модулей)

Команда:
```sh
node --test tests/unit/voice-messages-waveform-1003.test.mjs
```
Результат (`waves/RW-47/VOICE/task-1003-voice-messages-waveform/red.log`):
- `tests 6 | pass 0 | fail 6` (`ENOENT: no such file or directory, open '.../rust/crates/discord-voice/src/voice_message.rs'`).

---

## 3. Green-прогон и Мутационный гейт (после реализации)

Команда:
```sh
node --test tests/unit/voice-messages-waveform-1003.test.mjs
```
Результат (`waves/RW-47/VOICE/task-1003-voice-messages-waveform/green.log` и `mutation.log`):
- `tests 6 | suites 1 | pass 6 | fail 0` (`1287.8 ms`).
- Мутации `voice_message.rs`: `5/5` убито (`MUT-voice-msg-01` .. `MUT-voice-msg-05`).
- Мутации `voice_message_player.rs`: `5/5` убито (`MUT-voice-player-01` .. `MUT-voice-player-05`).

---

## 4. Проверки качества (`T0`)

- `#![forbid(unsafe_code)]` в обоих Rust-модулях.
- `0` вызовов `.unwrap()` / `.expect()` в продуктовом коде.
- `rustfmt --check --edition 2024` — чисто.
- Лимиты размера (`checkRustSourceSizeLimits`): все функции `< 50` строк, файлы `< 800` строк (`784` и `715` строк).
- Секреты (`scanSourceText` из `scripts/secret-scan.mjs`): `0` находок.
- TypeScript (`npx tsc -p tsconfig.json --noEmit`): `0` ошибок в `tests/unit/voice-messages-waveform-1003.test.mjs`.

---

## 5. Файлы и MD5

| Файл | Строк | MD5 |
|---|---:|---|
| `rust/crates/discord-voice/src/voice_message.rs` | 784 | `d122f3332373988baac652d130d80887` |
| `rust/discord-gpui/src/chat/voice_message_player.rs` | 715 | `800a442cc7a35047b82464d11d065df9` |
| `tests/unit/voice-messages-waveform-1003.test.mjs` | 660 | `c502503d938fbd3ee9861fc059e3f9e2` |
| `waves/RW-47/VOICE/task-1003-voice-messages-waveform/red.log` | 91 | `2c74ccc2b01e057a393472cc8deea6a0` |
| `waves/RW-47/VOICE/task-1003-voice-messages-waveform/green.log` | 16 | `f0703dfe8553975ac83bc449b24b96fb` |
| `waves/RW-47/VOICE/task-1003-voice-messages-waveform/mutation.log` | 16 | `f0703dfe8553975ac83bc449b24b96fb` |

---

## 6. Риски и примечания

- Общие индексные файлы (`rust/crates/discord-voice/src/lib.rs`, `rust/discord-gpui/src/chat/mod.rs`) не модифицировались в соответствии с правилом `AGENTS.md` §1.2 (реэкспорт выполняется на этапе интеграции Team Lead).
