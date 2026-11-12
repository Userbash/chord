# BACKLOG-RE-22 — Карточки модернизации по итогам RE-аудита `RE-GAP-22` (Voice WebRTC Diagnostics & Connection Stats Overlay)

- **Источник:** `/home/user/orca/project/waves/RW-40/RE-AUDIT/RE-GAP-22-VOICE-DIAGNOSTICS.md` (`task-274`)
- **Дата:** 2026-10-31
- **Охват отделов:** `G5 Voice / Platform`, `G1 Backend (Rust Core & Tauri BFF)`, `G2 Frontend (WebView UI & DOM/A11y)`, `G3 Design (UX/UI & Specs)`

---

## Сводная таблица карточек бэклога

| ID карточки | Отдел | Приоритет | Заголовок | Зависимости |
|---|---|---|---|---|
| **RE-22-01** | `G5 Voice` | `P1` | Эталонная модель `ConnectionQuality` (`250/500 мс`, `5%/10%`), буфер `_pings` (199/20) и `RtcConnectionStats` в `discord-voice` | Нет |
| **RE-22-02** | `G1 Backend` | `P1` | BFF-модуль `voice_diagnostics.rs` и IPC-контракт снимков RTC-статистики, спарклайна пинга и 30-значного `Voice Privacy Code` | `RE-22-01` |
| **RE-22-03** | `G3 Design` | `P2` | Спецификация и CSS-токены попапа `RTCConnectionPopout` (`300px`, спарклайн `258×80`, вкладки E2EE) и модалки `RTCDebugModal` | Нет |
| **RE-22-04** | `G2 Frontend` | `P1` | Исправление порогов `pingQuality` (`250/500 мс`, `5%/10% loss`) и реализация попапа `RTCConnectionPopout` + `RTCDebugModal` в UI | `RE-22-02`, `RE-22-03` |

---

### Карточка `RE-22-01` (`G5 Voice`): Эталонная модель `ConnectionQuality`, буфер `_pings` и `RtcConnectionStats` в `discord-voice`

- **Отдел:** `G5 Voice / Platform`
- **Приоритет:** `P1`
- **Цель:** Привести расчёт качества голосового соединения, буфер пингов, обработку таймаутов пинга и агрегацию RTP/RTCP/Transport-метрик в крейте `discord-voice` к 100% паритету с `RTCConnection` (модуль `613681`) и `NativeConnection` (модуль `206959`) оригинального клиента Discord.
- **Оракул (`reference-oracle/`):**
  - `decoded/3eefa7dae1e2c234_0`, модуль `613681` (`offset 2469479`, `2474810`, `2504200..2504450`):
    - `ConnectionQuality` (`bFR`): `Unknown` (вне `RTC_CONNECTED` или без `last_ping`), `Bad` (`last_ping > 500` мс ИЛИ `outbound_loss_rate > 10.0`), `Average` (`last_ping > 250` мс ИЛИ `outbound_loss_rate > 5.0`), `Fine` (`last_ping <= 250` мс И `outbound_loss_rate <= 5.0` либо `None`).
    - Буфер пингов `PingHistory`: ёмкость `199` точек `{ time_ms: u64, value_ms: u64 }` (`while len >= 200 { pop_front }`), `average_ping_ms()` по срезу первых `min(len, 20)` элементов (`0`, если пусто), `ping_bad_count` инкрементируется при `value_ms > 500`, 3 последовательных таймаута пинга (`seq[n-1] == seq[n-2]+1 && seq[n-2] == seq[n-3]+1`) записывают пинг `4000` мс (`DEFAULT_PING_TIMEOUT_MS = 4000`).
  - `decoded/3eefa7dae1e2c234_0`, модуль `206959` (`offset 10794300..10795800`, `10812800`):
    - Формула `outbound_loss_rate`: для двух соседних снимков `delta_sent = curr.packets_sent - prev.packets_sent`, `delta_lost = curr.packets_lost - prev.packets_lost`; при `delta_sent == 0` → `0.0`, при `delta_sent > 0 && delta_lost >= 0` → `((delta_lost as f64) / ((delta_sent + delta_lost) as f64)).clamp(0.0, 1.0) * 100.0`.
    - Снимок `RtcConnectionStatsSnapshot`: `transport` (`ping`, `bytes_sent`, `bytes_received`, `packets_sent`, `packets_received`, `decryption_failures`, `secure_frames_protocol_version`, `short_hostname`), `outbound` (`ssrc`, `codec`, `bytes_sent`, `packets_sent`, `packets_lost`, `fraction_lost_pct`, `bitrate_bps`, `target_bitrate_bps`, `encrypt_success_count`, `encrypt_failure_count`), `inbound` по `ssrc`/`user_id` (`packets_received`, `packets_lost`, `fraction_lost_pct`, `jitter_ms`, `decrypt_success_count`, `decrypt_failure_count`).
