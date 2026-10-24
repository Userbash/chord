# V01-SESSION-CONTRACT — RTC session manager (V-01): контракт без live

Дата: 2026-10-24. Owner контракта: **G5 Voice/Platform**; реализация: **G1 Backend**;
приёмка: Team Lead (+ G4 по evidence, + G4b по ключевому материалу). HEAD: `0239543`.

Режим: **read-only**. Не запускались live/сеть/audio/сборка/установки; выполнялись только чтение
файлов и read-only команды (`ls`, `grep`, `sed`, `wc`, `python3` по фикстурам, `git status`).
Документ — **контракт**, не отчёт о реализации. Кода, тестов, фикстур, `Cargo.toml`, гейта и
системных пакетов он не меняет; изменён только этот файл.

Маркировка утверждений: **[L]** — наблюдено в live-сессии/байтах; **[S]** — якорь в коде
эталонного клиента (по `NETWORK-CONTRACTS.md`); **[A]** — наше решение/дефолт, паритет не
заявлен; **[M]** — самосогласованная модель; **[P]** — правит Team Lead.

---

## 0. Итог

V-01 = **чистая машина состояний RTC-сессии** (JSON-over-WS) + типизированный разбор кадров +
эффекты для внешнего транспорта. В крейте есть **все парсеры и сериализаторы кадра**, но
**нет** ни manager'а, ни таймеров, ни heartbeat-цикла, ни типизированных op3/op6, ни
close-кодов, ни варианта op4 `sdp`. Offline-часть V-01 закрывается **без live, без новых
зависимостей и без сборки сети** — при условии, что транспорт и часы инжектируются.

Три факта, которые меняют привычную формулировку карточки:

1. **Порядок «op1 → op2» из `NETWORK-CONTRACTS.md` §2.1 физически невозможен** и в этом
   контракте не используется: discovery-запрос несёт аудио-SSRC (`9208` в сессии 3, [L]), а
   SSRC приходит только в op2 ⇒ `op2 → discovery → op1`. Подробно §3.2.
2. **op1 нельзя собрать без V-02**: `address/port` в op1 — публичный адрес из ответа discovery
   (`193.222.99.83:43689` в rtc1, [L]), а не локальный. Offline это закрывается fake-транспортом,
   live — только вместе с V-02.
3. **`reconnect.rs` — политика gateway, а не RTC**, и её таблица бэкоффа
   (`1000/2000/4000/8000/16000/30000`) **противоречит** уже зафиксированному UI-контракту голоса
   (`0/2000/5000`, 3 попытки, `DESIGN/VOICE.md` §2.6). Требуется решение D-V01-1 (§9), молча
   переиспользовать gateway-таблицу нельзя.

---

## 0.1 Расхождения постановки с деревом (проверено, не догадки)

| Что ожидалось в ТЗ | Факт в дереве | Следствие |
|---|---|---|
| `VOICE/G5-PREREQ.md`, `VOICE/OPUS-KAT.md` | в корне нет каталога `VOICE/`; документы лежат в `waves/RW-40/VOICE/` | этот файл создан там же |
| `rust/crates/discord-voice/src/signaling.rs` | файла **нет**; разбор/сериализация RTC-кадра — в `src/lib.rs` (761 строка), тесты — `tests/signaling.rs` (11 тестов) | «signaling» ≠ manager; выдать `lib.rs` за V-01 нельзя |
| `CONTRACTS/voice*` | в `CONTRACTS/` только `dispatch.md`, `read-state.md`; V-00 (`CONTRACTS/voice.md`) **не написан** | V-00 фиксируется как отсутствующая предпосылка (§7) |
| `DESIGN/VOICE.md` | в корне нет `DESIGN/`; документ — `waves/RW-40/DESIGN/VOICE.md` (+ `voice-spec.json`) | используется он |

Дополнительно: в `docs/re/voice-deep.md` §1 у op4 **два вида** (нет `sdp`), а `NETWORK-CONTRACTS.md`
§2.4 [S] фиксирует ветку `sdp` — расхождение §3.5.

---

## 1. Область V-01

### 1.1 Входит

- состояния RTC-сессии и переходы между ними;
- inbound: op8 Hello, op2 Ready, op4 Session Description, op6 Heartbeat ACK, close-кадр;
- outbound: op0 Identify, op1 Select Protocol, op3 Heartbeat, op16 `{op:16,d:{}}`, close;
- таймеры: hello, heartbeat, ACK-deadline, join, description, reconnect;
- типизированные ошибки сессии и классификация close-кодов;
- snapshot для V-06/V-07 **без секретов**.

### 1.2 Не входит

- UDP-сокет, discovery-байты, keepalive, ротация — V-02 (`udp.rs` уже есть);
- SRTP/KDF/nonce/AAD, ключ из op4 — V-03 (блокер B-08);
- Opus/ALSA/PTT — V-04/V-05;
- DAVE/MLS (op21/22/24/25/27/29/30) — `V-08-DAVE`, вне V-01;
- speaking (op5), client-connect/disconnect (op11/13), медиа-видео (op12/op14): **не наблюдались**
  [A] и в V-01 не моделируются — только `Other`-passthrough без побочных эффектов;
- UI-состояния и тексты — V-06/G3 (`DESIGN/VOICE.md` §2.7); здесь только их проекция (§2.5).

### 1.3 Пространства имён карточек (коллизия ID)

| Пространство | V-01 | V-02 | V-04 | V-05 | V-08 |
|---|---|---|---|---|---|
| `MASTER-PLAN.md:76-83` (карточки) | RTC session manager | UDP live | Opus FFI/KAT | ALSA/PTT/mute | DAVE/MLS |
| `PARITY/FUNCTION-INVENTORY.md` §7 | вход/выход в канал | RTC-сигнализация | SRTP | Opus/джиттер | UI-панель |

**Этот документ использует пространство `MASTER-PLAN`** (V-01 = RTC session manager). Ссылки на
«V-04/V-05» далее — только по `MASTER-PLAN`. Парити-нумерация в тексте не используется.

---

## 2. Состояния

### 2.1 Канонический набор

Пять состояний, как в ТЗ. Дополнительных состояний не вводится: подшаги подключения живут в
поле `phase` и **не являются** состояниями (иначе UI-контракт G3 и `data-state` разъедутся).

