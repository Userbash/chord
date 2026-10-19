# Нативные модули Discord Desktop 1.0.162 (Linux x64) — статический разбор и связи с JS

**Задача.** Статически разобрать `.node`-модули релиза `app-1.0.162` и проследить их связь с JS-кодом
main-процесса/прелоада. Ничего не запускалось, не патчилось, сеть не использовалась.

**Только чтение:** `~/.config/discord/app-1.0.162/**`, `/home/sanya/re-deep/asar-extract/**`,
`/home/sanya/re-deep/decoded/**` (кэш JS-чанков), `/home/sanya/re-deep/voice/ui/03-voice-module-api.json`
(живая выборка 110 имён), `/home/sanya/re-deep/backend-ipc.md` (карта IPC, использована как ссылка).

**Артефакты:** сырые выводы — `/home/sanya/re-deep/native-re/raw/<модуль>/*.txt`,
итоги — `native-re.md` (этот файл) и `native-re.json`, скрипты — `native-re/collect.sh`,
`native-re/extract.py`, `native-re/js-grep.mjs`, `native-re/crosscheck-voice.py`, `native-re/build-json.py`.

---

## 1. Методика

Для каждого `.node` (9 уникальных файлов; `cld.node` и `notificationstate.node` лежат по два раза —
`build/Release/` и `build/Release/obj.target/`, md5 совпадают, проанализированы один раз):

| шаг | команда | файл raw |
|---|---|---|
| инфо | `rabin2 -I` | `rabin2-info.txt` |
| импорты | `rabin2 -i` | `rabin2-imports.txt` |
| экспорты | `rabin2 -E` | `rabin2-exports.txt` |
| строки (секции данных) | `rabin2 -z` | `rabin2-strings.txt` |
| строки (весь файл) | `strings -a -n 6` | `strings-a-n6.txt` |
| зависимости | `readelf -dW` | `readelf-dyn.txt` |
| сегменты | `readelf -lW` | `readelf-segments.txt` |
| символы | `nm -D --demangle`, `nm --demangle` | `nm-dynamic-demangled.txt`, `nm-all-demangled.txt` |
| определение | `file` | `file.txt` |

Все внешние команды — под `timeout 300`.

### 1.1 Важное ограничение измерения строк

`rabin2 -z` читает **только `.rodata` + `.data`** и не заходит в `.text`. Для `discord_voice`:

| источник | строк |
|---|---:|
| `rabin2 -z` | 113 402 |
| `strings -a -n 6` (весь файл) | 410 806 |

Проверка по секциям (`dd` + `strings`): `.rodata` ≈ 21 тыс. строк, `.data` ≈ 53 тыс., `.text` (22 МБ кода)
≈ 244 тыс. Поэтому **все категорийные выборки ниже сделаны по `strings -a -n 6`**; выводы `rabin2 -z`
сохранены как есть (требование задачи) и служат второй точкой опоры.

`strings -n 6` отбрасывает имена короче 6 символов: так потерялось NAPI-имя `pack` (4 символа) в
`discord_erlpack` — найдено только через `nm` (`Encoder::pack`). Это тоже учтено.

### 1.2 Деманглинг

`nm --demangle` покрывает Itanium (`_ZN…`) и Rust v0 (`_R…`). Rust-модули (`discord_zstd`, `updater`)
деманглируются корректно, поэтому для них основная опора — `nm`, а не `strings`.

---

## 2. Сводка по модулям

| модуль | размер, Б | md5 (16) | NEEDED | строк (`-n 6`) | отфильтровано генерик-строк | NAPI-вход |
|---|---:|---|---|---:|---:|---|
| `discord_voice` | 126 136 024 | `5fe80ce0f5d89fd5` | 13 libs | 410 806 | 168 063 | `napi_register_module_v1` (symtab) |
| `discord_krisp` | 62 078 048 | `9be929d1d90a8290` | 6 libs | 164 989 | 58 297 | `napi_register_module_v1` (symtab) |
| `discord_utils` | 19 185 488 | `6bd62709ff518d50` | 7 libs | 65 723 | 40 386 | `napi_register_module_v1` (symtab) |
| `discord_game_utils` | 11 478 904 | `220450ece4bf9cf4` | 5 libs | 38 939 | 21 541 | `napi_register_module_v1` (symtab) |
| `discord_erlpack` | 7 755 832 | `a4bf9140d20dbf17` | 5 libs | 27 776 | 15 941 | `napi_register_module_v1` (symtab) |
| `cld.node` (spellcheck) | 6 646 552 | `303bc2870515fe61` | 5 libs | 7 359 | 910 | `napi_register_module_v1` (symtab) |
| `updater.node` | 5 812 960 | `6a77646b1d9e2b9c` | 6 libs | 13 188 | 24 | `napi_register_module_v1` (dynsym) |
| `discord_zstd` | 873 248 | `e6fb396136f1e24c` | 5 libs | 2 772 | 18 | `napi_register_module_v1` (symtab) |
| `notificationstate` | 16 840 | `63ad44ea5fec4949` | 5 libs | 58 | 5 | — (Linux-заглушка) |

**Размеры в задании были указаны неверно**: `discord_voice.node` — 126 МБ (не 37), `discord_krisp.node` —
62 МБ (не 48). 37 МБ — это `PT_LOAD.memsz` (объём маппинга), а не размер файла.

### 2.1 Зависимости (полностью)

* **discord_voice**: `libmediapipe.so`, `libX11.so.6`, `libwayland-client.so.0`, `libpulse.so.0`,
  `libasound.so.2`, `libatomic.so.1`, `librt.so.1`, `libdl.so.2`, `libpthread.so.0`, `libm.so.6`,
  `libgcc_s.so.1`, `libc.so.6`, `ld-linux-x86-64.so.2`; `SONAME discord_voice.node`, `RUNPATH $ORIGIN`.
* **discord_krisp**: `libstdc++.so.6` + libc-набор. **OpenBLAS и OpenSSL — статически** (нет в NEEDED).
* **discord_utils**: `libX11.so.6`, `libwayland-client.so.0` + libc-набор.
* **discord_game_utils / discord_erlpack / cld / notificationstate**: только libc + `libstdc++` (cld, notificationstate).
* **discord_zstd / updater**: только libc-набор; компилятор — `rustc 1.99.0-nightly (73dc9167f 2026-08-01)`.

### 2.2 Что грузится динамически (dlopen), а не линкуется

`discord_voice` **не содержит** в NEEDED: pipewire, vulkan, va, cuda, nvenc, EGL/GLES, libGL, ffmpeg.
Строки подтверждают `dlopen`-имена:

