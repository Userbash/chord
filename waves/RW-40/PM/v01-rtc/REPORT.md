# Отчёт: V-01-RTC-SESSION-OFFLINE — Ревизия и расширение офлайн-тестов машины состояний RTC-сессии (`discord-voice`)

- **Карточка:** `V-01-RTC-SESSION-OFFLINE`
- **Волна:** `RW-40`
- **Роль:** Воркер G5 Platform / Voice
- **Контракт-оракул:** `waves/RW-40/VOICE/V01-SESSION-CONTRACT.md` (§2–§6), `waves/RW-40/VOICE/G5-PREREQ.md`, `waves/RW-40/DESIGN/VOICE.md`, золотые дампы `fixtures/voice/rtc1-frames.jsonl`, `fixtures/voice/rtc-frames.jsonl`, `fixtures/voice/rtc1-summary.json`, `fixtures/voice/rtc-summary.json`.
- **Изоляция сборки:** все вызовы `cargo` выполнялись исключительно с `CARGO_TARGET_DIR=/tmp/target-g5-voice` и флагом `--offline`.

---

## 1. Ревизия исходного состояния `rust/crates/discord-voice`

До начала работы проведён аудит всех модулей и тестов крейта `discord-voice` (базовый прогон сохранён в `waves/RW-40/PM/v01-rtc/baseline-test.log`: 125 интеграционных тестов в 9 наборах + 6 unit-тестов):

1. **Отсутствие машины состояний RTC-сессии (`session.rs`)**:
   - В `src/lib.rs` присутствовали только парсеры отдельных кадров (`Hello`, `Identify`, `SelectProtocol`, `Ready`, `SessionDescription`) и построитель URL `VoiceServerUpdate::websocket_url()`.
   - Не существовало модуля `session` с состояниями `Disconnected` / `Connecting` / `Connected` / `Reconnecting` / `Closed`, фазами рукопожатия (`AwaitingHello` → `Identifying` → `SelectingProtocol` → `AwaitingDescription` → `Established`), таймерами (`Hello`, `Join`, `Description`, `Heartbeat`, `HeartbeatAck`, `Reconnect`) и детерминированным методом `Session::on(event, now_ms)`.
2. **Неполнота типизации и сериализации кадров в `src/lib.rs`**:
   - Кадр `op6 HeartbeatAck` проваливался в `VoicePayload::Other` вместо типизированного `VoicePayload::HeartbeatAck(HeartbeatAck { t })`.
   - Отсутствовал сериализатор исходящего `op3 Heartbeat` (`{"op":3,"d":{"t":...,"seq_ack":...}}`) и `op16` (`{"op":16,"d":{}}`).
   - В `SelectProtocol::to_json()` (`op1`) отсутствовали поля `experiments` и `rtc_connection_id`, зафиксированные в золотых дампах `rtc1-frames.jsonl` и `rtc-frames.jsonl` (`"experiments":["fixed_keyframe_interval"]`).
   - В `parse_session_description()` отсутствовала fail-closed проверка на `sdp` (до проверки `secret_key`).
3. **Разделение политики реконнекта (`reconnect.rs` vs голос)**:
   - Существующий `src/reconnect.rs` реализует gateway-таблицу `[1000, 2000, 4000, 8000, 16000, 30000]` с 6 попытками и `Resume`/`InvalidSession`. По решению `D-V01-1` и `D-V01-3` (`V01-SESSION-CONTRACT.md` §2.4, §4.4) голосовая RTC-сессия использует отдельный `VoiceReconnectConfig` с 3 попытками, бэкоффом `[0, 2_000, 5_000]` мс и полным рукопожатием без `op7 RESUME`. Существующий `reconnect.rs` сохранён без изменений.

---

## 2. Что сделано

1. **`rust/crates/discord-voice/src/lib.rs`**:
   - Добавлен модуль `pub mod session;` и обновлена документационная шапка крейта (риск R-12).
   - Добавлены структуры `Heartbeat` (`t`, `seq_ack`, `Heartbeat::to_json()`) и `HeartbeatAck` (`t`), а также `op16_to_json()`.
   - В `VoicePayload` и `parse_frame()` добавлен типизированный вариант `VoicePayload::HeartbeatAck(HeartbeatAck)` для `op6`.
   - Расширен `SelectProtocol`: добавлены `experiments: Vec<String>` и `rtc_connection_id: Option<String>` с сериализацией в `SelectProtocol::to_json()` и разбором в `parse_select_protocol()`.
   - В `parse_session_description()` добавлена явная проверка на `sdp` и `missing secret_key`.
