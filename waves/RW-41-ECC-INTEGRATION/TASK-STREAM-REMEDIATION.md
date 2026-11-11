# ТЗ: Исправление трансляции экрана (Go Live / Stream) в серверах и личных звонках (DM)

**Дата:** 2026-11-10  
**Статус:** Ready for Development  
**Приоритет:** P0 (Блокирующий дефект релиза)  
**Ветки:** `bugfix/task-770-stream-signaling-remediation` -> `unstable`  
**Исполнители:** G1 Backend (Rust Core / Voice), G2 Frontend (WebView UI / Stage Viewer), G4 QA  

---

## 1. Контекст и анализ инцидента по видео (`Video_2026-11-10_19-10-39.mp4`)

### 1.1. Симптомы из видеозаписи и телеметрии
1. **В голосовом канале сервера (Guild: `700482297027231765`, Channel: `700482297027231769` "Основной"):**
   - Пользователь выбирает окно `waves — Dolphin` или `Экран 1` и нажимает «В эфир».
   - В UI Rustery локально появляется плашка «В ЭФИРЕ Rustery | H264 NVIDIA 720p Поток: 4 FPS (82 мс)» и открывается окно `live-stream-stage-viewer`.
   - **Дефект:** На стороне официального клиента Discord (пользователь `ShadowByte` в том же канале) плашка «В ЭФИРЕ» / «LIVE» у пользователя `Rustery` **не появляется вообще** — Discord не получает или сбрасывает статус трансляции. Другие участники не видят возможность смотреть стрим.
2. **В личном звонке (DM Channel: `1566834523038294047` с `ShadowByte`):**
   - При звонке и запуске трансляции экрана `live-stream-stage-viewer` зависает на бесконечной надписи:  
     `«Подключение к видеопотоку…»` (порт `49281` не отвечает `200`, а отдаёт `404`). Собеседник стрим не видит.
3. **Падение/остановка через 10–12 секунд:**
   - Пользователь, видя что собеседник не может подключиться, нажимает кнопку «Прекратить стрим», либо Stage Viewer при минимизации / ошибке локального предпросмотра принудительно убивает всю сессию.

---

## 2. Корневые причины (Root Causes): Почему утром работало, а сейчас падает

При утренних коммитах (`c31aa24` v0.5.0 и `befb92f`) стрим отправлял базовую сигнализацию. В коммитах второй половины дня (`fd49bdc`, `838a478`, `42380e7`, `d07d70a`, `9f559f1`, `6f60b11`) возникло **4 критических дефекта сигнализации и маршрутизации**:

### Причина 1: Стример вызывал `stream_watch` (OP 20) для своего собственного стрима
- **Файл:** `rust/gui-spike/ui/app.js:8469-8477`
- **Проблема:**  
  В `openLiveStreamStageViewer`:
  ```javascript
  if (isLiveSession() && streamKey) {
    if (!isSelfStream) {
      playVoiceUiSound("stream_user_joined");
    }
    void invoke(VOICE_SET_AUDIO_DEVICE_IPC_CMD, {
      kind: "stream_watch",
      deviceId: streamKey,
    }).catch(() => {});
  }
  ```
  Условие `if (!isSelfStream)` защищало только звук `playVoiceUiSound`, а вызов `invoke("stream_watch")` выполнялся **всегда**, в том числе когда стримит сам пользователь (`isSelfStream === true`)!
- **Последствия в бэкенде (`data.rs:6941-6957` и `ws.rs:5283-5296`):**
  1. Вызов `stream_watch` вызывал `reset_live_stream_state()`: стирал `guard.stream_token`, `guard.stream_endpoint`, `guard.stream_launched_token`!
  2. Вызывал `queue_live_stream_watch`: отправлял шлюзу Discord `OP 20 STREAM_WATCH` на собственный `stream_key`. Шлюз Discord отклоняет попытку смотреть собственный стрим до завершения его регистрации.
  3. Устанавливал `LAST_WATCHED_STREAM_KEY = Some(skey)`.
  4. В `ws.rs:5286` переменная `is_watching_other` становилась `true`, из-за чего `is_self_broadcaster` сбрасывался в `false`!
  5. Стример вместо роли вещателя (broadcaster: `encode: true`, `op: 12 Video`, захват экрана) переключался в режим зрителя (`encode: false`), ожидающего чужих пакетов.