```
libpipewire-0.3.so.0   libpulse.so.0        libasound.so.2        libvulkan.so.1
libva.so.2            libva-drm.so.2        libcuda.so            libnvidia-encode.so.1
libEGL.so.1           libGLESv2.so.2        libGL.so.1            libwayland-client.so.0
libmediapipe.so
```

Дополнительно в символах есть трамплины-указатели: `pw_stream_flush_ptr`, `pw_stream_get_name_ptr`,
`pw_stream_set_control_ptr`, `pw_stream_set_error_ptr`, `pw_stream_get_properties_ptr` и
`InitializePipewire` / `IsPipewireInitialized` / `UninitializePipewire`
(`gen/discord_media/components/video/interop/linux-interop/cpp/capture/pipewire_stubs.cc`).
**Вывод: PipeWire подключается в рантайме и только при наличии; при отсутствии модуль работает без него.**

FFmpeg: в `rabin2 -i` видны сотни `avcodec_*`/`avformat_*`/`av_frame_*` (UND), но ни `libavcodec`,
ни `libffmpeg.so` нет ни в NEEDED, ни в списке dlopen. В `Discord` (ELF, 205 МБ) этих символов тоже нет
(`nm -D --defined-only | grep avcodec_find_decoder` → 0), а `libffmpeg.so` их экспортирует (2 совпадения).
**Итог: FFmpeg-импорты модуля не разрешаются из проанализированных файлов — это незакрытый вопрос**
(см. §7).

---

## 3. discord_voice.node — 126 МБ, ядро голоса/видео

### 3.1 Внутренняя структура кода (из `__FILE__`-тегов логов)

839 уникальных путей `src/**` извлечено из строк (файл `raw/discord_voice/src-files.txt`).
Крупные подсистемы:

| область | файлы (пример) |
|---|---|
| ядро | `src/discord.cpp`, `src/voice_connection.cpp`, `src/state.cpp`, `src/bindings.cpp`, `src/messages.cpp`, `src/socket_client.cpp`, `src/udp_socket.cpp`, `src/credential.cpp` |
| медиа-движок | `src/media/engine.cpp`, `src/media/connection.cpp`, `src/media/local_user.cpp`, `src/media/remote_user.cpp`, `src/media/user.cpp` |
| аудио-устройства | `src/media/audio_device/*` (`webrtc_adm_helper.cpp`, `device_selection.cpp`, `local_audio_combiner.cpp`, `wav_file_audio.cpp`, `sine_audio.cpp`, `silent_audio.cpp`, `fake_adm*.cpp`) |
| Opus | `src/src/opus_encoder.c`, `opus_decoder.c`, `opus_multistream_encoder.c`, `src/celt/*` (18 файлов), `src/silk/*` (43 файла), `src/silk/x86/NSQ_sse4_1.c`, `VAD_sse4_1.c` |
| крипто | `src/libsodium/**` (≈90 файлов), `src/crypto/**` (BoringSSL/OpenSSL), `src/cryptor.cpp`, `src/boringssl_cryptor.cpp`, `src/hpke.cpp`, `src/dhkem.cpp`, `src/aead_cipher.cpp` |
| MLS/DAVE | `src/mls/session.cpp`, `src/mls/parameters.cpp`, `src/mls/user_credential.cpp`, `src/mls/key_serialization.cpp`, `src/mls/persisted_key_pair.cpp`, `src/treekem.cpp`, `src/tree_math.cpp`, `src/mls_key_ratchet.cpp`, `src/key_schedule.cpp`, `src/group.cpp`, `src/credential.cpp`, `src/signature.cpp` |
| видео-кодеки | `src/media/video_coding/av1.cpp`, `decoding/h264_decoder_impl.cpp`, `encoding/webrtc_encoder_factory.cpp`, `encoding/linux/encoder_factory.cpp`, `decoding/pixel_capped_decoder.cpp` |
| share экрана | `src/media/videoshare/capture_broker.cpp`, `screenshare_helper.cpp`, `screenshare_capture_loop.cpp`, `webrtc_screenshare_source.cpp`, `wumpus.cpp`, `previews_null.cpp`, `external_video_source.cpp` |
| NDI | `src/media/ndi/ndi_process_bridge.cpp`, `ndi_process_bridge_posix.cpp`, `ndi_video_forwarder.cpp`, `ndi_video_source.cpp` |
| Clips | `src/media/clips/clips_process_bridge.cpp`, `clips_video_forwarder.cpp`, `clips_process_bridge_null.cpp` |
| фильтры видео | `src/media/filters/media_pipe_filter_impl.cpp`, `background_blur_filter.cpp`, `background_replacement_filter.cpp`, `media_filter_manager.cpp`, `gles_rgb_converter.cpp` |
| звук Linux | `src/media/soundshare/soundshare_audio_device_linux.cpp`, `pulseaudio_controller.cpp`, `controller.cpp`; `src/media/audio_device/*`; `src/audio_device_linux.cpp` |
| VAD/AEC | `src/vad_observer.cpp`, `src/media/echo_canceller_factory.cpp`, `echo_canceller_probe_bank.cpp`, `frame_activity_detector.cpp`, `volume_analyzer.cpp` |
| качество | `src/media/image_quality_vmaf_manager.cpp` + `src/feature/**` (libvmaf, ≈70 файлов), `vmaf_v0.6.1.json` |
| spaрital | `src/media/spatial_mixer.cpp`, `spatial_audio_options.cpp`, `reflections.cpp`, `third_party/steam_audio_src/**` (364 пути) |
| NAPI-мост | `src/bindings.cpp`, `src/core_types.cpp`, `src/filter_settings.cpp`, `src/media/codec_survey/*`, `src/transport_settings.cpp`, `src/userinfo_vc.cpp` |

Сборка: `GCC 9.3.0` + `clang 16.0.6` + `libc++` (`third_party/llvm/libcxx`), C++ = `std::__Cr`.

### 3.2 Таблица NAPI-свойств модуля — точное место в файле

В `.rodata` есть непрерывный блок имён свойств, строки **73279–73372** файла `strings-a-n6.txt`:
**94 имени**. Он и есть плоский API модуля. Первые и последние строки блока:

```
73275 MAINTAIN_RESOLUTION      ← enum DegradationPreference
73276 MAINTAIN_FRAMERATE
73277 BALANCED
73278 DISABLED
73279 SupportedSecureFramesProtocolVersion
73280 setOnVoiceCallback
...
73366 setSpatialAudioStatusCallback
73367 pollQueueMetrics
73368 setSystemMicrophoneModeChangeCallback
73369 getSystemMicrophoneMode
73370 showSystemCaptureConfigurationUI
73371 setKrispPath
73372 setMLPath
```

Отдельно, уже вне плоской таблицы, регистрируются NAPI-классы (точные строки):

```
73459 VoiceConnection
73460 VoiceReplayConnection
```