2. **`rust/crates/discord-voice/src/session.rs` (новый модуль)**:
   - Реализована детерминированная машина состояний `Session` (`Disconnected`, `Connecting`, `Connected`, `Reconnecting`, `Closed`) и внутренние фазы `Phase` (`AwaitingHello`, `Identifying`, `SelectingProtocol`, `AwaitingDescription`, `Established`).
   - Реализован переход в `Connected` **строго по валидному `op4 SessionDescription`** (`D-V01-2`), а не по `op2 Ready`.
   - Реализован физический порядок рукопожатия (`§3.2`): `op8 Hello` → `Send(op0 Identify)` + `Send(op16)` → `op2 Ready` → `UdpDiscover` → `UdpDiscovered` → `Send(op1 SelectProtocol)` → `op4 SessionDescription`.
   - Реализован heartbeat-цикл (`§3.3`): интервал берётся из `op8 Hello.heartbeat_interval_ms`, дедлайн ACK = `2 * interval`, правило `H-SEQ` (`seq_ack = last_inbound_root_seq + 1`, по умолчанию `1`), отмена `TimerId::HeartbeatAck` и расчёт диагностического `last_rtt_ms` по `op6 HeartbeatAck`, закрытие с кодом `4000` (`CLOSE_DELIBERATE`) и переход в `Reconnecting` при `TimerExpired(TimerId::HeartbeatAck)`.
   - Реализована классификация закрытия и ошибок: код `1000` (`CLOSE_CLEAN`) переводит в терминальный `Closed` с `Err(SessionError::ClosedByPeer { code: 1000 })` без реконнекта; коды `4000`, `4801`, `None` и `TransportFailed` запускают реконнект по таблице `[0, 2_000, 5_000]` мс не более `3` раз, после чего сессия переходит в `Closed` с `Err(SessionError::GaveUp { attempts: 3 })`. Успешный `op4` сбрасывает `attempts = 0`.
   - Реализована fail-closed защита для `op4` с `sdp` (`Err(SessionError::UnsupportedSessionDescription { sdp: true })`, переход в `Closed`, `media_ready = false`, `mode = None`), идемпотентность повторного `op2 Ready` с тем же `ssrc`, отказ при смене `ssrc` (`SessionError::UnexpectedReady`), отказ при повторном `op4` на `Connected` (`SessionError::DuplicateSessionDescription`), защита от монотонного отката часов `now_ms` и переполнения размера кадра `max_frame_bytes`.
   - Обеспечено редактирование секретов (`token`, `secret_key`, `media_session_id`, `session_id`) в `Debug`, `Display`, `SessionSnapshot`, `SessionEffect` и `VoiceOut`.
3. **`rust/crates/discord-voice/tests/session_contract.rs` (новый тестовый набор)** и **`rust/crates/discord-voice/tests/signaling.rs`**:
   - Реализованы все 16 контрактных офлайн-тестов `t_v01_01` … `t_v01_16` в точном соответствии с матрицей `V01-SESSION-CONTRACT.md` §6, включая интеграцию с реальным loopback UDP discovery (`t_v01_16`) и побайтовую/префиксную проверку без паник (`t_v01_15`).

---

## 3. Доказательство TDD: RED → GREEN

### 3.1. RED-прогон (`waves/RW-40/PM/v01-rtc/red.log`)

Команда:
```bash
CARGO_TARGET_DIR=/tmp/target-g5-voice cargo test --manifest-path rust/Cargo.toml --offline -p discord-voice
```
- **Код возврата:** `101`
- **Результат `tests/session_contract.rs`:** `0 passed; 16 failed` (`op6` не типизирован как `HeartbeatAck`, методы `Session::on` в `src/session.rs` на этапе заглушки возвращали `unimplemented!`).
- Все остальные 9 существующих тестовых наборов (125 интеграционных + 6 unit-тестов) остались зелёными.

