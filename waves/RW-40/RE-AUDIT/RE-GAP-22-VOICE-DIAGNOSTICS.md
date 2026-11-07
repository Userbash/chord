# RE-GAP-22 — Voice WebRTC Diagnostics & Connection Stats Overlay (G7 RE Audit)

- **Дата:** 2026-10-31
- **Карточка:** `task-274` (`re/task-274-voice-diagnostics-audit`)
- **Отдел:** `G7 Reverse Engineering & Parity Discovery (G7 RE / REA)`
- **Оракулы `/home/sanya/re-deep/` (read-only):**
  - `/home/sanya/re-deep/voice-deep.md` и `/home/sanya/re-deep/voice-deep.json` (протокол Voice Gateway v8, RTP/SRTCP, кодеки Opus `120` / AV1 `101` / H264 `103` / VP8 `105`, `aead_xchacha20_poly1305_rtpsize` / `aead_aes256_gcm_rtpsize`, DAVE MLS v1).
  - `/home/sanya/re-deep/voice/ui/01-voice-panel.html` и `/home/sanya/re-deep/voice/ui/01-panel-text.json` (живой DOM-слепок голосовой панели Discord).
  - `/home/sanya/re-deep/voice/ui/02-requiremodule-voice.json` и `/home/sanya/re-deep/voice/ui-s2/03-voice-module-api.json` (57 методов нативного `discord_voice.node`, включая `getFilteredStats`, `setTransportOptions`, `getEncryptionModes`, `setPingInterval`).
  - `/home/sanya/re-deep/decoded/3eefa7dae1e2c234_0`:
    - модуль `613681` (`offset 2463706..2505000`) — `class tg` (`RTCConnection`): геттер `quality`, буфер `_pings`, `_pingBadCount`, `_handlePing`, `_handlePingTimeout`, `getAveragePing`, `getLastPing`, `getOutboundLossRate`;
    - модуль `206959` (`offset 10790700..10824500`) — `NativeConnection` и нормализатор `function k(e, t, n, i)` (`getStats` / `getFilteredStats`), расчёт `OutboundLossRate` по дельтам `packetsSent`/`packetsLost`, `handlePingTimeout` (`4000 мс`);
    - модуль `731854` (`offset 10763674`) — константы `MediaEngine`: интервал опроса статистики `t1 = 1000` мс (`1e3`), интервал пинга `n8 = 5000` мс (`5e3`), битовая маска фильтра `yt` (`TRANSPORT=1`, `OUTBOUND=2`, `INBOUND=4`, `ALL=7`), состояния `x` (`DEFAULT="default"`, `STREAM="stream"`), состояния соединения `S7L` и качества `bFR` (также в модуле `652215` @ `253362..253750`);
    - модуль `624694` (`offset 5997681..6001500`) — `MediaEngineStatsStore` (скользящее окно из `30` снимков `MEDIA_ENGINE_CONNECTION_STATS`, агрегация с лагом `15` и `30` отсчётов);
    - модуль `412780` (`offset 9264635..9268500`) — `RTCDebugStore` (временные ряды `h` из 25 метрик по `600` точек `{value, time}`, секции `zWA`, запись и реплей соединения `createReplayConnection`);
    - модуль `978171` (`offset 7458500..7465900`) — UI-компонент попапа `RTCConnectionPopout` (вкладки `rtc_debug_panel` / `rtc_secure_frames`, спарклайн пинга, `averagePing`, `lastPing`, `outboundLossRate`, баннер E2EE, кнопки `Отладка` и `Загрузить журналы`);
    - модуль `999751` (`offset 7465980..7469800`) — `RTCConnectionStatus` (нижний бар статуса, индикатор пинга, `hoverRoll`-текст);
    - модуль `917592` (`offset 6025550..6028500`) и `998759` (`offset 7464200..7465200`) — вкладка конфиденциальности DAVE E2EE (`Voice Privacy Code`, сетка 6×5 цифр из `epochAuthenticator`);
    - модуль `229659` (`offset 1966150..1967500`) и `546738` (`offset 4695250..4699500`) — `TimelineGraphView` (`<canvas width="258" height="80">`, 30 FPS rAF, история `60 000` мс);
    - модуль `233545` (`offset 694029`) и `PopoutWindowRTCDebug` (`webpackId: 767675`, `offset 1896599`) — экшены и окно полной диагностики (`854×480`).
  - `/home/sanya/re-deep/decoded/e6811dfca6ffcd34_0` (модуль `473718`), `/home/sanya/re-deep/decoded/d1bfcf59dca06829_0` (модуль `203077`), `/home/sanya/re-deep/decoded/dc091883931cf040_0` (модули `300303`, `479137`, `68375`, `873353`) — CSS-модули и селекторы статуса/попапа.
  - `/home/sanya/re-deep/ru-catalog.json` и `/home/sanya/re-deep/en-catalog.json` — точные строки локализации (RU/EN).

---

## 1. Резюме аудита и ключевые расхождения

