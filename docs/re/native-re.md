# Native Modules of Discord Desktop 1.0.162 (Linux x64) — Static Analysis and JS Bindings

**Scope.** Statically analyze `.node` binary modules in the `app-1.0.162` release and trace bindings to main/preload JavaScript code. Non-invasive, read-only analysis without live execution or network requests.

**Inspected Assets (Read-Only):** `app-1.0.162/**`, `docs/re/asar-extract/**`, `docs/re/decoded/**` (cached JS chunks), `docs/re/voice/ui/03-voice-module-api.json` (live sample of 110 identifiers), `docs/re/backend-ipc.md` (IPC topology reference).

**Artifacts Generated:** Raw command outputs in `docs/re/native-re/raw/<module>/*.txt`, summary analysis in `native-re.md` (this document) and `native-re.json`.

---

## 1. Methodology

For each distinct `.node` binary (9 unique files; `cld.node` and `notificationstate.node` reside redundantly under `build/Release/` and `build/Release/obj.target/` with identical checksums):

| Step | Tool Command | Raw Output File |
|---|---|---|
| Binary metadata | `rabin2 -I` | `rabin2-info.txt` |
| Import table | `rabin2 -i` | `rabin2-imports.txt` |
| Export table | `rabin2 -E` | `rabin2-exports.txt` |
| Data section strings | `rabin2 -z` | `rabin2-strings.txt` |
| Full file strings | `strings -a -n 6` | `strings-a-n6.txt` |
| Dynamic section | `readelf -dW` | `readelf-dyn.txt` |
| Segment table | `readelf -lW` | `readelf-segments.txt` |
| Demangled symbols | `nm -D --demangle`, `nm --demangle` | `nm-dynamic-demangled.txt`, `nm-all-demangled.txt` |
| File type inspection | `file` | `file.txt` |

### 1.1 String Extraction Caveats

`rabin2 -z` inspects solely `.rodata` and `.data` sections, excluding `.text`. For `discord_voice`:
- `rabin2 -z`: 113,402 strings
- `strings -a -n 6` (full file): 410,806 strings
Sections breakdown: `.rodata` ~21k, `.data` ~53k, `.text` (22 MB code) ~244k. Categorical extractions in this report are based on `strings -a -n 6` with `rabin2 -z` preserved as baseline reference.

### 1.2 Symbol Demangling

`nm --demangle` resolves Itanium (`_ZN…`) and Rust v0 (`_R…`) mangling schemes. Rust modules (`discord_zstd`, `updater`) demangle cleanly via `nm`.

---

## 2. Module Summary

| Module | Size (Bytes) | MD5 (16) | NEEDED | Strings (`-n 6`) | Filtered Generic | NAPI Entrypoint |
|---|---:|---|---|---:|---:|---|
| `discord_voice` | 126,136,024 | `5fe80ce0f5d89fd5` | 13 libs | 410,806 | 168,063 | `napi_register_module_v1` (symtab) |
| `discord_krisp` | 62,078,048 | `9be929d1d90a8290` | 6 libs | 164,989 | 58,297 | `napi_register_module_v1` (symtab) |
| `discord_utils` | 19,185,488 | `6bd62709ff518d50` | 7 libs | 65,723 | 40,386 | `napi_register_module_v1` (symtab) |
| `discord_game_utils` | 11,478,904 | `220450ece4bf9cf4` | 5 libs | 38,939 | 21,541 | `napi_register_module_v1` (symtab) |
| `discord_erlpack` | 7,755,832 | `a4bf9140d20dbf17` | 5 libs | 27,776 | 15,941 | `napi_register_module_v1` (symtab) |
| `cld.node` (spellcheck) | 6,646,552 | `303bc2870515fe61` | 5 libs | 7,359 | 910 | `napi_register_module_v1` (symtab) |
| `updater.node` | 5,812,960 | `6a77646b1d9e2b9c` | 6 libs | 13,188 | 24 | `napi_register_module_v1` (dynsym) |
| `discord_zstd` | 873,248 | `e6fb396136f1e24c` | 5 libs | 2,772 | 18 | `napi_register_module_v1` (symtab) |
| `notificationstate` | 16,840 | `63ad44ea5fec4949` | 5 libs | 58 | 5 | — (Linux stub) |

Note: `discord_voice.node` on disk is 126 MB (37 MB represents in-memory mapped `PT_LOAD.memsz`), and `discord_krisp.node` is 62 MB (48 MB mapped).

### 2.1 Complete Dependency Listing