### 3.2. GREEN-прогон (`waves/RW-40/PM/v01-rtc/green.log`)

Команда:
```bash
CARGO_TARGET_DIR=/tmp/target-g5-voice cargo test --manifest-path rust/Cargo.toml --offline -p discord-voice
```
- **Код возврата:** `0`
- **Сводка по тестовым наборам (`147 passed; 0 failed`):**
  - `unittests src/lib.rs`: `6 passed; 0 failed`
  - `tests/crypto_vectors.rs`: `16 passed; 0 failed`
  - `tests/media_contract.rs`: `13 passed; 0 failed`
  - `tests/reception_contract.rs`: `10 passed; 0 failed`
  - `tests/reconnect_contract.rs`: `10 passed; 0 failed`
  - `tests/regions_contract.rs`: `10 passed; 0 failed`
  - `tests/rtcp_contract.rs`: `16 passed; 0 failed`
  - `tests/session_contract.rs`: `16 passed; 0 failed` (`t_v01_01` .. `t_v01_16`)
  - `tests/signaling.rs`: `11 passed; 0 failed`
  - `tests/srtp_contract.rs`: `22 passed; 0 failed`
  - `tests/stats_contract.rs`: `12 passed; 0 failed`
  - `tests/udp.rs`: `15 passed; 0 failed`

### 3.3. Форматирование и линтер (`waves/RW-40/PM/v01-rtc/clippy.log`)

- `CARGO_TARGET_DIR=/tmp/target-g5-voice cargo clippy --manifest-path rust/Cargo.toml --offline -p discord-voice --all-targets -- -D warnings` — `0 warnings`, exit code `0`.
- `rustfmt --check --edition 2024` по всем файлам `rust/crates/discord-voice/{src,tests}` — чисто, exit code `0`.

---

## 4. Мутационная проверка (`waves/RW-40/PM/v01-rtc/mutations.log`)

Проверены все 10 обязательных мутаций из таблицы `V01-SESSION-CONTRACT.md` §6.1. Каждая мутация вносилась в `src/session.rs` или `src/lib.rs`, фиксировалась MD5-хэшем мутанта, убивалась целевым тестом (`exit code 101`) и откатывалась к эталонному MD5:

| # | Мутация (§6.1) | Файл | MD5 мутанта | Убившие тесты | Статус |
|---|---|---|---|---|---|
| **M1** | Переход в `Connected` на `op2 Ready` вместо `op4 SessionDescription` | `src/session.rs` | `85f91b8da904460981173a42a9bb9284` | `t_v01_02`, `t_v01_10` (+ 11 зависимых) | **KILLED** (exit 101) |
| **M2** | Интервал heartbeat из константы `13_750` вместо `op8 Hello` | `src/session.rs` | `3126141ddc530f854c22a32029c98a25` | `t_v01_04` | **KILLED** (exit 101) |
| **M3** | `ACK_DEADLINE = 1 * interval` вместо `2 * interval` | `src/session.rs` | `2b714c63da24d5aa9cbaebec9759ede6` | `t_v01_04`, `t_v01_05` | **KILLED** (exit 101) |
| **M4** | `seq_ack` всегда `1` вместо правила `H-SEQ` (`last_inbound_root_seq + 1`) | `src/session.rs` | `7eca3ca0d11756f0facbe65d47e5d049` | `t_v01_04` | **KILLED** (exit 101) |
| **M5** | Убрать проверку `sdp` → провал в общий `missing secret_key` | `src/session.rs` | `740c837b9034f76042648344b7ead64d` | `t_v01_10` | **KILLED** (exit 101) |
| **M6** | Close `1000` → реконнект вместо терминального `Closed` | `src/session.rs` | `97d20abd8fefc7f2fdca661956ea82a0` | `t_v01_07` | **KILLED** (exit 101) |
| **M7** | Лимит попыток реконнекта `3` → `4` | `src/session.rs` | `8f1e982d09c9b5b70ff1c502fc643cb1` | `t_v01_08` | **KILLED** (exit 101) |
| **M8** | Убрать `CancelTimer(Hello)` после получения `op8` | `src/session.rs` | `ea93d14f933c74f2a98e988efede7fd3` | `t_v01_06`, `t_v01_02` | **KILLED** (exit 101) |
| **M9** | Утечка `token` в `Debug` реализации `Session` | `src/session.rs` | `facf2e62e03a3a7eda6ef364db382b74` | `t_v01_12` | **KILLED** (exit 101) |
| **M10** | Убрать `experiments` из сериализации `op1 SelectProtocol` | `src/lib.rs` | `b82902e426f02c4e074eaf5847d9859f` | `t_v01_14` | **KILLED** (exit 101) |