В эталонном клиенте Discord диагностика голосового соединения и WebRTC/RTC-статистика разделены на три связанных уровня:
1. **Индикатор статуса и качества связи в нижней голосовой панели (`RTCConnectionStatus`)**: отображает 10 состояний `S7L` (с цветовой кодировкой `CONNECTED` / `CONNECTING` / `ERROR`), 3-ступенчатую иконку качества сигнала (`bFR`: `fine` / `average` / `bad` / `unknown`), вычисляемую **одновременно по последнему пингу (`lastPing`) и проценту потерь исходящих пакетов (`outboundLossRate`)**, а также `hoverRoll`-подпись (`Голосовая связь подключена` → при наведении `О голосовом звонке`).
2. **Попап сведений о голосовом звонке (`RTCConnectionPopout`, ширина `300px`)**: открывается по клику на статус/иконку пинга, содержит вкладки `Интеграция` (`Connection`, `rtc_debug_panel`) и `Конфиденциальность` (`Privacy`, `rtc_secure_frames` при активном DAVE E2EE), Canvas-спарклайн пинга `258×80` за последние 60 секунд, короткое имя RTC-сервера (`hostname.split(".")[0]`), три метрики (`Средняя задержка`, `Последняя задержка`, `Потеря исходящих пакетов`), предупреждение о порогах (`250 мс` и `10%`), баннер шифрования и кнопку перехода в окно `Отладка` (`RTC_DEBUG_MODAL_OPEN`).
3. **Пайплайн сбора и нормализации статистики (`NativeConnection` → `MediaEngineStatsStore` → `RTCDebugStore`)**: опрашивает медиа-движок каждые `1000 мс` (`getFilteredStats(7)`), нормализует разделы `transport`, `rtp.outbound`, `rtp.inbound[userId]`, `screenshare`, `camera`, `audioDevice`, вычисляет дельту потерь исходящих пакетов `deltaLost / (deltaSent + deltaLost) * 100`, хранит скользящее окно из `30` снимков в `MediaEngineStatsStore` и временные ряды из `600` точек по 25 метрикам в `RTCDebugStore` для модального окна `RTCDebugModal` (`TRANSPORT`, `OUTBOUND`, `INBOUND`, `SCREENSHARE`, `CAMERA`, `CLIPS`, `STREAMS_OUTBOUND`, `STREAMS_INBOUND`).

### Что реализовано у нас сейчас vs чего не хватает:
- В **`rust/crates/discord-voice`**:
  - Есть `RttEstimator`, `StatsSample`, `StatsWindow` (`DEFAULT_WINDOW_CAPACITY = 32`) и `SessionStats` (`rust/crates/discord-voice/src/stats.rs`), а также `ReceptionStats` (`rust/crates/discord-voice/src/reception.rs`) и `last_rtt_ms` в `VoiceSessionDriver` (`rust/crates/discord-voice/src/session.rs:555`).
  - **Разрыв 1 (`G5 Voice`)**: `SessionStats` и `ReceptionStats` **не интегрированы** в `VoiceSessionDriver` (`session.rs` считает только `last_rtt_ms = now_ms.saturating_sub(ack.t)` при `OP 6 HEARTBEAT_ACK`, но не агрегирует счётчики отправленных/принятых пакетов и байт, `outbound_loss_rate`, `inbound` статистику по `ssrc`/`user_id`, `codec`, `transport`, буфер `_pings` на 199 элементов с средним по 20 отсчётам `getAveragePing()`, счётчик `_pingBadCount` при `ping > 500` мс и таймаут 3 подряд пингов `4000` мс).
  - **Разрыв 2 (`G5 Voice`)**: В `discord-voice` отсутствует функция классификации качества соединения по контракту Discord (`FINE` / `AVERAGE` / `BAD` / `UNKNOWN` с учётом `last_ping_ms` `250`/`500` мс и `outbound_loss_rate` `5%`/`10%`) и агрегированная структура `RtcConnectionStatsSnapshot` (`transport`, `rtp.outbound`, `rtp.inbound`).
- В **`rust/gui-spike/src/` (`G1 Backend BFF`)**:
  - Есть только `net_dispatch_stats` (`rust/gui-spike/src/net.rs:511`) для очередей шлюза и `voice_panel_diagnostic_state` (`"device-missing"` в `rust/gui-spike/src/voice_devices.rs:496`).
  - **Разрыв 3 (`G1 Backend`)**: Нет BFF-модуля и IPC-команд для получения снимка голосовой диагностики (`voice_diagnostics_snapshot` / `voice_rtc_stats_get` / `voice_privacy_code_get`), истории пинга для спарклайна и экспорта диагностического отчёта.
- В **`rust/gui-spike/ui/features/voice.js` и `voice-quality.js` (`G2 Frontend` / `G3 Design`)**:
  - **Разрыв 4 (Неверные пороги `pingQuality` в `voice.js:118-150`)**: в `voice.js` захардкожены выдуманные пороги `QUALITY_FINE_MAX = 120` и `QUALITY_AVERAGE_MAX = 250` без учёта `outboundLossRate`. В оригинальном Discord (`module 613681` @ `2469479`) пороги: **`FINE <= 250 мс` и `loss <= 5%`**, **`AVERAGE`: `250 < ping <= 500 мс` или `5% < loss <= 10%`**, **`BAD`: `ping > 500 мс` или `loss > 10%`**.
  - **Разрыв 5 (Отсутствует попап `RTCConnectionPopout` и модалка `RTCDebugModal`)**: клик по `#voice-ping` (`data-voice-action="ping"` в `voice.js:370`) вообще не имеет обработчика `addEventListener("click", ...)`! Нет попапа со спарклайном пинга (`258×80`), коротким hostname сервера, строками `Средняя задержка` / `Последняя задержка` / `Потеря исходящих пакетов`, вкладкой `Конфиденциальность` (`Код защиты конфиденциальности голосового звонка` 6×5 цифр) и модального окна расширенной отладки (`TRANSPORT` / `OUTBOUND` / `INBOUND`).

---

## 2. Точные константы и пороги индикатора качества (`RTCConnection.quality`)

### 2.1. Перечисление качества `bFR` и состояний `S7L`
**Оракул:** `/home/sanya/re-deep/decoded/3eefa7dae1e2c234_0`, модуль `652215` (`offset 253362..253750`) и модуль `731854` (`offset 10763674`):

