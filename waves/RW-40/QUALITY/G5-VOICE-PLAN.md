# G5-VOICE-PLAN — устройства и захват voice/video (read-only план)

Дата: 2026-10-24. Автор: субагент G5 Platform. Карточка `G5-VOICE-PLAN`, PM — DeepSeek.
HEAD: `0239543` (дерево грязное: чужие правки RW-35/36 и `rust/gui-spike/**` не трогались).
Режим: **только чтение** — код/тесты/фикстуры/доки не менялись; `cargo`, live, сеть, окно,
установки и `git commit` не запускались.

Основание: `QUALITY/PLATFORM-AUDIT.md` §4–§4.2, `DESIGN/VOICE.md` §3/§5/§8.6/§9,
`DESIGN/STREAM.md` §0/§5/§8.3/§8.5, `DESIGN/voice-spec.json`, `DESIGN/stream-spec.json`,
`docs/re/voice-deep.md`, `NETWORK-CONTRACTS.md` §2, `STUB-MAP.md` STUB-26/27, `TEST-POLICY.md`,
`rust/crates/{discord-voice,discord-media,discord-platform}`.

## 0. Легенда

| Метка | Значение | Источник |
|---|---|---|
| `[H]` | проверено **на этом хосте** read-only командой (команда — §1, §12) | эта карточка |
| `[L]` | подтверждено живым захватом | `docs/re/voice-deep.md`, `NETWORK-CONTRACTS` §2 |
| `[S]` | подтверждено протоколом/кодом клиента | `NETWORK-CONTRACTS`, `STREAM.md` |
| `[D]` | факт дампа (DOM/JSON/имена native API) | `DESIGN/VOICE.md`, `voice-spec.json` |
| `[A]` | наше решение, **не подтверждено**; требует PM до реализации | этот документ |
| `[T]` | токен проекта | `waves/RW-39/DESIGN/tokens.css` |

Правило: спецификация портала ниже цитируется по установленному
`/usr/share/dbus-1/interfaces/org.freedesktop.portal.{ScreenCast,Camera,GlobalShortcuts,Request}.xml` —
это `[H]`, а не наша реконструкция. Всё, чего нет в этих файлах, помечено `[A]`.

## 1. Что есть в системе (факты)

### 1.1 Библиотеки и заголовки

| Компонент | Версия | Лицензия | Заголовки | Роль в плане | Метка |
|---|---|---|---|---|---|
| `libopus` (`opus`) | 1.6.1-1.1 | BSD-3-Clause | `/usr/include/opus` | Opus encode/decode | `[H]` |
| `libasound` (`alsa-lib`) | 1.2.16.1-1.1 | LGPL-2.1-or-later | `/usr/include/alsa` | захват/воспроизведение PCM | `[H]` |
| `libpipewire-0.3` (`libpipewire`) | 1:1.6.10-1.1 | MIT + LGPL-2.1-or-later | `/usr/include/pipewire-0.3` | клиент PipeWire для кадра портала | `[H]` |
| `libspa-0.2` | 0.2 (пакет `libpipewire`) | MIT + LGPL-2.1-or-later | `/usr/include/spa-0.2` | форматы/буферы PipeWire | `[H]` |
| `v4l-utils` (runtime `libv4l2`) | 1.32.0-2.1 | LGPL | `/usr/include/libv4l2` **отсутствует** | только если выберем libv4l2-обёртку | `[H]` |
| kernel headers V4L2 | — | — | `/usr/include/linux/videodev2.h` есть | прямые ioctl камеры (не MVP) | `[H]` |
| `libudev` (`systemd-libs`) | 262.1 (`libudev.so.1.7.15`) | LGPL-2.1-or-later | `/usr/include/libudev.h` есть | hot-plug через udev — **отложено** | `[H]` |

`ldconfig -p` видит все четыре `.so`; `pkg-config --modversion` → `opus 1.6.1`,
`alsa 1.2.16.1`, `libpipewire-0.3 1.6.10`, `libspa-0.2 0.2` `[H]`.

### 1.2 Устройства, сессия, права

| Факт | Значение | Метка |
|---|---|---|
| Звуковые устройства | `/dev/snd` есть, 5 card, capture-PCM (`pcmC2D0c` и др.) | `[H]` |
| Доступ к PCM | `getfacl /dev/snd/pcmC2D0c` → `user:sanya:rw-` (ACL), при этом `id -nG` = `sanya ollama wheel` (**без** `audio`) | `[H]` |
| Камера | `/dev/video*` нет, `/sys/class/video4linux` нет, portal `Camera.IsCameraPresent=false` | `[H]` |
| Сессия | `XDG_SESSION_TYPE=wayland`, `WAYLAND_DISPLAY=wayland-1`, `DISPLAY=:0`, `XDG_CURRENT_DESKTOP=KDE` | `[H]` |
| PipeWire | демоны `pipewire` (pid 1258), `pipewire-pulse` (1446), `wireplumber` (1260); сокет `/run/user/1000/pipewire-0` | `[H]` |
| Ввод | `/dev/input/event*` только `root:input` (пользователь не в `input`); `/dev/uinput` доступен `sanya` — **не используем** (инъекция ввода, лишняя поверхность) | `[H]` |
| Инструменты | `dbus-daemon`, `python3` + `dbus`/`gi`, `pw-cli`, `wpctl`, `pactl`, `gst-launch-1.0`, `ffmpeg`/`ffprobe` | `[H]` |

