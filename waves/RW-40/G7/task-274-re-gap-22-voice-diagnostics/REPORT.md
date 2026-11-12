# Отчёт о выполнении задачи `task-274` (`RE-GAP-22`: Voice WebRTC Diagnostics & Connection Stats Overlay)

- **Batch / Карточка:** `RW-40` / `task-274` (`RE-GAP-22: Voice WebRTC Diagnostics & Connection Stats Overlay`)
- **Отдел:** `G7 Reverse Engineering & Parity Discovery (G7 RE / REA)`
- **Ветка:** `re/task-274-voice-diagnostics-audit` (HEAD: `b35c3e1120db4ae42c0b0da15649a5d83baaa224`)
- **Дата:** 2026-10-31

---

## 1. Сделано

1. **Проведён полный RE-аудит оракулов `reference-oracle/`** по панели статуса голосового соединения (`RTCConnectionStatus`), попапу сведений о звонке (`RTCConnectionPopout`), модальному окну расширенной диагностики (`RTCDebugModal` / `PopoutWindowRTCDebug`) и пайплайну сбора/нормализации метрик `RTCConnection` / `MediaEngineStatsStore` / `RTCDebugStore`:
   - В чанке `reference-oracle/decoded/3eefa7dae1e2c234_0` декомпилированы и задокументированы с точными смещениями:
     - **Модуль `613681` (`offset 2463706..2505000`, `class tg` = `RTCConnection`)**: точная формула геттера `quality` (`bFR.FINE` при `lastPing <= 250` мс и `outboundLossRate <= 5%`; `bFR.AVERAGE` при `250 < lastPing <= 500` мс или `5% < outboundLossRate <= 10%`; `bFR.BAD` при `lastPing > 500` мс или `outboundLossRate > 10%`; `bFR.UNKNOWN` вне `RTC_CONNECTED` или без пинга), кольцевой буфер `_pings` до `199` элементов (`while (this._pings.length >= 200) this._pings.shift()`), расчёт `getAveragePing()` по первым `min(len, 20)` отсчётам, счётчик `_pingBadCount` при `ping > 500` мс и обработка 3 подряд таймаутов пинга (`_handlePingTimeout` → `_handlePing(4000)`).
     - **Модуль `206959` (`offset 10790700..10824500`, `NativeConnection`)**: нормализатор `function k(e, t, n, i)` для `transport`, `rtp.outbound` (аудио/видео), `rtp.inbound[userId]` (аудио/видео), `screenshare`, `camera`, `clips`, `audioDevice`, а также формула `OutboundLossRate = clamp(deltaLost / (deltaSent + deltaLost), 0, 1) * 100`.
     - **Модуль `731854` (`offset 10763674`) и `652215` (`offset 239671..253750`)**: интервал опроса статистики `1000` мс (`O.t1 = 1e3`), интервал пинга `5000` мс (`O.n8 = 5e3`), перечисления `S7L` (10 состояний RTC), `bFR` (4 уровня качества) и `zWA` (8 секций `RTCDebugModal`: `TRANSPORT`, `OUTBOUND`, `SCREENSHARE`, `INBOUND`, `STREAMS_OUTBOUND`, `STREAMS_INBOUND`, `CAMERA`, `CLIPS`).
     - **Модуль `624694` (`offset 5997681`) и `412780` (`offset 9264635`)**: `MediaEngineStatsStore` (скользящее окно из `30` снимков) и `RTCDebugStore` (25 метрик во временных рядах до `600` точек `{value, time}`).
     - **Модули `978171` (`offset 7458500`), `999751` (`offset 7465980`), `917592` (`offset 6025550`), `998759` (`offset 7464200`), `229659` (`offset 1966150`) и `546738` (`offset 4695250`)**: UI-компоненты `RTCConnectionStatus`, `RTCConnectionPopout` (`300px`, вкладки `rtc_debug_panel` / `rtc_secure_frames`, Canvas-спарклайн `258×80`, `getShortHostname`, `averagePing`/`lastPing`/`outboundLossRate`, баннер шифрования, сетка 6×5 цифр `Voice Privacy Code`).
   - Извлечены точные CSS-селекторы из `reference-oracle/voice/ui/01-voice-panel.html`, `e6811dfca6ffcd34_0` (модуль `473718`), `d1bfcf59dca06829_0` (модуль `203077`) и `dc091883931cf040_0` (модули `300303`, `479137`, `68375`, `873353`).
   - Сверена таблица всех 30+ ключей локализации по `reference-oracle/ru-catalog.json` и `reference-oracle/en-catalog.json`.