| Константа в бандле | Ключ | Значение | Назначение |
|---|---|---|---|
| `bFR.UNKNOWN` | `UNKNOWN` | `"unknown"` | Состояние `!== RTC_CONNECTED` или `lastPing === undefined` |
| `bFR.FINE` | `FINE` | `"fine"` | Зелёный индикатор (отличное соединение) |
| `bFR.AVERAGE` | `AVERAGE` | `"average"` | Жёлтый индикатор (среднее соединение) |
| `bFR.BAD` | `BAD` | `"bad"` | Красный индикатор (плохое соединение) |

**Состояния `S7L` (`RTCConnectionState`):**
1. `DISCONNECTED = "DISCONNECTED"`
2. `AWAITING_ENDPOINT = "AWAITING_ENDPOINT"`
3. `AUTHENTICATING = "AUTHENTICATING"`
4. `CONNECTING = "CONNECTING"`
5. `RTC_DISCONNECTED = "RTC_DISCONNECTED"`
6. `RTC_CONNECTING = "RTC_CONNECTING"`
7. `RTC_CONNECTED = "RTC_CONNECTED"`
8. `NO_ROUTE = "NO_ROUTE"`
9. `ICE_CHECKING = "ICE_CHECKING"`
10. `DTLS_CONNECTING = "DTLS_CONNECTING"`

### 2.2. Формула и точные пороги `quality` (Green / Yellow / Red)
**Оракул:** `/home/sanya/re-deep/decoded/3eefa7dae1e2c234_0`, модуль `613681` (`class tg`, `offset 2469479`):

```javascript
get quality() {
  let e = this.getLastPing();
  return this.state !== eO.S7L.RTC_CONNECTED || void 0 === e
    ? eO.bFR.UNKNOWN
    : e > 500 || (null != this._outboundLossRate && this._outboundLossRate > 10)
      ? eO.bFR.BAD
      : e > 250 || (null != this._outboundLossRate && this._outboundLossRate > 5)
        ? eO.bFR.AVERAGE
        : eO.bFR.FINE;
}
```

| Уровень качества (`bFR`) | Цвет / токен | Условие по `lastPing` (мс) | Условие по `outboundLossRate` (%) | Логический оператор |
|---|---|---|---|---|
| `UNKNOWN` (`"unknown"`) | Серый / скрыт | `state !== "RTC_CONNECTED"` ИЛИ `lastPing === undefined` | — | — |
| `BAD` (`"bad"`, красный) | `--status-danger` / `--background-feedback-critical` | `lastPing > 500` | `outboundLossRate != null && outboundLossRate > 10` | **ИЛИ (`||`)** |
| `AVERAGE` (`"average"`, жёлтый) | `--status-warning` / `--background-feedback-warning` | `250 < lastPing <= 500` | `outboundLossRate != null && 5 < outboundLossRate <= 10` | **ИЛИ (`||`)** (если не `BAD`) |
| `FINE` (`"fine"`, зелёный) | `--status-positive` / `--background-feedback-positive` | `lastPing <= 250` | `outboundLossRate == null \|\| outboundLossRate <= 5` | **И (`&&`)** |

> **Критическое отличие от текущего `rust/gui-spike/ui/features/voice.js:118-150`**:
> В `voice.js` сейчас используются пороги `120 мс` и `250 мс` (`fine <= 120`, `average <= 250`, `bad > 250`), а `outboundLossRate` полностью игнорируется. По эталону Discord зелёный индикатор (`fine`) держится вплоть до **`250 мс`** (включительно) при потерях **`<= 5%`**, жёлтый (`average`) — **`251..500 мс`** или потери **`(5%..10%]`**, красный (`bad`) — **`> 500 мс`** или потери **`> 10%`**.

### 2.3. Буфер пингов, средний пинг, таймауты и расчёт `outboundLossRate`
**Оракулы:**
- Модуль `613681` (`offset 2474800..2504500` в `3eefa7dae1e2c234_0`)
- Модуль `206959` (`offset 10812800..10823900` в `3eefa7dae1e2c234_0`)
- Модуль `731854` (`offset 10763674` в `3eefa7dae1e2c234_0`)