### 1.3 Portal (D-Bus)

`org.freedesktop.portal.Desktop` активен (pid 1535), бэкенды `desktop.kde`, `desktop.gtk`,
`desktop.hyprland` (activatable), плюс `PermissionStore` `[H]`.

| Интерфейс | version | Свойства (значение) | Метка |
|---|---:|---|---|
| `org.freedesktop.portal.ScreenCast` | **6** | `AvailableSourceTypes=7` (MONITOR=1, WINDOW=2, VIRTUAL=4), `AvailableCursorModes=7` (Hidden=1, Embedded=2, Metadata=4) | `[H]` |
| `org.freedesktop.portal.Camera` | **1** | `IsCameraPresent=false` | `[H]` |
| `org.freedesktop.portal.GlobalShortcuts` | **2** | — | `[H]` |
| `org.freedesktop.portal.Session` / `Request` | — | `Close`, сигнал `Response(u, a{sv})` | `[H]` |

### 1.4 Чего нет / что мешает

1. **Камеры нет физически** (`/dev/video*`, sysfs, portal) ⇒ камера не может быть принята ни в
   одном DoD без нового железа `[H]`.
2. **`/usr/include/libv4l2` отсутствует** ⇒ libv4l2-обёртка недоступна без установки; прямые
   V4L2 ioctl возможны по kernel headers `[H]`.
3. **D-Bus-клиент не умеет `t` (u64)** — `Value` содержит только `Byte/Bool/U32/I32/String/
   UnixFd/Array/Struct/Dict/Variant`; сигнал `GlobalShortcuts.Activated` содержит `timestamp t`
   ⇒ без доработки PTT-сигнал не разбирается `[H]` (проверено чтением `dbus/{mod,message}.rs`).
4. `Connection::call_typed_with_fds` — приватный, а публичный `call_with_fds` не умеет пустой
   `a{sv}` («empty container has no element type») ⇒ `OpenPipeWireRemote(session, {})` и
   `Camera.OpenPipeWireRemote({})` требуют типизированного вызова `[H]`.
5. Пользователь не в `audio`, `video`, `input`; доступ к аудио даёт ACL на узлах `/dev/snd`
   (`user:sanya:rw-`), к камере/evdev такого доступа нет `[H]`.

## 2. Порядок работ

### 2.1 Почему именно такой

Порядок выбран так, чтобы **каждый шаг имел независимый офлайн-оракул** и не требовал live:

1. **Portal (D-Bus)** — чистая проводка, уже есть mock-шина (`power_contract`/`dbus_contract`),
   fd-передача (RW-35) и `a{sv}`. Снимает главный неизвестный риск (согласие/сессия/токены)
   без единого кадра.
2. **libopus** — единственный кодек, подтверждённый `[L]` (PT 120, 48 кГц, 20 мс); офлайн-оракул —
   ffmpeg с **нативной** реализацией Opus (не libopus).
3. **ALSA** — минимальная FFI-поверхность без callback'ов; `null`-PCM позволяет проверить
   capture/playback без железа.
4. **PipeWire** — только после этого: клиент PipeWire требует Rust-callback'ов в C, это отдельная
   дисциплина (`catch_unwind`), её нельзя вводить «попутно» с Opus.
5. **Live-окно** — согласие портала, реальный микрофон/экран; камера заблокирована железом.

Обратный порядок (начать с устройств) даёт код, который нельзя проверить: ни консента, ни
оракула кадров, ни прав.

### 2.2 Шаги

| Шаг | Содержание | `unsafe` | Офлайн-оракул | Live | Статус |
|---|---|---|---|---|---|
| **0** | Заморозка контракта: Rust-типы состояний, ошибки, границы потоков; фиксация `[A]`-решений §10 | нет | code review + meta | нет | prerequisite |
| **1** | Portal: `ScreenCast` (сессия/источники/старт/fd), `Camera` (property + fd), `GlobalShortcuts` (PTT); mock-портал | **нет** (только `dbus`) | приватная шина + python-мок, журнал TSV | нет | offline DoD |
| **2** | `libopus` FFI + KAT + DTX/CN | да, 1 модуль | ffmpeg native `opus` enc/dec, фикстуры + md5 | нет | offline DoD |
| **3** | ALSA capture/playback + enumeration + hot-plug poll + mute/deafen gate | да, 1 модуль | `null`/`plug:null`, fake enumerator | частично (реальный микрофон — опционально) | offline DoD |
| **4** | PipeWire-клиент для кадра портала + `catch_unwind`-дисциплина | да, 1 модуль | `pw_check_library_version`, socketpair error-path, fake backend | только consent+кадр | live DoD |
| **5** | Live-окно: согласие, реальный микрофон, экран; камера/системный звук — **вне** | — | — | да | blocked (см. §11) |