### 3.3 Сверка живой выборки (110 имён) с бинарём

Результат (`native-re.json → voiceApiCrosscheck`):

| метрика | значение |
|---|---:|
| имён в живой выборке | 110 |
| имён в плоской таблице бинаря (пересечение) | **92** |
| порядок пересечения совпадает с порядком таблицы | **да** |
| имён есть в бинаре, но нет в выборке | 2 (`setVideoOutputSink`, `signalVideoOutputSinkReady`) |
| имён выборки, отсутствующих как строки в бинаре | 18 |

18 отсутствующих объясняются **полностью** — это ключи JS-обёртки
`modules/discord_voice-1/discord_voice/index.js` (22 447 Б), которая оборачивает `require('./discord_voice.node')`
и добавляет свои свойства:

```
index.js:191  VoiceEngine.createTransport = VoiceEngine._createTransport;
index.js:197  VoiceEngine.createVoiceConnectionWithOptions = function (userId, connectionOptions, onConnectCallback) {...}
index.js:208  VoiceEngine.createOwnStreamConnectionWithOptions = VoiceEngine.createVoiceConnectionWithOptions;
index.js:209  VoiceEngine.createReplayConnection = function (audioEngineId, callback, replayLog) {...}
index.js:233  VoiceEngine.setAudioSubsystem = function (subsystem) {...}
index.js:236  VoiceEngine.queueAudioSubsystem = function (subsystem) {...}
index.js:239  VoiceEngine.setOffloadAdmControls = function (doOffload) {...}
index.js:242  VoiceEngine.setDebugLogging = function (enable) {...}
index.js:255  VoiceEngine.getDebugLogging = function () {...}
index.js:261  VoiceEngine.setActiveSinksChangeCallback = function (callback) {...}
index.js:331  VoiceEngine.getNextVideoOutputFrame = function (streamId) {...}
index.js:358  VoiceEngine.platform = process.platform;
index.js:359  VoiceEngine.initialize({ audioSubsystem, logLevel, dataDirectory, ... });
index.js:372  VoiceEngine.setupKrispPath = function () { const krispPath = discordNative?.nativeModules?.getModulePath('discord_krisp'); if (krispPath != null) { VoiceEngine.setKrispPath(krispPath); } };
index.js:378  VoiceEngine.setupMLPath  = function () { ... getModulePath('discord_ml') ... VoiceEngine.setMLPath(mlPath); };
```

Остальные 3 из 18: `VoiceConnection` / `VoiceReplayConnection` (NAPI-классы, §3.2) и
`DegradationPreference` (enum-блок 73275–73278 + подстроки в символах).
**Вывод: «живой список 110» — это `Object.keys()` JS-обёртки, а не голого NAPI-модуля.**
Ровно 94 из них — плоский NAPI, 2 — NAPI-классы, 14 — добавлено `index.js`, 2 (`setVideoOutputSink`,
`signalVideoOutputSinkReady`) в живую выборку не попали.

### 3.4 Capability-detection: 13 вызовов через `lE()` ведут в никуда на этой сборке

JS-код рендерера получает нативный модуль через аксессор:

```js
// decoded/3eefa7dae1e2c234_0
function c(){return a(), r ?? (r = i.getVoiceEngine())}   // экспорт lE
// ...
getVoiceEngine(){ if(__OVERLAY__) throw Error("cannot require discord_voice in overlay");
                  let e=this.requireModule("discord_voice"); ... return e }
```

Из этого чанка вытащено 65 вызовов вида `(0,X.lE)().<имя>`. **51 имя присутствует в бинаре 1.0.162,
14 — нет** (у `DegradationPreference`/`platform` совпадает только подстрока):

```
нет в бинаре: getDebugLogging, getDeviceOSMuted, getDeviceOSVolume, getSingleWindowPreview, getStats,
              setAsyncVideoInputDeviceInit, setAsyncVideoInputDeviceInitSetting, setAudioSubsystem,
              setDebugLogging, setDeviceHardwareMutedChangeCallback, setHasFullbandPerformance,
              setNativeThreadsPriority, setNcModels
          (+) DegradationPreference, platform — только подстрока
```

Все они вызываются с защитой (`?.`, `typeof …=== 'function'`, `null !=`), т.е. это **capability
detection**: JS-чанк (кэш веб-приложения) новее, чем нативный модуль релиза 1.0.162. Запуск нативного
кода при этом не падает — отсутствующие методы просто не вызываются.

### 3.5 Подтверждённые подсистемы (со строковыми/символьными доказательствами)

**MLS / DAVE (E2EE).** `mlspp` встречается 17 287 раз — это Cisco `mlspp` (RFC 9420). В символах:

```
b mlspp::sign_label::mls_content
t mlspp::mls_1_0_plus(...)
t discord::dave::codec_utils::ProcessFrameOpus(...)      ProcessFrameVp8/Vp9/H264/H265/Av1
t discord::dave::codec_utils::BytesCoveringH264PPS(...)  FindNextH26XNaluIndex(...)
t discord::dave::codec_utils::ValidateEncryptedFrame(...)
r discord::dave::codec_utils::kH26XNaluLongStartCode
t discord::dave::Leb128Size/ReadLeb128/SetLogSink
```
Строки: `dave` ×903, `Proposal` ×457, `LeafNode` ×419, `KeyPackage` ×253, `Commit` ×202,
`Welcome` ×178, `GroupInfo` ×158, `treekem` ×128, `MLSMessage` ×101, `MLSCommitCallback`,
`MLSWelcomeCallback`, `MLSProposalsCallback`, `MLSPairwiseFingerprintCallback`.
→ DAVE = mlspp-реализация MLS + **отдельные шифровальщики кадров для Opus/VP8/VP9/H264/H265/AV1**.

**Krisp-интеграция (внутри voice).** `discord::KrispWrapper`: `Load`, `SetupNCE(int16,float)`,
`ResetNCE`, `ProcessNCE`/`ProcessNCFloat`, `SetupVADE`, `ResetVADE`, `ProcessVADE`/`ProcessVADFloat`,
`SetNCModel(NCModel)`, `ApplyNCModel`, `GetSessionStats(noise_cancellation::NCSessionStats)`,
`SetHasFullbandPerformance`, `HasFloat`; extern-хук `KrispNCSetModel`, строки `discord_krisp`, `discord_ml`.
Исходники: `src/krisp/krisp_wrapper.cpp`, `krisp_wrapper_posix.cpp`, `noise_canceller.cpp`,
`voice_activity_detector.cpp`.