| Параметр / метод | Точное значение / формула из RE-дампа | Ссылка на оракул |
|---|---|---|
| Интервал пинга `MediaEngine` (`O.n8`) | `5000` мс (`5e3`) | `3eefa7dae1e2c234_0`, mod `731854` @ `10763674`, mod `206959` @ `10796200` (`setPingInterval(O.n8)`) |
| Интервал опроса `getFilteredStats` (`O.t1`) | `1000` мс (`1e3`) с таймаутом промиса `1000` мс | `3eefa7dae1e2c234_0`, mod `731854` @ `10763674`, `offset 10860750` |
| Первичный пинг при открытии RTC WS | `Math.round(elapsedMs / 2)` в `RTCControlSocket.onopen` | `3eefa7dae1e2c234_0`, mod `613681` @ `2393600` |
| Ёмкость буфера `this._pings` | Хранит до **`199`** точек `{time: Date.now(), value: pingMs}` (`while (this._pings.length >= 200) this._pings.shift()`) | `3eefa7dae1e2c234_0`, mod `613681` @ `2504200` (`_handlePing`) |
| Порог `_pingBadCount` | Инкрементируется (`this._pingBadCount++`), когда `ping > 500` мс | `3eefa7dae1e2c234_0`, mod `613681` @ `2504260` |
| Обработка таймаута пинга (`_handlePingTimeout`) | При **3 последовательных** таймаутах (`seq[n-1] === seq[n-2]+1 && seq[n-2] === seq[n-3]+1`) записывает в `_handlePing(timeoutMs)`, где fallback `timeoutMs = 4000` мс (`4e3`) | `3eefa7dae1e2c234_0`, mod `613681` @ `2504320`, mod `206959` @ `10823850` |
| `getLastPing()` | `this._pings[this._pings.length - 1]?.value` | `3eefa7dae1e2c234_0`, mod `613681` @ `2475020` |
| `getAveragePing()` | Среднее арифметическое по `this._pings.slice(0, Math.min(this._pings.length, 20))` (до 20 отсчётов; `0`, если буфер пуст или `_socket == null`) | `3eefa7dae1e2c234_0`, mod `613681` @ `2474810` |
| `OutboundLossRate` (`handleStats`) | По всем потокам `rtp.outbound` суммируются `packetsSent` и `packetsLost`. Для двух соседних снимков `prev` и `curr`: `deltaSent = curr.sent - prev.sent`, `deltaLost = curr.lost - prev.lost`. Если `deltaSent === 0` → `0`; если `deltaSent > 0 && deltaLost >= 0` → `clamp(deltaLost / (deltaSent + deltaLost), 0, 1) * 100` | `3eefa7dae1e2c234_0`, mod `206959` @ `10812800` |
| Пороги подсказки в попапе (`badPing`, `badLossRate`) | `badPing = 250` (мс), `badLossRate = 10` (%) | `3eefa7dae1e2c234_0`, mod `978171` @ `7459160` |
| Форматирование в попапе | `averagePing.toFixed(0)`, `lastPing?.toFixed(0)`, `outboundLossRate?.toFixed(1)` | `3eefa7dae1e2c234_0`, mod `978171` @ `7459850..7460350` |
| Короткое имя хоста (`getShortHostname`) | `hostname.split(".")[0]` (например, `"finland10192.discord.media"` → `"finland10192"`) | `3eefa7dae1e2c234_0`, mod `978171` @ `7459580` (`ed.A.getShortHostname`) |

---

## 3. Полная схема статистики `RTCConnection` / `MediaEngine` (`getStats`)

**Оракул:** `/home/sanya/re-deep/decoded/3eefa7dae1e2c234_0`, модуль `206959`, функции нормализации `k(e, t, n, i)`, `G(e)` (outbound video), `x(e)` (inbound video) (`offset 10790700..10795800`), а также `MediaEngineStatsStore` (модуль `624694` @ `5997681`) и `RTCDebugStore` (модуль `412780` @ `9264635`).

### 3.1. Корневой объект снимка `MediaEngineStats`
```ts
interface MediaEngineConnectionStats {
  mediaEngineConnectionId: string; // напр. "Native-0"
  version: number;                 // монотонный счётчик снимков данной сессии
  context: "default" | "stream";   // O.x.DEFAULT ("default") | O.x.STREAM ("stream")
  stats: {
    mediaEngineConnectionId: string;
    transport: TransportStats;
    rtp: {
      inbound: Record<string, Array<InboundAudioStats | InboundVideoStats>>; // ключ — userId
      outbound: Array<OutboundAudioStats | OutboundVideoStats>;
    };
    screenshare: ScreenshareStats | null;
    camera: CameraStats | null;
    clips: ClipsStats | null;
    audioDevice: AudioDeviceStats | null;
  };
}
```

### 3.2. Раздел `transport` (`TransportStats`)
Нормализуется из нативного `d = rawStats.transport` (`function k` @ `10794350`):

| Поле в нормализованном `stats.transport` | Источник в сыром `raw.transport` | Тип / Единицы | Описание |
|---|---|---|---|
| `availableOutgoingBitrate` | `d.sendBandwidth` | `number` (bps) | Оценка доступной исходящей полосы пропускания (BWE) |
| `ping` | `d.rtt` | `number` (ms) | RTT до медиа-сервера |
| `decryptionFailures` | `d.decryptionFailures` | `number` | Число ошибок расшифровки SRTP/DAVE |
| `routingFailures` | `d.routingFailures` | `number` | Ошибки маршрутизации |
| `localAddress` | `d.localAddress` | `string` | Локальный IP-адрес сокета |
| `pacerDelay` | `d.pacerDelay` | `number` (ms) | Задержка pacer-очереди отправки |
| `receiverReports` | `d.receiverReports` | `array` | Сырые блоки RTCP Receiver Reports |
| `receiverBitrateEstimate` | `d.receiverBitrateEstimate` | `number` (bps) | Оценка битрейта получателем (REMB) |
| `outboundBitrateEstimate` | `d.outboundBitrateEstimate` | `number` (bps) | Оценка исходящего битрейта |
| `inboundBitrateEstimate` | `d.inboundBitrateEstimate ?? 0` | `number` (bps) | Оценка входящего битрейта (по умолчанию `0`) |
| `packetsReceived` | `d.packetsReceived` | `number` | Всего получено транспортных пакетов |
| `packetsSent` | `d.packetsSent` | `number` | Всего отправлено транспортных пакетов |
| `secureFramesProtocolVersion` | `d.secureFramesProtocolVersion` | `number` | Версия протокола DAVE E2EE (`0` — выкл, `1` — DAVE v1) |
| `transportDelayMs` | `d.transportDelayMs` | `number` (ms) | Суммарная транспортная задержка |
| `transportPacketCount` | `d.transportPacketCount` | `number` | Счётчик пакетов для расчёта задержки |
| `bytesReceived` | `d.bytesReceived` | `number` (bytes) | Всего принято байт на транспорте |
| `bytesSent` | `d.bytesSent` | `number` (bytes) | Всего отправлено байт на транспорте |

### 3.3. Раздел `rtp.outbound` (Исходящее аудио `type: "audio"`)
Нормализуется из `rawStats.rtp.outbound` (`offset 10794750`):