| Состояние | Смысл | Терминальное |
|---|---|---|
| `Disconnected` | сессии нет; WS не открыт, keepalive/медиа V-01 не касается | нет |
| `Connecting` | WS открывается/открыт, рукопожатие RTC не завершено | нет |
| `Connected` | рукопожатие завершено, ключ медиа применён (`media_ready`) | нет |
| `Reconnecting` | сессия была установлена, связь потеряна, идёт пауза/попытка | нет |
| `Closed` | сессия завершена; объект переиспользованию не подлежит | да |

`Closed` создаётся один раз: дальнейшие события дают `SessionError::Closed` (эффектов нет),
новый вход = новый `Session`.

### 2.2 Что именно означает `Connected` — решение D-V01-2

- `phase`: `AwaitingHello → Identifying → SelectingProtocol → AwaitingDescription → Established`.
- `Connected` наступает **на применённом op4** (есть `mode`, ключ, `media_session_id`), а не на
  op2. Обоснование: op2 означает лишь «сервер готов принять Select Protocol»; до op4 клиент не
  может ни шифровать, ни расшифровывать, и утверждение «голос подключён» было бы непроверяемым.
- `phase = Established` (op2 получен, op4 ещё нет) остаётся видимым в snapshot для диагностики и
  для строки UI «Ожидание подключения...» (`EuwcxO`, `DESIGN/VOICE.md` §2.6) — та же `connecting`.
- **Открытый вопрос Q-V01-2** (§9): в логе движка `Connected` пишется до `Sending UDP info`, т.е.
  эталонный клиент считает «connected» уже на op2. Если live подтвердит, меняется одна граница
  (`Connected` на op2 + `media_ready` отдельным флагом), а не машина.

### 2.3 Переходы

Событие → эффекты. `now_ms` — параметр каждого шага (часы инжектируются, см. §4.2).

| # | Из | Событие | Условие | В | Эффекты |
|---|---|---|---|---|---|
| 1 | `Disconnected` | `JoinRequested{channel,guild}` | — | `Connecting` | `SendGatewayOp4Join`, `StartTimer(Join, now+JOIN_TIMEOUT_MS)` |
| 2 | `Connecting` | `GatewayVoiceServer(VoiceServerUpdate)` | endpoint валиден | `Connecting` | `CancelTimer(Join)`, `OpenSocket{url}` |
| 3 | `Connecting` | `WsOpened` | — | `Connecting` | `StartTimer(Hello, now+HELLO_TIMEOUT_MS)` |
| 4 | `Connecting` | `Inbound(Hello{interval})` | — | `Connecting` | `CancelTimer(Hello)`, `Send(op0 Identify)`, `phase=Identifying`, `StartTimer(Heartbeat, now+interval)` |
| 5 | `Connecting` | `Inbound(Ready)` | `phase ∈ {Identifying, SelectingProtocol}` | `Connecting` | `phase=SelectingProtocol`, `StartTimer(Description, now+DESCRIPTION_TIMEOUT_MS)`, `UdpDiscover{ssrc,ip,port}` |
| 6 | `Connecting` | `UdpDiscovered{address,port}` | `phase=SelectingProtocol` | `Connecting` | `Send(op1 SelectProtocol)` |
| 7 | `Connecting` | `Inbound(SessionDescription{mode,key,msid})` | `phase≠AwaitingHello` | `Connected` | `phase=Established`, `MediaKey{mode}` (ключ не логируется), `StartTimer(Description)` отменяется |
| 8 | `Connecting` | `Inbound(SessionDescription` c `sdp`) | — | `Closed` | `CloseSocket(1000)`, `SessionError::UnsupportedSessionDescription` |
| 9 | `Connecting` | `Timer(Hello)` | op8 не пришёл | `Closed` | `CloseSocket(4000)`, `SessionError::HelloTimeout` |
| 10 | `Connecting` | `Timer(Join)` | `VOICE_SERVER_UPDATE` не пришёл | `Closed` | `SessionError::JoinTimeout` |
| 11 | `Connecting`/`Connected` | `Timer(Heartbeat)` | — | без изменений | `Send(op3 Heartbeat)`, `StartTimer(HeartbeatAck, now+ACK_DEADLINE_MS)` |
| 12 | `Connecting`/`Connected` | `Inbound(HeartbeatAck{t})` | `t` непустой | без изменений | `CancelTimer(HeartbeatAck)`, `note_rtt(now - sent_at)` (диагностика) |
| 13 | `Connecting`/`Connected`/`Reconnecting` | `Timer(HeartbeatAck)` | op6 не пришёл | `Reconnecting` | `CloseSocket(4000)`, `note_lost`, `Reconnect` (§2.4) |
| 14 | `Connecting`/`Connected`/`Reconnecting` | `WsClosed{Some(1000), _}` | — | `Closed` | `SessionError::ClosedByPeer{1000}`, никаких реконнектов |
| 15 | `Connecting`/`Connected`/`Reconnecting` | `WsClosed{code:None}` или код ≠ 1000; `TransportFailed` | — | `Reconnecting` | `Reconnect` |
| 16 | `Reconnecting` | `Timer(Reconnect)` | попытка ≤ 3 | `Connecting` | `OpenSocket{url}`, `phase=AwaitingHello`, attempt+=1 |
| 17 | `Reconnecting` | `Timer(Reconnect)` | попытка > 3 | `Closed` | `SessionError::GaveUp{attempts}` |
| 18 | любое, кроме `Closed` | `LeaveRequested` | — | `Closed` | `CloseSocket(1000,"Force Close")` (если WS открыт), все таймеры стоп |
| 19 | `Disconnected` | `LeaveRequested` | — | `Closed` | без эффектов (локальный отказ, WS не было) |
| 20 | `Connecting` | `Timer(Description)` | op4 не пришёл | `Closed` | `CloseSocket(4000)`, `SessionError::DescriptionTimeout` |

Инварианты:

- после `Closed` эффектов нет никогда (кроме `SessionError::Closed` вызывающему);
- `Send(op3)` не отправляется до `Send(op0)`; `Send(op1)` — только после `UdpDiscovered`;
- **ровно один** `Send(op1)` на попытку подключения (повторный op2 не пересоздаёт UDP и не
  дублирует op1; смена `ssrc` в повторном op2 — ошибка `UnexpectedReady`, не реконнект);