**MediaPipe (фон).** Символы/строки: `MediaPipeFilter` ×343, `MediaPipeFilterImpl` ×147,
`MediaPipeGraph`, `MediaPipePacketOpaque`, `MediaPipeGpuBufferOpaque`, методы `Initialize(FilterSettings)`,
`ProcessFrame(VideoFrame, callback)`, `CompletionThreadMain`, `RunOnContext`. Файлы моделей, лежащие
рядом с модулем: `selfie_segmentation.tflite` (249 505 Б), `selfie_segmentation_landscape.tflite`
(250 145 Б). Соответствует `features.declareSupported('video_background_filter')` в `index.js`.

**Кодеки видео.** `Vulkan` ×222, `libvpx` ×73, `vp8/VP8` ×48, `vaapi` ×24, `av1/AV1` ×42,
`h264/H264` ×32, `VP9/Vp9` ×18, `H265/h265` ×20, `NVENC` ×6, `OpenH264` ×1, `DRM` ×12.
Исходники: `src/media/video_coding/av1.cpp`, `decoding/h264_decoder_impl.cpp`,
`encoding/linux/encoder_factory.cpp`. Плюс отдельный бинарь `gpu_encoder_helper` (1 027 416 Б)
в каталоге модуля.

**Захват экрана/PipeWire/портал.**

```
org.freedesktop.portal.Desktop / .ScreenCast / .Session / .Request
org.freedesktop.DBus + полная таблица org.freedesktop.portal.Error.* (Failed, InvalidArgument,
   NotFound, Exist, NotAllowed, Cancelled, WindowDestroyed)
InitializePipewire / IsPipewireInitialized / UninitializePipewire, pw_thread_loop_*, pw_stream_*,
   pw_properties_*, pw_main_loop_*, CcPipewireConfig, GetPickedPipewireConfig / ClearPickedPipewireConfig
BaseCapturerPipeWire, ScreenCast/screencast, CaptureBroker, WebRtcScreenshareSource,
   ScreenshareHelper, LinuxVideoSource, ScreenshareCaptureLoop, WumpusVideoSource/FrameBuffer/Pool
```
Портал реализован через **Rust-zbus** (строка `org.freedesktop.zbus.Error…`, `PortalError` и т.п.);
PipeWire — через dlopen-трамплины. Соответствует `index.js`: `native_screenshare_picker` объявляется
только при Wayland, `gamescope_capture` — при Wayland-gamescope + VAAPI + существующем сокете
`$XDG_RUNTIME_DIR/$PIPEWIRE_REMOTE|pipewire-0`.

**NDI.** `NdiProcessBridge` ×1686, `NdiVideoSource` ×151, `NdiSourceDescription` ×121,
методы `SetNdiModulePath`, `SetNdiSentryConfig`, `SetNdiEnabled`, `SetNdiParticipantInfo`, `SetOnNdiEvent`,
`OnNdiBridgeEvent`; `src/media/ndi/ndi_process_bridge_posix.cpp`.

**Clips.** `ClipsProcessBridge` ×1074, `ClipsRecordingEvent` ×325, `ClipsMlDetection` ×103,
`ClipsPerfMonitor`, `ClipsAudioStreamPermissions`, `ClipsMlDataPoint`, `ClipsSourceEnded`,
`ClipsSentryConfig`, `ClipsAudioModelOverride`. → запись/обработка клипов вынесена в **отдельный процесс**
(`clips_process_bridge.cpp` + `clips_video_forwarder.cpp`).

**VAD / AEC / сеть.**

```
AEC:  EchoCanceller3Config ×65, EchoCanceller3Factory, EchoCanceller3, EchoCancellerProbeBank,
      src/media/echo_canceller_factory.cpp  (WebRTC AEC3)
VAD:  VadObserver ×23, VadSettings, webrtc_vad, silk/VAD.c + silk/x86/VAD_sse4_1.c,
      src/audio_mode_vad.cpp, src/media/frame_activity_detector.cpp
сеть: SetIdleJitterBufferFlush, kDiscordBandwidthEstimationExperimentTrendlineWindowDuration,
      GetSupportedBandwidthEstimationExperiments, Probe*, twcc-заголовок
      (http://www.ietf.org/id/draft-holmer-rmcat-transport-wide-cc-extensions-01),
      RTP-заголовки abs-capture-time (http://www.webrtc.org/experiments/rtp-hdrext/abs-capture-time)
```

**Spatial audio.** `Ambisonics{Binaural,Decode,Encode,Panning,Rotate}Effect`, `HRTF`, `SOFA`
(`mysofa/share/libmysofa/default.sofa`), `speaker_layout.cpp`, `hybrid_reverb_effect.cpp`,
`steam_audio_src/**` (364 пути) — Steam Audio (Phonon), включая реверберацию и окклюзию.
В `index.js`: `spatial_audio` объявляется только если `VoiceEngine.isSpatialAudioEnabled()`.

**Качество видео.** `libvmaf` (`src/feature/**`, `vmaf_v0.6.1.json`), `image_quality_vmaf_manager.cpp`,
`video_quality_observer.cpp`; фича `image_quality_measurement`.

### 3.6 Публичные NAPI-свойства 94/94 (полный список)

```
SupportedSecureFramesProtocolVersion setOnVoiceCallback setEmitVADLevel setEmitVADLevel2 setTransportOptions
getNoiseCancellationStats setNoiseCancellationEnableStats setDeviceChangeCallback setOutputDevice setInputDevice
setVideoInputDevice getOutputDevices getInputDevices getVideoInputDevices setVideoOutputSink
addDirectVideoOutputSink removeDirectVideoOutputSink signalVideoOutputSinkReady setImageDataAllocator
setInputVolume setOutputVolume setVolumeChangeCallback setNoInputThreshold setVoiceSampleRateCap
setVoiceChannelCountCap setNoInputCallback getCodecCapabilities getCodecSurvey getAudioSubsystem isVaapiEnabled
isSpatialAudioEnabled pingVoiceThread getScreenPreviews getWindowPreviews setPreviewsUseWgc setClipBufferLength
getSystemSteadyClockNowMs saveClipEx updateClipMetadata saveScreenshot exportClipToFile consoleLog
writeAudioDebugState setAecDump startRecordingRawSamples stopRecordingRawSamples processBatchAudioFiles
cancelBatchAudioProcessing rankRtcRegions setVideoInputInitializationCallback setAudioInputInitializationCallback
applyMediaFilterSettings applyMediaFilterSettingsWithCallback setMaxSyncDelayOverride startLocalAudioRecording
stopLocalAudioRecording setLoopback setClipsModulePath setClipsDataPath setClipsSentryConfig setClipsUIActive
setClipsV3MLEnabled setClipsAudioModelOverride setClipsRecordingEnabled setOnClipsMlDetection
setClipsPerfMonitoring setNdiModulePath setNdiSentryConfig setNdiEnabled setNdiParticipantInfo setOnNdiEvent
applyClipsSettings applyClipsQualitySettings setClipsSource setOnClipsRecordingEvent
getSupportedBandwidthEstimationExperiments getMLSSigningKey setSidechainCompression
applySidechainCompressionSettings updateFieldTrial setNativeDesktopVideoSourcePickerActive
presentNativeScreenSharePicker releaseNativeDesktopVideoSourcePickerStream setNativeScreenSharePickerCallbacks
setAudioDeviceModuleErrorCallback setVoiceProcessingErrorCallback setVideoFilterErrorCallback
setSpatialAudioStatusCallback pollQueueMetrics setSystemMicrophoneModeChangeCallback getSystemMicrophoneMode
showSystemCaptureConfigurationUI setKrispPath setMLPath
```

