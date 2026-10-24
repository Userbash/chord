# G5-VOICE-PREREQ — предпосылки voice MVP и офлайн-план Opus KAT

Дата: 2026-10-24. Owner: G5 Voice/Platform. HEAD: `0239543`.
Режим: read-only аудит; код, тесты, зависимости, фикстуры и системные пакеты не менялись.
Не запускались live/сеть/audio/сборка/установки; выполнялись только чтение файлов и
read-only запросы (`pkg-config --modversion/--libs`, `ls`, `ldconfig -p`, `ffmpeg -version`,
`ffmpeg -h encoder|decoder=opus`, `grep`).
Этот документ — **финальный voice-prereq план**, не отчёт о выполненной реализации и не
доказательство live-совместимости.

Ревизия 2 (G5, OpenCode): независимая перепроверка ревизии 1 и правки по фактам — исправлены
номера строк nonce (`srtp.rs:522-535`), количество файлов `fixtures/voice/` (6, не 5),
формулировка про коллизию ID (это два параллельных пространства имён, а не один дубль),
точность длительности в KAT (без «±1 кадр»), выравнивание через `OPUS_GET_LOOKAHEAD`,
разделение «DTX энкодера» и «подавление передачи», добавлены структурные/негативные проверки,
вариадический `*_ctl`, интеграция в манифест фикстур и в гейт, ограничения ffmpeg-энкодера.

## 0. Итог

Voice MVP **не готов к live**. Есть разбор RTC-сигнализации, gateway-состояние голоса,
UDP/RTP/RTCP-примитивы, SRTP-кадрирование на самосогласованной криптосхеме и
крипто-примитивы с KAT. Нет RTC session manager, известной схемы SRTP KDF/nonce/AAD
(ключ op4 сам по себе бесполезен), libopus FFI, устройств/захвата, сквозного пайплайна и
DAVE/MLS. Opus-этап (V-04) от live не зависит и может быть выполнен первым; всё, что
касается wire-совместимости, остаётся blocked-by-live.

## 1. Проверка черновика: подтверждено / исправлено

Проверялись утверждения Codex-черновика по коду, RE-документам и планам RW-40.