- повторный op4 на `Connected` — `SessionError::DuplicateSessionDescription`, состояние не
  меняется, ключ не перезаписывается;
- `now_ms`, ушедший назад, не двигает таймеры (насыщающие вычисления, без паник).

### 2.4 Реконнект: без RESUME (решение D-V01-3)

`op7 RESUME` / `op9 RESUMED` для RTC — только **[S]** (код клиента), в capture **не наблюдались**,
и семантика `seq_ack` не восстановлена. Поэтому V-01:

- реконнект = **новый WS + полный рукопожатие** `op8 → op0 → op2 → discovery → op1 → op4`;
- `V-01` **не отправляет** `op7` и не разбирает `op9`; полученный `op9` = `Other` без эффектов
  (не ошибка, счётчик `unsupported_resume`);
- закрытие по собственной инициативе — код **4000** ([S] `reconnect()`), чистое завершение от
  сервера/по нашей инициативе при выходе — **1000** ([L], «Disconnected clean: true, code: 1000,
  reason: Force Close»).

### 2.5 Проекция в UI (V-06 не обязана совпадать 1:1)

`DESIGN/VOICE.md` §2.7 — 12 значений `data-state`. V-01 отдаёт только транспортную часть:

| V-01 | `data-state` (V-06) |
|---|---|
| `Disconnected` | `idle` / `disabled` (по доступности сборки) |
| `Connecting` | `connecting` (+ «Ожидание подключения...» при `phase=Established`) |
| `Connected` | `connected` / `empty` / `server-mute` (по участникам и guild-состоянию) |
| `Reconnecting` | `reconnecting` |
| `Closed` | `idle` (после leave) / `error` (по `SessionError`) |

`no-permission`, `ptt-required`, `device-missing` — **не** состояния V-01: это условия входа и
устройства (gateway-ошибка / V-05 / V-06).

---

## 3. Провод: что именно принимаем и отправляем

### 3.1 Опкоды, покрытые V-01 (только наблюдаемое/якорное)

| op | Напр. | Payload (дословно из фикстур) | Статус | В V-01 |
|---:|---|---|---|---|
| 8 | in | `{"v":8,"heartbeat_interval":13750}` | **[L]** `rtc1/rtc-frames.jsonl` | типизирован (`Hello`) |
| 0 | out | `server_id,channel_id,user_id,session_id,token,max_dave_protocol_version:1,video:true,streams:[{type:"video",rid:"100",quality:100}]` | **[L]** | `Identify::to_json` есть |
| 2 | in | `ssrc,port,ip,modes[],experiments[],streams[{type,ssrc,rtx_ssrc,rid,quality,active}]` | **[L]** | типизирован (`Ready`) |
| 4 | in | `mode,secret_key(32 Б b64),media_session_id,dave_protocol_version:1,secure_frames_version:1,audio_codec,video_codec` | **[L]** | типизирован (`SessionDescription`) |
| 6 | in | `{"op":6,"d":{"t":<ms>}}` | **[L]** (7 шт. в rtc1, 6 в rtc2) | **не типизирован** → `Other` (гэп) |
| 3 | out | `{"op":3,"d":{"t":<ms>,"seq_ack":1}}`, шаг **13 750 мс** | **[L]** | **сериализатора нет** (гэп) |
| 1 | out | `protocol:"udp"`, `data{address,port,mode}`, те же три поля в корне, `codecs[5]`, `experiments:["fixed_keyframe_interval"]`, `rtc_connection_id:"<uuid>"` | **[L]** | есть, но **без** `experiments`/`rtc_connection_id` (гэп) |
| 16 | out | `{"op":16,"d":{}}` | **[L]** | **нет** (гэп) |
| 16 | in | `{"voice":"0.22.1+…","rtc_worker":"1.6.98"}` | **[L]** | `Other` (ок) |
| 15 | in | `{"seq":0,"op":15,"d":{"any":100}}` | **[L]** | `Other` (ок; `seq` читается в `VoiceFrame.seq`) |
| close | in | code **1000**, reason `Force Close` | **[L]** `VD:108` | **типа нет** (гэп) |
| close | out | **4000** (`reconnect()`), **4801** «Cannot resume connection.» при неудачном resume | **[S]** | **типа нет** (гэп) |
| bin | in/out | `in-bin` 100 Б, `out-bin` 392 Б (sha256 `f729b3b8…`) — MLS/DAVE, op не восстановлен | **[L]/[A]** | вне V-01 |

Опкоды 5/7/9/11/12/13/14/17/18/20/21/22/24/25/26/27/28/29/30/31/32/33 — **[S]** или не наблюдались:
V-01 их **не отправляет** и не интерпретирует. Новые опкоды/поля не изобретаются;
неизвестный op = `Other` + счётчик.

### 3.2 Реальный порядок (и почему строка RE «op1 → op2» неверна)

Свидетельства:

- **[L]** `EL1`: `Creating connection with audio ssrc` → `Connected` → `codecs` → `Sending UDP info`
  → `media-session-id` → `DAVE init` → `sink wants`. `Sending UDP info` = op1, `media-session-id` = op4.
- **[L]** discovery-запрос сессии 3 (`voice-udp4.rtp.json`): `ssrc: 9208` — это **аудио-SSRC из op2**
  (сессия 3: 9208/9209/9210), и он же SSRC RTP-пакетов (`rtp-pt120-oracle.json`: `ssrc 9208`).
- **[L]** discovery делают **два сокета-кандидата** одновременно (два flow с одинаковым `ts`
  `2026-10-19T07:39:27.891Z`, ssrc 9208).
- **[L]** op1 несёт `address:"193.222.99.83", port:43689` — публичный адрес из ответа discovery,
  не локальный (`0.0.0.0:55599` / `10.1.0.140:…`).

Следствие: **SSRC нужен для discovery, discovery нужен для op1, SSRC приходит в op2** ⇒

```
op8 Hello → op0 Identify → op2 Ready → UDP discovery → op1 Select Protocol → op4 Session Description → media
```