### 3.7 Экспортируемые фичи (из `index.js`, 52 шт.)

```
async_video_input_device_init audio_codec_red audio_debug_state bandwidth_estimation_experiments clips
clips_recording_ready_events clips_thumbnail debug_logging direct_video electron_video elevated_hook
experiment_config first_frame_callback fixed_keyframe_interval gamescope_capture go_live_hardware
hybrid_video image_quality_measurement krisp_native_error loopback media_devices media_video
mls_pairwise_fingerprints native_screenshare_picker offload_adm_controls port_aware_latency_testing
remote_locus_network_control remote_user_multi_stream screen_capture_kit screen_previews screen_soundshare
set_audio_device_by_id set_video_device_by_id sidechain_compression simulcast simulcast_bugfix soundshare
soundshare_loopback spatial_audio udp_endpoint_update vaapi video_background_filter video_effects
voice_automatic_subsystem voice_bypass_system_audio_input_processing voice_experimental_subsystem
voice_legacy_subsystem voice_multiple_connections voice_panning voice_subsystem_deferred_switch window_previews
wumpus_video
```
Платформенные ограничения (Linux): `native_screenshare_picker` — только Wayland; `vaapi` — если
`VoiceEngine.isVaapiEnabled()`; `gamescope_capture` — gamescope+vaapi+сокет pipewire;
`clips*`, `screen_previews`, `window_previews`, `audio_debug_state`, `video_effects`, `elevated_hook`,
`wumpus_video`, `hybrid_video`, `soundshare_loopback`, `voice_legacy_subsystem` — **только win32**.

---

## 4. discord_krisp.node — 62 МБ, шумоподавление и VAD

### 4.1 Модели — реальные файлы рядом с модулем

| файл | размер, Б | sha256 |
|---|---:|---|
| `krisp-bvc-o-pro-v3.kef` | 15 122 004 | `3f65d975ee4487931aff462e71e740a37b581817f9bad860590b194409688859` |
| `krisp-nc-o-lite-v1.kef` | 2 895 574 | `7fdff67057090f402fb7688b29bf0afbd65cb1ed6a6956d2e304b5f3d84afa48` |
| `krisp-nc-o-med-v7.kef` | 5 880 640 | `1b421bd27f892c6aef041496c939da4f31360fda36d55b83e48435cae89d246f` |
| `krisp-nc-o-nb-v2.kef` | 2 822 586 | `b3d2737fae2e6f900f7efb33aa17a3b6e6548e48ebcfbf5bd2f0746eec04c086` |
| `krisp-vad-o-v2.kef` | 592 233 | `092c397f12443e5f2bdd07428b9e193ce359f6a08bfb403807c540f18647b3f3` |

Рядом также пустой каталог `KMS/logs/` (создан при установке; строка `KMS` в бинаре не встречается —
назначение не установлено).

В JS-коде есть и другие имена (кэш рендерера): `krisp-nc-o-nb-v2.kef`, `krisp-nc-o-med-v7-fp16.kef`,
`krisp-nc-o-lite-v1.kef` и браузерные `krisp_browser_models/v1.0.11_1/model_{8,16,32}.kw`,
а также `krisp.ai/v2/resource/link/discord`.

### 4.2 NAPI-таблица и C-API

Плоская таблица свойств (непрерывный блок в дампе):

```
71289 Teardown
71290 _initialize         71291 _getNcModels     71292 _getNcModelFilename
71293 setNcModel          71294 _getVadModels    71295 setVadModel
71296 getSdkVersion       71297 setSuppressionLevel  71298 getSuppressionLevel
71299 logDirectory        71300 logLevel
```
`KrispModule._initialize(initializationParams)` вызывается на уровне модуля, `getNcModels`/`getVadModels`/
`getNcModelFilename` добавлены в JS (`discord_krisp/index.js`, 1 088 Б) как Promise-обёртки.

Внутренний C-API Krisp SDK (static, символы/строки):
`krispGlobalInit`, `krispGlobalDestroy`, `krispGetVersion`, `krispCreateNcInt16`, `krispCreateNcFloat`,
`krispProcessNcInt16`, `krispProcessNcFloat`, `krispDestroyNc`, `krispGetNcSessionStats`,
`krispCreateVadInt16`, `krispCreateVadFloat`, `krispProcessVadInt16`, `krispProcessVadFloat`, `krispDestroyVad`.

### 4.3 Рантайм моделей: ONNX Runtime 1.14 + GRU + OpenBLAS

* ONNX: строка `https://github.com/microsoft/onnxruntime/blob/rel-1.14.0/docs/ORT_Format_Update_in_1.13.md`,
  `ai.onnx`, protobuf-дескрипторы `00101_descriptor.pb`/`00101_onnx_ml.pb`/`00102_*`,
  `onnxruntime/src/onnx_executable_network.cpp`.
* Сети (реальные имена классов из `build/<sha>/src/native/src/...`):
  `krisp_gru_executable_network_{base,common,factory}`, `krisp_gru_layer_norm_executable_network`,
  `krisp_gru_mini_executable_network`, `krisp_gru_modify_executable_network`,
  `krisp_gru_modify_vad_executable_network`, `krisp_gru_sep_dense_executable_network`,
  плюс runtime-версии (`gru_layer_norm_executable_network_runtime.cpp` и др.).
* NC-конвейер: `krisp_nc_df_{pre,post}processor.cpp`, `krisp_nc_maincleaner.cpp`, `krisp_nc_df_config_loader`,
  `krisp_nc_processor_registery`, `krisp-inference-engine`, `krisp-dsp`, `krisp-blas`, `krisp-mlops`.
* VADv2: `audio/frame_processors/vad-v2/src/vad_v2_{maincleaner,processor}.cpp`, `krisp-vad-v2-processor`,
  `krisp-audio-sdk-vad-c`. Есть также `src/feature/{adm,motion,vif,ssim,cambi}.c`-подобные файлы,
  но в krisp — только DSP-часть.