Видео-энкод (AV1/H264), vaapi, Go Live, второй RTC — **не** в этом плане (N3, `STREAM.md` §8.3).

### 2.3 Куда кладётся код (без изменения линтов)

| Место | Что | Линт |
|---|---|---|
| `discord-platform/src/portal/{mod,screen_cast,camera,shortcuts}.rs` | marshalling/состояния портала, без FFI | `deny(unsafe_code)` сохраняется |
| `discord-platform/src/ffi_opus.rs` | `#[link(name="opus")]`, KAT-обёртка | `#[allow(unsafe_code)]` на модуле |
| `discord-platform/src/ffi_alsa.rs` | PCM in/out, enumeration | то же |
| `discord-platform/src/ffi_pipewire.rs` | `pw_*` + callback | то же |
| `discord-media/src/{codec.rs,devices.rs,capture.rs}` | чистые типы/политика (формат, буфер, DTX-решение), без FFI | `forbid(unsafe_code)` сохраняется |
| `discord-voice/tests/voice_pipeline.rs` | интеграция fake-PCM → RTP → SRTP | `forbid` сохраняется |

`discord-media` и `discord-voice` объявлены `unsafe_code = "forbid"` — **нельзя** ослаблять
(PLAT-12/PLAT-15). FFI живёт только в `discord-platform`, как keyring/hunspell; каждый блок —
`// SAFETY:` + строка в `UNSAFE-REGISTRY` (PLATFORM-AUDIT §2.3). Направление зависимости:
`discord-platform → discord-media` (типы устройств/политика), обратной связи нет `[A]`.

### 2.4 Пайплайн

```
ALSA capture 960 frames / 20 ms
  → bounded ring (8 frames)
  → VAD + mute/PTT gate
  → opus_encode (PT 120, 48 kHz, 20 ms)
  → RtpSender::next_packet  (sequence, ts +960, marker talkspurt)
  → SrtpContext::encrypt_rtp (aead_aes256_gcm_rtpsize)
  → UdpMediaSocket

UdpMediaSocket → decrypt_rtp → JitterBuffer → opus_decode → ALSA playback
```

Существующие части: `media/rtp.rs` (`RtpSender`, `classify_opus_payload`), `srtp.rs`,
`media/jitter.rs`, `reception.rs`, `udp.rs` — их не дублируем, только соединяем.

## 3. Portal: точный контракт

### 3.1 ScreenCast (version 6, `[H]`)

| Метод | Вход | Выход |
|---|---|---|
| `CreateSession` | `a{sv} options` (`handle_token s`, `session_handle_token s`) | `o handle` (Request) |
| `SelectSources` | `o session_handle`, `a{sv}` (`types u`, `multiple b`, `cursor_mode u`, `restore_token s`, `persist_mode u`) | `o handle` |
| `Start` | `o session_handle`, `s parent_window`, `a{sv}` (`handle_token s`) | `o handle` |
| `OpenPipeWireRemote` | `o session_handle`, `a{sv}` | `h fd` |

`Request.Response(u response, a{sv} results)`; значения `response`: **0 успех, 1 отмена
пользователем, 2 иное завершение** `[H]`. Результаты `Start`: `streams a(ua{sv})`
(узел + свойства), `restore_token s`. Свойства стрима: `id s`, `position (ii)`, `size (ii)`,
`source_type u`, `mapping_id s`; с version 6 для таргетинга предпочтителен `pipewire-serial`
(+ `PW_KEY_TARGET_OBJECT`), node id устарел `[H]`.

`persist_mode`: 0 не сохранять, 1 до конца процесса, 2 до отзыва `[H]`. `cursor_mode`
обязан быть из `AvailableCursorModes`, иначе сессия закрывается `[H]`.

### 3.2 Camera (version 1, `[H]`)

`AccessCamera(a{sv}) -> o Request`; `OpenPipeWireRemote(a{sv}) -> h fd` (**без** session handle);
свойства `IsCameraPresent b`, `version u`. `IsCameraPresent=false` на этом хосте `[H]`.

### 3.3 GlobalShortcuts (version 2, `[H]`)

`CreateSession(a{sv}) -> o`; `BindShortcuts(o session, a(sa{sv}) shortcuts, s parent_window,
a{sv} options) -> o`; `ListShortcuts(o, a{sv}) -> o`; `ConfigureShortcuts(o session,
s parent_window, a{sv})` (без возврата, открывает системный диалог).
Сигналы: `Activated(o session, s shortcut_id, t timestamp, a{sv})`,
`Deactivated(...)`, `ShortcutsChanged(o, a(sa{sv}))`. Ключи shortcut: `description s`,
`preferred_trigger s`; в результатах — `description`, `trigger_description` `[H]`.

