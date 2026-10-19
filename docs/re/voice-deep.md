# Голос Discord 1.0.162 — глубокий разбор (Linux x64, сессии 1–3, 2026-10-19)

Наблюдательная база: три живые голосовые сессии в копии клиента (отдельный процесс, CDP 9223),
движковый лог, CDP-захват WS-кадров RTC, tcpdump, strace рендерера (полные байты UDP), `ss`.
Токены/сессии/ключи вырезаны; публичные IP заменены на `<PUB_IP>`; секрет `secret_key` — `REDACTED`.

## 0. Источники

| Артефакт | Что даёт |
|---|---|
| `voice/rtc1-frames.jsonl` / `rtc2-frames.jsonl` (`rtc-summary`) | RTC WS-кадры сессий 1 и 2 (op, payload, санитизировано) |
| `discord-run3.log` | движковый лог: RTCConnection, codecs, modes, DAVE/MLS, disconnect |
| `voice/voice-udp3.pcap` (46 МБ) / `voice-udp4.pcap` (2.7 МБ, snap 96) | UDP-медиа сессий 2 и 3 |
| `voice/voice-udp3.rtp.json`, `voice-udp4.rtp.json` | классификация и каденция (analyzer) |
| `voice/strace-renderer-s3.log` (37 МБ) + `voice/strace-media-s3.jsonl` | **полные байты** каждого медиа-пакета сессии 3 + сетевые syscalls |
| `voice/ss-*.txt` | какие сокеты кому принадлежат (fd/PID) |
| `voice/ui/`, `voice/ui-s2/`, `voice/ui7/` | голосовая панель (HTML/текст/скриншоты), UI-поиск, попап |

Методика: `pcap-analyze.mjs` (потоки), `rtp-analyze.mjs` (класс discovery/keepalive/RTP/RTCP),
`pcap-dump.mjs`/`hex-offsets.mjs` (hex), `strace-extract-packets.mjs` (декод C-escape из strace → jsonl).
Сессия 3 проводилась под strace и `ss`, с ротацией сокета на +60 с.

## 1. Сигнализация RTC (WebSocket)

Подключение: `RTCControlSocket` → `wss://c-<region>-<hash>.discord.media:<2053|2096>/` (v=9), TLS ~300 мс.

| op | направление | содержимое (сжато) |
|---|---|---|
| 8 Hello | in | `{"v":8,"heartbeat_interval":13750}` |
| 0 Identify | out | `server_id`, `channel_id`, `user_id`, `session_id`(REDACTED), `token`(REDACTED), `max_dave_protocol_version:1`, `video:true`, `streams:[{type:"video",rid:"100",quality:100}]` |
| 1 Select Protocol | out | `protocol:"udp"`, `data:{address,port,mode}`, `codecs:[{name:"opus",type:"audio",priority:1000,payload_type:120},{name:"AV1",type:"video",priority:1000,payload_type:101,...}]` |
| 2 Ready | in | `ssrc`(audio), `port`, `ip`, `modes:["aead_aes256_gcm_rtpsize","aead_xchacha20_poly1305_rtpsize"]`, `experiments:["fixed_keyframe..."]`, `streams:[{type:"video",ssrc,rtx_ssrc,rid:"100",quality:100,active:false}]` |
| 4 Session Description | in | `mode:"aead_aes256_gcm_rtpsize"`, `secret_key` (32 байта, REDACTED), `media_session_id` (REDACTED), `dave_protocol_version:1`, `secure_frames_version:1`, `audio_codec:"opus"`, `video_codec:"AV1"` |
| 15 | in | `{"any":100}` — sink wants (сессия 2); сессия 3 — `{"any":100}` |
| 16 | in | `{"voice":"0.22.1+denbeigh.remove-voice-curator-74a3bd0c5","rtc_worker":"1.6.98"}` |
| 3 / 6 | out/in | heartbeat 13 750 мс, ACK (`seq: s`) |
| close | in | code 1000, reason «Force Close» при корректном выходе |