| Поле | Формула / Источник | Тип / Единицы |
|---|---|---|
| `type` | `"audio"` | `string` |
| `ssrc` | `e.ssrc` | `number` (`u32`) |
| `sinkWant` | `w(n, e.ssrc, !1)` | `string \| number` |
| `sinkWantAsInt` | `w(n, e.ssrc, !0)` | `number` |
| `codec` | `{ id: e.codecPayloadType, name: e.codecName }` (напр. `{id: 120, name: "opus"}`) | `{id: number, name: string}` |
| `bytesSent` | `e.bytesSent` | `number` (bytes) |
| `packetsSent` | `e.packetsSent` | `number` |
| `packetsLost` | `Math.max(0, e.packetsLost)` | `number` (неотрицательное!) |
| `fractionLost` | `100 * e.fractionLost` | `number` (`0..100` %) |
| `audioLevel` | `U(e.audioLevel)` = `e.audioLevel / 32768` | `number` (`0.0..1.0`) |
| `bitrate` | `e.mediaBitrate` | `number` (bps) |
| `bitrateTarget` | `e.targetMediaBitrate` | `number` (bps) |
| `audioDetected` | `e.speaking` | `boolean` |
| `framesCaptured` | `e.framesCaptured` | `number` |
| `framesRendered` | `e.framesRendered` | `number` |
| `noiseCancellerFrames` | `e.noiseCancellerFrames` | `number` |
| `noiseCancellerProcessTime` | `e.noiseCancellerProcessTime` | `number` (ms) |
| `voiceActivityDetectorProcessTime` | `e.voiceActivityDetectorProcessTime` | `number` (ms) |
| `passthroughCount` | `e.passthroughCount` | `number` |
| `encryptSuccessCount` | `e.encryptSuccessCount` | `number` |
| `encryptFailureCount` | `e.encryptFailureCount` | `number` |
| `encryptDuration` | `e.encryptDuration` | `number` |
| `encryptAttempts` | `e.encryptAttempts` | `number` |
| `encryptMaxAttempts` | `e.encryptMaxAttempts` | `number` |
| `encryptMissingKeyCount` | `e.encryptMissingKeyCount` | `number` |
| `sampleRateMismatchPercent` | `e.sampleRateMismatchPercent` | `number` (%) |
| `currentSampleRate` | `e.currentSampleRate` | `number` (Hz, напр. `48000`) |
| `captureProcessingDelayMs` / `captureProcessingFrameCount` | `e.captureProcessingDelayMs` / `e.captureProcessingFrameCount` | `number` |
| `apmProcessTimeMs` / `apmFrameCount` | `e.apmProcessTimeMs` / `e.apmFrameCount` | `number` |
| `sendDelayMs` / `sendPacketCount` / `totalPacketSendDelayMs` | задержки отправки | `number` |

### 3.4. Раздел `rtp.inbound[userId]` (Входящее аудио `type: "audio"`)
Нормализуется из `rawStats.rtp.inbound[userId]` (`offset 10795180`):

| Поле | Формула / Источник | Тип / Единицы |
|---|---|---|
| `type` | `"audio"` | `string` |
| `ssrc` | `e.ssrc` | `number` (`u32`) |
| `codec` | `{ id: e.codecPayloadType, name: e.codecName }` | `{id: number, name: string}` |
| `bytesReceived` | `e.bytesReceived` | `number` (bytes) |
| `packetsReceived` | `e.packetsReceived` | `number` |
| `packetsLost` | `e.packetsLost` | `number` |
| `fractionLost` | `100 * e.fractionLost` | `number` (`0..100` %) |
| `fecPacketsReceived` / `fecPacketsDiscarded` | `e.fecPacketsReceived` / `e.fecPacketsDiscarded` | `number` |
| `audioLevel` | `e.audioLevel / 32768` | `number` (`0.0..1.0`) |
| `audioDetected` | `e.speaking` | `boolean` |
| `currentSampleRate` | `e.currentSampleRate` | `number` (Hz) |
| `jitter` | `e.jitter` | `number` (ms) |
| `jitterBuffer` / `jitterBufferPreferred` | `e.jitterBuffer` / `e.jitterBufferPreferred` | `number` (ms) |
| `decodingCNG` / `decodingMutedOutput` / `decodingNormal` / `decodingPLC` / `decodingPLCCNG` | счётчики декодера NetEq/Opus | `number` |
| `nackCount` | `e.nackCount` | `number` |
| `accelerateRate` / `expandRate` / `preemptiveExpandRate` / `speechExpandRate` / `secondaryDecodedRate` | `100 * e.<rate>` | `number` (`0..100` %) |
| `opSilence` / `opNormal` / `opMerge` / `opExpand` / `opAccelerate` / `opPreemptiveExpand` / `opCNG` | счётчики операций NetEq | `number` |
| `delayEstimate` | `e.delayEstimate` | `number` (ms) |
| `passthroughCount` / `decryptSuccessCount` / `decryptFailureCount` / `decryptDuration` / `decryptAttempts` / `decryptMissingKeyCount` / `decryptInvalidNonceCount` | счётчики расшифровки SRTP / DAVE E2EE | `number` |
| `audioJitterBuffer` / `audioJitterDelay` / `audioJitterTarget` / `audioPlayoutUnderruns` / `relativeReceptionDelay` / `relativePlayoutDelay` | телеметрия буфера воспроизведения | `object` / `number` |

### 3.5. Временные ряды в `RTCDebugStore` (модуль `412780` @ `9264635`)
- **Секции модального окна (`d.zWA` в `652215` @ `239671`)**:
  `TRANSPORT`, `OUTBOUND`, `SCREENSHARE`, `INBOUND`, `STREAMS_OUTBOUND`, `STREAMS_INBOUND`, `CAMERA`, `CLIPS`.