| # | Утверждение черновика | Вердикт | Уточнение / доказательство |
|---|---|---|---|
| C-01 | RTC signaling: моделирование op0/1/2/4, нет session manager | **подтверждено** | `discord-voice/src/lib.rs:350` `parse_frame`, `:642` `VoiceServerUpdate`, `:655` `websocket_url`; транспорта WS нет |
| C-02 | UDP: discovery/socket/keepalive/rotation primitives, live не подтверждён | **подтверждено, уточнено** | `udp.rs:247` `UdpMediaSocket` — это **реальный** `std::net::UdpSocket` (bind/send/recv), тесты `tests/udp.rs` идут на loopback; шапка `lib.rs:9-10` («транспортных и сетевых операций здесь нет») устарела |
| C-03 | RTP/jitter/reception проверены на capture-oracle | **частично** | RTP — да (`fixtures/voice/rtp-pt120-oracle.json`: 8 валидных + 2 аномальных 43 Б). Reception stats — да (`waves/RW-26/research/RE-VOICE-STATS.md`). Jitter-буфер capture-оракула **не имеет** — только детерминированный контракт push/pop |
| C-04 | SRTP/crypto primitives есть, live interop нет | **подтверждено, усилено** | См. C-05: interop невозможен не только из-за вырезанного ключа, но и из-за самосогласованной схемы |
| C-05 | «KDF и RTP/SRTCP frame handling присутствуют» | **исправлено (существенно)** | `srtp.rs:44-46` выводит ключ через HKDF-SHA1 с **локальными** метками `discord-lab/srtp/aead-key` и `.../nonce-seed`; `crypto/kdf.rs:1-10` прямо пишет: «это **не** label-KDF RFC 3711 §4.3.1… совместимость не заявляется». Nonce тоже сконструирован локально (`srtp.rs:522-535`: GCM = seed8‖trailer4, XChaCha = trailer4‖seed16‖0⁴; `srtp.rs:215-216` — вывод ключа/seed). Хуже: `SrtpContext::new` требует 32-байтный ключ **и 14-байтную соль**, а op4 отдаёт только `secret_key` 32 Б без поля соли ⇒ из протокола не выводится и сигнатура API. Значит key из op4 **нельзя применить** без восстановления схемы — это отдельный блокер `NETWORK-CONTRACTS` B-08 (`NETWORK-CONTRACTS.md:473`), а не следствие редакции ключа |
| C-06 | Режимы: AES-GCM/ChaCha | **уточнено** | Поддержаны 3 из 7 объявленных движком режимов; 4 `xsalsa20_poly1305*` возвращают `UnsupportedMode` (`srtp.rs:48-53`) |
| C-07 | Opus: только константы, нет FFI/encoder/decoder | **подтверждено** | `discord-media/src/lib.rs` — 26 строк, зависимостей нет, `unsafe_code = "forbid"` |
| C-08 | Devices/capture отсутствуют | **подтверждено** | В `discord-platform` нет аудио-FFI; есть только `ffi_hunspell.rs`/`ffi_secret.rs` как образец дисциплины |
| C-09 | UI: fake-store панель V-08 существует, к сервису не подключена | **подтверждено, уточнён путь и ID** | `rust/gui-spike/ui/features/voice.js` (710 строк) + `tests/unit/voice-ui.test.mjs`; каталога `ui/` в корне нет |
| C-10 | V-01 = «Gateway op8 → Identify op0 → Ready op2 → Session op4» | **исправлено** | Смешаны слои. Join — gateway **op4** (`NETWORK-CONTRACTS.md`, таблица §1 строка 16) → `VOICE_STATE_UPDATE` + `VOICE_SERVER_UPDATE`; затем отдельный RTC WS, где op8 Hello, op0 Identify, op2 Ready, op4 Session Description. «Gateway op8» неверно. Дополнительно для контракта V-01: клиент допускает **два вида** RTC op4 — `mode`+`secret_key` или `sdp` (`NETWORK-CONTRACTS.md:240`), обработку `sdp` нельзя молча отбрасывать |
| C-11 | Граф зависимостей V-01…V-07, V-08 = UI | **исправлено** | В `MASTER-PLAN.md:83` **V-08 = DAVE/MLS** (G1+G4, XL, зависит от V-04). Причина — не один дубль, а **два параллельных пространства имён**: карточки реализации (`MASTER-PLAN.md:76-83`) и фичи паритета (`PARITY/FUNCTION-INVENTORY.md` §7, `QUEUE.md:69-71`, `TZ.md:78`), сдвинутые на единицу: там `V-04` = SRTP и `V-05` = Opus, а в MASTER-PLAN `V-04` = Opus; `TZ.md:78` использует `V-02` для RTT-статистики. `MERGE-MANIFEST.md:25` фиксирует «V-08 UI». DAVE/MLS в черновике отсутствовал вовсе, хотя op4 объявляет `dave_protocol_version:1` и в capture виден MLS key package |
| C-12 | Граф: V-04 нарисован потомком V-03 | **исправлено** | Противоречило тексту «V-04 does not require live V-01..V-03». Перерисовано: V-04 — независимая ветка от V-00-контракта |
| C-13 | V-01 offline = контракт + фикстуры | **дополнено** | Gateway-половина уже есть и подключена: `discord-core/src/state/voice/{parser,reducer,selectors}.rs` (P2-10, memory-only), вызов из `state/reducer.rs:79`. Нет именно RTC-половины и сервиса, связывающего их |
| C-14 | libopus FFI «в platform по прошлому плану» | **подтверждено линтами** | `discord-voice` и `discord-media` — `unsafe_code = "forbid"` (ослаблять нельзя, PLAT-12/15); `discord-platform` — `deny` + `#[allow(unsafe_code)]` на модулях. Место FFI — `discord-platform/src/ffi_opus.rs` |
| C-15 | Системная таблица (opus/ALSA/PipeWire/SPA) | **подтверждено** | См. §3; у `libspa-0.2` действительно нет `.so` и пустой `--libs` |
| C-16 | D-113 цитируется, но решения нет | **подтверждено** | `D-113` встречается только как ссылка (`TASKS.md:118-119`, `QUEUE.md:147`, `MASTER-PLAN.md:79`) — записи решения в RW-40 нет |
| C-17 | KAT: фикстура 50×960, ffmpeg native oracle | **подтверждено, дополнено** | ffmpeg n9.0.2 содержит **нативную** реализацию (`-h decoder=opus` без `exp`); нативный энкодер помечен `exp` и поддерживает только 48 кГц/fltp, mono или stereo. Нужны `-strict -2`, конверсия в fltp, **явный `-opus_delay`** (по умолчанию 360 мс) и учёт задержки/`pre-skip` (см. §5.2–5.4) |
| C-18 | Нет фикстур Opus | **подтверждено, число исправлено** | `fixtures/voice/` = **6** файлов (`rtc{,1}-frames.jsonl`, `rtc{,1}-summary.json`, `rtp-pt120-oracle.json`, `session3.timing`); каталога `fixtures/voice/opus/` нет; в `fixtures/MANIFEST.json` записей про Opus нет |
| C-19 | «API объявлен в `opus.h`» | **подтверждено, добавлен риск FFI** | `/usr/include/opus/`: `opus.h`, `opus_defines.h`, `opus_types.h`, `opus_multistream.h`, `opus_custom.h`, `opus_projection.h`. Но `opus_encoder_ctl`/`opus_decoder_ctl` — **вариадические** (`opus_encoder_ctl(st, request, ...)`); макросы `OPUS_SET_*` в Rust не переносятся. Объявление — только `unsafe extern "C" { fn opus_encoder_ctl(st: *mut OpusEncoder, request: c_int, ...) -> c_int; }` с фиксированным типом аргумента (`c_int`) и одним `// SAFETY:` на вызов |
| C-20 | «ffmpeg native Opus как независимый оракул» | **уточнено (границы)** | Нативный **декодер** независим (без `exp`, `-h decoder=opus` → «Decoder opus [Opus]»), нативный **энкодер** помечен `exp` и имеет только `-opus_delay`/`-apply_phase_inv`: ни bitrate, ни DTX, ни `application`, ни FEC не задаются. Значит encode-путь ffmpeg годится как генератор **валидного** потока для decode-KAT, но не как модель настроек Discord |
| C-21 | «ALSA `null` проверит capture/playback» | **уточнено (нет внешнего оракула)** | На хосте нет `alsa-utils` (`arecord`/`aplay`) и `opus-tools` (`opusdec`/`opusenc`); `ffmpeg -devices` даёт ALSA только как **output**. Поэтому `null` доказывает лишь успешные коды `open/params/write/read` и учёт `snd_pcm_avail`; независимого аудио-оракула для ALSA на этом хосте нет |

## 2. Что есть и чего нет