* **OpenBLAS статически**: непрерывный блок из 7 515 строк `openblas_verbose`, `openblas_block_factor`,
  `openblas_omp_adaptive_env`, `support_avx512_bf16`, `gotoblas_corename`, `sgemm_kernel_*`.
  В NEEDED OpenBLAS нет → вкомпилирован.
* OpenSSL статически (2 362 строки `NCONF_dump_bio`, `WPACKET_*`, `OPENSSL_INIT_*`).

Строки-статусы: `Initializing krisp SDK...`, `Krisp SDK initialized`, `Teardown`,
`Failed to setup Krisp module, error code: {}`, `Invalid noise cancellation model`,
`Invalid voice activity detection model`.

---

## 5. discord_utils.node — 19 МБ, ОС-интеграция

### 5.1 Назначение (исходники)

`src/input_events.cpp` + `src/input_events_linux.cpp` + `src/input_events_service_null.cpp`,
`src/process_observer.cpp` + `src/process_observer_linux.cpp`, `src/steam_observer.cpp`,
`src/battle_net_observer.cpp`, `src/executable_fingerprint.cpp`, `src/wayland_idle_manager.cpp`,
`src/xutils_linux.cpp`, `src/native_permissions_null.cpp`, `src/utils_bindings.cpp`,
`third_party/jsoncpp/**`.

### 5.2 NAPI-свойства (из плоской таблицы строк, порядок файла)

```
inputEventRegister  inputEventUnregister  inputWatchAll  inputSetFocused  inputGetRegisteredEvents
setCandidateGamesCallback  setProcessObserverUserId  getExecutableFingerprintForProcess
setObservedGamesCallback  setObservedGamesCallback2  setGameCandidateOverrides  setGameDetectionErrorCallback
setObserverDebugCallback  setGameDetectionCallback  notifyGameLaunched  setProcessPriority
getPidFromWindowHandle  getDiscordMemoryUsage  getDiscordMemoryUsageElectronRenderer
getDiscordMemoryPrivUsageElectronRenderer  getDiscordClipsCumulativeCpuUsage  getDiscordClipsPid
getWindowHandleFromPid  generateSessionFromPid  getAudioPid  setForegroundProcess  isSystemDarkMode
getDiscordIsElevated  nativePermssionRequestAuthorization  nativePermssionHasAuthorization
nativePermissionOpenSettings  pollQueueMetrics  isWaylandIdleAvailable  getWaylandSystemIdleTimeMs
```
(опечатка `Permssion` — в оригинале). Плюс C++-функции: `InputWatchAll`, `PollQueueMetrics`,
`InputEventRegister(vector<ScanCodeCombo>,…)`, `SetObservedGamesCallback(vector<ObservedProcess>,…)`,
`GetExecutableFingerprintForProcess(unsigned int, callback)`, `Init(Optional<Object>)`.

### 5.3 X11 + Wayland + PulseAudio

* **X11**: импорты `XOpenDisplay`, `XInternAtom`, `XGetWindowProperty`, `XQueryExtension`,
  `XISelectEvents`, `XIQueryDevice`, `XIQueryVersion`, `XNextEvent`, `XGetEventData`, `XFreeEventData`,
  `XSetErrorHandler`, `XPending`, `XFlush`, `XIFreeDeviceInfo`; внутренний `discord::TrapXErrors()`,
  `src/xutils_linux.cpp`.
* **Wayland**: `wl_display_connect/flush/read_events/prepare_read/dispatch_pending/disconnect`,
  `wl_registry`, `wl_seat`, `wl_proxy_marshal*`, `wl_proxy_add_listener`, `wl_display_roundtrip`;
  `src/wayland_idle_manager.cpp` — и именно его работу видно в логе модуля:
  `discord_utils.log: "Wayland idle manager shut down"` (12 записей за 18–19 окт).
* **PipeWire / xdg-desktop-portal в utils — не найдены** (0 совпадений). Портал используется только
  в `discord_voice`.

### 5.4 JS-прослойка (`discord_utils/index.js`, 4 726 Б)

```
nativeUtils.clearCandidateGamesCallback = nativeUtils.setCandidateGamesCallback;      // алиас
inputCaptureSetWatcher(nativeUtils.inputWatchAll); delete nativeUtils.inputWatchAll;  // в рендерере
nativeUtils.inputCaptureRegisterElement = inputCaptureRegisterElement;
nativeUtils.inputEventRegister   = wrapInputEventRegister(nativeUtils.inputEventRegister);
nativeUtils.inputEventUnregister = wrapInputEventUnregister(...);
nativeUtils.init({ logDirectory, logLevel });              // или nativeUtils.init()
nativeUtils.setServiceChannel?.(releaseChannel);           // win32
nativeUtils.getGPUDriverVersions = async () => {…}         // JS-переопределение через nvidia-smi.exe
nativeUtils.submitLiveCrashReport = async (channel, sentryMetadata) => {
   const path = nativeUtils._generateLiveMinidump(dataDirectory);
   … fetch('https://o64374.ingest.sentry.io/api/146342/minidump/?sentry_key=f11e8c3e62cb46b5a006c339b2086ba3')
}
nativeUtils.shouldDisplayNotifications = () => …           // JS-переопределение (win32 + getNotificationState)
```
Дополнительно `input_capture.js` (3 674 Б) реализует перехват последовательности ввода для биндов
(до 4 событий, таймаут 5 с, левая кнопка мыши исключается; на Linux код = 1), а `input_event.js` (2 014 Б) —
запрет одиночных символьных биндов на сканах Linux `[24..33], [38..46], [52..58]`.

---

## 6. Остальные модули (кратко)