- **Формат ключа секции**: `N(context, section, index) = "${context}:${section}:${index}"` (по умолчанию `"default:TRANSPORT:0"`).
- **25 отслеживаемых во времени метрик (`h`)**, которые превращаются в массив точек `{ value: number, time: number }` длиной до **`600` точек** (`10 минут` при опросе раз в `1000 мс`):
  1. `availableOutgoingBitrate`
  2. `bitrate`
  3. `bitrateTarget`
  4. `bytesReceived`
  5. `bytesSent`
  6. `encoderQualityPsnr`
  7. `encoderQualityVmaf`
  8. `encodeUsage`
  9. `frameRateDecode`
  10. `frameRateEncode`
  11. `frameRateInput`
  12. `frameRateNetwork`
  13. `frameRateRender`
  14. `keyFramesEncoded`
  15. `keyFramesDecoded`
  16. `inboundBitrateEstimate`
  17. `packetsLost`
  18. `packetsReceived`
  19. `packetsSent`
  20. `ping`
  21. `qpSum`
  22. `videoEntropy`
  23. `audioLevel`
  24. `screenshareCapturedFps`
  25. `screenshareCapturedFpsUnique`

---

## 4. DOM-структура, CSS-классы и Canvas-график

### 4.1. Нижний бар статуса голосовой панели (`RTCConnectionStatus`)
**Оракулы:** `/home/sanya/re-deep/voice/ui/01-voice-panel.html`, `/home/sanya/re-deep/decoded/e6811dfca6ffcd34_0` (модуль `473718`), `/home/sanya/re-deep/decoded/3eefa7dae1e2c234_0` (модуль `999751` @ `7465980`).

```html
<div class="rtcConnectionStatusWrapper__06d62">
  <div class="rtcConnectionStatus__06d62 rtcConnectionQualityFine__06d62">
    <div class="pingContainer_cdcd10">
      <div class="clickablePing__06d62 largeClickablePing__06d62" role="button" tabindex="0">
        <div class="ping__06d62 largePing__06d62" aria-label="Голосовая связь подключена, 42 мс">
          <!-- SVG 3 полоски сигнала -->
        </div>
      </div>
    </div>
    <div class="labelWrapper__06d62" role="button" tabindex="0">
      <div class="rtcConnectionStatusLabel__06d62 rtcConnectionStatusConnected__06d62">
        <div class="hoverableStatus_cdcd10 hoverRoll__0263c">
          <div class="default__0263c">
            <div class="text-md/medium_cf4812">Голосовая связь подключена</div>
          </div>
          <div class="hovered__0263c" aria-hidden="true">
            <div class="text-md/medium_cf4812">О голосовом звонке</div>
          </div>
        </div>
      </div>
    </div>
  </div>
  <a class="channel_e131a9" href="/channels/...">...</a>
</div>
```

**Ключевые CSS-правила (модуль `473718` в `e6811dfca6ffcd34_0`):**
- `.rtcConnectionStatusConnected__06d62`: `color: var(--text-feedback-positive)`
- `.rtcConnectionStatusConnecting__06d62`: `color: var(--text-feedback-warning)`
- `.rtcConnectionStatusError__06d62`: `color: var(--text-feedback-critical)`
- `.largePing__06d62`: `width: 18px; height: 18px; padding: 7px; border-radius: var(--radius-sm, 8px)`
- Ховер по иконке пинга подсвечивает фон:
  - `.rtcConnectionQualityFine__06d62 .largePing__06d62:hover`: `background-color: var(--background-feedback-positive)`
  - `.rtcConnectionQualityAverage__06d62 .largePing__06d62:hover`: `background-color: var(--background-feedback-warning)`
  - `.rtcConnectionQualityBad__06d62 .largePing__06d62:hover`: `background-color: var(--background-feedback-critical)`

### 4.2. Попап диагностики соединения (`RTCConnectionPopout`)
**Оракулы:** `/home/sanya/re-deep/decoded/3eefa7dae1e2c234_0` (модуль `978171` @ `7458500`, модуль `917592` @ `6025550`, модуль `229659` @ `1966150`, модуль `546738` @ `4695250`), CSS-модули `203077` (`d1bfcf59dca06829_0`), `300303`/`479137`/`68375`/`873353` (`dc091883931cf040_0`).