| Область | Есть | Где | Статус |
|---|---|---|---|
| RTC signaling | op8/0/1/2/4/15/16/3/6, `Identify`/`SelectProtocol`/`Ready`/`SessionDescription` с redaction в `Debug`, `VoiceServerUpdate`, выбор региона | `discord-voice/src/lib.rs`, `regions.rs` | primitives; нет manager/reconnect WS |
| Gateway voice state | парсер `VOICE_STATE_UPDATE`, reducer, селекторы, `session_id` не сохраняется | `discord-core/src/state/voice/*` (P2-10) | есть и подключено к dispatch, I/O нет |
| WebRTC (ICE/DTLS/peer connection) | — | — | отсутствует; по RE не наблюдался и для MVP не требуется (Discord: свой RTC WS + UDP + SRTP) |
| UDP | discovery 74 Б (encode/parse), keepalive 8 Б, `UdpMediaSocket`, ротация 60.08 с, пустой RR/SRTCP-RR | `discord-voice/src/udp.rs` | код есть; сервер не проверялся |
| RTP | парсер/энкодер заголовка + RFC 5285 ext, `RtpSender` (seq/ts+960/marker), `classify_opus_payload` | `media/rtp.rs` | проверено на capture-oracle (10/10 дельт = 960; в `voice-deep` §2.3 есть окно 960/1050/870 — среднее 960, см. §7 п.16) |
| Jitter | детерминированный буфер (окно 2^16, дубли/опоздавшие/overflow, история 512) | `media/jitter.rs` | контракт есть; capture-оракула нет |
| Reception/RTCP/stats | RFC 3550 §6.4.1 счётчики, парсер compound RTCP/SRTCP, `RttEstimator`, окно статистики | `reception.rs`, `rtcp.rs`, `stats.rs` | парсер — на capture; RTT — на синтетике |
| Reconnect | gateway-политика RESUME/IDENTIFY/GiveUp, бэкофф | `reconnect.rs` | gateway, **не RTC WS** |
| Crypto | AES-256 (FIPS-197), GCM (SP 800-38D), ChaCha20-Poly1305 (RFC 8439), XChaCha (draft-irtf), HKDF-SHA1 (RFC 5869) | `crypto/*`, `tests/crypto_vectors.rs` (16 тестов) | KAT по опубликованным векторам |
| SRTP | кадрирование `[clear][ct][tag16][ctr4 LE]`, AAD=header, replay-окно 1024 nonce, 3 режима | `srtp.rs` (22 теста) | формат самосогласован; wire — нет; API требует 14-байтную соль, которой в op4 нет (см. §4 п.4) |
| Opus | только строковые константы `"opus"`/`"AV1"` | `discord-media/src/lib.rs` | **нет** FFI/enc/dec/PLC |
| Devices/capture/playback | — | — | **нет** (ни ALSA, ни PipeWire, ни portal-аудио) |
| Streaming pipeline | — | — | **нет**: PCM→Opus→RTP→SRTP→UDP и обратно не собраны |
| Mute/deafen/PTT | — | — | **нет** (STUB-17) |
| DAVE/MLS (E2EE) | только поля `dave_protocol_version`/`secure_frames_version` | `lib.rs:35-39`, `:274` | **нет** MLS-слоя. В клиенте Opus-кадры обрабатывает `discord::dave::codec_utils::ProcessFrameOpus` / `ValidateEncryptedFrame` (`native-re.md` §2.5) ⇒ при активном DAVE payload внутри SRTP — DAVE-кадр, а не сырой Opus |
| Voice UI | панель 12 состояний на fake store + unit-тесты | `rust/gui-spike/ui/features/voice.js`, `tests/unit/voice-ui.test.mjs` | mock, к сервису не подключена |
| Voice service/backend (JS) | — | в `src/` voice-модуля нет; `src/core/report/domains.mjs` содержит только домен-метку `voice` для RE-отчётов | **нет** |

Итого по формулировке задачи: RTC/UDP/SRTP/RTP/jitter/crypto — **примитивы есть**;
WebRTC (в смысле ICE/DTLS/peerconnection) — **нет**; по RE (`voice-deep.md` §1–2) голос Discord
идёт своим RTC WS + UDP + SRTP, ICE/DTLS/peerconnection не наблюдались, поэтому WebRTC-стек для
MVP не требуется (термин «WebRTC» здесь — только историческое название технологии). Opus,
устройства, захват/воспроизведение, streaming и DAVE — **нет**.

## 3. Системные предпосылки (только чтение, без установки)

Команды и наблюдения (этот хост, 2026-10-24):

| Библиотека | `pkg-config --modversion` | Заголовки | Shared object |
|---|---:|---|---|
| libopus | 1.6.1 | `/usr/include/opus/{opus.h,opus_defines.h,opus_types.h,opus_multistream.h,opus_custom.h,opus_projection.h}` | `libopus.so`, `libopus.so.0` |
| ALSA | 1.2.16.1 | `/usr/include/alsa/asoundlib.h` | `libasound.so`, `libasound.so.2`; плагины `libasound_module_pcm_pipewire.so`, `libasound_module_pcm_pulse.so` |
| libpulse | 17.0-134-g77d25 | `/usr/include/pulse/{pulseaudio.h,simple.h}` | `libpulse.so`, `libpulse-simple.so`; сокет `/run/user/1000/pulse/native` |
| PipeWire | 1.6.10 | `/usr/include/pipewire-0.3/pipewire/pipewire.h` | `libpipewire-0.3.so`, `.so.0` |
| SPA | 0.2 | `/usr/include/spa-0.2/spa/buffer/buffer.h` | **нет** `libspa-0.2.so` в `ldconfig`; `pkg-config --libs libspa-0.2` пуст |

Дополнительно:

- `opus_encoder.h` отдельно не поставляется — C-API (`opus_encoder_create`, `opus_encode`,
  `opus_encoder_ctl`, `opus_decoder_*`, `opus_strerror`, `opus_get_version_string`) объявлен
  в `opus.h`/`opus_defines.h`; это совпадает с планом G5 §4.1.
- ALSA `pcm.null` определён в `/usr/share/alsa/alsa.conf:335-344` («Discard all samples
  (playback) or generate zero samples (capture)») — офлайн-тесты capture/playback возможны
  без железа. Это **не** проверка устройства, consent и PipeWire-сессии.
- **PulseAudio — второй доступный путь, не замена решения.** Эталонный клиент линкует в
  `discord_voice.node` и `libpulse.so.0`, и `libasound.so.2` (`native-re.md` §2.1); PipeWire в
  NEEDED нет — он грузится `dlopen` и в RE привязан к screencast (`native-re.md` §2.2), а не к
  аудио. На хосте есть плагины `libasound_module_pcm_pipewire.so` /
  `libasound_module_pcm_pulse.so`, т.е. ALSA-путь плана (`G5-VOICE-PLAN` §5.1) доходит до
  `pipewire-pulse`, а не до голого железа. Выбор ALSA остаётся в силе, но какой протокол клиент
  реально использует для списка устройств, из NEEDED не следует ⇒ паритет **списка устройств** с
  клиентом не гарантирован; это `[A]`-риск, а не блокер.
- `ffmpeg` n9.0.2 собран с `--enable-libopus`, но нативный кодек присутствует отдельно:
  `ffmpeg -h encoder=opus` → «Encoder opus [Opus] … dr1 delay small exp», только 48 кГц,
  `fltp`, mono/stereo; `-h decoder=opus` → нативный декодер (без `exp`).
  Для независимого оракула обязательны `-c:a opus` (не `libopus`) и `-strict -2`.