`NETWORK-CONTRACTS.md` §2.1 (`op8 → op0 → op1 → op2 → op4`) противоречит этим трём фактам;
вероятная причина — сам документ предупреждает, что `RF` пишется по направлениям и порядок строк
не причинный. **Правку строки §2.1 делает владелец RE/Team Lead** (не G5); до правки контракт
действует по физической зависимости выше. Машина состояний при этом **терпима** к порядку:
op2 принимается в любом момент после op0, op4 — после op0 (жёсткая проверка только на «op1 не
раньше Discovered»).

### 3.3 Heartbeat

| Свойство | Значение | Метка |
|---|---|---|
| Интервал | берётся **из op8** (`heartbeat_interval` = 13 750 в обеих сессиях) | **[L]** |
| `VOICE_HEARTBEAT_INTERVAL_MS = 13_750` в `lib.rs:42` | сейчас не используется никем, кроме собственного теста `lib.rs:756` | факт кода |
| `t` | клиентские часы, мс; **op6 возвращает тот же `t`** (7/7 и 6/6 совпадений) | **[L]** |
| Шаг | ровно 13 750 мс в обеих сессиях (11 дельт) | **[L]** |
| `seq_ack` | константа `1` во всех 13 исходящих op3; единственный входящий кадр с корневым `seq` — op15 `seq:0` | **[L]** |
| Правило `seq_ack` | **H-SEQ (гипотеза, [A])**: `seq_ack = last_inbound_root_seq + 1` (с единственной точкой данных согласуется: 0+1=1, и значение не меняется, пока нового `seq` нет). Live-подтверждения нет | **[A]** |
| ACK-deadline | `ACK_DEADLINE_MS = 2 × interval` (одна пропущенная ACK) | **[A]**, не измерено |
| `heartbeat_interval` отсутствует/0 | `SessionError::InvalidHello`, закрытие 4000 | **[A]** |

`H-SEQ` обязана быть **поименованной гипотезой в коде** (`// H-SEQ: см. V01-SESSION-CONTRACT §3.3`),
а не молчаливым правилом: при первом live-расхождении меняется одна функция.

### 3.4 `rtc_connection_id` в op1 — открытый вопрос

**[L]** op1 содержит `rtc_connection_id: "5363ba05-d6fa-4736-8c04-0d09144e7a8d"` (обе сессии).
**[S]** в клиенте есть `getRTCConnectionId()` (телеметрия `rtc_connection_id` в
`pretty/mod-730852.pretty.js`), но **определение метода в доступных артефактах не найдено**:
происхождение значения (UUIDv4? идентификатор попытки? время?) не установлено.
Контракт: поле **обязательно** для wire-паритета, значение — непрозрачная строка, генерируемая
клиентом; формат в контракте **не закрепляется** (тест проверяет «непустая строка, ≤ 64 Б,
без пробелов/контроля»), вопрос Q-V01-4 (§9).

### 3.5 op4 с `sdp` — fail-closed

**[S]** клиент принимает `sdp` **или** `mode`+`secret_key`; в трёх live-сессиях `sdp` не встречался.
Текущий `parse_session_description` (`lib.rs:456`) **требует** `secret_key` 32 Б, поэтому кадр со
`sdp` падает в общий `VoiceError("missing or empty string field")` — то есть «молча» теряет
причину, а вызывающему неотличимо от битого JSON.

Контракт:

- распознать наличие `sdp` **до** разбора `secret_key` и вернуть
  `SessionError::UnsupportedSessionDescription{ sdp: true }`;
- **не** применять частично: ни `mode`, ни ключ, ни `media_session_id` не меняются;
- SDP-парсер не пишем (нет ни одного наблюдённого образца; выдумывать формат запрещено);
- закрытие сокета 1000, состояние `Closed` (не реконнект-петля: повтор даст тот же кадр).

---

## 4. Типы, ошибки, таймауты

### 4.1 Что реально есть в крейте

| Элемент | Где | Пригодность для V-01 |
|---|---|---|
| `VoiceOp` (0,1,2,3,4,6,8 + `Unknown`) | `lib.rs:55-94` | годится; нет `as_str`/`Display`, нет валидации «клиентский/серверный» |
| `parse_frame` (`{"op","d","seq?"}`) | `lib.rs:350` | годится как вход; **требует `d`** ⇒ close/бинарные кадры невыразимы; op3/op6 → `Other` |
| `Hello/Ready/SessionDescription/Identify/SelectProtocol` | `lib.rs:116-302` | годится; `to_json` только у `Identify`/`SelectProtocol` |
| `VoiceError(String)` | `lib.rs:98-112` | **непригоден как ошибка сессии**: один строковый тип, 5 разных сообщений, нет `source`, нет вариантов, нельзя отличить таймаут от битого кадра |
| `VoiceServerUpdate{token,guild_id,endpoint}` + `websocket_url()` | `lib.rs:625-692` | годится (redaction есть, `wss://host:port/?v=9`) |
| `select_region`, `RegionLatency`, `plan_probes` | `lib.rs:695`, `regions.rs` | вне V-01 (V-07) |
| `ReconnectPolicy` | `reconnect.rs:43-110` | **gateway-политика**; таблица бэкоффа не совпадает с голосом (D-V01-1) |
| `UdpError` (типизирован, `PartialEq`) | `udp.rs:44-95` | образец стиля ошибок для `SessionError` |
| `SrtpError`, `RegionError` | `srtp.rs:113`, `regions.rs:16` | образец стиля |
| `RttEstimator`, `StatsWindow` | `stats.rs:25,78` | RTT по RTCP; **не** ping UI (§9 Q-V01-6) |
| `VOICE_HEARTBEAT_INTERVAL_MS`, `MEDIA_ENCRYPTION_MODE`, `DAVE_PROTOCOL_VERSION`, `SECURE_FRAMES_VERSION`, `OBSERVED_RTC_OPS` | `lib.rs:33-51` | объявлены и не используются продуктом (только тесты) — оживить в V-01 |
| heartbeat scheduler, close-типы, `session`-модуль, транспорт | — | **отсутствуют** |

### 4.2 Форма API (эскиз; реализует G1, ревью — G4)