| Элемент попапа | CSS-класс в оригинале | Размеры / стили | Содержимое |
|---|---|---|---|
| Внешний контейнер (без E2EE-вкладок) | `.debugPanelStandalone__50387` | `width: 300px; padding: 16px; border-radius: 8px; background: var(--background-surface-high); border: 1px solid var(--border-strong); box-shadow: var(--shadow-high)` | Обёртка одиночной панели |
| Внешний контейнер (с вкладками E2EE) | `.container__50387` | `width: 300px; border-radius: 8px; overflow: hidden` | Панель с таб-баром `.tabs__50387` |
| Вкладки (`H.Rj`) | `.tabs__50387` / `.tabBarItem__50387` | `gap: 16px; padding: 16px 16px 0` | `rtc_debug_panel` («Интеграция» / «Connection») и `rtc_secure_frames` («Конфиденциальность» / «Privacy») |
| Секция отладки | `.debugPanelSection__50387` → `.container_effb26` | `padding: 16px` | Тело вкладки соединения |
| Заголовок статуса в попапе | `.status__66f0f` | Иконка состояния + `Heading` (`text-md/medium`) | Текущий статус (напр. «Голосовая связь подключена» зелёным цветом) |
| Спарклайн пинга (только при `RTC_CONNECTED` и `pings.length > 0`) | `.graphContainer_effb26` → `<canvas>` | `width="258" height="80"`, `border-radius: 4px`, `border: 1px solid var(--border-subtle)`, `background: var(--background-mod-subtle)` | `TimelineGraphView` (`module 546738`): 30 FPS, окно `60 000` мс, 4 горизонтальные линии сетки, цвет линии `BRAND_500` (`--brand-500`) |
| Короткое имя RTC-сервера | `Heading` (`text-sm/semibold`, `text-strong`) | `margin-bottom: 4px` | `getShortHostname(hostname)` = `hostname.split(".")[0]` (напр. `stockholm1024`) |
| Строки метрик (`averagePing`, `lastPing`, `outboundLossRate`) | `.statsRow_effb26` → `.statsText_effb26` (`text-xs/normal`, `text-default`) | `display: flex; justify-content: space-between; margin-bottom: 4px` | 1) `Средняя задержка: <b>{averagePing.toFixed(0)} мс</b>`<br>2) `Последняя задержка: <b>{lastPing.toFixed(0)} мс</b>` (если есть)<br>3) `Потеря исходящих пакетов: <b>{outboundLossRate.toFixed(1)}%</b>` (если `!= null`) |
| Текст-подсказка по порогам | `Text` (`text-xs/normal`, `text-muted`) | Под строками метрик | Ключ `3pFz1P` (`badPing: 250`, `badLossRate: 10`) или `vggaMt` (`badPing: 250`) |
| Разделитель и кнопки действий | `.separator__66f0f` + `.buttons_effb26` (`grid-template-columns: 1fr 1fr; gap: 8px`) | `margin-top: 12px` | Кнопка `Отладка` (`KBoWg9`, вызывает `RTC_DEBUG_MODAL_OPEN`) и `Загрузить журналы` (`EbwFfR` → `i4jeWR` «Готово») |
| Баннер защищённого соединения (внизу попапа) | `.securedBanner__66f0f` | `display: flex; align-items: center; gap: 4px; color: var(--text-feedback-positive)` | `LockIcon` + `Защищено сквозным шифрованием` (`3BogKe`, при DAVE) или `Защищённое соединение` (`ETIVvg`, при DTLS/SRTP) |
| Вкладка `rtc_secure_frames` (DAVE E2EE) | `.container__9351a` → `.codeGrid__9351a` → `.code__1a1f5` | `--secure-frames-columns: 3; display: grid; grid-template-columns: repeat(3, 1fr); gap: 8px; font-family: var(--font-code)` | Заголовок `Код защиты конфиденциальности голосового звонка` (`cTQI5t`), 6 блоков по 5 цифр (`30` десятичных цифр из `epochAuthenticator`), кнопка `Копировать` (`OpuAlK` → `t5VZ88` «Скопировано») |

---

## 5. Словарь ключей локализации (`i18n`) из `/home/sanya/re-deep/ru-catalog.json` и `en-catalog.json`

Все ключи проверены напрямую по `/home/sanya/re-deep/ru-catalog.json` и `/home/sanya/re-deep/en-catalog.json`:

| Хеш-ключ (`i18n`) | Русская строка (`ru-catalog.json`) | Английская строка (`en-catalog.json`) | Где используется |
|---|---|---|---|
| `daXg45` | `Голосовая связь подключена` | `Voice Connected` | Статус `RTC_CONNECTED` (аудио) |
| `HtVOdd` | `Видео подключено` | `Video Connected` | Статус `RTC_CONNECTED` (с видео) |
| `WsOisp` | `О голосовом звонке` | `Voice Details` | Ховер-текст статуса (`.hovered__0263c`) / тултип |
| `IlHdW8` | `Сведения о видео` | `Video Details` | Ховер-текст статуса при активном видео |
| `MzW9sN` | `Подключение` | `Connecting` | Статус `CONNECTING` |
| `GxXwE2` | `Аутентификация` | `Authenticating` | Статус `AUTHENTICATING` |
| `uQle7a` | `Ожидание голосового сервера` | `Waiting for Voice Server` | Статус `AWAITING_ENDPOINT` |
| `Gp51dl` | `Подключение к RTC` | `RTC Connecting` | Статус `RTC_CONNECTING` |
| `rdCyA/` | `Проверка маршрута` | `Checking Route` | Статус `ICE_CHECKING` |
| `UvB3gV` | `Подключение DTLS` | `DTLS Connecting` | Статус `DTLS_CONNECTING` |
| `mGhOIi` | `Не установлен маршрут` | `No Route` | Статус `NO_ROUTE` (ошибка) |
| `M7LDmE` | `RTC отключено` | `RTC Disconnected` | Статус `RTC_DISCONNECTED` |
| `NLKQbx` | `Отключено` | `Disconnected` | Статус `DISCONNECTED` |
| `MBY1Pm` | `Интеграция` | `Connection` | Вкладка `rtc_debug_panel` в попапе |
| `zC6o3s` | `Конфиденциальность` | `Privacy` | Вкладка `rtc_secure_frames` в попапе |
| `X58/lN` | `Средняя задержка: **{averagePing} мс**` | `Average ping: **{averagePing} ms**` | Строка средней задержки в попапе |
| `6iv2TF` | `Последняя задержка: **{lastPing} мс**` | `Last ping: **{lastPing} ms**` | Строка последней задержки в попапе |
| `VIBJM+` | `Потеря исходящих пакетов: **{outboundLossRate}%**` | `Outbound packet loss rate: **{outboundLossRate}%**` | Строка потерь пакетов в попапе |
| `3pFz1P` | `При задержке {badPing} мс или больше звук может отставать. При потере более {badLossRate}% исходящих пакетов ваш голос может искажаться. Если проблема не исчезнет, отключитесь и повторите попытку. Подробности можно найти в [руководстве по устранению неполадок]({url}).` | `You may notice delayed audio at {badPing} ms or higher. You may sound robotic if your packet loss rate is over {badLossRate}%...` | Подсказка в попапе при `outboundLossRate != null` (`badPing=250`, `badLossRate=10`) |
| `vggaMt` | `При задержке {badPing} мс или больше звук может отставать. Если проблема не исчезнет, отключитесь и повторите попытку. Подробности можно найти в [руководстве по устранению неполадок]({url}).` | `You may notice delayed audio at {badPing} ms or higher...` | Подсказка в попапе при `outboundLossRate == null` (`badPing=250`) |
| `KBoWg9` | `Отладка` | `Debug` | Кнопка открытия `RTCDebugModal` |
| `EbwFfR` | `Загрузить журналы` | `Upload Logs` | Кнопка выгрузки отладочных логов |
| `i4jeWR` | `Готово` | `Done` | Состояние кнопки после выгрузки логов |
| `3BogKe` | `Защищено сквозным шифрованием` | `End-to-end encrypted` | Баннер внизу попапа при DAVE E2EE |
| `ETIVvg` | `Защищённое соединение` | `Secure Connection` | Баннер внизу попапа при стандартном SRTP |
| `cTQI5t` | `Код защиты конфиденциальности голосового звонка` | `Voice Privacy Code` | Заголовок вкладки DAVE E2EE |
| `OpuAlK` | `Копировать` | `Copy` | Кнопка копирования кода защиты |
| `t5VZ88` | `Скопировано` | `Copied` | Состояние кнопки после копирования |
| `B9JNsl` | `В этом звонке вас слышат только друзья. Больше никто — даже Вампус! — не может вас подслушать!` | `Only you and your friends on this call can hear you. Nobody else – not even Wumpus – can listen in!` | Описание внизу вкладки Конфиденциальность |