- `docs/rust-client/UNSAFE-REGISTRY.md` **отсутствует**; PLATFORM-AUDIT §2.3 (строка 147) прямо
  возлагает перенос таблицы «блок → инвариант → тест» на **владельца G5**, но файл лежит в общем
  `docs/` ⇒ содержимое готовит G5, правку согласует Team Lead.
- Инструментов-оракулов для аудио нет: `arecord`/`aplay` (alsa-utils) и
  `opusdec`/`opusenc` (opus-tools) не установлены; `ffmpeg` даёт ALSA только как **output**
  (`-devices` → `DE alsa`). Независимый оракул для Opus есть (`-c:a opus`), для ALSA — нет.
- Устройства и демоны (только наблюдение, без открытия): `/dev/snd` содержит `controlC0…C4`,
  `pcmC*D0c/p`; запущены `pipewire`, `wireplumber`, `pipewire-pulse` (`pgrep -af`). ACL на
  узлах: `user:sanya:rw-` при `group:audio` и отсутствии `sanya` в группе `audio`
  (`getfacl /dev/snd/controlC0`, `pcmC2D0c`) — то есть права даёт ACL, а не группа. Это
  объясняет, почему нельзя опираться на «пользователь в audio».
- Вариадический `*_ctl` — единственная сложная точка ABI: тип аргумента проверяется только
  глазами (C-19); ошибка типа даёт неопределённое поведение, не ошибку компиляции.

Чего эти факты **не** доказывают: доступность и права на реальный PCM (в G5-плане ACL
`user:sanya:rw-` на `/dev/snd`, пользователь вне группы `audio`), наличие PipeWire-согласия,
корректность ABI на уровне конкретных символов, работу портала. Аудиоустройства и ALSA
`null` в этой карточке **не открывались** (запрет на audio).

## 4. Dependency graph (V-01…V-08) и офлайн-граница

```text
V-00 контракт (G5+G1): формат кадра, владение буферами, ошибки, границы потоков
 │                        (файла нет: в `CONTRACTS/` только dispatch.md и read-state.md)
 │
 ├─ V-04 libopus FFI + KAT (G5; ревью G1/G4)   офлайн-часть ПОЛНОСТЬЮ; live — только настройки
 │    └─ V-05a capture/send: ALSA null → PCM → encode (G5)   offline: null + fake enum
 │         └─ V-06 UI, реальная проводка (G2+G3)             offline: fake-панель уже есть
 │
 ├─ V-01 RTC session manager (G1)     offline: state machine + санитизированные фикстуры
 │    └─ V-02 UDP live (G1)           offline: loopback/fake transport (код уже есть)
 │         └─ V-03 SRTP live (G1+G4b) offline: ТОЛЬКО crypto KAT; схема KDF/nonce/AAD = B-08
 │              ├─ V-05b playback: decrypt → jitter → decode → ALSA (G5)
 │              └─ V-08-DAVE/MLS (G1+G4, XL; также после V-04) — scope MVP не подтверждён
 │
 └─ V-07 reconnect/stats/regions (G1; зависит от V-04 по MASTER-PLAN)
      offline: fake-clock, фикстуры RTCP, ранжирование регионов; live: семантика сервера
```

Правила чтения графа:

1. **V-04 не зависит от V-01…V-03.** Кодек и KAT делаются и принимаются при отсутствии live:
   единственный live-остаток V-04 — выбор bitrate/application/DTX и совместимость с потоком
   Discord (§8), а не сам FFI или KAT.
2. **V-00 — реальная карточка, а не заголовок графа.** Контракт формата и владения (PCM-раскладка,
   кто освобождает буферы, что видит UI, какие ошибки typed) нужен до V-04/V-05a, иначе
   кодировщик и ALSA-слой зафиксируют разные предположения. Прецедент формы —
   `CONTRACTS/read-state.md`; voice-контракта в `CONTRACTS/` нет.
3. **V-05 делится на две ветки.** Capture/send (V-05a) не требует SRTP: `RtpSender` уже есть,
   шифрование можно подменить identity-обёрткой в тесте. Playback (V-05b) без рабочего
   decrypt бессмысленен — до V-03 гонять его можно только на fake-пакетах.
4. **V-03 нельзя закрыть офлайн.** Ни ключ (вырезан), ни KDF/nonce/AAD-схема (B-08) из
   capture не выводятся; текущая схема — изобретённая нами. Офлайн-работа — только vectors/
   tamper/replay и фиксация формата. Конкретно: `SrtpContext::new(master_key: &[u8;32],
   master_salt: &[u8;14], mode)` требует 14-байтную соль, а Session Description (op4) отдаёт
   только 32-байтный `secret_key` и поля соли не имеет ⇒ из протокола не выводится не только
   KDF, но и сигнатура API. При первом live-ключе код V-03 придётся менять, поэтому не
   наращивать на нём пайплайн.
5. **V-08 = DAVE/MLS, а не UI.** «V-08 UI» — историческая коллизия ID; в этом плане
   используется `V-08-DAVE` и `V-08-UI`. DAVE поверх SRTP виден в capture (MLS key package,
   `dave_protocol_version:1`), но требуется ли он для слышимости в MVP — **неизвестно** и
   решается только live-окном/RE.
6. **V-01 offline шире, чем «контракт».** Gateway-часть (парсер/reducer/селекторы) уже есть;
   офлайн-остаток — RTC state machine, таймауты, коды close, reconnect-политика WS.

### Офлайн сейчас / blocked-by-live