| модуль | суть | ключевое доказательство |
|---|---|---|
| `discord_erlpack` | только **распаковка** ETF | NAPI-строки: `unpack`, `releaseAsBuffer`. `nm`: `Pack(discord::node_api::Value)`, `Unpack(discord::node_api::Buffer)`, `Encoder::pack(Value,int)`, `Decoder::unpack()`; исходники `src/bindings.cpp`, `src/decoder.h`; внутренние `erlpack_buffer_write`, `erlpack_append_nil/…`, `erlpack_append_small_integer`. Имя `pack` (4 буквы) не попало в `strings -n 6`. |
| `discord_game_utils` | детект игр, наблюдатели процессов, ярлыки | NAPI: `isProtocolSchemeRegistered` (win), `removeShortcuts` (win); исходники `src/game_utils_bindings.cpp`, `src/game_utils.cpp`, `src/game_utils_linux.cpp`; статический libsodium + jsoncpp. |
| `discord_zstd` | потоковая распаковка Zstandard, Rust | NEEDED: только libc; `rustc 1.99.0-nightly`. Rust v0-символы: `discord_zstd::bindings::DecompressStream` c `decompress`, `get_last_error`, `reset`; `initializeLogging`; пул napi-rs-ошибок (`FunctionExpectedPendingException…`); статический `ZSTD_decompressStream`. `index.js` держит **один** глобальный контекст и бросает `Attempting to use a stale zstd context`. |
| `updater.node` | апдейтер, Rust + встроенный JS/HTML UI | Rust v0-пути: `.../discord_updater/updater_client/src/{api,db,install,lib,query,repair}.rs`, `.../rust/{backoff-utils,delta_update,endsley_bsdiff,fs_utils,napi}`. Пути API: `/updater`, `/manifests/latest`, `/src/channel`, `/src/install`. Строки содержат упакованный словарь UI (`http://addEventListenerresponsible`, `HTTP/0.9HTTP/2.0HTTP/3.0trailersif-…`). `.symtab` отсутствует (stripped). |
| `cld.node` | определение языка (Compact Language Detector) | экспорты `CLD::*`: `GetLanguage`, `LanguageSummary`, `ScoringContext`, `OffsetMap`, `Tote`, `ScriptScanner`, таблицы `kLanguageToCode`, `kULScriptToName`; 197-строчный блок языков (`ABKHAZIAN, AFRIKAANS, …`); грузится из `discord_spellcheck/node_modules/cld`. |
| `notificationstate.node` | Windows-only заглушка | 16 840 Б, NAPI-входа нет, экспортов нет, в `index.js` utils импортируется безусловно (`require('windows-notification-state')`) — на Linux не используется. |

Связь с IPC: модули не создают собственных IPC-каналов, кроме namespace `NATIVE_MODULES_*`
(см. §7), которые полностью описаны в `backend-ipc.md` — здесь не дублируются.

---

## 7. Связь с JS — где реально вызывается нативный код

### 7.1 Где искать и где НЕ искать

| файл | содержит вызовы 110 нативных методов? |
|---|---|
| `app/bundle.js` (main, 3 066 301 Б) | **нет.** 13 совпадений — ложные: `createTransport`/`consoleLog` из Sentry SDK (`app:490`), `platform`/`initialize` — общие идентификаторы минифицированного кода |
| `core/bundle.js` (402 296 Б) | **нет.** `updateClipMetadata` в `core:1` — это локальная JS-функция `updateClipMetadata(filepath, metadata)` с `withDiscordMP4Lock`, а не нативный метод |
| `core/mainScreenPreload.js` (1 105 621 Б) | содержит **реестр** нативных модулей (модуль `7927`), но не вызовы API |
| `decoded/3eefa7dae1e2c234_0` и `decoded/4d2a64cb5df5fc4e_0` (12 410 287 Б, md5 идентичны) | **да — это основной потребитель**: 98/110 имён, 65 вызовов `(0,X.lE)().<метод>` |
| `decoded/d8b0ffc3255e8113_0` (587 591 Б) | 12 имён, но это Flux action-creators/UI-настройки, не нативные вызовы |

**Прямое следствие: main-процесс Discord (Electron main) не вызывает голосовой нативный API вообще.**
Цепочка: рендерер → `mainScreenPreload.js` (модуль 7927) → `require('discord_voice')` → `index.js` → `.node`.

### 7.2 Мост (`core/mainScreenPreload.js`, модуль 7927)

```js
// char 78292
exports.requireModule = function(name){
  if(!/^discord_[a-z0-9_-]+$/.test(name) && "erlpack"!==name || PRELOAD_ONLY_MODULES.has(name))
     throw Error('"'+String(name)+'" is not a whitelisted native module');
  return (0,requireModuleHook_1.runBeforeRequireModuleHook)(name), nativeModule.require(name) };
exports.getModulePath = function(name){ return electron.ipcRenderer.sendSync(NATIVE_MODULES_GET_MODULE_PATH,name) };
// ...
async function ensureModule(name){ modulePromises[name] ??= electron.ipcRenderer.invoke(NATIVE_MODULES_INSTALL,name); … }
let PRELOAD_ONLY_MODULES = new Set(["discord_premap"]);
```
Плюс: `PREMAP_MODULES = ["discord_voice","discord_zstd","discord_cloudsync","discord_media"]` с
`discord_premap.node` (`premapModules`, `dequeuePremap`) — предварительная подгрузка модулей.
`PREMAP_MODULES` **не существует на диске** для `discord_cloudsync`/`discord_media`/`discord_premap`.

### 7.3 Реальные вызовы (цитаты)

**Инициализация и логирование** (`decoded/3eefa7dae1e2c234_0`, char 9903732):
```js
getVoiceEngine(){
  if(__OVERLAY__) throw Error("cannot require discord_voice in overlay");
  let e=this.requireModule("discord_voice");
  return b||(0,_.si)((t,n,i)=>{ e.consoleLog(n, `[${t}] ${i}`) }), b=!0, e
}
```

**Krisp-путь** (там же, char 9172668 — цепочка `discord_krisp` → `discord_voice`):
```js
await T.Ay.ensureModule("discord_krisp"); let e=T.Ay.requireModule("discord_krisp");
tM=!0, u=e.getSdkVersion?.(), _=e.getSuppressionLevel?.()??100,
e.getNcModels?.().then(e=>{tU=e,i.emitChange()}), i.emitChange(),
await T.Ay.ensureModule("discord_voice"); let t=T.Ay.requireModule("discord_voice"); t.setupKrispPath?.()
```

**Параметры транспорта** (char 10804771):
```js
t.getEncryptionModes(i=>{ … t.setTransportOptions(this.getConnectionTransportOptions()),
  t.setSelfMute(this.selfMute||this.context===O.x.STREAM), t.setSelfDeafen(this.selfDeaf),
  t.setOnSpeakingCallback(this.handleSpeakingNative), … t.setPingCallback(this.handlePing) …
```

**MLS-ключ** (char 7452318):
```js
async function x(e){ let t=u.default.getStaticAuthSessionId();
  return a()(null!=t,"[getCurrentUserPublicKey] session id should not be null"), await _.Ay.getMLSSigningKey(t,e) }
```
Обёртка в чанке (char 10853900): `getMLSSigningKey(e,t){ … r.getMLSSigningKey(e,t,(e,t)=>n({key:e,signature:t})) … }`

**Выбор источника для стрима** (char 5104783):
```js
function a(e){ r.Ay.getMediaEngine()?.presentNativeScreenSharePicker?.(e),
  i.h.dispatch({type:"NATIVE_SCREEN_SHARE_PICKER_PRESENT",style:e}) }
function s(){ r.Ay.getMediaEngine()?.releaseNativeDesktopVideoSourcePickerStream?.(),
  i.h.dispatch({type:"NATIVE_SCREEN_SHARE_PICKER_RELEASE"}) }
```