2. **Выполнено сравнение с текущей кодовой базой `discord-lab`** (`rust/crates/discord-voice/src/{stats.rs,rtcp.rs,reception.rs,session.rs}`, `rust/gui-spike/src/{voice_devices.rs,voice_stage.rs,net.rs}`, `rust/gui-spike/ui/features/{voice.js,voice-quality.js}`):
   - Обнаружено 5 конкретных расхождений (`GAP-22-1..5`), включая неверные пороги `QUALITY_FINE_MAX = 120` в `rust/gui-spike/ui/features/voice.js:118` (вместо `250 мс` и учёта `outboundLossRate` `5%`/`10%`), отсутствие интеграции `SessionStats`/`ReceptionStats` в `VoiceSessionDriver`, отсутствие BFF-модуля `voice_diagnostics.rs` и отсутствие обработчика клика по `#voice-ping` для открытия попапа `RTCConnectionPopout`.
3. **Оформлены артефакты аудита и карточки бэклога для PM**:
   - `/home/user/orca/project/waves/RW-40/RE-AUDIT/RE-GAP-22-VOICE-DIAGNOSTICS.md`
   - `/home/user/orca/project/waves/RW-40/PM/BACKLOG-RE-22.md` (карточки `RE-22-01` для `G5 Voice`, `RE-22-02` для `G1 Backend`, `RE-22-03` для `G3 Design`, `RE-22-04` для `G2 Frontend`).

---

## 2. Red / Green / Targeted / Gate прогоны

- Задача относится к отделу `G7 RE / REA` (read-only аудит и документация в `waves/RW-40/`). Продуктовый код в `rust/` и `ui/` не изменялся.
- Все данные, формулы, константы, селекторы и переводы верифицированы прямыми запросами к эталонному дампу `reference-oracle/`.

---

## 3. Созданные файлы и контрольные суммы (`md5sum`)

| Файл | MD5 | Назначение |
|---|---|---|
| `/home/user/orca/project/waves/RW-40/RE-AUDIT/RE-GAP-22-VOICE-DIAGNOSTICS.md` | `5524a357942a6f4c1f9426881a604720` | Полный RE-аудит диагностики голосового соединения и WebRTC-статистики |
| `/home/user/orca/project/waves/RW-40/PM/BACKLOG-RE-22.md` | `1028f4617cb1f79068afc751ba486998` | Карточки модернизации `RE-22-01..04` для отделов `G5`, `G1`, `G3`, `G2` |
| `/home/user/orca/project/waves/RW-40/G7/task-274-re-gap-22-voice-diagnostics/REPORT.md` | *(см. вывод `md5sum` при сдаче)* | Итоговый отчёт воркера G7 по шаблону `PROCESS.md` §4.2 |

---

## 4. Риски и зависимости

1. **Обратная совместимость существующих юнит-тестов `voice.js`**: при исправлении порогов `pingQuality` в карточке `RE-22-04` (`120/250 мс` → `250/500 мс` + `5%/10% loss`) необходимо синхронно обновить оракулы в тестах `voice.js` и `waves/RW-40/DESIGN/voice-spec.json`, где ранее пороги `120/250` были помечены как временное допущение `[A]`.
2. **Безопасность диагностических логов (`P0 Security`)**: при реализации `RE-22-02` (`voice_diagnostics.rs`) и кнопки «Загрузить журналы» (`EbwFfR`) необходимо строго гарантировать, что `secret_key` из `OP 4 SESSION_DESCRIPTION` и токены сессии никогда не сериализуются в диагностический дамп.