Чистое ядро без I/O, без времени изнутри, без `unsafe` (в крейте `unsafe_code = "forbid"`), в
стиле уже принятых `gateway::Session` (`discord-core/src/ws.rs`) и `ReconnectPolicy`.

```rust
pub enum SessionState { Disconnected, Connecting, Connected, Reconnecting, Closed }

pub enum Phase { AwaitingHello, Identifying, SelectingProtocol, AwaitingDescription, Established }

pub enum SessionEvent {
    JoinRequested { guild_id: String, channel_id: String },
    GatewayVoiceServer(VoiceServerUpdate),
    WsOpened,
    Inbound(VoiceFrame),                                  // parse_frame + типизация op6
    UdpDiscovered { address: String, port: u16 },         // от V-02
    TimerExpired(TimerId),
    WsClosed { code: Option<u16>, clean: bool },
    TransportFailed(TransportFailure),                    // kind: Dns|Tls|Io|Protocol|Oversize
    HeartbeatSent { t_ms: u64 },                          // для RTT-диагностики
    LeaveRequested,
}

pub enum SessionEffect {
    SendGatewayOp4Join { .. },
    OpenSocket { url: String },
    Send(VoiceOut),                                        // Identify|SelectProtocol|Heartbeat|Op16
    CloseSocket { code: u16, reason: &'static str },
    UdpDiscover { ssrc: u32, address: String, port: u16 }, // в V-02
    StartTimer { id: TimerId, at_ms: u64 },
    CancelTimer { id: TimerId },
    Notice(SessionNotice),                                 // наружу в сервис/V-06
}

pub enum TimerId { Join, Hello, Heartbeat, HeartbeatAck, Reconnect }

impl Session {
    pub fn new(config: SessionConfig) -> Self;
    pub fn on(&mut self, event: SessionEvent, now_ms: u64) -> Result<Vec<SessionEffect>, SessionError>;
    pub fn state(&self) -> SessionState;
    pub fn snapshot(&self) -> SessionSnapshot;             // без секретов
}
```

Правила формы:

- **один** вход (`on`) и **один** выход (эффекты) — транспорт/таймеры снаружи, поэтому offline
  тесты детерминированы и не требуют `tokio`;
- `SessionConfig` содержит `hello_timeout_ms`, `join_timeout_ms`, `description_timeout_ms`,
  `ack_deadline_ms`, `reconnect` (структура `VoiceReconnectConfig`) и `max_frame_bytes`; дефолты —
  §4.4, переопределяемы в тестах;
- `Session` **не** реализует `Clone`; `Debug` печатает только `state`, `phase`, `ssrc`, `attempts`;
- ключ (`secret_key`), `media_session_id`, `token` не хранятся в полях, доступных `Debug`/`snapshot`
  (по образцу `Identify`/`SessionDescription`, `lib.rs:145-302`); наружу — только
  `media_ready: bool` и `mode: Option<String>`.

### 4.3 Ошибки

| Вариант | Когда | Источник |
|---|---|---|
| `HelloTimeout` | op8 не пришёл за `hello_timeout_ms` | **[S]** `helloTimeout` + **[A]** значение |
| `JoinTimeout` | `VOICE_SERVER_UPDATE` не пришёл за `join_timeout_ms` | **[A]** `DESIGN/VOICE.md` §2.8 (15 с) |
| `DescriptionTimeout` | op2 есть, op4 не пришёл | **[A]** |
| `InvalidHello{..}` | `heartbeat_interval == 0` или `v` вне 1..=9 | **[A]** (границы — наши) |
| `InvalidEndpoint` | `websocket_url()` вернул ошибку | `lib.rs:668` (`VoiceError`) → маппинг |
| `UnexpectedReady` | повторный op2 с другим `ssrc` | **[A]** |
| `DuplicateSessionDescription` | повторный op4 на `Connected` | **[A]** |
| `UnsupportedMode{name}` | op4 `mode` не из 3 поддерживаемых `srtp::EncryptionMode` | `srtp.rs:69` |
| `UnsupportedSessionDescription{sdp:true}` | op4 со `sdp` | §3.5 |
| `MissingMediaKey` | op4 без `secret_key`, но и без `sdp` | **[A]** |
| `ClosedByPeer{code}` | close 1000 | **[L]** |
| `GaveUp{attempts}` | исчерпаны попытки реконнекта | **[A]** |
| `Transport(TransportFailure)` | транспортный сбой | **[A]** |
| `Closed` | событие после `Closed` | **[A]** |
| `Protocol(VoiceError)` | не разобрался кадр | `lib.rs:98` |

Требования: `PartialEq` (как `UdpError`), `Display` на русском, `std::error::Error`, **`source()`
для `Protocol`**, никакой строки, содержащей `token`/`secret_key`/`media_session_id`.

### 4.4 Таймауты и константы

| Имя | Значение | Метка | Обоснование |
|---|---:|---|---|
| `HELLO_TIMEOUT_MS` | 15 000 | **[A]** | таймаут входа в канал `DESIGN/VOICE.md` §2.8; `helloTimeout` в клиенте есть **[S]**, значение не найдено |
| `JOIN_TIMEOUT_MS` | 15 000 | **[A]** | там же |
| `DESCRIPTION_TIMEOUT_MS` | 10 000 | **[A]** | op2→op4 в capture — в пределах секунды, но меток времени покадрово нет ⇒ значение наше |
| `ACK_DEADLINE_MS` | 2 × interval (27 500 при 13 750) | **[A]** | одна пропущенная ACK; live не измерено |
| `VOICE_RECONNECT_ATTEMPTS` | 3 | **[A]** | `DESIGN/VOICE.md` §2.6 |
| `VOICE_RECONNECT_BACKOFF_MS` | `[0, 2_000, 5_000]` | **[A]** | там же; **не** `BACKOFF_MS` из `reconnect.rs` (D-V01-1) |
| `CLOSE_DELIBERATE` | 4000 | **[S]** | `reconnect()` |
| `CLOSE_CLEAN` | 1000 | **[L]** `VD:108` | |
| `MAX_FRAME_BYTES` | 65 536 | **[A]** | защита от гигантского кадра; реальные кадры ≤ 828 Б **[L]** |
| `HEARTBEAT_INTERVAL` | из op8 (13 750 в capture) | **[L]** | константа крейта — только значение-свидетель |