* **`discord_voice`**: `libmediapipe.so`, `libX11.so.6`, `libwayland-client.so.0`, `libpulse.so.0`, `libasound.so.2`, `libatomic.so.1`, `librt.so.1`, `libdl.so.2`, `libpthread.so.0`, `libm.so.6`, `libgcc_s.so.1`, `libc.so.6`, `ld-linux-x86-64.so.2`; `SONAME discord_voice.node`, `RUNPATH $ORIGIN`.
* **`discord_krisp`**: `libstdc++.so.6` + standard libc suite. **OpenBLAS and OpenSSL are statically compiled into the binary** (absent from NEEDED).
* **`discord_utils`**: `libX11.so.6`, `libwayland-client.so.0` + libc suite.
* **`discord_game_utils` / `discord_erlpack` / `cld` / `notificationstate`**: libc + `libstdc++`.
* **`discord_zstd` / `updater`**: libc suite only; toolchain: `rustc 1.99.0-nightly (73dc9167f 2026-08-01)`.

### 2.2 Dynamically Loaded Libraries (`dlopen`)

`discord_voice` omits direct dynamic link references (NEEDED) to: PipeWire, Vulkan, VA-API, CUDA, NVENC, EGL/GLES, libGL, FFmpeg.
String tables reveal runtime `dlopen` resolution targets:

```
libpipewire-0.3.so.0   libpulse.so.0        libasound.so.2        libvulkan.so.1
libva.so.2            libva-drm.so.2        libcuda.so            libnvidia-encode.so.1
libEGL.so.1           libGLESv2.so.2        libGL.so.1            libwayland-client.so.0
libmediapipe.so
```

Dynamic trampolines: `pw_stream_flush_ptr`, `pw_stream_get_name_ptr`, `pw_stream_set_control_ptr`, `pw_stream_set_error_ptr`, `pw_stream_get_properties_ptr`, `InitializePipewire`, `IsPipewireInitialized`, `UninitializePipewire`.
PipeWire is initialized dynamically and fails gracefully if absent.

---

## 3. `discord_voice.node` — Core Voice and Video Engine (126 MB)

### 3.1 Code Architecture from Source References

839 unique `src/**` file paths extracted from logging identifiers:

| Subsystem | Source Modules (Selected) |
|---|---|
| Core Networking | `src/discord.cpp`, `src/voice_connection.cpp`, `src/state.cpp`, `src/bindings.cpp`, `src/messages.cpp`, `src/socket_client.cpp`, `src/udp_socket.cpp`, `src/credential.cpp` |
| Media Core | `src/media/engine.cpp`, `src/media/connection.cpp`, `src/media/local_user.cpp`, `src/media/remote_user.cpp`, `src/media/user.cpp` |
| Audio Devices | `src/media/audio_device/*` (`webrtc_adm_helper.cpp`, `device_selection.cpp`, `local_audio_combiner.cpp`, `wav_file_audio.cpp`, `sine_audio.cpp`, `silent_audio.cpp`) |
| Opus Codec | `src/src/opus_encoder.c`, `opus_decoder.c`, `opus_multistream_encoder.c`, `src/celt/*` (18 files), `src/silk/*` (43 files), `src/silk/x86/NSQ_sse4_1.c` |
| Cryptography | `src/libsodium/**` (~90 files), `src/crypto/**` (BoringSSL/OpenSSL), `src/cryptor.cpp`, `src/boringssl_cryptor.cpp`, `src/hpke.cpp`, `src/dhkem.cpp`, `src/aead_cipher.cpp` |
| MLS / DAVE E2EE | `src/mls/session.cpp`, `src/mls/parameters.cpp`, `src/mls/user_credential.cpp`, `src/mls/key_serialization.cpp`, `src/mls/persisted_key_pair.cpp`, `src/treekem.cpp`, `src/tree_math.cpp`, `src/mls_key_ratchet.cpp`, `src/key_schedule.cpp`, `src/group.cpp`, `src/signature.cpp` |
| Video Coding | `src/media/video_coding/av1.cpp`, `decoding/h264_decoder_impl.cpp`, `encoding/webrtc_encoder_factory.cpp`, `encoding/linux/encoder_factory.cpp`, `decoding/pixel_capped_decoder.cpp` |
| Screenshare | `src/media/videoshare/capture_broker.cpp`, `screenshare_helper.cpp`, `screenshare_capture_loop.cpp`, `webrtc_screenshare_source.cpp`, `wumpus.cpp`, `external_video_source.cpp` |
| NDI Integration | `src/media/ndi/ndi_process_bridge.cpp`, `ndi_process_bridge_posix.cpp`, `ndi_video_forwarder.cpp`, `ndi_video_source.cpp` |
| Clips Recording | `src/media/clips/clips_process_bridge.cpp`, `clips_video_forwarder.cpp`, `clips_process_bridge_null.cpp` |
| Video Segmentation | `src/media/filters/media_pipe_filter_impl.cpp`, `background_blur_filter.cpp`, `background_replacement_filter.cpp`, `media_filter_manager.cpp`, `gles_rgb_converter.cpp` |
| Linux Sound | `src/media/soundshare/soundshare_audio_device_linux.cpp`, `pulseaudio_controller.cpp`, `controller.cpp`; `src/audio_device_linux.cpp` |
| VAD / AEC | `src/vad_observer.cpp`, `src/media/echo_canceller_factory.cpp`, `echo_canceller_probe_bank.cpp`, `frame_activity_detector.cpp`, `volume_analyzer.cpp` |
| Video Quality | `src/media/image_quality_vmaf_manager.cpp` + `src/feature/**` (libvmaf, ~70 files), `vmaf_v0.6.1.json` |
| Spatial Audio | `src/media/spatial_mixer.cpp`, `spatial_audio_options.cpp`, `reflections.cpp`, `third_party/steam_audio_src/**` (364 paths) — Steam Audio (Phonon) |
| NAPI Bindings | `src/bindings.cpp`, `src/core_types.cpp`, `src/filter_settings.cpp`, `src/media/codec_survey/*`, `src/transport_settings.cpp`, `src/userinfo_vc.cpp` |