- **Write-Scope (файлы):**
  - `rust/crates/discord-voice/src/stats.rs`
  - `rust/crates/discord-voice/src/session.rs`
- **Definition of Done (DoD):**
  1. В `stats.rs` реализованы `ConnectionQuality` (`Unknown`, `Fine`, `Average`, `Bad`), `PingSample`, `PingHistory` (лимит `199` элементов, среднее по `20`, `ping_bad_count` при `> 500`, 3 подряд таймаута → `4000` мс), функция `compute_outbound_loss_rate_pct(prev_sent, prev_lost, curr_sent, curr_lost) -> Option<f64>` и структура `RtcConnectionStatsSnapshot` + `short_hostname(endpoint) -> &str`.
  2. `VoiceSessionDriver` в `session.rs` интегрирован с `SessionStats` / `PingHistory` и предоставляет `rtc_stats_snapshot(&self) -> RtcConnectionStatsSnapshot` и `connection_quality(&self) -> ConnectionQuality`.
  3. Не сломан ни один существующий тест в `discord-voice`.
- **Тестовая стратегия (9 измерений):**
  - *Позитивные:* расчёт `Fine` (`ping = 45`, `loss = 0.0`), `Average` (`ping = 300`, `loss = 0.0` и `ping = 80`, `loss = 6.5`), `Bad` (`ping = 501`, `loss = 0.0` и `ping = 40`, `loss = 10.1`), `short_hostname("finland10192.discord.media:443") == "finland10192"`.
  - *Негативные / Ошибки:* `Unknown` при фазе `!== Connected` или пустом `PingHistory`; 2 непоследовательных таймаута пинга (`seq 1, 2, 4`) не триггерят fallback `4000` мс, а 3 последовательных (`seq 5, 6, 7`) записывают `4000` мс.
  - *Граничные:* точные границы `ping = 250` (`Fine`), `ping = 251` (`Average`), `ping = 500` (`Average`), `ping = 501` (`Bad`), `loss = 5.0%` (`Fine`), `loss = 5.0001%` (`Average`), `loss = 10.0%` (`Average`), `loss = 10.0001%` (`Bad`), переполнение `PingHistory` на 200-м элементе (остаётся 199).
  - *Пустые / Восстановление / Наблюдаемость:* пустой `PingHistory` возвращает `average_ping_ms() == 0`; сброс при реконнекте сессии.

---

### Карточка `RE-22-02` (`G1 Backend`): BFF-модуль `voice_diagnostics.rs` и IPC-контракт диагностики голоса