### 3.4 Поток и инварианты

1. Подписаться на `Response` **до** вызова (спека прямо предупреждает о гонке токена) `[H]`.
2. `handle_token` — уникальный, непредсказуемый; проверить, что вернувшийся `handle` совпал с
   ожидаемым, иначе переподписаться `[H]`.
3. Порядок: `CreateSession → SelectSources → Start → Response → OpenPipeWireRemote` `[A]`.
4. `parent_window` = `""` (нет xdg-foreign handle в спайке) `[A]`, подтвердить в live.
5. `restore_token` — capability, не секрет аккаунта: файл в `XDG_STATE_HOME` c `0600`, не в
   логах; между профилями не шарить (PLATFORM-AUDIT §4.2) `[A]`.
6. `Session.Close` на любом выходе (drop/stop/ошибка); fd из `OpenPipeWireRemote` — владение
   вызывающего до передачи в `pw_context_connect_fd`; **после успешной передачи не закрывать**
   (PipeWire закрывает сокет сам, `core.h` §`pw_context_connect_fd`) `[H]`.
7. Ошибки `org.freedesktop.portal.Error.*` → типизированные `PortalError`; `response=1` →
   `ConsentDenied`, `2` → `ConsentAborted`, таймаут → `ConsentTimeout` `[A]`.

### 3.5 Что нужно доработать в D-Bus-клиенте (до кода портала)

| # | Доработка | Тест |
|---|---|---|
| P-1 | `Value::U64` + writer/reader + выравнивание 8 (`t`) | сигнал `Activated` с `timestamp` разбирается |
| P-2 | публичный typed-вызов с fd и явной сигнатурой (пустой `a{sv}`) | `OpenPipeWireRemote` с `{}` на моке |
| P-3 | ожидание `Response` с настраиваемым таймаутом и отменой через `Request.Close` | мок молчит → `ConsentTimeout`, `Close` вызван |
| P-4 | несколько одновременных request'ов (screen + PTT) не теряют сигналы | два мока, ответы в обратном порядке |

### 3.6 Чего не делаем

Никакого шелл-спавна (`gst-launch`, `pw-cat`, `xdotool`), чтения заголовков окон, своих превью
источников, `getUserMedia`/`getDisplayMedia`, скрытого перечисления устройств.

## 4. Opus (`libopus`, FFI)

### 4.1 Контракт FFI

`opus_encoder_create`, `opus_encode`, `opus_encoder_ctl`, `opus_decoder_create`, `opus_decode`,
`opus_decoder_ctl`, `opus_strerror`, `opus_get_version_string`; `#[link(name="opus")]` `[H]`.
Callback'ов нет ⇒ `catch_unwind` не нужен, PLAT-06 не затрагивается.

### 4.2 Параметры

| Параметр | Значение | Метка |
|---|---|---|
| Частота | 48 000 Гц, 20 мс = 960 сэмплов | `[L]` (`media/rtp.rs`) |
| PT | 120 | `[L]`/`[S]` |
| Каналы | **mono** (до подтверждения live) | `[A]` |
| Application | `OPUS_APPLICATION_VOIP` | `[A]` |
| DTX | включён (наблюдались CN/DTX 3 Б) | `[L]`+`[A]` |
| Bitrate | не задаём (default libopus) до измерения live | `[A]` |
| Максимум пакета | 1275 Б, буфер фиксированный | `[A]` |

Владелец состояния один, `Drop` → `opus_encoder_destroy`/`opus_decoder_destroy`; `!Clone`,
`!Send` не форсируем (энкодер живёт в одном потоке) `[A]`.

### 4.3 KAT (независимый оракул)

`ffmpeg n9.0.2` содержит **нативную** реализацию Opus (`-h encoder=opus`, `-h decoder=opus`) `[H]`,
то есть оракул не является libopus:

1. наш encoder(тишина/тон 1 кГц, 50×960) → пакеты → ffmpeg native decode → длина/каденс/SNR;
2. ffmpeg native encode → пакеты → наш decoder → PCM;
3. DTX: тишина → `classify_opus_payload(len)` = `ComfortNoise` (1..=3) или `Empty`;
4. фикстуры — маленькие `.opus`/`.wav` в `fixtures/voice/opus/` + md5 + генератор
   (`waves/<ID>/gen-opus-fixtures.sh`), чтобы тест не зависел от ffmpeg в CI; при отсутствии
   фикстур — `SKIP: нет фикстур` (не «зелёный»).

### 4.4 Границы