После завершения всех 10 мутаций файлы восстановлены до baseline MD5 (`src/lib.rs` = `3688883a78f4daa43e9f62e0a564eb8a`, `src/session.rs` = `030303adeef732029c70806312f367d2`) и выполнен контрольный прогон `cargo test` (`exit code 0`).

---

## 5. MD5 всех проверенных и изменённых файлов

### Изменённые и созданные файлы в `rust/crates/discord-voice`

| Файл | Статус | MD5 |
|---|---|---|
| `rust/crates/discord-voice/src/lib.rs` | modified | `3688883a78f4daa43e9f62e0a564eb8a` |
| `rust/crates/discord-voice/src/session.rs` | created | `030303adeef732029c70806312f367d2` |
| `rust/crates/discord-voice/tests/session_contract.rs` | created | `1d6b0e2d4e76ce3602b199eac848da0f` |
| `rust/crates/discord-voice/tests/signaling.rs` | modified | `263184bbb3f38bcbc232b53e95351b26` |

### Проверенные (неизменённые) файлы в `rust/crates/discord-voice`

| Файл | Статус | MD5 |
|---|---|---|
| `rust/crates/discord-voice/Cargo.toml` | unchanged | `0ba871fe017590a2160ab56458037fd2` |
| `rust/crates/discord-voice/src/reception.rs` | unchanged | `e067e1ecbe0f7907843b2466ef82b090` |
| `rust/crates/discord-voice/src/reconnect.rs` | unchanged | `817abac1645a6743cfbe32c6d98993e0` |
| `rust/crates/discord-voice/src/regions.rs` | unchanged | `31d5e542fd1df5aa3cbdc2ab560868ae` |
| `rust/crates/discord-voice/src/rtcp.rs` | unchanged | `36d8ec252d00e703f025de32774e162b` |
| `rust/crates/discord-voice/src/srtp.rs` | unchanged | `f06531afd891e4c38bf873b808cd9eb9` |
| `rust/crates/discord-voice/src/stats.rs` | unchanged | `05fa2b04a0f2c6bc623d748cf84895cd` |
| `rust/crates/discord-voice/src/udp.rs` | unchanged | `6b9282be2b67d82ff6adca42cf90cd5e` |
| `rust/crates/discord-voice/src/crypto/aes.rs` | unchanged | `77cf89a01bd3a001785b43f67d550974` |
| `rust/crates/discord-voice/src/crypto/chacha.rs` | unchanged | `f292f3cdb95317a7a37ee7a24563a7ae` |
| `rust/crates/discord-voice/src/crypto/gcm.rs` | unchanged | `5f0308ccbe7189736ab04cd58dc0f129` |
| `rust/crates/discord-voice/src/crypto/kdf.rs` | unchanged | `596094464fabfdbc557a7fa84beb7424` |
| `rust/crates/discord-voice/src/crypto/mod.rs` | unchanged | `5286b0d8fa2e3657a0dc8476b329e08e` |
| `rust/crates/discord-voice/src/media/jitter.rs` | unchanged | `51d4bc9e3c4f309c62c6cb7a69bd89af` |
| `rust/crates/discord-voice/src/media/mod.rs` | unchanged | `c62ac72aae09a2b1ea5bb8d8cdcdeb54` |
| `rust/crates/discord-voice/src/media/rtp.rs` | unchanged | `c33f185bd1a3779757f528041f049eab` |
| `rust/crates/discord-voice/tests/crypto_vectors.rs` | unchanged | `9b91ade1070d3d19db50e20123451c78` |
| `rust/crates/discord-voice/tests/media_contract.rs` | unchanged | `9651677bab3bc785b94ddb952626697a` |
| `rust/crates/discord-voice/tests/reception_contract.rs` | unchanged | `8350e2794e8e96e4cba86b96bcd94187` |
| `rust/crates/discord-voice/tests/reconnect_contract.rs` | unchanged | `3005a9eb2cf6a89c1eec8906aef7e7bb` |
| `rust/crates/discord-voice/tests/regions_contract.rs` | unchanged | `3fd71f7c53273fe4521a98c5f8540037` |
| `rust/crates/discord-voice/tests/rtcp_contract.rs` | unchanged | `af400b5549bed2135da087bbc5e97a3b` |
| `rust/crates/discord-voice/tests/srtp_contract.rs` | unchanged | `b8b5a183189903ba4fa6f97b5d9ff29f` |
| `rust/crates/discord-voice/tests/stats_contract.rs` | unchanged | `e0924fe222ebc55d350c33d3b7b5e3f6` |
| `rust/crates/discord-voice/tests/udp.rs` | unchanged | `a4c0a20ad9bc4478f9f7d01725a77c18` |