| Работа | Офлайн-объём | Явный live-блокер |
|---|---|---|
| V-01 | state machine, разбор close/ошибок, санитизированные фикстуры op8→0→2→4 (оба вида op4: `mode`+`secret_key` и `sdp`), таймауты | реальная последовательность Gateway voice-state → RTC WS, актуальные endpoint/token |
| V-02 | побайтовые тесты (уже есть), loopback, fake transport, детерминированная ротация | ответ сервера на discovery, NAT-ротация, поведение при смене IP |
| V-03 | crypto KAT (есть), tamper/replay, проверка рамок формата | **схема KDF/nonce/AAD (B-08)** + secret_key из op4 + packet oracle; ключ в capture вырезан |
| V-04 | всё: ABI smoke, encode/decode KAT, DTX/CN, PLC, лимиты | выбор bitrate/application/DTX и фактическая совместимость с потоком Discord |
| V-05a | ALSA `null`/`plug:null`, enumeration через trait, mute/PTT gate, ring 8 кадров | права на `/dev/snd`, реальный микрофон, hot-plug |
| V-05b | сборка цепочки на fake-пакетах, xrun-политика | рабочий decrypt (V-03) и реальный playback |
| V-06 | fake-store состояния (есть), a11y, визуальные контракты | реальные события участников/speaking/mute сервера |
| V-07 | fake-clock backoff, парсер RTCP (есть), ranking на фикстуре | семантика ping/потерь сервера, реальный disconnect/recovery |
| V-08-DAVE | ничего нельзя: нет MLS-слоя и нет спецификации DAVE в репозитории | обязательность DAVE, MLS-обмен, E2EE-ключи |

M3 (`MASTER-PLAN.md:126`) = V-01…V-06 + два live-клиента. Из офлайн-тестов M3 не
закрывается; V-08-DAVE в M3 формально не входит, но его необходимость для слышимости —
открытый вопрос (см. §9.2).

## 5. Libopus KAT (офлайн, без live-аудио)

### 5.1 Формат и фикстуры

- **Формат входа:** PCM `S16_LE`, mono, 48 000 Гц. Обоснование: PT120 и 48 кГц — `[L]`;
  mono — решение `[A]` до live (`G5-VOICE-PLAN` §4.2). Stereo/битрейт/DTX/FEC/PLC —
  follow-up решения (D-113 отсутствует).
- **Кадр:** 960 сэмплов = 20 мс, ровно 50 кадров = 48 000 сэмплов = 1.000 с.
- **Фикстура A (тон):** 1 кГц, амплитуда 0.5 FS, фаза непрерывная, fade-in/out 10 мс.
- **Фикстура B (речеподобная):** детерминированная сумма 5 гармоник с формантными
  огибающими 250/450/1200/2400/3200 Гц, амплитуды 0.30/0.20/0.12/0.06/0.03, AM 3 Гц,
  peak ≤ 0.5 FS, DC = 0. Без RNG: генератор полностью детерминирован.
- **Фикстура C (тишина):** нули, 50 кадров — только для DTX-теста.
- Файлы: `fixtures/voice/opus/{tone-1k-48k-mono-s16.wav, speechlike-48k-mono-s16.wav,
  silence-48k-mono-s16.wav}` + `manifest.json` (sample rate, channels, frames, samples,
  generator, SHA-256, MD5, provenance/license). Генератор — `waves/RW-40/VOICE/gen-opus-fixtures.sh`
  (позже, отдельной карточкой; в этой — только план).
- Дом: фикстуры живут по общей конвенции репозитория — запись в `fixtures/MANIFEST.json`
  (путь, источник/генератор, байты, md5, назначение, `usedBy`) + строка в `fixtures/README.md`.
  Оба файла общие: правит Team Lead, G5 приносит готовые значения. `fixtures/tools/make-fixtures.mjs`
  копирует из лаборатории и для генерируемых файлов не годится — генератор коммитится отдельно.
- Лицензия: PCM синтезируется в репозитории (не запись человека, не из capture) → лицензия
  репозитория; в манифесте так и пишется, без «provenance: unknown».
- Ничего не выводить из текущего capture: talkspurt — comfort-noise/DTX, речи там нет.

### 5.2 Encode-путь (libopus → независимый декодер)

1. `opus_encoder_create(48000, 1, OPUS_APPLICATION_VOIP, &err)`; `err == OPUS_OK`.
2. `OPUS_SET_COMPLEXITY(10)`, `OPUS_SET_DTX(0)` для A/B; bitrate не задаём (default) до live.
3. 50 вызовов `opus_encode(..., frame_size=960, buf, max_len=1275)`.
4. Инварианты: ровно 50 пакетов, каждый `1..=1275` Б, ни одного отрицательного кода
   (иначе `opus_strerror`). Пакеты пишутся как `*.packets` (длины — в манифесте). Для ffmpeg
   этого **недостаточно**: CLI декодирует Opus только через контейнер (Ogg/Matroska) и
   конкатенированные length-prefixed пакеты не примет. Поэтому харнесс обязан собрать из тех же
   пакетов минимальный **Ogg Opus** (RFC 7845): `pre-skip` = `OPUS_GET_LOOKAHEAD`, гранула +960
   на кадр. Иначе шаг 6 неисполним.
5. Структурные проверки **без декодирования** (дешёвые и не зависят от порогов качества):
   для каждого пакета `opus_packet_get_nb_frames(pkt,len) == 1` и
   `opus_packet_get_nb_samples(pkt,len,48000) == 960`. Это доказывает 20 мс/48 кГц на уровне
   битстрима, даже если SNR-гейт потом калибруется. Проверки идут через функции самой libopus —
   это **self-check** битстрима (TOC/кадры), а не независимый оракул; независимость даёт только
   ffmpeg-путь.
6. Декодирование оракулом: `ffmpeg -c:a opus` (нативно, **не** `libopus`) в `s16le`; вход — Ogg
   из шага 4, а не набор пакетов.
7. Метрики: после снятия `pre-skip` ожидается ровно `50×960 − pre_skip` сэмплов (при
   `pre_skip = lookahead` это ≈47 688, а не 48 000); если `pre-skip = 0`, то ровно 48 000.
   Число пакетов — 50, декодер отдаёт по 960 сэмплов на пакет. Плюс нет NaN/Inf (float-путь),
   lag кросс-корреляции, SNR, RMS, max|err|. Ожидаемая длина пишется в манифест, чтобы «48 000»
   не превратилось в ложный гейт.

### 5.2.1 Негативные/устойчивость (обязательная часть V-04 DoD)