Ошибки: `opus_encoder_create == NULL` → typed `CodecInit`, отрицательный результат encode/decode
→ `opus_strerror` в текст (без данных PCM). Не логировать payload/ключи. Валидация: `frame_size
== 960`, `max_len <= 1275`, `channels == 1`, иначе `InvalidArg` до FFI.

## 5. ALSA: вход/выход

### 5.1 Почему ALSA, а не PipeWire

Минимальная поверхность, **нет callback'ов** (мы владеем циклом), детерминированные тесты на
`null`. PipeWire-клиент нужен только для кадра портала (шаг 4). Имена устройств ALSA могут быть
алиасами pipewire/pulse-плагина `[A]` (в дампе метка вида «Система: …» `[D]`) — наружу отдаём
только тип и индекс, `DESC` не логируем (PII, VOICE §5.1 п.6).

### 5.2 FFI

`snd_pcm_open`, `snd_pcm_set_params` (не variadic `[H]`), `snd_pcm_readi`, `snd_pcm_writei`,
`snd_pcm_recover`, `snd_pcm_prepare`, `snd_pcm_drop`, `snd_pcm_close`, `snd_pcm_wait`,
`snd_strerror`, `snd_asoundlib_version`, `snd_device_name_hint`, `snd_device_name_free_hint`
(`control.h:812` `[H]`).

### 5.3 Поток, формат, буферы

| Решение | Значение | Метка |
|---|---|---|
| Формат | `S16_LE`, mono, 48 кГц | `[A]` |
| Чтение | blocking `snd_pcm_readi` в отдельном потоке, ровно 960 frames | `[A]` |
| Буфер | bounded ring 8 кадров (160 мс); переполнение → drop + счётчик | `[A]` |
| Ошибки | `-EPIPE`/`-ESTRPIPE` → `snd_pcm_recover` + счётчик; `-ENODEV` → typed `DeviceUnavailable` | `[A]` |
| Паники | запрещены в аудиопотоке: ошибки только кодом/счётчиком | `[A]` |

### 5.4 Устройства и hot-plug

Перечисление — `snd_device_name_hint(-1, "pcm", …)`, фильтр `IOID` = Input/Output. Hot-plug:
**периодический re-enumeration** (1000 мс `[A]`) в сервисном потоке; событие только обновляет
список, **не** переключает устройство (VOICE §3.2 `[D]`/`[A]`). Если выбранное устройство
исчезло → `device-missing`, разговор не рвётся. udev/libudev — отложено (заголовки есть, но
лишняя FFI-поверхность); PipeWire registry — только вместе с шагом 4.

### 5.5 Тесты без железа

`null` PCM определён в `/usr/share/alsa/alsa.conf:335` `[H]`: open `"null"`/`"plug:null"`,
чтение тишины, запись, xrun-восстановление, неверное имя → `-ENODEV`/typed. Enumeration —
инжектируемый trait (`DeviceEnumerator`), fake-последовательность для hot-plug/missing.

## 6. PTT / hot-plug / mute / deafen

| Функция | API/путь | Состояние | Тест без live | Что нельзя без live | Метка |
|---|---|---|---|---|---|
| **PTT hold** | portal `GlobalShortcuts` v2: `CreateSession`→`BindShortcuts`→`Activated`/`Deactivated` | `idle/held/blocked`, по умолчанию **не назначено** | mock-портал эмитит сигналы; `keydown` UI — фейк | реальная клавиша, поведение бэкенда, «ограниченный режим» | `[D]`+`[A]` |
| **PTT-ключ** | пользователь назначает в системном диалоге; `preferred_trigger` не задаём | `null` | мок: пустой trigger | какой code/key выберет ОС | `[A]` |
| **Hot-plug** | poll ALSA 1000 мс | список обновился, выбор не менялся | fake enumerator | физическое подключение | `[D]`+`[A]` |
| **Mute** | локальный gate до encoder + gateway op4 `self_mute` | `muted`=0 кадров в SRTP | fake source: 0 вызовов encode | слышимость у собеседника | `[S]`+`[A]` |
| **Deafen** | стоп playback + op4 `self_deaf` (и `self_mute` — см. §10 D-5) | 0 записей PCM в выход | fake sink: 0 write | поведение сервера/собеседника | `[S]`+`[A]` |
| **Индикатор** | из фактических счётчиков кадров, не из флага UI | `aria-pressed` из потока | fake counters | видимость OS-индикатора | `[A]` (PLAT-10) |

PTT не отправляет op4 сам: он управляет локальным `self_mute` через сервис (VOICE §3.3).
`deafen ⇒ mute` не подтверждено дампом — решает PM (D-5); до решения шлём `self_deaf` и
локально глушим playback, микрофон не трогаем.

## 7. Экран, камера, приватность

### 7.1 Экран (Wayland — основной путь)