### Проверенные документы, оракулы и артефакты прогона

| Файл | Статус | MD5 |
|---|---|---|
| `AGENTS.md` | read-only | `aa6bbb2d2f5b669263a9dd2119750783` |
| `TEST-POLICY.md` | read-only | `eb9087faa42bf89ac3de62ef97a24bbc` |
| `waves/RW-40/VOICE/V01-SESSION-CONTRACT.md` | read-only | `4a8519be82b1533f2c27e6adf6ecadff` |
| `waves/RW-40/VOICE/G5-PREREQ.md` | read-only | `5dbd92c07150c55571bb7421f220167d` |
| `waves/RW-40/DESIGN/VOICE.md` | read-only | `157a5cbdaa6feebb885f56715c91b194` |
| `fixtures/voice/rtc1-frames.jsonl` | read-only | `7f913c8fe659f3e20bee2cb1d5269fd8` |
| `fixtures/voice/rtc-frames.jsonl` | read-only | `e8003f12f2eef3136bc31a5bba16ed5b` |
| `fixtures/voice/rtc1-summary.json` | read-only | `844bc5c2dee845cd03bc72779998d6de` |
| `fixtures/voice/rtc-summary.json` | read-only | `1e5cd42b95893e95bfeac66c988944e0` |
| `waves/RW-40/PM/v01-rtc/baseline-test.log` | created | `8e60d3ad7c76d4feb81f3492e770774d` |
| `waves/RW-40/PM/v01-rtc/red.log` | created | `ff558acea6aade593195ee3439608ded` |
| `waves/RW-40/PM/v01-rtc/green.log` | created | `3e444a144f0a6ec0c563e58633383167` |
| `waves/RW-40/PM/v01-rtc/clippy.log` | created | `85f3ecbb652594e370ee83f826255dde` |
| `waves/RW-40/PM/v01-rtc/mutations.log` | created | `aa3ac09f52ea20ee461422864a471d56` |

---

## 6. Риски и открытые зависимости (вне скоупа офлайн-карточки V-01)

1. **Интеграция с `discord-core` (`G5-PREREQ.md` §1.1, R-3)**: для живого подключения `discord-core` должен извлекать `session_id` из `VOICE_STATE_UPDATE` и разбирать `VOICE_SERVER_UPDATE` (сейчас вне скоупа `discord-voice`).
2. **Калибровка `[A]`-таймаутов и правила `H-SEQ` в live-окне**: значения `HELLO_TIMEOUT_MS = 15_000`, `JOIN_TIMEOUT_MS = 15_000`, `DESCRIPTION_TIMEOUT_MS = 10_000` и формула `seq_ack = last_inbound_root_seq + 1` помечены как архитектурные дефолты `[A]` (`V01-SESSION-CONTRACT.md` §4.4, §8.1) и параметризованы через `SessionConfig` для калибровки без переписывания машины состояний.
3. **Запреты соблюдены**: `discord-core`, `gui-spike`, `ui/**`, золотые каталоги (`logs/`, `functions/`, `evidence/`, `fixtures/`) не затрагивались; сеть и живой Discord не запускались; `git commit` не выполнялся.