| Вход | Ожидание |
|---|---|
| `opus_encoder_create(12345, 1, …)` / `channels = 0` | `NULL` + `OPUS_BAD_ARG` в `err` (документировано: Fs ∈ {8000,12000,16000,24000,48000}, channels ∈ {1,2}), без паники |
| `opus_encode` с `frame_size` вне {120,240,480,960,1920,2880} (напр. 961 или 100) | `OPUS_BAD_ARG`, энкодер остаётся рабочим. **480/1920 валидны** (10/40 мс) и должны кодироваться — тест не имеет права запрещать законные размеры |
| `opus_encode` с `max_len` меньше необходимого | отрицательный код (ожидается `OPUS_BUFFER_TOO_SMALL`; точный код фиксируется первым прогоном), **нет** записи за границу буфера |
| обрезанный/испорченный пакет в `opus_decode` | отрицательный код (ожидается `OPUS_INVALID_PACKET`), не паника и не чтение за границей |
| `opus_decode(NULL, 0, out, 960, 0)` | 960 сэмплов PLC (скрытие потери), не ошибка; `960` кратно 2.5 мс — требование PLC соблюдено |
| двойное освобождение | невозможно by design: владелец один, `Drop`, `!Clone` |

### 5.3 Decode-путь (независимый энкодер → libopus)