`ScreenCast` → PipeWire-клиент. В callback'ах только копирование `spa_buffer` в bounded-очередь
(без аллокаций в hot path, без энкода). Первый кадр → `data-stream="live"` + видимый индикатор;
останов — `Session.Close`, `pw_stream_destroy`, `pw_core_disconnect`, `pw_context_destroy`.
Suspend: `power.rs` уже слушает `PrepareForSleep` `[H]`; lock-сигнала в `power.rs` нет —
добавить отдельной карточкой `[A]`.

### 7.2 X11

Портал может дать ScreenCast и на X11 (бэкенд KDE), но это `[A]`, не проверено. Свой
XShm/XComposite-захват **вне MVP**: нет оракула, нет превью, больше поверхности. Список окон не
читаем, заголовки не логируем. Если портал недоступен → `portal-unavailable` `[A]`.

### 7.3 Камера — блокер и конфликт источников

`STREAM.md` §5.3: путь «v4l2 `/dev/video*`» `[A]`; `PLATFORM-AUDIT` §4.1: «камера — через
portal» `[A]`. Это **противоречие двух наших же решений**, его нельзя закрыть кодом (D-2).
Факты: камеры нет; portal `IsCameraPresent=false`; `/dev/video*` нет. Следствие: камера не
входит в DoD этой волны; допустимы только property-тест и fake-backend. Если PM выберет portal:
`AccessCamera → OpenPipeWireRemote → pw_stream` (тот же клиент, что экран); если v4l2 — нужен
`video`-доступ и другое железо.

### 7.4 Инварианты приватности (нарушение = блокирующий дефект)

1. Нет захвата при монтировании панели/входе в канал/открытии настроек (VC-01…VC-03).
2. Захват только по явному действию; источник — только портал (кроме `[A]`-решения по камере).
3. Индикатор = фактическое состояние потока кадров, а не флаг UI (PLAT-10).
4. Имена устройств, `id`/`mapping_id` стримов, `restore_token`, node/serial — не в логах.
5. Нет записи (`startLocalAudioRecording`, `setAecDump`, `setLoopback`, raw samples) — запрещено.
6. Нет превью экрана/окон и списков окон; нет `getDisplayMedia`/`getUserMedia`.
7. Конец сессии/сон → немедленный стоп потока и снятие индикатора.
8. Кадры не уходят в рендерер/IPC в MVP (только счётчики/состояние).

### 7.5 Что нельзя утверждать без live

«Согласие портала работает», «кадр приходит», «индикатор соответствует», «микрофон
захватывается», «hot-plug переключает», «PTT работает с реальной клавишей», «камера работает»,
«системный звук шарится», «X11-захват работает».

## 8. Зависимости, лицензии, разрешения

### 8.1 Системные библиотеки (динамическая линковка)

| Библиотека | Лицензия | Совместимость | Установка |
|---|---|---|---|
| `libopus` | BSD-3-Clause | да | не нужна (есть) |
| `libasound` | LGPL-2.1-or-later | да, **только** динамически; при распространении — возможность замены | не нужна (есть) |
| `libpipewire-0.3` | MIT + LGPL-2.1-or-later | да, динамически | не нужна (есть) |
| `libv4l2`/`v4l-utils` | LGPL | да, но заголовков нет | **не** требуется для MVP |
| `libudev` | LGPL-2.1-or-later | да | не нужна, не используем |

Статическая линковка LGPL-библиотек не планируется. Новых Rust-крейтов нет: `libc 0.2.191`
уже в `Cargo.lock`; FFI объявляется `#[link(name=…)]`, как keyring/hunspell.

### 8.2 Разрешения

| Что | Сейчас | Нужно |
|---|---|---|
| Микрофон ALSA | ACL `user:sanya:rw-` на `/dev/snd/pcm*` `[H]` | ничего; группа `audio` не нужна |
| Экран/камера | согласие портала (интерактивно) | пользователь подтверждает в live |
| Камера v4l2 (если D-2=v4l2) | нет устройства, нет `video` | железо + группа/ACL — отдельная карточка |
| PTT evdev (не выбираем) | нет `input` | не требуется при portal GlobalShortcuts |
| Установка пакетов | — | **запрещена** без PM; всё нужное уже есть |

### 8.3 Политика установок

Ни `pacman`, ни `cargo add`, ни `pip`, ни сеть в этой волне. Если понадобится
`libv4l2`-заголовки/камера/новый пакет — отдельная карточка с перечнем, лицензией, размером и
разрешением PM (offline-источник, если возможно). `Cargo.lock` не меняется.

## 9. Тесты и DoD

Все тесты — red-first (`TEST-POLICY` §3), логи в `waves/<ID>/logs/`, полный гейт один раз на
batch в integration-лейне (`AGENTS.md` §3). `skip` только с причиной и видимым счётчиком
(урок PLAT-02).