- **Отдел:** `G1 Backend (Rust Core & Tauri BFF)`
- **Приоритет:** `P1`
- **Цель:** Создать BFF-модуль `rust/gui-spike/src/voice_diagnostics.rs`, который формирует сериализуемый JSON-контракт для попапа `RTCConnectionPopout` и модального окна `RTCDebugModal` (включая историю точек для спарклайна `258×80`, `RTCDebugStore` ряды до `600` точек и форматирование 30-значного `Voice Privacy Code` 6×5 цифр из `epochAuthenticator`).
- **Оракул (`reference-oracle/`):**
  - `decoded/3eefa7dae1e2c234_0`, модуль `978171` (`offset 7458500..7465900`) — поля попапа (`state`, `quality`, `shortHostname`, `averagePing`, `lastPing`, `outboundLossRate`, `pings`, `isEndToEndEncrypted`, `secureFramesProtocolVersion`).
  - `decoded/3eefa7dae1e2c234_0`, модуль `412780` (`offset 9264635`) — секции `zWA` (`TRANSPORT`, `OUTBOUND`, `INBOUND`, `SCREENSHARE`, `CAMERA`, `CLIPS`, `STREAMS_OUTBOUND`, `STREAMS_INBOUND`) и скользящее окно `600` точек `{ value, time }`.
  - `decoded/3eefa7dae1e2c234_0`, модуль `917592` (`offset 6025550`) и `998759` (`offset 7464200`) — `Voice Privacy Code`: 6 групп по 5 десятичных цифр (`30` цифр).
- **Write-Scope (файлы):**
  - `rust/gui-spike/src/voice_diagnostics.rs`
  - `rust/gui-spike/tests/voice_diagnostics_contract.rs`
- **Definition of Done (DoD):**
  1. Реализован `voice_diagnostics.rs` со структурами `VoiceDiagnosticsPopoutDto`, `VoiceDebugModalSection` (`Transport`, `Outbound`, `Inbound`, `Screenshare`, `Camera`, `Clips`, `StreamsOutbound`, `StreamsInbound`), `TimeSeriesPoint`, `VoiceDebugHistoryStore` (лимит `600` точек на метрику и `30` снимков `MediaEngineStatsStore`) и функцией форматирования 30-значного `Voice Privacy Code` (6 групп по 5 цифр).
  2. Никакие секретные ключи (`secret_key`, токены сессии) не попадают в диагностический DTO или выгружаемый журнал (маскирование / исключение на уровне типов).
  3. Контрактный тест `rust/gui-spike/tests/voice_diagnostics_contract.rs` покрывает все 9 измерений тестирования и убивает 5/5 мутаций.
- **Тестовая стратегия (9 измерений):**
  - *Позитивные / Контракт:* сериализация `VoiceDiagnosticsPopoutDto` с `averagePing` (`toFixed(0)`), `lastPing` (`toFixed(0)`), `outboundLossRate` (`toFixed(1)`), 6 группами по 5 цифр `privacyCodeChunks`.
  - *Граничные:* вытеснение 601-й точки в `VoiceDebugHistoryStore` (остаётся 600 точек), вытеснение 31-го снимка в `MediaEngineStatsStore` (остаётся 30).
  - *Безопасность / Наблюдаемость:* проверка отсутствия `secret_key` и `token` в экспортируемом JSON-журнале диагностики.

---

### Карточка `RE-22-03` (`G3 Design`): Спецификация и CSS-токены `RTCConnectionPopout` и `RTCDebugModal`

- **Отдел:** `G3 Design (UX/UI & Specs)`
- **Приоритет:** `P2`
- **Цель:** Зафиксировать дизайн-спецификацию и CSS-токены для индикатора качества голосовой связи (`.rtcConnectionQualityFine` / `.rtcConnectionQualityAverage` / `.rtcConnectionQualityBad`), попапа `RTCConnectionPopout` (`width: 300px`, Canvas-спарклайн `258×80`, вкладки `Интеграция` / `Конфиденциальность`, сетка 3 колонки `--secure-frames-columns: 3` для `Voice Privacy Code`) и модального окна `RTCDebugModal` (`854×480`).
- **Оракул (`reference-oracle/`):**
  - `voice/ui/01-voice-panel.html`
  - `decoded/e6811dfca6ffcd34_0` (модуль `473718`), `decoded/d1bfcf59dca06829_0` (модуль `203077`), `decoded/dc091883931cf040_0` (модули `300303`, `479137`, `68375`, `873353`).