1. `ffmpeg -c:a opus -strict -2 -ar 48000 -ac 1 -sample_fmt fltp -opus_delay 20` кодирует
   фикстуры A/B (нативный энкодер `exp`; PCM конвертируется ffmpeg'ом в fltp сам). `-opus_delay`
   обязателен: по умолчанию 360 мс (видно в `-h encoder=opus`), без явного пина задержка и
   выравнивание фикстуры дрейфуют между версиями ffmpeg; значение и версия пишутся в манифест,
   а 50 пакетов / 48 000 сэмплов проверяются на выходе. **Граница:** у нативного
   энкодера нет опций bitrate/DTX/`application`/FEC (только `-opus_delay`, `-apply_phase_inv`),
   поэтому этот путь доказывает **декодер** (и что наш поток — валидный Opus), но ничего не
   говорит о настройках Discord. Совпадение с Discord-потоком проверяется только live.
2. Пакеты пинуются как `fixtures/voice/opus/native-{tone,speechlike}.packets` + манифест
   (версия ffmpeg, конфигурация, хэши) — чтобы CI не зависел от ffmpeg.
3. `opus_decoder_create(48000, 1, &err)`, 50× `opus_decode(..., 960, 0)`; PLC-тест:
   `opus_decode(NULL, 0, ...)` на «потерянном» кадре даёт 960 сэмплов, не падает.
4. Сравнение с исходным PCM фикстуры: длительность точная, SNR/RMS/max|err|, доминирующая
   частота ±2 %.

### 5.4 Выравнивание и допуски (обязательно до SNR)

- Opus вносит задержку кодека. Значение **не хардкодим**: `OPUS_GET_LOOKAHEAD` (ctl)
  возвращает задержку энкодера в сэмплах (для 48 кГц VOIP ожидается ~6.5 мс ≈ 312, но это
  проверяемое значение, а не константа). Сравнение «по индексу» для 1 кГц даёт противофазу
  и ложный провал. Поэтому:
  1. базовая схема — Ogg с `pre-skip = OPUS_GET_LOOKAHEAD` (§5.2 п.4): ffmpeg сам срезает
     задержку, ожидаемый lag = 0 (±1 сэмпл). Вариант `pre-skip = 0` допустим, только если Ogg не
     триммит: тогда ожидаемый lag равен lookahead. Выбор фиксируется в манифесте, смешивать эти
     два режима нельзя;
  2. lag измеряется кросс-корреляцией в окне ±960 сэмплов (±20 мс) и должен совпасть с выбранным
     ожиданием (±1 сэмпл) **и** быть одинаков на 3 прогонах; иначе — провал KAT
     (нестабильность), а не «подгонка порога»;
  3. SNR считается на перекрытии после сдвига, без первых/последних 20 мс (краевые эффекты).
- Кандидаты порогов (**требуют D-113/PM**, не принятый контракт):

| Путь | Метрика | Кандидат |
|---|---|---|
| encode→native decode, тон | SNR | ≥ 20 дБ |
| encode→native decode, speechlike | SNR | ≥ 15 дБ |
| любой | max abs err | ≤ 0.25 FS (8192/32768) |
| native encode→libopus decode | SNR | ≥ 10 дБ (нативный энкодер слабый) |
| native encode→libopus decode | доминирующая частота | ±2 % |
| все | кадры/длительность | 50 пакетов; длина = `50×960 − pre_skip` (см. §5.2 п.7) |

- Байтовое равенство **не** проверяется: Opus-энкодеры не обязаны быть побитово
  идентичными (и libopus не гарантирует это между версиями). Детерминизм проверяется
  иначе: 3 прогона в одном процессе/на одном хосте дают одинаковый SHA-256 пакетов.
  Кросс-машинный побайтовый детерминизм **не заявляется**: libopus выбирает SIMD-путь по
  CPU (RTCD), поэтому хэш пакетов пинуется вместе с версией и хостом/CPU, а не как
  универсальная константа.

### 5.5 DTX/CN — отдельный тест

- **Два разных «DTX», их нельзя смешивать.** (а) *DTX энкодера* libopus: при `OPUS_SET_DTX(1)`
  на тишине энкодер отдаёт короткие пакеты, но **продолжает** их отдавать (по одному на кадр);
  (б) *подавление передачи*: в capture после talkspurt (~10 пакетов ≈ 200 мс) пакеты вообще не
  шлются (`voice-deep.md` §2.3). Наш RTP-слой уже моделирует (б) через
  `RtpSender::silence(elapsed_frames)`: `sequence` не растёт, `timestamp` прыгает на
  `960 × elapsed`, взводится `marker`.
- Контракт KAT для (а): на фикстуре C (тишина) пакеты короткие (тип кадра и точный размер
  фиксируются по факту, не подгоняются), число кадров = числу кадров входа, декодер отдаёт по
  960 сэмплов на каждый пакет. Контракт (б) — отдельный юнит-тест `RtpSender`, не тест libopus.
- Классификация — по **открытому** Opus-пакету (TOC/размер), а не по
  `classify_opus_payload`. Тот порог 1..=3 Б снят с *зашифрованных* длин; AEAD длину
  сохраняет, но без ключа тип кадра не доказан, а короткий речевой кадр порог не покрывает.
  `classify_opus_payload` остаётся эвристикой для capture и **не** является Opus-декодером.
- Проверять только до границы шифрования; после SRTP длина меняется на tag+trailer.

### 5.6 Сборка, скипы, evidence

- FFI живёт в `discord-platform/src/ffi_opus.rs` (`#[allow(unsafe_code)]`, `// SAFETY:` на
  каждый блок, строка в `UNSAFE-REGISTRY`). `discord-media`/`discord-voice` остаются
  `forbid(unsafe_code)`.
- Линковка `#[link(name="opus")]` ломает сборку на хосте без libopus. Поэтому — **opt-in
  feature** (например `system-opus`, off by default) и `cargo test -p discord-platform
  --features system-opus`; workspace без фичи собирается и не падает.
- Skip-aware: нет фичи/библиотеки/фикстуры → `SKIP: <причина>` (не «зелёный» и не красный).
- Evidence: манифест фикстур + хэши, генератор, `opus_get_version_string()`, версия и
  конфигурация ffmpeg, транскрипт команд, `KAT-OPUS.json` (50 пакетов, длина выхода
  `50×960 − pre_skip`, lag/lookahead, SNR/RMS/max err по 3 прогонам), targeted-лог теста.
  Без PCM/ключей/токенов в логах.
- Независимость оракула фиксируется в тесте: при `ffmpeg` с `--enable-libopus` обязателен
  явный `-c:a opus`; тест проверяет, что декодер — нативный (версия/баннер), иначе SKIP.
- Более сильный будущий оракул — официальные Opus test vectors (RFC 8251). Он требует
  согласованной загрузки, поэтому в текущем офлайн-плане **не заявляется**.
- **Гейт.** Тесты под feature-флагом `npm run gate:worker` не запускает. Значит без отдельной
  фазы KAT существует, но не enforced. Варианты (решение PM/Team Lead): добавить фазу
  `cargo test -p discord-platform --features system-opus --test opus_kat` в
  `scripts/gate.mjs --scope worker`; либо прогонять её вручную и прикладывать лог к отчёту
  карточки. Второй вариант допустим только как временный, с записью в `DECISIONS.md`.
- **ID тестов.** В `docs/rust-client/TEST-PLAN.md` §T-VOICE сейчас **01…23**, и Opus-KAT среди
  них нет (`T-VOICE-11` — джиттер, `T-VOICE-17` — шумоподавление; `T-VOICE-20` — устойчивость,
  `21` — MLS-фрейминг, `22` — openmls-ротация, `23` — live-интероп E2EE). Занимать 20…23 нельзя:
  предлагаются **`T-VOICE-24`** (ABI/структурный), **`25`** (encode→native decode), **`26`**
  (native encode→libopus decode), **`27`** (DTX/CN), **`28`** (негативные коды). Правка
  TEST-PLAN — Team Lead.
- **DoD V-04** (что считается сданным): T-VOICE-24…28 зелёные на фиче `system-opus`;
  `KAT-OPUS.json` с хэшами фикстур, версией libopus, lag/lookahead, SNR/RMS/max|err| по
  3 прогонам; фикстуры в `fixtures/voice/opus/` + `fixtures/MANIFEST.json`; ревью G4
  (независимость оракула) и G4b (нет PCM/ключей в логах); SAFETY-строки в UNSAFE-REGISTRY.

## 6. Распределение ролей

| Owner | Объём | Зависимость/риск |
|---|---|---|
| **G1** | V-01 RTC state machine/manager, V-02 транспортная оркестрация, V-03 потребление ключа по схеме, V-07 reconnect/stats/regions, V-08-DAVE c G4 | live-значения протокола; не выдавать существующие парсеры за manager; reconnect сейчас только gateway |
| **G5** | V-00 контракт, V-04 libopus FFI/KAT, V-05a ALSA null → устройства/PTT/mute, portal/PipeWire шаг | FFI только в platform + feature-флаг + UNSAFE-REGISTRY (файла нет); фикстуры/формат первыми |
| **G2** | V-06 UI: проводка к сервису, fake-state покрытие | fake-панель ≠ voice-сервис; без V-01/V-05 реальных событий нет |
| **G3** | VOICE-спека, состояния/участники/speaking/ping, a11y-контракты | speaking/ping/device-состояния без live-наблюдений |
| **G4** | независимая приёмка KAT, mutation, аудит evidence (нет ли оракула-кода) | требует доступа к фикстурам/логам, но не к live |
| **G4b** | SRTP-схема (B-08), ключевой материал, privacy, DAVE-ревью | live-окно для decrypt; ключи/аудио не в логах |

## 7. Риски

1. **Изобретённая SRTP-схема** (KDF-метки, nonce, AAD) — самая дорогая ошибка: выглядит
   «готово», но с реальным ключом не расшифрует ни один пакет. Митигация: V-03 не считать
   сделанным до packet-oracle из live-окна; B-08 вести как отдельный блокер.
2. **DAVE/MLS вне плана.** Если сервер требует E2EE для слышимости, MVP без V-08-DAVE
   не заработает, а это XL. Митигация: live-проверка «слышно ли без DAVE» в первом же окне плюс
   дешёвый индикатор: `opus_packet_get_nb_frames` по *расшифрованному* payload — успех на всех
   кадрах сильно говорит об отсутствии DAVE-обёртки, провал — о её наличии (окончательно решает
   MLS-разбор; индикатор не доказательство).
3. **Коллизия ID V-08** (DAVE vs UI) уже породила пропуск DAVE в черновике. Митигация:
   в TASKS/плане писать `V-08-DAVE` / `V-08-UI` (правка общих доков — Team Lead).
4. **Оракул KAT слабее, чем кажется.** Нативный энкодер ffmpeg помечен `exp`; порог SNR
   может быть недостижим. Митигация: раздельные пороги по путям, фиксация фактических
   значений, при провале — не ослаблять тест, а менять оракул/порог решением PM.
5. **Задержка кодека без выравнивания** даёт ложный провал (противофаза на 1 кГц).
   Митигация: lag-поиск и допуск на стабильность lag.
6. **Сборка без libopus.** Прямая линковка ломает CI. Митигация: opt-in feature.
7. **`unsafe`-дисциплина.** `forbid` в voice/media соблазняет «временно ослабить».
   Митигация: PLAT-12/15, только `#[allow]`-модуль в platform, SAFETY-комментарии,
   UNSAFE-REGISTRY (создать до первой FFI).
8. **ALSA null ≠ устройства.** Успех на `null` не доказывает ни прав, ни PipeWire-consent,
   ни hot-plug. Митигация: отдельный live-DoD для V-05a.
9. **PTT через portal заблокирован** отсутствием `u64` в D-Bus-клиенте (`G5-VOICE-PLAN`
   §1.4.3). Митигация: P-1 (`Value::U64`) отдельной карточкой до PTT.
10. **Privacy.** PCM, ключи, `secret_key`, токены не должны попадать в логи/фикстуры/отчёты;
    capture-путь требует явного согласия (portal).
11. **Оверинтерпретация capture:** 43-байтные пакеты без валидного расширения — аномалия,
    не «второй профиль»; писать только как `unknown`, не закреплять в контракте.
12. **Документационный дрейф:** `lib.rs` утверждает «сетевых операций нет», хотя `udp.rs`
    их выполняет; `reconnect.rs` — gateway, а не RTC. Читать код, а не шапки.
13. **KAT «есть, но не enforced».** Под feature-флагом он не попадает в `gate:worker`;
    зелёный отчёт без прогона KAT создаёт ложное впечатление проверенного кодека.
    Митигация: фаза в гейте (или ручной прогон + лог в отчёте карточки, с записью решения).
14. **Вариадический `*_ctl`.** Неверный тип аргумента (`opus_int32` vs `c_int`, передача
    указателя вместо значения) — UB без ошибки компиляции. Митигация: один типизированный
    хелпер на каждый ctl-запрос, meta-grep «`opus_encoder_ctl` напрямую вне хелперов = 0»,
    ревью G4b.
15. **Кросс-машинная нестабильность хэшей** (RTCD/SIMD, версии libopus) — ложный красный
    на другом CPU. Митигация: хэш пакетов пинуется с версией и хостом; SNR-гейт — основной,
    байтовый — только как «детерминизм в рамках хоста».
16. **RTP-таймстемпы в capture не равномерны.** В запиненном oracle все 10 дельт = 960, но
    `voice-deep.md` §2.3 фиксирует окно 960/1050/870 (среднее 960 на 4 кадра). Live-конформанс
    должен проверять номинальные/средние 960 тиков, а не поштучное `+960`, иначе тест покраснеет
    на реальном потоке по неизвестной причине.

## 8. Blocked-by-live (окончательный список)

- V-01: реальная последовательность Gateway op4 → `VOICE_SERVER_UPDATE` → RTC WS op8/0/2/4,
  close-коды, таймауты, актуальные endpoint.
- V-02: ответ discovery, NAT/порт, ротация сокета на реальном сервере.
- V-03: **схема KDF/nonce/AAD (B-08)** + `secret_key` из op4 + packet-oracle; без этого
  wire-совместимость не проверяема.
- V-04: финальные codec-настройки (bitrate/application/DTX) и совместимость с потоком
  Discord; офлайн-часть закрывается независимо.
- V-05a/b: права на `/dev/snd`, PipeWire-consent, реальный capture/playback, hot-plug.
- V-06: реальные participant/speaking/mute-события сервера.
- V-07: семантика ping/потерь, реальный disconnect/recovery.
- V-08-DAVE: обязательность DAVE для слышимости, MLS-обмен, E2EE-ключи.
- M3 (V-01…V-06 + два live-клиента) не может быть объявлен по офлайн-тестам.

## 9. Вопросы PM (до кода)

1. **D-113:** решения нет — нужен явный документ или отказ от ссылки. Без него фиксируются
   только `[A]`-дефолты (mono, `OPUS_APPLICATION_VOIP`, bitrate default, DTX on).
2. **DAVE в MVP:** входит ли V-08-DAVE в критерий «слышно», или допускается live-проверка
   «без DAVE» в первом окне?
3. **Переименование V-08:** `V-08-DAVE` / `V-08-UI` в TASKS/MASTER-PLAN (правит Team Lead).
4. **Feature-флаг `system-opus`** и создание `docs/rust-client/UNSAFE-REGISTRY.md` — это
   общие файлы; нужно разрешение/владелец.
5. **Порог KAT** (SNR 20/15/10 дБ) — утвердить как контракт или оставить калибруемым до
   первого прогона.
6. **Гейт для KAT:** добавлять фазу `system-opus` в `scripts/gate.mjs` (тогда он станет
   обязательным для всех хостов с libopus) или оставить ручной прогон с логом в отчёте?
7. **Общие файлы, нужен владелец:** `fixtures/MANIFEST.json` (+ строка в `fixtures/README.md`),
   `docs/rust-client/TEST-PLAN.md` (ID `T-VOICE-24…28`), `docs/rust-client/UNSAFE-REGISTRY.md`
   (содержимое — G5 по PLATFORM-AUDIT §2.3, правка — Team Lead), `CONTRACTS/voice.md`. G5 готовит
   содержимое, но правит не он.
8. **CPU-зависимость хэшей** — согласен ли PM, что байтовый детерминизм ограничен хостом
   (тогда в evidence фиксируется CPU), или требуется воспроизводимость на любом CI-хосте
   (тогда нужен другой оракул, дороже)?

## 10. Что этот документ не утверждает

KAT не запускался; libopus не линковался; аудиоустройства не открывались; live/сеть не
использовались. Ни одна строка кода, тестов, фикстур и системных пакетов не изменена.
Изменён только этот файл.

Отдельно: здесь **нет** утверждений о (а) wire-совместимости SRTP (нужны ключ + схема B-08),
(б) кросс-машинной побитовой воспроизводимости Opus-потока, (в) работоспособности ALSA/PipeWire
на реальных устройствах (наличие `libasound`/`pipewire` и узлов `/dev/snd` — не доказательство
захвата), (г) необходимости DAVE для слышимости, (д) выполнении M3.