Build chain: GCC 9.3.0 + Clang 16.0.6 + LLVM `libc++` (`std::__Cr`).

### 3.2 NAPI Properties Table

A contiguous block of 94 property identifiers resides in `.rodata` (lines 73279–73372 of `strings-a-n6.txt`).
Bound classes registered separately:
- `VoiceConnection`
- `VoiceReplayConnection`

### 3.3 Reconciliation with Live 110-Property Sample

Comparison between the live sample and binary symbols:
- Sample contains 110 identifiers.
- Direct intersection with contiguous NAPI table: **92 identifiers**.
- Sequence order matches exactly.
- 2 properties present in binary but absent from runtime sample: `setVideoOutputSink`, `signalVideoOutputSinkReady`.
- 18 properties present in sample but absent from binary: All 18 are wrapper utilities defined in JavaScript by `modules/discord_voice-1/discord_voice/index.js` (e.g. `createTransport`, `setupKrispPath`, `setupMLPath`, `setDebugLogging`).
- Conclusion: The 110-property catalog reflects `Object.keys()` of the JavaScript wrapper module rather than bare C++ NAPI exports.

### 3.4 Capability Detection

JavaScript client chunks access the native engine through accessor patterns with capability probes (`lE()`):
51 calls match symbols in release 1.0.162, while 14 identifiers (`getDeviceOSMuted`, `setAsyncVideoInputDeviceInit`, `setNativeThreadsPriority`, etc.) represent newer web client capabilities that fail gracefully via optional chaining (`?.`).

### 3.5 Confirmed Engine Subsystems

* **MLS / DAVE Protocol (E2EE):** Cisco `mlspp` implementation (RFC 9420) with frame encryptors for Opus, VP8, VP9, H.264, H.265, AV1.
* **Krisp Integration:** Native wrapper managing model setup, VAD processing, and session metrics.
* **MediaPipe Filters:** Background blur and replacement via TFLite segmentation graphs (`selfie_segmentation.tflite`).
* **Hardware Video Codecs:** VA-API via `cros_codecs`, NVENC via Rust media encoding, Vulkan Video compute shader `fused_scale_to_nv12.glsl`, accompanied by `gpu_encoder_helper`.
* **Wayland & PipeWire Desktop Capture:** Desktop sharing via `xdg-desktop-portal` DBus interfaces and PipeWire stream pipelines, with custom capture support for Gamescope.
* **Spatial Audio Engine:** Full integration of Steam Audio (Phonon) with HRTF / SOFA support and binaural effects via `src/media/spatial_mixer.cpp` and `spatial_audio_options.cpp`. In JavaScript wrapper modules, spatial processing is initialized when `VoiceEngine.isSpatialAudioEnabled()` returns true, and audio topology status changes trigger `setSpatialAudioStatusCallback`.
* **Network & BWE Congestion Control:** Bandwidth estimation via GCC, REMB, and TWCC (`http://www.ietf.org/id/draft-holmer-rmcat-transport-wide-cc-extensions-01`), trendline filters with `kDiscordBandwidthEstimationExperimentTrendlineWindowDuration`, RTP abs-capture-time (`http://www.webrtc.org/experiments/rtp-hdrext/abs-capture-time`), probing engine, and jitter buffer flushing (`SetIdleJitterBufferFlush`).