### Причина 2: Двойная отправка `OP 18 STREAM_CREATE`
- **Файлы:** `rust/crates/discord-core/src/ws.rs:1264-1304` и `rust/gui-spike/src/main.rs:1497-1500`
- **Проблема:**  
  1. В `main.rs:1498`: вызывается `data::connect_or_update_live_voice_ext(..., self_stream: true)`, который внутри вызывает `queue_live_voice_state_ext`, а тот сам генерирует и отправляет в шлюз первый кадр `OP 18 STREAM_CREATE`.
  2. В `main.rs:1499`: сразу же следом вызывается `discord_core::ws::queue_live_stream_create(...)`, отправляя **второй идентичный кадр `OP 18 STREAM_CREATE` подряд**.
  3. Шлюз Discord Elixir Gateway при получении дублирующего `OP 18` сбрасывает создание стрима.

### Причина 3: Изоляция очередей нативных команд (`native_stream_cmd_mutex` не читался)
- **Файл:** `rust/crates/discord-core/src/ws.rs:6450-6454`
- **Проблема:**  
  ```rust
  let q_mutex = if is_stream_conn {
      native_stream_cmd_mutex()
  } else {
      native_voice_cmd_mutex()
  };
  ```
  Команды `set_stream` (`data.rs:4764`) и `set_stream_source` (`main.rs:1495`) ставятся в `native_stream_cmd_mutex()`.
  Однако соединение стрима (`is_stream_conn == true`) не запускается до прихода `STREAM_SERVER_UPDATE`!
  А основное голосовое соединение (`is_stream_conn == false`) считывает **только** `native_voice_cmd_mutex()`.
  В результате `set_stream` и `set_stream_source` оставались в очереди непрочитанными, и нативный процесс `discord_voice.node` не получал команду запуска вещания.

### Причина 4: Десинхронизация звонков один-на-один (DM Call)
- **Файлы:** `rust/gui-spike/src/main.rs:1474-1476`, `rust/crates/discord-core/src/ws.rs:1275-1280`
- **Проблема:**  
  В звонке DM `guild_id` равен `null` / `None`, тип стрима — `"call"`.  
  При вызове `stream_watch` очищался `guard.stream_rtc_channel_id`, из-за чего параметры сервера брались из `guard.voice_guild_id` (которого нет в ЛС), и запуск RTC-сессии стрима блокировался.

---

## 3. Детальный план работ (TDD & Реализация)

### Этап 1: G2 Frontend (`rust/gui-spike/ui/app.js`)
1. **Изолировать `stream_watch` строго для зрителей:**
   В `openLiveStreamStageViewer` (`app.js:8469-8477`):
   ```javascript
   if (isLiveSession() && streamKey) {
     if (!isSelfStream) {
       playVoiceUiSound("stream_user_joined");
       void invoke(VOICE_SET_AUDIO_DEVICE_IPC_CMD, {
         kind: "stream_watch",
         deviceId: streamKey,
       }).catch(() => {});
     }
   }
   ```
2. **Исключить ложные остановки стрима при сворачивании Stage Viewer:**
   При клике на крестик или Escape (`closeStageViewer(false)`) переводить стрим в плавающий PiP-режим без вызова `voice_stream_stop`.

### Этап 2: G1 Backend (`rust/crates/discord-core/src/ws.rs` и `rust/gui-spike/src/main.rs`)
1. **Устранить дублирование `OP 18 STREAM_CREATE`:**
   В `main.rs:1480-1501`: если `connect_or_update_live_voice_ext` уже отправил `OP 18`, не отправлять повторно `queue_live_stream_create`.
2. **Сквозная вычитка очереди стрим-команд:**
   В `ws.rs:6450`: основное голосовое соединение (`!is_stream_conn`) должно также дренировать `native_stream_cmd_mutex()`, если отдельный воркер стрима ещё не запущен, чтобы `discord_voice.node` получал `set_stream_source` и `set_stream`.
3. **Корректная обработка `guild_id: null` для звонков ЛС (DM):**
   В `queue_live_stream_create` и `ingest_raw_gateway_dispatch("STREAM_SERVER_UPDATE")` для стримов типа `call` брать `channel_id` как идентификатор RTC-сервера, если `guild_id` отсутствует.

### Этап 3: G4 QA & Верификация
1. Добавить юнит-контракт `tests/unit/stream_broadcaster_watch_isolation.test.mjs`:
   - Проверка, что `openLiveStreamStageViewer` при `isSelfStream === true` **никогда** не вызывает `stream_watch`.
   - Проверка, что `voice_stream_start` отправляет ровно один `OP 18`.
2. Запустить `npm run test:unit -- tests/unit/stream_broadcaster_watch_isolation.test.mjs`.
3. Собрать релизный бинарник `rust/dist/discord-gui-spike`.
4. Провести тестовый запуск и верифицировать отображение статуса «В ЭФИРЕ» у второго клиента.