Все значения — **именованные константы в одном месте** (`session::config`), не литералы в коде;
каждая [A]-константа обязана иметь комментарий-ссылку (иначе она станет «фактом» следующей волны).

---

## 5. Dependency graph и офлайн-граница

```
V-00 контракт формата/владения буферами (G5+G1)  ← НЕ НАПИСАН (CONTRACTS/voice.md отсутствует)
 │
 ├─ V-01 RTC session manager (G1) ◄── этот документ
 │    ├─ нужен V-02 на шаг discovery (offline: fake transport / реальные байты udp.rs)
 │    │    └─ V-02 UDP live (G1)            offline: loopback + fake transport (код есть)
 │    │         └─ V-03 SRTP live (G1+G4b)  offline: ТОЛЬКО crypto KAT; KDF/nonce/AAD = B-08
 │    │              ├─ V-05b playback (G5)
 │    │              └─ V-08-DAVE (G1+G4)
 │    ├─ V-04 Opus FFI/KAT (G5)   — НЕ зависит от V-01: своя ветка от V-00
 │    │    └─ V-05a capture/send (G5)
 │    └─ V-07 reconnect/stats/regions (G1) — использует snapshot V-01, но не блокирует его
```

Читать так:

1. **V-01 → V-02**: V-01 владеет порядком («после op2 заказать discovery, получить адрес, отправить
   op1»), V-02 владеет байтами и сокетом. Offline V-01 не требует кода V-02: `DiscoveryPacket`
   `encode`/`parse` уже есть (`udp.rs:111-199`), `UdpMediaSocket` — реальный `UdpSocket`
   (`udp.rs:247`), тесты на loopback (`tests/udp.rs`) — тоже.
2. **V-02 → V-03**: V-03 не может закрыться офлайн (B-08). Значит V-01 **не должен** зависеть от
   V-03 по типам: `media_ready` — булев флаг, а `MediaKey` — непрозрачный токен, который
   потребляет V-03. Иначе V-01 станет невалидируемым до live.
3. **V-04/V-05 отдельной ветвью**: Opus и ALSA не требуют V-01 и наоборот. `tests/voice_pipeline.rs`
   (`T-OPUS-10`) — G1, но это соединение «кодек → `RtpSender`», без сессии.
4. **V-07** потребляет `SessionSnapshot` (ssrc, mode, attempts, last_close), но политику бэкоффа
   берёт из своего конфига — это место конфликта D-V01-1.

### 5.1 Офлайн / blocked-by-live для V-01

| Работа | Офлайн | Live-блокер |
|---|---|---|
| Машина состояний + переходы §2.3 | полностью | — |
| Типизация op6, сериализатор op3, op16, op1 c `experiments`/`rtc_connection_id` | полностью (оракул — фикстуры) | только origin `rtc_connection_id` (Q-V01-4) |
| Close-классификация 1000/4000/4800-4899/None | полностью по **[L]/[S]** таблице | реальные коды сервера при ошибках |
| Таймеры/таймауты | полностью (fake clock) | фактические значения Discord |
| `sdp`-ветка op4 (fail-closed) | полностью | появление `sdp` на практике |
| Реконнект (новая сессия, без op7) | полностью | нужен ли RESUME и что в `seq_ack` |
| Discovery-шаг | fake transport + loopback | ответ сервера, NAT/порт |
| Heartbeat | fake clock + эхо `t` | zombie-порог сервера, смысл `seq_ack` |
| Реальный WS (TLS, JSON-текст, ping/pong) | **нет**: нужен транспорт и зависимости (§7) | — |

---

## 6. Тест-план (fake WS / fake clock / loopback UDP)

Новая серия ID `T-V01-nn` — **не** пересекается с `T-VOICE-01…23` и `T-OPUS-01…10`;
строки в `docs/rust-client/TEST-PLAN.md` вносит Team Lead (`[P]`).

Стенд: `FakeTransport` (скрипт входящих кадров из `fixtures/voice/rtc*.jsonl` + запись исходящих),
`FakeClock` (u64 мс, шаг вручную), `FakeUdp` (возвращает `UdpDiscovered` из фикстуры),
`FakeSink` для эффектов. Никакой сети, TLS и реального времени.