- **Write-Scope (файлы):**
  - `waves/RW-40/DESIGN/VOICE-DIAGNOSTICS-SPEC.md`
  - `rust/gui-spike/ui/styles/voice-diagnostics.css` (по согласованию координатора)
- **Definition of Done (DoD):**
  1. Описаны все размеры (`300px` попап, `258×80` спарклайн, `18×18px` иконка пинга с `padding: 7px` и `border-radius: 8px`, `854×480` модалка), цветовые токены (`--text-feedback-positive`, `--text-feedback-warning`, `--text-feedback-critical`, `--background-feedback-positive/warning/critical`, `--brand-500`), состояния фокуса и клавиатурной навигации (WCAG AA).

---

### Карточка `RE-22-04` (`G2 Frontend`): Пороги `pingQuality` (`250/500 мс`, `5%/10% loss`), попап `RTCConnectionPopout` и `RTCDebugModal`

- **Отдел:** `G2 Frontend (WebView UI & DOM/A11y)`
- **Приоритет:** `P1`
- **Цель:** Исправить пороги функции `pingQuality` в `rust/gui-spike/ui/features/voice.js` по оракулу модуля `613681` (`FINE <= 250 мс && loss <= 5%`, `AVERAGE <= 500 мс && loss <= 10%`, `BAD > 500 мс || loss > 10%`) и реализовать открытие попапа `RTCConnectionPopout` по клику на `#voice-ping` / `#voice-status-label`, включая спарклайн пинга `258×80`, вкладку `Конфиденциальность` (код 6×5 цифр с кнопкой `Копировать` → `Скопировано`) и переход в модалку `Отладка` (`TRANSPORT` / `OUTBOUND` / `INBOUND`).
- **Оракул (`reference-oracle/`):**
  - `decoded/3eefa7dae1e2c234_0`: модули `613681` (`offset 2469479`), `978171` (`offset 7458500`), `999751` (`offset 7465980`), `917592` (`offset 6025550`), `229659` (`offset 1966150`), `546738` (`offset 4695250`).
  - `ru-catalog.json` / `en-catalog.json`: ключи `WsOisp`, `IlHdW8`, `MBY1Pm`, `zC6o3s`, `X58/lN`, `6iv2TF`, `VIBJM+`, `3pFz1P`, `vggaMt`, `KBoWg9`, `EbwFfR`, `i4jeWR`, `3BogKe`, `ETIVvg`, `cTQI5t`, `wKxADe`, `OpuAlK`, `t5VZ88`, `B9JNsl`.
- **Write-Scope (файлы):**
  - `rust/gui-spike/ui/features/voice-diagnostics.js`
  - `rust/gui-spike/ui/features/voice.js` (подключение `pingQuality(ping, outboundLossRate)` и обработчика клика по `#voice-ping`)
  - `tests/unit/voice-diagnostics.test.mjs`
- **Definition of Done (DoD):**
  1. `pingQuality(ping, outboundLossRate)` возвращает `"fine"` при `ping <= 250 && (loss == null || loss <= 5)`, `"average"` при `(ping <= 500 && loss <= 10)` (если не `"fine"`), `"bad"` при `ping > 500 || loss > 10`, и `null` при невалидном пинге.
  2. Клик по `#voice-ping` открывает/закрывает попап `RTCConnectionPopout` (`role="dialog"`, `aria-labelledby`, закрытие по `Escape`), рендерит Canvas-спарклайн `258×80`, `shortHostname`, `Средняя задержка: {avg} мс`, `Последняя задержка: {last} мс`, `Потеря исходящих пакетов: {loss}%`, предупреждение (`badPing=250`, `badLossRate=10`), баннер шифрования (`Защищено сквозным шифрованием` / `Защищённое соединение`), вкладку `Конфиденциальность` (сетка 6×5 цифр + копирование) и кнопку `Отладка` (открывающую секции `TRANSPORT` / `OUTBOUND` / `INBOUND`).
  3. Юнит-тесты `tests/unit/voice-diagnostics.test.mjs` проходят на 100% и проверяют все 9 измерений.