| ID | Тест / файл | Оракул | Offline | Owner | DoD |
|---|---|---|---|---|---|
| T-01 | `discord-platform/tests/portal_contract.rs` — ScreenCast happy path | python-мок на приватной шине, журнал TSV | да | G5 | 4 вызова + `Response` в порядке, `streams` разобран, сессия закрыта |
| T-02 | там же — consent `response=1/2` | мок эмитит Response | да | G5 | `ConsentDenied`/`ConsentAborted`, 0 fd, `Session.Close`; мок моделирует **только протокол** — диалог ОС не подделывается |
| T-03 | там же — fd-leak | `/proc/self/fd` до/после, 200 итераций | да | G5 | 0 прироста на success/error/cancel |
| T-04 | там же — handle-token race | мок возвращает другой handle | да | G5 | переподписка/ошибка, сигнал не потерян |
| T-05 | `dbus_contract.rs` — `U64`/`t` | байты сигнала `Activated` | да | G5 | timestamp разобран, выравнивание 8 |
| T-06 | `portal_contract.rs` — timeout | мок молчит | да | G5 | `ConsentTimeout` + `Request.Close` |
| T-07 | `opus_contract.rs` — smoke | `opus_get_version_string` | да | G5 | версия непустая, `1.6.x` |
| T-08 | `opus_contract.rs` — KAT | ffmpeg **native** enc/dec + фикстуры/md5 | да | G5 | 50 кадров, 960 сэмплов, SNR ≥ порога (PM), DTX |
| T-09 | `opus_contract.rs` — ошибки | NULL/negative коды | да | G5 | typed `CodecInit`/`CodecCall`, без паник |
| T-10 | `opus_contract.rs` — CN/DTX | `classify_opus_payload` | да | G5 | тишина → CN/Empty, речь → Speech |
| T-11 | `alsa_contract.rs` — `null` PCM | `alsa.conf` `pcm.null` | да | G5 | open/read/write 960, `snd_asoundlib_version` |
| T-12 | `alsa_contract.rs` — xrun | `-EPIPE` инъекция | да | G5 | recover, счётчик, поток жив |
| T-13 | `alsa_contract.rs` — нет устройства | bogus name | да | G5 | `DeviceUnavailable`, без паники |
| T-14 | `devices_contract.rs` — hot-plug | fake enumerator | да | G5 | список обновился, выбор не изменился, `device-missing` |
| T-15 | `devices_contract.rs` — mute/deafen | fake source/sink | да | G5 | mute → 0 encode; deafen → 0 write |
| T-16 | `portal_contract.rs` — PTT | мок `Activated`/`Deactivated` | да | G5 | `held`/`idle`, `blocked` без сессии/фокуса |
| T-17 | `discord-voice/tests/voice_pipeline.rs` | fake PCM + SRTP + fake sink | да | G1+G5 | seq/ts 960, marker на talkspurt, 0 plaintext |
| T-18 | `ffi_pipewire` smoke | `pw_check_library_version(1,6,0)` | да | G5 | true, `pw_init/deinit` без паник |
| T-19 | `ffi_pipewire` error-path | socketpair + мусор | да | G5 | NULL → typed, fd закрыт ровно один раз |
| T-20 | `tests/meta/voice-privacy.test.mjs` | grep `ui/**`, `rust/**` | да | G4b | нет `mediaDevices`/`getDisplayMedia`/имён устройств в логах |
| T-21 | `ffi_pipewire` fake backend | trait `PipeWireBackend` | да | G5 | стоп/close по событию, без callback в тесте |
| T-22 | UI fake consent/state | fake `CaptureState`/`ConsentState` (без портала) | да | G2+G5 | в `off`/`denied` — 0 вызовов захвата; индикатор только при фактических кадрах |
| L-1 | live: согласие + кадр | портал + `pw_stream` | **нет** | G5+PM | окно, PNG/лог, `data-stream=live` |
| L-2 | live: микрофон + mute | реальный PCM | **нет** | G5+PM | 0 пакетов при mute, индикатор |
| L-3 | live: PTT клавиша | системный диалог | **нет** | G5+PM | `held`/`idle`, `blocked` |
| L-4 | live: камера | железо | **нет** | — | **заблокировано** (§11) |

Мутации (обязательны для новых инвариантов): убрать `close_fd` → T-03 краснеет; убрать
`session.Close` → T-02 краснеет; `960` → `480` → T-08/T-17 краснеют; отключить DTX → T-10;
убрать `catch_unwind` → T-21/мутация паники в callback; `U64`-выравнивание 4 → T-05.

## 10. Открытые решения `[A]` (нужен PM до кода)