| ID | Ур. | Проверка | Оракул | Offline |
|---|---|---|---|---|
| **T-V01-01** | L1 | карта опкодов: op8/op2/op4 типизированы, op6 — **типизирован** (новое), op3/op15/op16 → `Other`; неизвестный op (17/19/31) не паникует и не меняет состояние | `fixtures/voice/rtc{,1}-frames.jsonl` (13 кадров) | да |
| **T-V01-02** | L1 | happy path: `JoinRequested` → `GatewayVoiceServer` → `WsOpened` → op8 → op2 → `UdpDiscovered` → op4 даёт **ровно** `[op0, op1, …]`, `Connected`, `media_ready=true`; ни один эффект не содержит ключ | фикстуры + §3.1 | да |
| **T-V01-03** | L1 | порядок: op1 **не** уходит до `UdpDiscovered`; op2 без op1 принимается; op4 без op2 — `UnexpectedReady`→`Closed` | §3.2 (физическая зависимость) | да |
| **T-V01-04** | L1 | heartbeat: интервал берётся из op8 (передай 5000 — шаг 5000, не 13 750); `t` из инъекции; `seq_ack` по H-SEQ; op6 с тем же `t` снимает ACK-таймер | `rtc1-frames` (13 750 × 11 дельт, echo 7/7) | да |
| **T-V01-05** | L1 | zombie: нет op6 → на `2×interval` `CloseSocket(4000)` + `Reconnecting`, attempt=1 | **[A]** §4.4 | да |
| **T-V01-06** | L1 | hello-timeout: `WsOpened`, op8 нет → ровно на `hello_timeout_ms` закрытие + `HelloTimeout` | **[S]/[A]** | да |
| **T-V01-07** | L1 | close-классификация: 1000 → `Closed` без реконнекта; 4000 и «нет кода» → `Reconnecting`; 4801 → `Reconnecting` (resume мы не шлём) | `VD:108`, `[S]` | да |
| **T-V01-08** | L1 | реконнект: 3 попытки, паузы 0/2000/5000, затем `Closed{GaveUp{3}}`; `WsOpened`+op4 сбрасывает счётчик | `DESIGN/VOICE.md` §2.6 [A] | да |
| **T-V01-09** | L1 | leave: `LeaveRequested` из `Connected` → `CloseSocket(1000,"Force Close")` → `Closed`; после — `SessionError::Closed` и ноль эффектов | `VD:108` **[L]** | да |
| **T-V01-10** | L1 | op4 со `sdp` → `UnsupportedSessionDescription`, состояние `Closed`, ключ/mode/msid **не** применены, паники нет | §3.5 **[S]** | да |
| **T-V01-11** | L1 | дубли: повторный op2 (тот же ssrc) идемпотентен, второй `op1` не отправлен; повторный op4 → `DuplicateSessionDescription` | **[A]** инварианты §2.3 | да |
| **T-V01-12** | L1 | приватность: `Debug`/`Display`/`snapshot`/эффекты не содержат `token`, `secret_key`, `media_session_id`; в `snapshot` только `media_ready`/`mode` | образец `lib.rs:145` (тест `unknown_payload_debug_does_not_leak_secrets`) | да |
| **T-V01-13** | L1 | `cap` по времени: `now_ms` назад/на пределе `u64` не паникует и не двигает таймеры; `max_frame_bytes` превышен → `Transport(Oversize)` | **[A]** | да |
| **T-V01-14** | L2 | op1-паритет: `experiments` и `rtc_connection_id` присутствуют, `codecs` содержит opus/120/1000 и AV1/101; поля `data{}` дублированы в корне | `rtc1-frames.jsonl` op1 (828 Б) | да |
| **T-V01-15** | L2 | префиксы кадров: каждый префикс op8/op2/op4/op6 — без паники, `Protocol`-ошибка (расширение существующего `every_prefix_of_frame_is_panic_free`) | `tests/signaling.rs:386` | да |
| **T-V01-16** | L3 | V-01↔V-02 сшивка: настоящий `UdpMediaSocket` на loopback отдаёт 74 Б discovery → адрес/порт попадают в op1 и совпадают с принятым пакетом | `udp.rs` + `tests/udp.rs` | да |

Интеграционный (не в V-01, ставится как зависимость): `T-V01-17` [L4] — реальный WS+TLS и живой
сервер; **вне офлайн-гейта**, только в live-окне.

### 6.1 Мутации (обязательны, на приёмке G4)

| Мутация | Должен краснеть |
|---|---|
| `Connected` на op2 вместо op4 | T-V01-02, T-V01-10 |
| интервал heartbeat из константы вместо op8 | T-V01-04 |
| `ACK_DEADLINE = 1 × interval` | T-V01-05 |
| `seq_ack` всегда 1 | T-V01-04 |
| убрать проверку `sdp` (падать как «нет `secret_key`») | T-V01-10 |
| close 1000 → реконнект | T-V01-07 |
| 3 попытки → 4 | T-V01-08 |
| снять `CancelTimer(Hello)` после op8 | T-V01-06 |
| `snapshot` печатает `mode`+`ssrc`+`media_session_id` | T-V01-12 |
| убрать `experiments` из op1 | T-V01-14 |

### 6.2 Гейт

В batch: `cargo test -p discord-voice --test session_contract` (targeted) + существующий
`--test signaling` (регрессия). Полный `gate:worker` — один раз на batch в integration-лейне.
`--offline` обязателен: новых зависимостей в офлайн-части нет (§7).

---

## 7. Владение файлами

| Файл | Owner | Замечание |
|---|---|---|
| `waves/RW-40/VOICE/V01-SESSION-CONTRACT.md` | **G5** | этот документ — единственная правка G5 |
| `rust/crates/discord-voice/src/session.rs` (+ `session/` при дроблении) | **G1** | новый модуль; `pub mod session;` — 1 строка в `lib.rs` |
| `rust/crates/discord-voice/src/lib.rs` | **G1** | типизация op3/op6, `experiments`+`rtc_connection_id` в `SelectProtocol`, отделение `sdp`-ошибки |
| `rust/crates/discord-voice/tests/session_contract.rs` | **G1** | `T-V01-01…16` |
| `rust/crates/discord-voice/tests/signaling.rs` | **G1** | только расширение `every_prefix…` и op6 (не ломать существующие 11 тестов) |
| `discord-voice/src/udp.rs`, `srtp.rs`, `reconnect.rs` | **G1** | G5 не правит; `reconnect.rs` остаётся gateway-политикой |
| `rust/crates/discord-core/src/state/voice/{parser,reducer}.rs` | **G1** | `session_id` из `VOICE_STATE_UPDATE` (§8 риск R-3) |
| `rust/crates/discord-core/src/state/types.rs` | **Lead `[P]`** | общий файл: поле `session_id` в `VoiceState` правит Lead |
| `rust/crates/discord-core/src/dispatch.rs` | **Lead `[P]`** | `VOICE_SERVER_UPDATE` сейчас `Unsupported` (§8 R-3) |
| `discord-voice/Cargo.toml`, `rust/Cargo.toml` | **Lead `[P]`** | только для **live**-транспорта (`tokio`, `tokio-tungstenite`); офлайн-часть — без изменений |
| `docs/rust-client/TEST-PLAN.md` (`T-V01-01…16`) | **Lead `[P]`** | G5/G1 приносят готовые строки |
| `CONTRACTS/voice.md` (V-00) | **G1+G5 → Lead `[P]`** | отсутствующая предпосылка; без неё V-01 фиксирует только свои границы |
| `TASKS.md`, `waves/*`, `docs/map.md` | **Lead `[P]`** | — |

Пересечений нет: G5 в этой карточке **не пишет код**; G1 не трогает `discord-media`/`discord-platform`
(ветка V-04). Правки `state/types.rs`/`dispatch.rs` — только через Lead (AGENTS.md §«Общие файлы»).

---

## 8. Blocked-by-live и риски

### 8.1 Blocked-by-live (окончательно для V-01)