Сессии:

| | сессия 2 (11:43) | сессия 3 (12:39) |
|---|---|---|
| RTC endpoint | `c-fra19-4da7fe5f.discord.media:2053` | `c-arn02-5229f1fc.discord.media:2096` |
| Регионы (латентс-тест) | frankfurt, rotterdam, paris, milan, brussels | stockholm, helsinki, finland, denmark, warsaw |
| Медиа-сервер | `104.29.147.145:19309` | `104.29.146.60:19329` |
| ssrc (audio / video / rtx) | 17843 / 17844 / 17845 | 9208 / 9209 / 9210 |
| Публичный адрес клиента | `<PUB_IP>:52287` | `<PUB_IP>:35251` (после ротации — другой NAT-порт) |
| Продолжительность | ~88 с | ~104 с |

`RTCLatencyTestManager` перед входом шлёт UDP-зонды по регионам (в strace: `socket()` → серия
`sendmsg`), ранжирует и берёт первый — отсюда разные `c-<region>`.

## 2. UDP-транспорт (байтово, по strace)

### 2.1 Discovery (IP-discovery)
Запрос 74 Б: `[u16 type=1][u16 len=0x0046][u32 ssrc][64 Б нулей]`
Ответ 74 Б: `[u16 type=2][u16 len=0x0046][u32 ssrc][64 Б ASCII-адреса NUL-паддинг][u16 порт BE @72]`.
Пример ответа: `0002 0046 000023f8 "<PUB_IP>\0…" 89b3` → порт 35251.
При входе **два сокета-кандидата** одновременно шлют запрос; побеждает тот, чей ответ пришёл первым.

### 2.2 Keepalive
8 Б: клиент `13 37 ca fe 01 00 00 00`, сервер `13 37 f0 0d 01 00 00 00`. Каденция 5.000 с
(интервалы 4.96–5.02 с), обе стороны.

### 2.3 RTP (медиа)
Заголовок `V=2, P=0, X=1, CC=0`, `PT=120` (opus), расширение RFC 5285: профиль `0xBEDE`, 2 слова
(8 Б данных). Хвост: `[ciphertext][GCM tag 16 Б][4 Б LE-счётчик]`, счётчик = `0x80000000 | n`,
сквозной для RTP и RTCP на сокете, n с 0.
Каденция 20 мс (`RTP ts` дельты 960/1050/870 — сумма 3840 на 4 кадра = 960 @48 кГц).
За голосовой сеанс: один talkspurt ~200 мс (10 пакетов) → далее тишина (DTX, пакеты не шлются).

Аномалия: **первые 2 пакета talkspurt — 43 Б** без валидного расширения и без трейлера-счётчика
(см. §5, unknowns). Далее 8 пакетов по 47 Б со счётчиками 0..7. Шифротекст opus всего 3 Б —
это comfort-noise/DTX-кадры, а не речь (в тестовом канале никто не говорил).

### 2.4 RTCP / SRTCP
- `SR` (PT 200) клиент → сервер: `[8 Б: V2/RC=0/PT/len=6][ssrc][20 Б шифрованного sender info][tag 16][счётчик 4]` = 60 Б.
  Интервалы 3.0–6.3 с (медиана 5.2 с) — RTCP-таймер с рандомизацией/лимитами.
- `RR` (PT 201) сервер → клиент: `[8 Б: V2/RC=1/PT/len=7][ssrc][24 Б шифрованного report block][tag 16][4 Б]` = 52 Б, ровно 1/с (медиана 1001 мс).
  Трейлер сервера — без флага `0x80…` (значения 0,1,2,…). Поля отчёта (потери/джиттер) зашифрованы.
- Пустой `RR` (28 Б) клиент → сервер: отправляется **сразу после ротации сокета** (см. 2.5).
- Клиент не шлёт receiver-отчёты о сервере (нечего отчитывать: сервер не слал медиа — в канале никого не было).