| ID | Вопрос | Варианты | Рекомендация |
|---|---|---|---|
| D-1 | Где живёт FFI | `discord-platform` (deny) / ослабить `forbid` в media/voice | `discord-platform`, линты не менять |
| D-2 | Камера | portal Camera / прямой v4l2 | portal (consent, один клиент PipeWire); v4l2 — позже |
| D-3 | X11-экран | portal-only / свой XShm | portal-only в MVP |
| D-4 | Opus | mono, default bitrate, DTX on | принять; подтвердить live |
| D-5 | Deafen | `self_deaf` только / `+self_mute` | до live: только `self_deaf` + локальный стоп playback |
| D-6 | PTT | portal GlobalShortcuts / evdev / X11 | portal; ключ не назначаем |
| D-7 | Hot-plug | poll 1000 мс / udev / PipeWire registry | poll; udev позже |
| D-8 | `restore_token` | файл `0600` в `XDG_STATE_HOME` | принять; формат — отдельная карточка |
| D-9 | PipeWire callback | `catch_unwind` + флаг отказа, стоп потока | принять; дополнить PLAT-06 и UNSAFE-REGISTRY |
| D-10 | Таймаут согласия | 300 с / без предела / отмена | 300 с + `Request.Close`, тест T-06 |
| D-11 | Opus-оракул | ffmpeg native / vendored RFC 6716 vectors | ffmpeg сейчас; vectors — если PM даст фикстуры |
| D-12 | Имена IPC/JSON | контракт G2 | **не** фиксируем здесь; тесты — на Rust-API |

## 11. Blockers

| ID | Блокер | Что блокирует | Разблокирует |
|---|---|---|---|
| B-1 | Камеры нет (`/dev/video*`, `IsCameraPresent=false`) `[H]` | L-4, любой DoD камеры | новое железо + окно PM |
| B-2 | Согласие портала интерактивно | L-1, L-3 | окно PM (пользователь подтверждает) |
| B-3 | PipeWire требует Rust-callback в C; политика PLAT-06 | шаг 4 | решение D-9 + meta-тест |
| B-4 | `discord-media`/`discord-voice` — `forbid(unsafe_code)` | размещение FFI | решение D-1 |
| B-5 | D-Bus-клиент без `t`/u64 и typed-fd | PTT + `OpenPipeWireRemote` | P-1/P-2 (маленькие аддитивные правки) |
| B-6 | Ключ PTT не наблюдался (`keybind_value=null`) `[D]` | «PTT как в Discord» | live-редактор или принять «не назначено» |
| B-7 | В репозитории нет офлайн-векторов RFC 6716 | строгая conformance Opus | ffmpeg-оракул (D-11) или фикстуры от PM |
| B-8 | `parent_window`/xdg-foreign в спайке нет | привязка диалога к окну | `""` (§3.4 п.4) + live |
| B-9 | Нет lock-сигнала в `power.rs` | стоп захвата при блокировке экрана | отдельная карточка G5 |

## 12. Проверки (evidence) и артефакты

| Команда (read-only) | Результат |
|---|---|
| `pacman -Q opus alsa-lib pipewire v4l-utils` | 1.6.1-1.1 / 1.2.16.1-1.1 / 1:1.6.10-1.1 / 1.32.0-2.1 |
| `pkg-config --modversion opus alsa libpipewire-0.3 libspa-0.2` | 1.6.1 / 1.2.16.1 / 1.6.10 / 0.2 |
| `ldconfig -p \| grep -E 'libopus\|libasound\|libpipewire\|libv4l2'` | все `.so` есть |
| `ls /usr/include/{opus,alsa,pipewire-0.3,spa-0.2}` | есть; `/usr/include/libv4l2` — нет |
| `ls /dev/video*; ls /sys/class/video4linux` | нет обоих |
| `busctl --user get-property … ScreenCast version/AvailableSourceTypes/AvailableCursorModes` | 6 / 7 / 7 |
| `busctl --user get-property … Camera version/IsCameraPresent` | 1 / false |
| `busctl --user get-property … GlobalShortcuts version` | 2 |
| `ls -l /dev/snd; getfacl /dev/snd/pcmC2D0c; id -nG` | ACL `user:sanya:rw-`, групп audio/video/input нет |
| `pgrep -a pipewire; pgrep -a wireplumber` | 1258, 1446, 1260 |
| `python3 -c "import dbus, gi"` | OK (mock-портал возможен) |
| `ffmpeg -h encoder=opus; ffmpeg -h decoder=opus` | нативные enc/dec есть |
| `grep pcm.null /usr/share/alsa/alsa.conf` | строка 335 |
| `grep -E '<method\|<arg' /usr/share/dbus-1/interfaces/org.freedesktop.portal.*.xml` | сигнатуры §3 |
| `grep opus_get_version_string /usr/include/opus/opus_defines.h` | есть |
| `grep pw_get_library_version /usr/include/pipewire-0.3/pipewire/version.h` | есть |

Артефакт этой карточки: `waves/RW-40/QUALITY/G5-VOICE-PLAN.md`. Код, тесты, фикстуры, `Cargo.*`,
`waves/db.jsonl` и `STATE.md` не изменялись; `logs/`, `functions/`, `evidence/`,
`/home/sanya/re-deep` — только чтение. `git status` до/после — без новых правок, кроме этого файла.