1. Фактические значения `helloTimeout`, ACK/zombie-порога, `description`-таймаута.
2. Смысл `seq_ack` (H-SEQ) и вообще семантика корневого `seq` (единственная точка: op15 `seq:0`).
3. Нужен ли RTC `op7 RESUME` (в capture не наблюдался) и что в нём `seq_ack`.
4. Происхождение/формат `rtc_connection_id`.
5. Реальные close-коды сервера при ошибках (наблюдался только 1000).
6. Появление op4 со `sdp` в живых сессиях.
7. Порядок op2/op1 на реальном сервере — **контракт построен на физической зависимости §3.2**;
   live должен подтвердить (или опровергнуть вместе с правкой RE-строки).
8. Точка «connected» в эталонном UI (Q-V01-2).
9. Ping UI: источник не установлен; V-01 **не** объявляет `rtt_hb` пингом.

### 8.2 Риски

| # | Риск | Митигация |
|---|---|---|
| R-1 | `reconnect.rs` переиспользуют для RTC и получат 30-секундный бэкофф при UI-контракте 0/2/5 | D-V01-1: отдельный `VoiceReconnectConfig`; тест T-V01-08 с 3 попытками |
| R-2 | `lib.rs` выдали за «manager» (парсеры ≠ машина состояний) | §4.1: явный список отсутствующего; T-V01-02…05 падают до реализации |
| R-3 | `Identify` невозможно собрать: `session_id` не читается в `discord-core` (`state/voice/parser.rs:6`), `VOICE_SERVER_UPDATE` = `Unsupported` (`dispatch.rs:82`) | G1+Lead: поле в `state/types.rs` + разбор события; **без этого V-01 live невозможен** |
| R-4 | `sdp`-кадр молча теряет причину (общий `VoiceError`) | fail-closed `UnsupportedSessionDescription` + T-V01-10 |
| R-5 | op1 без `experiments`/`rtc_connection_id` — не паритетный кадр | T-V01-14; origin — Q-V01-4 |
| R-6 | Таймауты [A] станут «фактами» следующей волны | §4.4: каждая [A]-константа с комментарием-ссылкой; §10 |
| R-7 | `Connected` на op4 может оказаться позже UI-ожидания (1 кадр, но лишняя строка «connecting») | Q-V01-2; менять одну границу, не машину |
| R-8 | RTC-WS = JSON-текст, а транспорт ядра — ETF/zstd (`ws.rs:203`) | V-01 offline не использует транспорт; live-адаптер — отдельная карточка с `Cargo.toml` |
| R-9 | Секреты в логах/снапшотах (token, secret_key, media_session_id) | T-V01-12; redaction по образцу `lib.rs:145` |
| R-10 | Правило H-SEQ «выглядит доказанным» на одной точке данных | помечено **[A]** в коде и тесте; live-проверка в §8.1 п.2 |
| R-11 | 43-байтные аномальные RTP-пакеты притащат в V-01 «второй профиль» | V-01 медиа не разбирает; аномалия остаётся `unknown` (риск 11 `G5-PREREQ`) |
| R-12 | Документационный дрейф шапки `lib.rs:9-10` («транспортных и сетевых операций здесь нет») | читать код; правку шапки делает G1 вместе с `session`-модулем |

---

## 9. Вопросы к PM/Lead (решить до кода V-01)

| # | Вопрос | Предложение G5 | Блокирует |
|---|---|---|---|
| **D-V01-1** | Бэкофф реконнекта RTC: `reconnect.rs` (1/2/4/8/16/30) или `DESIGN/VOICE.md` §2.6 (0/2/5, 3 попытки)? | **0/2/5, 3 попытки** (уже в UI-контракте G3); `reconnect.rs` оставить gateway | T-V01-08 |
| **D-V01-2** | `Connected` на op2 (как в логе движка) или на op4 (как в этом контракте)? | **op4** (§2.2), границу пересмотреть после live | T-V01-02/03 |
| **D-V01-3** | Нужен ли V-01 `op7 RESUME` в v1? | **нет** (не наблюдался [S]-only), реконнект = полный handshake | T-V01-07/08 |
| **Q-V01-4** | `rtc_connection_id`: кто и как генерирует? | непрозрачная строка; формат не закреплять | T-V01-14 |
| **Q-V01-5** | Значения `HELLO/JOIN/DESCRIPTION/ACK` таймаутов — принять как `[A]`-дефолты или калибровать в первом live-окне? | принять §4.4 как контракт, калибровать одной правкой | T-V01-05/06 |
| **Q-V01-6** | Ping UI (63…92 мс в дампе) не привязан к источнику; делать ли `rtt_hb` (эхо `t` op3→op6) кандидатом? | **не** объявлять пингом; `rtt_hb` — только диагностика V-01, V-06 решает отдельно (гипотеза H-PING: 13 750 мс грубее наблюдаемого темпа обновления ~9 с, но не опровергает его) | V-06 |
| **Q-V01-7** | Кто правит строку `NETWORK-CONTRACTS.md` §2.1 (`op1 → op2`)? | Lead/RE: заменить на `op2 → discovery → op1` по §3.2 | весь контракт |
| **Q-V01-8** | V-00 (`CONTRACTS/voice.md`) не написан — начинать V-01 без него? | разрешить V-01 в границах §1 и зафиксировать V-00 отдельной карточкой (PCM-раскладка/владение буферами нужны V-04/V-05a) | V-04/V-05a |

---

## 10. Что этот документ не утверждает

- Не утверждает, что голос работает: медиа не шло ни в одной из трёх сессий (канал был пуст,
  микрофон молчал) — см. `NETWORK-CONTRACTS.md` §2.7.
- Не даёт wire-совместимости: ни SRTP (схема B-08), ни реального WS-обмена не проверялось.
- Не закрепляет `[A]`-значения таймаутов, бэкоффа и H-SEQ как измеренные; live их не подтверждал.
- Не описывает `seq`-механику за пределами H-SEQ, `op7/op9`, `op5/11/12/13/14`, MLS-кадры
  (op21/22/24/25/27/29/30) и бинарный `sendBinary` (`[1 байт op][payload]`, `[S]`).
- Не решает DAVE, Opus, устройства, PTT, UI-тексты и ping.
- Не является V-00: формат PCM-буферов, владение и границы потоков остаются в `CONTRACTS/voice.md`
  (файла нет).

Не запускались: live, сеть, аудио, сборка/тесты (`cargo`), установки. Изменён только этот файл.