### 2.5 Ротация медиа-сокета (+60 с)
Наблюдение воспроизведено в обеих сессиях и подтверждено strace:
`12:39:27.880 socket() → sendmsg(discovery)` (2 кандидата) → **`12:40:27.961 socket()`** (+60.08 с) →
discovery с тем же ssrc → новый NAT-порт → пустой RR → SR/keepalive продолжаются до дисконнекта.
Старый сокет после ротации молчит. Ротация в лог движка не пишется (внутренняя логика RTC-воркера).
`ss` в момент голоса: медиа-сокет `0.0.0.0:55599` принадлежит процессу рендерера (`pid 95974, fd=44`).

## 3. Движок (лог)

```
[Connection] Available engine codecs: AV1(dec+enc), H264, H265, VP8
[Connection] Experimental codecs: AV1, H265
[Connection] Audio codecs: opus
[Connection] Encryption modes: aead_aes256_gcm_rtpsize, aead_aes256_gcm,
  aead_xchacha20_poly1305_rtpsize, xsalsa20_poly1305_lite_rtpsize,
  xsalsa20_poly1305_lite, xsalsa20_poly1305_suffix, xsalsa20_poly1305
[Connection] Selected encryption mode: aead_aes256_gcm_rtpsize
[Connection] updateVideoQuality: 1280x720@30, bitrate 2.5 Мбит/с (±150к), rid 100,
  ssrc 9209 (rtx 9210), remoteSinkWantsMaxFramerate 20
[ConnectionEventFramerateReducer] Reduced framerate after 15000 ms
[RTCConnection] Remote media sink wants: {"any":100}
[RTCConnection] DAVE protocol init with protocol version: 1
[RTCConnection] Received MLS external sender package / Got MLS key package, sending to RTC server
[RTCConnection] Setting media-session-id: <REDACTED>
[RTCConnection] Disconnected clean: true, code: 1000, reason: Force Close
[RTCConnection] Error: codec survey is not available
[VOICE_DISCONNECT] krisp_nc_model: krisp-nc-o-med-v7.kef
```

Замечания:
- Шумоподавление Krisp: модель `krisp-nc-o-med-v7.kef`; селект «Шумоподавление» в настройках
  «Голос и видео» (см. `frontend-d03.md`).
- DAVE/E2EE (MLS): «external sender package», «key package» — подтверждение E2EE поверх SRTP;
  крипто-слой DAVE (Cisco mlspp в `discord_voice.node`, см. `native-re.md`) в этом отчёте не разобран.

## 4. UI голоса
Панель: `[class*=panels]`, кнопки «Микрофон/Звук/Настройки/Отключиться», статус-текст с ping
(`voice/ui/01-voice-panel.html`, 61 КБ), вход выполнен через QuickSwitcher (Ctrl+K → «тестируем твики» → Enter).
Выход — кнопка «Отключиться» (aria-label), дисконнект чистый (1000).

## 5. Пробелы и unknowns
1. **43-байтные первые 2 пакета talkspurt** — нет валидного расширения/трейлера:
   `90f84438 109919cc 000023f8 19f5e987 …` и `90784439 10991d8c 000023f8 2d6d6f13 …`.
   Гипотезы: прогревочные DTX-кадры без AEAD-трейлера; требует отдельного разбора (расшифровки).
2. Места закрытия старых fd при ротации в strace не видны (нет `close(94/95)`); закрываются иначе.
3. RTCP-тела (sender info/report block) шифрованы → потери/джиттер снаружи не читаются; для тестов
   jitter-буфера нужен собственный оракул (или GCM-ключ из op4 + AAD/nonce-схема).
4. `VOICE-CRYPTO.md` прошлой волны (режимы 0–6, AAD/nonce/ключ) был в `/tmp` и **потерян** (tmpfs);
   в этом отчёте крипто-поля даны наблюдательно (mode 5, tag 16, 4 Б LE-счётчик в хвосте, ключ 32 Б).