---

## 6. Матрица расхождений (Gap Analysis) с текущим клиентом `discord-lab`

| № | Область / Компонент | Эталон Discord (`/home/sanya/re-deep/`) | Текущая реализация в `discord-lab` | Критичность | Целевой отдел |
|---|---|---|---|---|---|
| **GAP-22-1** | Пороги качества соединения (`quality`: `fine` / `average` / `bad` / `unknown`) | `FINE`: `ping <= 250` и `loss <= 5%`; `AVERAGE`: `250 < ping <= 500` или `5% < loss <= 10%`; `BAD`: `ping > 500` или `loss > 10%`; `UNKNOWN` вне `RTC_CONNECTED` (`mod 613681` @ `2469479`) | В `rust/gui-spike/ui/features/voice.js:118-150` захардкожено `QUALITY_FINE_MAX = 120`, `QUALITY_AVERAGE_MAX = 250`, а `outboundLossRate` не учитывается вовсе. В `discord-voice` функции оценки качества нет | **P1 (High)** | `G5 Voice`, `G2 Frontend` |
| **GAP-22-2** | Буфер пинга `_pings` (199 точек), `getAveragePing` (20 точек), `_pingBadCount` и таймаут 3 подряд пингов (`4000 мс`) | `_pings` до 199 точек `{time, value}`, `getAveragePing()` по `slice(0, 20)`, `_pingBadCount` при `ping > 500`, 3 подряд таймаута → `_handlePing(4000)` (`mod 613681` @ `2504200`) | В `session.rs:555` хранится только скаляр `last_rtt_ms: Option<u64>`. В `stats.rs` `StatsWindow` считает среднее по всему окну (32), без буфера 199 точек и без обработки 3 подряд таймаутов пинга | **P1 (High)** | `G5 Voice` |
| **GAP-22-3** | Расчёт `OutboundLossRate` по дельтам `packetsSent` и `packetsLost` и агрегация `RtcConnectionStats` в `VoiceSessionDriver` | `deltaLost / (deltaSent + deltaLost) * 100` (`clamp 0..100%`), нормализованный снимок `transport` + `rtp.outbound` + `rtp.inbound[userId]` (`mod 206959` @ `10794300`, `10812800`) | `SessionStats` (`stats.rs`) и `ReceptionStats` (`reception.rs`) существуют изолированно и не подключены к `VoiceSessionDriver` (`session.rs`); нет агрегации `transport` / `outbound` / `inbound` | **P1 (High)** | `G5 Voice` |
| **GAP-22-4** | BFF-модуль и Tauri IPC-контракт диагностики голоса (`voice_diagnostics`) | Ежесекундный снимок `MEDIA_ENGINE_CONNECTION_STATS`, история пинга, короткое имя хоста `getShortHostname`, статус E2EE (`secureFramesProtocolVersion`, 30-значный `Voice Privacy Code`) | В `rust/gui-spike/src/` нет модуля `voice_diagnostics.rs` и нет IPC-команд получения статистики RTC/истории пинга/кода конфиденциальности DAVE | **P1 (High)** | `G1 Backend` |
| **GAP-22-5** | Попап `RTCConnectionPopout` по клику на `#voice-ping` / статус и модальное окно `RTCDebugModal` | Попап `300px` с вкладками `Интеграция` / `Конфиденциальность`, спарклайном `258×80`, `getShortHostname`, `averagePing`/`lastPing`/`outboundLossRate`, подсказкой `3pFz1P`/`vggaMt`, баннером `3BogKe`/`ETIVvg` и окном `Отладка` (`TRANSPORT`, `OUTBOUND`, `INBOUND`) | Кнопка `#voice-ping` в `rust/gui-spike/ui/features/voice.js:370` не открывает попап (нет обработчика клика); отсутствует UI-модуль диагностики и секции `TRANSPORT`/`OUTBOUND`/`INBOUND` | **P1 (High)** | `G2 Frontend`, `G3 Design` |