### 3.6 Full Listing of 94 Exported NAPI Properties

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

---

## 4. `discord_krisp.node` — Audio Processing and VAD (62 MB)

### 4.1 Bundled Deep Learning Models

| Model File | Size (Bytes) | SHA-256 |
|---|---:|---|
| `krisp-bvc-o-pro-v3.kef` | 15,122,004 | `3f65d975ee4487931aff462e71e740a37b581817f9bad860590b194409688859` |
| `krisp-nc-o-lite-v1.kef` | 2,895,574 | `7fdff67057090f402fb7688b29bf0afbd65cb1ed6a6956d2e304b5f3d84afa48` |
| `krisp-nc-o-med-v7.kef` | 5,880,640 | `1b421bd27f892c6aef041496c939da4f31360fda36d55b83e48435cae89d246f` |
| `krisp-nc-o-nb-v2.kef` | 2,822,586 | `b3d2737fae2e6f900f7efb33aa17a3b6e6548e48ebcfbf5bd2f0746eec04c086` |
| `krisp-vad-o-v2.kef` | 592,233 | `092c397f12443e5f2bdd07428b9e193ce359f6a08bfb403807c540f18647b3f3` |

### 4.2 Runtime Architecture: ONNX Runtime 1.14 + OpenBLAS

Contains embedded ONNX Runtime 1.14 with GRU neural network pipelines (`krisp_gru_executable_network_base`, `krisp_gru_layer_norm_executable_network`).
Statically incorporates OpenBLAS (7,515 internal BLAS symbols) and OpenSSL. Execution targets CPU exclusively (`CPUExecutionProvider`).

---

## 5. `discord_utils.node` — Desktop Integration (19 MB)

NAPI exports include:
`inputEventRegister`, `inputEventUnregister`, `inputWatchAll`, `inputSetFocused`, `inputGetRegisteredEvents`,
`setCandidateGamesCallback`, `setObservedGamesCallback`, `setObservedGamesCallback2`, `notifyGameLaunched`, `getWindowHandleFromPid`,
`setProcessObserverUserId`, `getExecutableFingerprintForProcess`, `setGameDetectionCallback`, `getPidFromWindowHandle`, `getDiscordMemoryUsage`,
`setForegroundProcess`, `nativePermssionRequestAuthorization`, `isWaylandIdleAvailable`, `getWaylandSystemIdleTimeMs`.

Linux subsystems:
- X11 event hooks via `XInput2` and `XGetWindowProperty`;
- Wayland idle tracking via `wayland_idle_manager.cpp`;
- Process monitoring and game executable detection.

---

## 6. Peripheral Native Modules

* **`discord_erlpack`**: ETF term decoding via `unpack`. No serialization exports.
* **`discord_game_utils`**: Game detection, URI scheme handlers, embedded `libsodium` crypto.
* **`discord_zstd`**: Streaming decompression using Rust `napi-rs` wrapper over `libzstd`.
* **`updater.node`**: Stripped WebAssembly-compiled Rust updater with SQLite database and HTTP client.
* **`cld.node`**: Google Compact Language Detector 2 (CLD2) for language identification.
* **`notificationstate.node`**: Windows-only notification state stub; inactive on Linux.

---

## 7. JavaScript Loading Architecture

Main process execution does not call the voice engine directly. Instead:
`Renderer UI` -> `mainScreenPreload.js` (Module 7927) -> `requireNative('discord_voice')` -> `index.js` -> `discord_voice.node`.
Module requests pass through a security whitelist (`/^discord_[a-z0-9_-]+$/`).

---

## 8. Filtered Redundancies

During extraction, 309,575 generic compiler symbols (C++ STL templates, OpenBLAS lookup tables, LLVM metadata) were filtered out to expose genuine Discord business logic and API surfaces.

---

## 9. Artifact Directory

| Path | Contents |
|---|---|
| `docs/re/native-re.json` | Complete machine-readable symbol and API catalog |
| `docs/re/native-re/raw/*.txt` | Rabin2 and strings raw outputs per module |
| `docs/re/nm/*.txt` | Demangled symbol tables |