**Статистика шумоподавления при дисконнекте** (char 2489503):
```js
Z.Ay.getKrispModel(),
el.Ay.getNoiseCancellationStats():Promise.resolve(null) … this.logger.log(`[VOICE_DISCONNECT] krisp_nc_model: ${n??"null"}`)
```
→ в трекинг уходят `krisp_nc_model`, `duration_{low,medium,high}_noise_detected_ms`.

**Аналитика качества звука** (char 539860): `A.Ay.getMediaEngine().getAudioSubsystem()` и
`.getAudioLayer()` попадают в трекинг `MEDIA_DEVICE_CHANGED` как `audio_subsystem`, `audio_layer`.

**Адаптеры `applyMediaFilterSettings` / `getNextVideoOutputFrame`** (char 10853900 и 2255973):
```js
applyMediaFilterSettings(e){ let{applyMediaFilterSettings:t,applyMediaFilterSettingsWithCallback:n}=(0,b.lE)();
  return null!=n?new Promise((t,i)=>{n(e,t)}):(null!=t&&t(e),Promise.resolve()) }
// …
n.getNextVideoOutputFrame(e).then(function(e){ try{ null!=e&&t(e)&&i(e) }catch(e){r(e)} },t=>{ d===e&&r(t) })
```

**Fallback-реализация.** Рядом лежит класс-заглушка (`getCodecCapabilities(e){e("")}`,
`startLocalAudioRecording(){return Promise.reject(Error("startLocalAudioRecording is not supported."))}`,
`getMLSSigningKey(e,t){return Promise.reject(Error("NOT_IMPLEMENTED"))}`) — используется, когда нативный
модуль недоступен; поэтому «отсутствие» метода в живом списке не означает отсутствие фичи.

**erlpack в рантайме** (`decoded/3eefa7dae1e2c234_0`, char ~9180000):
```js
try{ e=M.Ay.requireModule("discord_erlpack") }catch{ try{ e=M.Ay.requireModule("erlpack") }catch{ e=void 0 } }
if(null!=e) return class{ static canUse(){return null!=e}
  pack(t){ … this._useWetf ? eu.packToArrayBuffer(t) : e.pack(t).buffer }
  unpack(t){ … }
```
→ есть более новый JS-энкодер (`EtfEncoding`, `packToArrayBuffer`), который **вытесняет** нативный `pack`;
нативный `unpack` остаётся.

---

## 8. Отброшено (шум)

| что | сколько | пояснение |
|---|---:|---|
| генерик-строки libc/libstdc++/libcxx, Itanium-мангалы `_Z*`, пути `third_party/llvm/libcxx/include` | **309 575** суммарно по 9 модулям | фильтр `native-re/build-json.py`; из них `discord_voice` — 168 063, `discord_krisp` — 58 297, `discord_utils` — 40 386 |
| символы LLVM-обезличивания `.llvm.` | 0 в этих модулях | внутренний мангалинг есть только у `gpu_encoder_helper` (не входит в задачу) |
| дубликаты файлов `obj.target/*.node` | 2 | `cld.node`, `notificationstate.node` — md5 совпадают с основными копиями |
| повторные строки `.rodata` vs `.data` от `rabin2 -z` | не считались как дубли | `rabin2 -z` выдаёт каждую секцию отдельно |
| ложные срабатывания JS-grep по main-процессу | 13 | Sentry SDK (`createTransport`, `consoleLog`), Flux action-creators (`setInputVolume`, `setDebugLogging`, …), локальная `updateClipMetadata` в `core` |
| «шумные» camelCase-блоки (не NAPI-таблицы) | 6+6 блоков | `AV_PICTURE_TYPE_*` (902), `ASN1_*`/`OPENSSL_*` (705/2362), OpenBLAS (7515), `UV_FS_*` (151), `ABKHAZIAN…` (197, cld) — это enum'ы и пулы строк библиотек, а не API модуля; опознаны по размеру и содержимому и исключены из таблиц §3.2/§4.2 |

Фильтр считает «отфильтрованным» строку, начинающуюся с `std::`, `__gnu_cxx`, `__cxa_`, `_Z…`,
пути `third_party/llvm/libcxx/include`, `GCC:`, `GLIBC_`/`GLIBCXX_`/`CXXABI_`, `.comment/.text/…`.

---

## 9. Ограничения и нерешённые вопросы

1. **Не запускалось ничего**: ни Discord, ни `ldd` живого процесса, ни Frida. Поэтому «какие символы
   реально резолвятся» не проверено — только статические `UND`/`NEEDED`/dlopen-строки.
2. **FFmpeg-импорты не разрешены** (§2.2): `avcodec_*` есть как UND, но ни один из проанализированных
   ELF их не предоставляет в глобальном скоупе. Требует проверки в рантайме (`/proc/<pid>/maps`).
3. **Расхождение версий JS и нативного кода.** JS-чанк из кэша (`decoded/`) содержит 13 имён, которых
   нет в бинаре 1.0.162. Это либо новее релиза, либо иной канал. Абсолютной даты сборки чанка нет.
4. **`strings -n 6`** отбрасывает короткие имена (`pack`, `init`); там, где это важно, использовался `nm`.
5. **Rust-модули** (`discord_zstd`, `updater`) держат NAPI-имена в пуле строк, а не по одному на строку —
   автоматическая нарезка плоской таблицы для них не работает, использованы `nm` + точечные проверки.
6. **Модулей, которые JS требует, но которых нет на диске**: `discord_sysimg`, `discord_media`,
   `discord_cloudsync`, `discord_clips`, `discord_premap`, `discord_overlay2`, `discord_hook`,
   `discord_dispatch`, `discord_ml`, `discord_updater_bootstrap`. Часть из них (`discord_clips`,
   `discord_media`, `discord_ml`) упоминается и как пути (`setClipsModulePath`, `setMLPath`) — значит
   поставляются отдельно, возможно как отдельные загрузки/обновления.
7. **`krisp-bvc-o-pro-v3.kef`** (15 МБ) — самый большой артефакт моделей; в JS-строках он не упомянут,
   назначение не установлено.
8. **`KMS/logs/`** рядом с krisp — создан при установке, строка `KMS` в бинаре отсутствует; назначение
   не установлено (вероятно, журнал Krisp SDK).
9. **Опечатка в API**: `nativePermssionRequestAuthorization`, `nativePermssionHasAuthorization` — так
   названы функции и в бинаре, и в JS; переименование сломало бы совместимость.
10. `index.js` содержит мёртвую ветку `RUN_MEDIA_HOST = false` → `require('./MediaHost')` никогда не
    исполняется; путь `MediaHost` на диске отсутствует.
