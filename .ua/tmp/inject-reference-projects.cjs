const fs = require("fs");
const path = "/home/sanya/orca/project/.ua/knowledge-graph.json";
const g = JSON.parse(fs.readFileSync(path, "utf8"));

// Remove any previously injected reference-projects nodes/edges/layer/tour so this script is idempotent
const refPrefix = "docs/reference-projects/";
g.nodes = g.nodes.filter(n => !(n.filePath && n.filePath.startsWith(refPrefix)));
const validIds = new Set(g.nodes.map(n => n.id));
g.edges = g.edges.filter(e => !e.source.includes(refPrefix) && !e.target.includes(refPrefix));
g.layers = g.layers.filter(l => l.id !== "layer:reference-projects");
g.tour = g.tour.filter(t => t.order !== 13);

const newNodes = [
  // === 1. Master Reference Documentation Nodes ===
  {
    id: "document:docs/reference-projects/README.md",
    type: "document",
    name: "README.md (Эталонные проекты: Serein, LemonCord, rust-discord-activity)",
    filePath: "docs/reference-projects/README.md",
    summary: "Сводный индекс и кросс-проектная матрица эталонных проектов (ViceVerse-cz/Serein, japandotorg/LemonCord, rust-discord-activity v0.3.1 DiscordClient). Сопоставляет подсистемы discord-lab (Go Live Op 18..22, RTP/SRTP/DAVE E2EE, H.264/AV1 захват и декодирование, wgpu YUV шейдеры, WebView, Discord RPC IPC) с конкретными файлами и функциями эталонных реализаций.",
    complexity: "moderate",
    tags: ["эталонные-проекты", "serein", "lemoncord", "rust-discord-activity", "документация", "карта-вызовов", "сравнение"]
  },
  {
    id: "document:docs/reference-projects/SEREIN-FUNCTION-MAP.md",
    type: "document",
    name: "SEREIN-FUNCTION-MAP.md (Карта кода и вызовов ViceVerse-cz/Serein)",
    filePath: "docs/reference-projects/SEREIN-FUNCTION-MAP.md",
    summary: "Полная архитектурная документация, графы вызовов и пофункционное описание нативного Rust-клиента Serein (11 крейтов: serein-desktop, discord-voice, discord-gateway, platform, media, storage, voice-crypto, discord-api, discord-types). Описывает двойной сокет Голос+Go Live, RTP/RTCP, RTX/NACK/PLI, H.264/AV1 депакетизацию, GStreamer/VideoToolbox/MediaFoundation и рендер NV12/I420 через wgpu.",
    complexity: "complex",
    tags: ["serein", "go-live", "webrtc", "rtp", "srtp", "dave-e2ee", "wgpu", "gstreamer", "карта-функций"]
  },
  {
    id: "document:docs/reference-projects/LEMONCORD-FUNCTION-MAP.md",
    type: "document",
    name: "LEMONCORD-FUNCTION-MAP.md (Карта кода и вызовов japandotorg/LemonCord)",
    filePath: "docs/reference-projects/LEMONCORD-FUNCTION-MAP.md",
    summary: "Подробная карта архитектуры, граф вызовов и описание всех функций клиента LemonCord (Rust + tao + wry WebView). Документирует цикл событий tao EventLoop, инициализацию WebViewBuilder с User-Agent и навигационным перехватчиком внешних ссылок через open::that, горячие клавиши и логирование.",
    complexity: "simple",
    tags: ["lemoncord", "webview", "wry", "tao", "desktop-wrapper", "карта-функций"]
  },
  {
    id: "document:docs/reference-projects/RUST-DISCORD-ACTIVITY-MAP.md",
    type: "document",
    name: "RUST-DISCORD-ACTIVITY-MAP.md (Карта кода и протокола rust-discord-activity DiscordClient)",
    filePath: "docs/reference-projects/RUST-DISCORD-ACTIVITY-MAP.md",
    summary: "Полная спецификация бинарного протокола Discord Local RPC IPC ([u32 LE opcode][u32 LE length][JSON]), граф вызовов и пофункционный разбор структуры DiscordClient из крейта rust-discord-activity v0.3.1, а также серверной реализации IPC-слушателя discord-ipc-0 в Serein (game_activity.rs).",
    complexity: "moderate",
    tags: ["rust-discord-activity", "discord-ipc", "rich-presence", "DiscordClient", "unix-socket", "карта-функций"]
  },
  {
    id: "document:docs/reference-projects/SEREIN-FULL-SYMBOL-INDEX.txt",
    type: "document",
    name: "SEREIN-FULL-SYMBOL-INDEX.txt (Полный индекс символов и строк Serein)",
    filePath: "docs/reference-projects/SEREIN-FULL-SYMBOL-INDEX.txt",
    summary: "Автоматически сгенерированный полный каталог всех структур, перечислений, типажей и функций во всех 11 крейтах репозитория ViceVerse-cz/Serein с точными номерами строк определения.",
    complexity: "moderate",
    tags: ["serein", "индекс-символов", "справочник", "функции", "структуры"]
  },
  {
    id: "document:docs/reference-projects/sources/Serein/docs/architecture.md",
    type: "document",
    name: "Serein docs/architecture.md",
    filePath: "docs/reference-projects/sources/Serein/docs/architecture.md",
    summary: "Официальный архитектурный обзор проекта Serein: разделение воркспейса на 11 крейтов, поток данных от Discord Gateway (JSON/ETF + zlib-stream) и REST v10 через mpsc-каналы к GPUI-интерфейсу и SQLite-кэшу.",
    complexity: "moderate",
    tags: ["serein", "архитектура", "воркспейс", "gpui", "gateway"]
  },
  {
    id: "document:docs/reference-projects/sources/Serein/docs/voice.md",
    type: "document",
    name: "Serein docs/voice.md",
    filePath: "docs/reference-projects/sources/Serein/docs/voice.md",
    summary: "Официальная спецификация голосового и видео-стека Serein: сигнализация Voice Gateway v8, UDP IP Discovery, режимы шифрования (xsalsa20, aead_xchacha20_poly1305_rtpsize, DAVE MLS E2EE), захват экрана и аудио, пакетизация H.264/AV1 и GPU-рендеринг.",
    complexity: "complex",
    tags: ["serein", "voice", "go-live", "srtp", "dave", "video-pipeline"]
  },

  // === 2. Serein Source Files, Structs & Functions ===
  {
    id: "file:docs/reference-projects/sources/Serein/apps/desktop/src/gpu_video.rs",
    type: "file",
    name: "gpu_video.rs (Serein wgpu NV12/I420 -> RGBA GPU Renderer)",
    filePath: "docs/reference-projects/sources/Serein/apps/desktop/src/gpu_video.rs",
    summary: "Аппаратный GPU-конвертер видеокадров Serein на базе wgpu. Принимает декодированные плоскости NV12 или I420, загружает их в R8Unorm/Rg8Unorm текстуры через write_texture и выполняет цветовую конверсию BT.709/BT.601 YUV -> BGRA8Unorm в WGSL-шейдерах без участия CPU.",
    complexity: "complex",
    tags: ["serein", "wgpu", "nv12", "i420", "yuv-to-rgb", "gpu-rendering", "wgsl"]
  },
  {
    id: "class:docs/reference-projects/sources/Serein/apps/desktop/src/gpu_video.rs:Converter",
    type: "class",
    name: "Converter (Serein gpu_video)",
    filePath: "docs/reference-projects/sources/Serein/apps/desktop/src/gpu_video.rs",
    lineRange: [13, 35],
    summary: "Хранит wgpu::Device, wgpu::Queue, скомпилированные RenderPipeline для NV12 и I420, BindGroupLayout и линейный Sampler для конвертации видеокадров на GPU.",
    complexity: "moderate",
    tags: ["serein", "wgpu", "render-pipeline", "nv12", "i420"]
  },
  {
    id: "function:docs/reference-projects/sources/Serein/apps/desktop/src/gpu_video.rs:upload_gpu",
    type: "function",
    name: "upload_gpu",
    filePath: "docs/reference-projects/sources/Serein/apps/desktop/src/gpu_video.rs",
    lineRange: [295, 421],
    summary: "Загружает плоскости Y/UV (NV12) или Y/U/V (I420) в текстуры wgpu через queue.write_texture, запускает RenderPass с шейдером fs_nv12/fs_i420 и возвращает готовую GPU-текстуру кадра.",
    complexity: "complex",
    tags: ["serein", "wgpu", "write_texture", "render-pass", "zero-cpu-color"]
  },

  {
    id: "file:docs/reference-projects/sources/Serein/apps/desktop/src/watch.rs",
    type: "file",
    name: "watch.rs (Serein Go Live Stream Viewer State)",
    filePath: "docs/reference-projects/sources/Serein/apps/desktop/src/watch.rs",
    summary: "Контроллер просмотра чужой демонстрации экрана (Go Live) в Serein. Управляет жизненным циклом зрителя (Idle, Connecting, Watching, Reconnecting), атомарным буфером последнего кадра Arc<Mutex<Option<VideoFrame>>> и polling-циклом на 60 FPS (16 мс).",
    complexity: "moderate",
    tags: ["serein", "go-live", "viewer", "60fps", "frame-polling"]
  },
  {
    id: "function:docs/reference-projects/sources/Serein/apps/desktop/src/watch.rs:store_frame",
    type: "function",
    name: "store_frame",
    filePath: "docs/reference-projects/sources/Serein/apps/desktop/src/watch.rs",
    lineRange: [54, 65],
    summary: "Колбэк приёмника видеокадров: сохраняет самый свежий декодированный VideoFrame в Arc<Mutex<Option<VideoFrame>>>, перезаписывая устаревший кадр без накопления очереди и задержки.",
    complexity: "simple",
    tags: ["serein", "go-live", "latest-frame", "zero-lag"]
  },
  {
    id: "function:docs/reference-projects/sources/Serein/apps/desktop/src/watch.rs:Watch::poll",
    type: "function",
    name: "Watch::poll",
    filePath: "docs/reference-projects/sources/Serein/apps/desktop/src/watch.rs",
    lineRange: [158, 226],
    summary: "Таймер зрителя (каждые 16 мс / 60 FPS): забирает кадр из shared.lock().take(), вызывает gpu_video::upload_gpu для конвертации на GPU или fallback_rgba и инициирует перерисовку окна cx.notify().",
    complexity: "moderate",
    tags: ["serein", "go-live", "60fps", "poll", "gpu-upload"]
  },

  {
    id: "file:docs/reference-projects/sources/Serein/apps/desktop/src/screen.rs",
    type: "file",
    name: "screen.rs (Serein Outgoing Screen Share UI Controller)",
    filePath: "docs/reference-projects/sources/Serein/apps/desktop/src/screen.rs",
    summary: "Контроллер исходящей демонстрации экрана в Serein: перечисление мониторов и окон, выбор пресета разрешения/FPS/битрейта, запуск голосового стрима и локальное превью на 30 FPS.",
    complexity: "moderate",
    tags: ["serein", "screen-share", "go-live", "preview"]
  },

  {
    id: "file:docs/reference-projects/sources/Serein/apps/desktop/src/game_activity.rs",
    type: "file",
    name: "game_activity.rs (Serein Discord RPC IPC Server & Process Scanner)",
    filePath: "docs/reference-projects/sources/Serein/apps/desktop/src/game_activity.rs",
    summary: "Серверная реализация локального сокета discord-ipc-0..9 (Unix socket и WebSocket 6463..6472) в Serein. Принимает подключения от игр и библиотек вроде rust-discord-activity, разбирает 8-байтные заголовки [op][len], отвечает READY на Handshake (Op 0) и транслирует SET_ACTIVITY в статус пользователя.",
    complexity: "complex",
    tags: ["serein", "discord-ipc", "rpc-server", "rich-presence", "set-activity"]
  },
  {
    id: "function:docs/reference-projects/sources/Serein/apps/desktop/src/game_activity.rs:serve_ipc",
    type: "function",
    name: "serve_ipc",
    filePath: "docs/reference-projects/sources/Serein/apps/desktop/src/game_activity.rs",
    lineRange: [446, 515],
    summary: "Создаёт UnixListener на сокете discord-ipc-0 в XDG_RUNTIME_DIR/TMPDIR и обрабатывает входящие клиентские RPC-сессии.",
    complexity: "moderate",
    tags: ["serein", "discord-ipc", "unix-socket", "server"]
  },

  {
    id: "file:docs/reference-projects/sources/Serein/crates/discord-voice/src/transport.rs",
    type: "file",
    name: "transport.rs (Serein Voice & Go Live Dual-Session Transport)",
    filePath: "docs/reference-projects/sources/Serein/crates/discord-voice/src/transport.rs",
    summary: "Ядро голосового и Go Live транспорта Serein (~2100 строк). Реализует две независимые параллельные сессии (основная голосовая run_inner и выделенная Go Live сессия run_stream_inner со своим voice.gateway, stream_key, SSRC, UDP-сокетом и DAVE-сессией), сигнализацию Op 1 Select Protocol, Op 12 Video, Op 15 Media Sink Wants, мультиплексирование RTP/RTCP и отправку звука/видео.",
    complexity: "complex",
    tags: ["serein", "voice-transport", "go-live", "dual-socket", "rtp", "rtcp", "dave-mls"]
  },
  {
    id: "function:docs/reference-projects/sources/Serein/crates/discord-voice/src/transport.rs:run_inner",
    type: "function",
    name: "run_inner",
    filePath: "docs/reference-projects/sources/Serein/crates/discord-voice/src/transport.rs",
    lineRange: [215, 760],
    summary: "Главный цикл основного голосового соединения: подключение к Voice WS v8, Identify (Op 0), Ready (Op 2), UDP IP Discovery, Select Protocol (Op 1), Session Description (Op 4), Speaking (Op 5), DAVE MLS (Op 21..31) и tokio::select! по UDP/WS/командам.",
    complexity: "complex",
    tags: ["serein", "voice-ws", "udp-discovery", "select-protocol", "dave"]
  },
  {
    id: "function:docs/reference-projects/sources/Serein/crates/discord-voice/src/transport.rs:run_stream_inner",
    type: "function",
    name: "run_stream_inner",
    filePath: "docs/reference-projects/sources/Serein/crates/discord-voice/src/transport.rs",
    lineRange: [885, 1390],
    summary: "Выделенный цикл Go Live сессии (исходящий стрим или просмотр чужого стрима): открывает второй независимый Voice WS + второй UDP-сокет с SSRC = audio_ssrc + 1 (video) и audio_ssrc + 2 (rtx), отправляет Op 12 Video и обрабатывает входящие/исходящие видео- и аудиопакеты стрима.",
    complexity: "complex",
    tags: ["serein", "go-live", "stream-session", "op12-video", "rtx-ssrc"]
  },
  {
    id: "function:docs/reference-projects/sources/Serein/crates/discord-voice/src/transport.rs:announce_video",
    type: "function",
    name: "announce_video",
    filePath: "docs/reference-projects/sources/Serein/crates/discord-voice/src/transport.rs",
    lineRange: [1435, 1485],
    summary: "Формирует и отправляет Voice Opcode 12 (Video) с полями audio_ssrc, video_ssrc, rtx_ssrc и массивом streams (type: screen, rid: 100, quality: 100, max_bitrate, max_framerate, max_resolution).",
    complexity: "moderate",
    tags: ["serein", "op12-video", "ssrc", "streams", "go-live"]
  },

  {
    id: "file:docs/reference-projects/sources/Serein/crates/discord-voice/src/video_receive.rs",
    type: "file",
    name: "video_receive.rs (Serein RTP Depacketizer, NACK/PLI & Hardware Decoder Thread)",
    filePath: "docs/reference-projects/sources/Serein/crates/discord-voice/src/video_receive.rs",
    summary: "Приёмный видеоконвейер Serein: парсинг RTP-расширений (One-Byte 0xBEDE и Two-Byte), восстановление RTX-пакетов (restore_rtx), сборка фрагментированных кадров H.264 (Single NAL, STAP-A, FU-A) и AV1 (OBU aggregation), генерация RTCP NACK (PT 205 FMT 1), PLI (PT 206 FMT 1), FIR (PT 206 FMT 4), REMB (PT 206 FMT 15) и декодирование в выделенном OS-потоке std::thread::Builder::new().name(\"serein-video-decode\").",
    complexity: "complex",
    tags: ["serein", "video-receive", "h264", "av1", "fu-a", "nack", "pli", "rtx", "decoder-thread"]
  },
  {
    id: "class:docs/reference-projects/sources/Serein/crates/discord-voice/src/video_receive.rs:Assembler",
    type: "class",
    name: "Assembler (Serein H.264/AV1 RTP Frame Assembler)",
    filePath: "docs/reference-projects/sources/Serein/crates/discord-voice/src/video_receive.rs",
    lineRange: [52, 165],
    summary: "Буфер сборки видеокадров по RTP timestamp и sequence numbers: отслеживает пропущенные пакеты для NACK, собирает FU-A/STAP-A (H.264) и OBU (AV1) в цельный Access Unit и запрашивает PLI при разрыве последовательности или ожидании ключевого кадра.",
    complexity: "complex",
    tags: ["serein", "rtp-assembler", "h264", "av1", "nack", "pli"]
  },
  {
    id: "function:docs/reference-projects/sources/Serein/crates/discord-voice/src/video_receive.rs:Receivers::restore_rtx",
    type: "function",
    name: "Receivers::restore_rtx",
    filePath: "docs/reference-projects/sources/Serein/crates/discord-voice/src/video_receive.rs",
    lineRange: [390, 445],
    summary: "Восстанавливает ретранслированный пакет RTX (RFC 4588): извлекает оригинальный 16-битный sequence number (OSN) из первых 2 байт payload и подменяет rtx_ssrc/rtx_pt на основной video_ssrc/video_pt.",
    complexity: "moderate",
    tags: ["serein", "rtx", "rfc4588", "packet-recovery"]
  },
  {
    id: "function:docs/reference-projects/sources/Serein/crates/discord-voice/src/video_receive.rs:spawn_decoder",
    type: "function",
    name: "spawn_decoder",
    filePath: "docs/reference-projects/sources/Serein/crates/discord-voice/src/video_receive.rs",
    lineRange: [560, 650],
    summary: "Запускает выделенный синхронный поток ОС serein-video-decode с ограниченным каналом sync_channel(16), инициализирует аппаратный декодер (GStreamer/VideoToolbox/MediaFoundation + OpenH264 fallback) и вызывает store_frame при получении каждого кадра.",
    complexity: "complex",
    tags: ["serein", "decoder-thread", "hardware-decode", "openh264", "gstreamer"]
  },

  {
    id: "file:docs/reference-projects/sources/Serein/crates/discord-voice/src/video.rs",
    type: "file",
    name: "video.rs (Serein Outgoing RTP Packetizer, Pacer & Bitrate Controller)",
    filePath: "docs/reference-projects/sources/Serein/crates/discord-voice/src/video.rs",
    summary: "Исходящий видеоконвейер Serein: разбиение Annex-B H.264 NAL-единиц на RTP-пакеты (Single NAL <= 1180 байт и FU-A фрагменты с заголовками FU indicator / FU header), RTP-расширения playout-delay / abs-send-time / video-rotation, кольцевой буфер History для ответов на NACK через RTX и адаптивный регулятор битрейта Rate по RTCP REMB.",
    complexity: "complex",
    tags: ["serein", "rtp-packetizer", "fu-a", "pacer", "remb", "rtx-history"]
  },
  {
    id: "function:docs/reference-projects/sources/Serein/crates/discord-voice/src/video.rs:packetize",
    type: "function",
    name: "packetize",
    filePath: "docs/reference-projects/sources/Serein/crates/discord-voice/src/video.rs",
    lineRange: [240, 360],
    summary: "Разбивает закодированный H.264 Annex-B кадр на RTP-пакеты (PT 101/103) с установкой бита маркера M=1 на последнем пакете кадра и сохранением копий в буфер RTX History.",
    complexity: "complex",
    tags: ["serein", "h264", "annex-b", "fu-a", "rtp"]
  },

  {
    id: "file:docs/reference-projects/sources/Serein/crates/discord-voice/src/crypto.rs",
    type: "file",
    name: "crypto.rs (Serein SRTP & DAVE MLS E2EE Crypto Pipeline)",
    filePath: "docs/reference-projects/sources/Serein/crates/discord-voice/src/crypto.rs",
    summary: "Криптографический уровень RTP/RTCP в Serein: двухслойное шифрование и расшифровка (1-й слой — сквозное шифрование DAVE MLS через крейт davey, 2-й слой — транспортный SRTP aead_xchacha20_poly1305_rtpsize или aead_aes256_gcm_rtpsize с 4-байтным счётчиком nonce в хвосте пакета).",
    complexity: "complex",
    tags: ["serein", "srtp", "dave-e2ee", "xchacha20poly1305", "aes256gcm"]
  },
  {
    id: "function:docs/reference-projects/sources/Serein/crates/discord-voice/src/crypto.rs:Encryption::open",
    type: "function",
    name: "Encryption::open",
    filePath: "docs/reference-projects/sources/Serein/crates/discord-voice/src/crypto.rs",
    lineRange: [210, 310],
    summary: "Расшифровывает входящий RTP-пакет: снимает транспортный слой SRTP (учитывая unencrypted header bytes для RTP-расширений в режиме _rtpsize), а затем при активном DAVE E2EE расшифровывает медиа-фрейм через dave.decrypt().",
    complexity: "complex",
    tags: ["serein", "srtp-decrypt", "dave-decrypt", "rtpsize"]
  },

  {
    id: "file:docs/reference-projects/sources/Serein/crates/discord-voice/src/screen.rs",
    type: "file",
    name: "screen.rs (Serein Screen Capture & Hardware Encoder Worker)",
    filePath: "docs/reference-projects/sources/Serein/crates/discord-voice/src/screen.rs",
    summary: "Оркестратор захвата и кодирования экрана Serein: запускает два выделенных потока ОС (serein-screen-capture и serein-screen-encode), связанных ограниченным каналом sync_channel(2) с отбрасыванием устаревших кадров try_send, управляет аппаратным кодировщиком H.264 и принудительной генерацией IDR-кадров по запросу PLI/FIR.",
    complexity: "complex",
    tags: ["serein", "screen-capture", "hardware-encoder", "worker-threads", "zero-backpressure"]
  },
  {
    id: "function:docs/reference-projects/sources/Serein/crates/discord-voice/src/screen.rs:encode_loop",
    type: "function",
    name: "encode_loop",
    filePath: "docs/reference-projects/sources/Serein/crates/discord-voice/src/screen.rs",
    lineRange: [155, 250],
    summary: "Цикл потока кодирования serein-screen-encode: принимает сырые кадры из канала захвата, проверяет атомарный флаг force_keyframe (от RTCP PLI/FIR), адаптирует битрейт кодировщика (от RTCP REMB) и передаёт закодированный H.264 битовый поток в транспорт.",
    complexity: "complex",
    tags: ["serein", "encoder-thread", "h264", "keyframe-pli", "bitrate-adaptation"]
  },

  {
    id: "file:docs/reference-projects/sources/Serein/crates/discord-voice/src/screen/gstreamer.rs",
    type: "file",
    name: "gstreamer.rs (Serein Linux PipeWire/XDG Portal + VAAPI/NVENC Capture)",
    filePath: "docs/reference-projects/sources/Serein/crates/discord-voice/src/screen/gstreamer.rs",
    summary: "Модуль захвата экрана на Linux в Serein: интеграция с XDG Desktop Portal (ashpd Screencast) и PipeWire (pipewiresrc) через GStreamer с аппаратным кодированием H.264 (vah264enc, nvh264enc, vaapih264enc, x264enc).",
    complexity: "complex",
    tags: ["serein", "linux", "pipewire", "xdg-portal", "gstreamer", "nvenc", "vaapi"]
  },

  {
    id: "file:docs/reference-projects/sources/Serein/crates/discord-voice/src/screen/audio_linux.rs",
    type: "file",
    name: "audio_linux.rs (Serein Linux PulseAudio/PipeWire System Soundshare)",
    filePath: "docs/reference-projects/sources/Serein/crates/discord-voice/src/screen/audio_linux.rs",
    summary: "Захват системного звука демонстрации экрана на Linux через PulseAudio/PipeWire monitor source с фильтрацией собственного PID процесса Serein (OwnApplication::allows), чтобы зрители стрима не слышали эхо своих голосов.",
    complexity: "moderate",
    tags: ["serein", "linux-audio", "pulseaudio", "pipewire", "echo-prevention", "soundshare"]
  },

  {
    id: "file:docs/reference-projects/sources/Serein/crates/discord-voice/src/mixer.rs",
    type: "file",
    name: "mixer.rs (Serein Per-Stream & Per-User Audio Volume Mixer)",
    filePath: "docs/reference-projects/sources/Serein/crates/discord-voice/src/mixer.rs",
    summary: "Многоканальный аудиомикшер Serein: независимое хранение индивидуальной громкости каждого участника и отдельного коэффициента громкости аудиопотока демонстрации экрана (pop_with_volumes), soft-clipping через tanh и подавление эха дублирующегося голоса стримера.",
    complexity: "moderate",
    tags: ["serein", "audio-mixer", "stream-volume", "per-user-gain", "soft-clipping"]
  },
  {
    id: "function:docs/reference-projects/sources/Serein/crates/discord-voice/src/mixer.rs:Mixer::pop_with_volumes",
    type: "function",
    name: "Mixer::pop_with_volumes",
    filePath: "docs/reference-projects/sources/Serein/crates/discord-voice/src/mixer.rs",
    lineRange: [95, 165],
    summary: "Суммирует 20-мс фреймы всех активных говорящих участников с применением индивидуального множителя громкости каждого пользователя и глобального output_gain без сброса состояния при обновлениях UI.",
    complexity: "moderate",
    tags: ["serein", "mixer", "volume-control", "pcm-mixing"]
  },

  {
    id: "file:docs/reference-projects/sources/Serein/crates/discord-voice/src/stream_playout.rs",
    type: "file",
    name: "stream_playout.rs (Serein Adaptive Stream Audio Jitter Buffer)",
    filePath: "docs/reference-projects/sources/Serein/crates/discord-voice/src/stream_playout.rs",
    summary: "Адаптивный джиттер-буфер для воспроизведения стерео-аудио демонстрации экрана (Opus 48 кГц): выравнивает сетевой джиттер, компенсирует потери пакетов через Opus PLC и изолирует регулятор громкости демки от основного голосового тракта.",
    complexity: "moderate",
    tags: ["serein", "stream-audio", "jitter-buffer", "opus-plc"]
  },

  {
    id: "file:docs/reference-projects/sources/Serein/crates/platform/src/video/live_gst.rs",
    type: "file",
    name: "live_gst.rs (Serein GStreamer Hardware H.264/AV1 Decoder & Encoder)",
    filePath: "docs/reference-projects/sources/Serein/crates/platform/src/video/live_gst.rs",
    summary: "Аппаратный декодер и энкодер видео на базе GStreamer в крейте platform проекта Serein: автовыбор vah264dec / nvh264dec / avdec_h264 и выдача кадров напрямую в формате NV12/I420 для загрузки в wgpu.",
    complexity: "complex",
    tags: ["serein", "gstreamer", "hardware-decoder", "nv12", "h264", "av1"]
  },

  {
    id: "file:docs/reference-projects/sources/Serein/crates/discord-gateway/src/voice.rs",
    type: "file",
    name: "voice.rs (Serein Main Gateway Go Live Opcodes 18..22 Signaling)",
    filePath: "docs/reference-projects/sources/Serein/crates/discord-gateway/src/voice.rs",
    summary: "Сигнализация демонстрации экрана (Go Live) в основном Gateway Serein: формирование пакетов Op 18 (StreamCreate), Op 19 (StreamDelete), Op 20 (StreamWatch), Op 21 (StreamPing) и обработка событий STREAM_CREATE, STREAM_SERVER_UPDATE, STREAM_DELETE.",
    complexity: "moderate",
    tags: ["serein", "gateway", "op18-stream-create", "op20-stream-watch", "stream-server-update"]
  },

  // === 3. LemonCord Source Files & Functions ===
  {
    id: "file:docs/reference-projects/sources/LemonCord/src/main.rs",
    type: "file",
    name: "main.rs (LemonCord Tao + Wry WebView Desktop Client)",
    filePath: "docs/reference-projects/sources/LemonCord/src/main.rs",
    summary: "Главный модуль клиента LemonCord: инициализация окна tao WindowBuilder, создание WebView через wry::WebViewBuilder на https://discord.com/app с кастомным User-Agent, перехват внешних ссылок через with_navigation_handler (открытие в системном браузере через open::that) и цикл обработки событий.",
    complexity: "simple",
    tags: ["lemoncord", "wry", "tao", "webview", "navigation-handler"]
  },
  {
    id: "function:docs/reference-projects/sources/LemonCord/src/main.rs:discord",
    type: "function",
    name: "discord (LemonCord WebView Builder)",
    filePath: "docs/reference-projects/sources/LemonCord/src/main.rs",
    lineRange: [50, 99],
    summary: "Создаёт главное окно и WebView для Discord, настраивает перехватчик навигации для фильтрации внешних URL от внутренних маршрутов discord.com/captcha/hcaptcha и возвращает кортеж (EventLoop, Window, WebView).",
    complexity: "simple",
    tags: ["lemoncord", "webview-builder", "navigation-guard", "discord-app"]
  },
  {
    id: "file:docs/reference-projects/sources/LemonCord/src/log.rs",
    type: "file",
    name: "log.rs (LemonCord Coloured Terminal Logger)",
    filePath: "docs/reference-projects/sources/LemonCord/src/log.rs",
    summary: "Модуль цветного логирования LemonCord с макросами info!, warn!, error!, debug! и уровнями приоритета Priority.",
    complexity: "simple",
    tags: ["lemoncord", "logging", "macros"]
  },

  // === 4. rust-discord-activity Source Files, Structs & Functions ===
  {
    id: "file:docs/reference-projects/sources/rust-discord-activity-0.3.1/src/client/ipc.rs",
    type: "file",
    name: "ipc.rs (rust-discord-activity DiscordClient Local RPC IPC)",
    filePath: "docs/reference-projects/sources/rust-discord-activity-0.3.1/src/client/ipc.rs",
    summary: "Реализация структуры DiscordClient из крейта rust-discord-activity v0.3.1: поиск сокета discord-ipc-0 в XDG_RUNTIME_DIR/TMPDIR, установка UnixStream-соединения, выполнение Handshake (OpCode 0) и отправка/чтение бинарных кадров [u32 LE opcode][u32 LE length][JSON payload] для управления Discord Rich Presence.",
    complexity: "moderate",
    tags: ["rust-discord-activity", "DiscordClient", "discord-ipc-0", "rich-presence", "binary-framing"]
  },
  {
    id: "class:docs/reference-projects/sources/rust-discord-activity-0.3.1/src/client/ipc.rs:DiscordClient",
    type: "class",
    name: "DiscordClient (rust-discord-activity)",
    filePath: "docs/reference-projects/sources/rust-discord-activity-0.3.1/src/client/ipc.rs",
    lineRange: [12, 16],
    summary: "Клиент локального IPC-протокола Discord Rich Presence: хранит client_id приложения, состояние подключения is_connected и активный сокет Option<UnixStream>.",
    complexity: "simple",
    tags: ["rust-discord-activity", "DiscordClient", "ipc-client", "struct"]
  },
  {
    id: "function:docs/reference-projects/sources/rust-discord-activity-0.3.1/src/client/ipc.rs:DiscordClient::connect",
    type: "function",
    name: "DiscordClient::connect",
    filePath: "docs/reference-projects/sources/rust-discord-activity-0.3.1/src/client/ipc.rs",
    lineRange: [28, 36],
    summary: "Подключается к локальному Unix-сокету discord-ipc-0 (найденному через fetch_process_pathbuf) и сразу выполняет рукопожатие handshake(OpCode::HANDSHAKE, {\"v\": 1, \"client_id\": ...}).",
    complexity: "simple",
    tags: ["rust-discord-activity", "connect", "handshake", "discord-ipc-0"]
  },
  {
    id: "function:docs/reference-projects/sources/rust-discord-activity-0.3.1/src/client/ipc.rs:DiscordClient::send_payload",
    type: "function",
    name: "DiscordClient::send_payload",
    filePath: "docs/reference-projects/sources/rust-discord-activity-0.3.1/src/client/ipc.rs",
    lineRange: [38, 43],
    summary: "Сериализует структуру Payload (cmd: SET_ACTIVITY, args: {pid, activity}, nonce: UUID v4) в JSON, отправляет кадр с OpCode::FRAME (1) через send() и читает ответ сервера через recv().",
    complexity: "simple",
    tags: ["rust-discord-activity", "send_payload", "set-activity", "opcode-frame"]
  },
  {
    id: "function:docs/reference-projects/sources/rust-discord-activity-0.3.1/src/client/ipc.rs:DiscordClient::send",
    type: "function",
    name: "DiscordClient::send",
    filePath: "docs/reference-projects/sources/rust-discord-activity-0.3.1/src/client/ipc.rs",
    lineRange: [73, 91],
    summary: "Кодирует 8-байтный бинарный заголовок кадра Discord IPC ([opcode: u32 LE][payload_len: u32 LE]) вместе с байтами JSON-строки и записывает пакет в UnixStream.",
    complexity: "simple",
    tags: ["rust-discord-activity", "binary-framing", "little-endian", "ipc-write"]
  },
  {
    id: "function:docs/reference-projects/sources/rust-discord-activity-0.3.1/src/client/ipc.rs:DiscordClient::recv",
    type: "function",
    name: "DiscordClient::recv",
    filePath: "docs/reference-projects/sources/rust-discord-activity-0.3.1/src/client/ipc.rs",
    lineRange: [93, 108],
    summary: "Читает из UnixStream ответный пакет Discord IPC и передаёт буфер в extract_payload для распаковки u32 LE заголовка и JSON-ответа.",
    complexity: "simple",
    tags: ["rust-discord-activity", "ipc-read", "recv"]
  },
  {
    id: "file:docs/reference-projects/sources/rust-discord-activity-0.3.1/src/models/activity.rs",
    type: "file",
    name: "activity.rs (rust-discord-activity Rich Presence Data Models)",
    filePath: "docs/reference-projects/sources/rust-discord-activity-0.3.1/src/models/activity.rs",
    summary: "Модели данных Discord Rich Presence: Activity, Assets, Timestamp, Party, Secrets, Button и перечисление ActivityType (Playing=0, Listening=2, Watching=3, Competing=5).",
    complexity: "simple",
    tags: ["rust-discord-activity", "activity-model", "rich-presence", "serde"]
  },
  {
    id: "file:docs/reference-projects/sources/rust-discord-activity-0.3.1/src/models/client/payload.rs",
    type: "file",
    name: "payload.rs (rust-discord-activity SET_ACTIVITY Payload & OpCode)",
    filePath: "docs/reference-projects/sources/rust-discord-activity-0.3.1/src/models/client/payload.rs",
    summary: "Формат конверта команды SET_ACTIVITY (cmd, args: {pid, activity}, evt, data, nonce) и перечисление кодов операций OpCode (HANDSHAKE=0, FRAME=1, CLOSE=2, PING=3, PONG=4).",
    complexity: "simple",
    tags: ["rust-discord-activity", "payload", "opcode", "set-activity"]
  }
];

g.nodes.push(...newNodes);

const newEdges = [
  // Document links
  { source: "document:docs/reference-projects/README.md", target: "document:docs/reference-projects/SEREIN-FUNCTION-MAP.md", type: "documents", direction: "forward", weight: 0.9 },
  { source: "document:docs/reference-projects/README.md", target: "document:docs/reference-projects/LEMONCORD-FUNCTION-MAP.md", type: "documents", direction: "forward", weight: 0.9 },
  { source: "document:docs/reference-projects/README.md", target: "document:docs/reference-projects/RUST-DISCORD-ACTIVITY-MAP.md", type: "documents", direction: "forward", weight: 0.9 },
  { source: "document:docs/reference-projects/README.md", target: "document:docs/reference-projects/SEREIN-FULL-SYMBOL-INDEX.txt", type: "documents", direction: "forward", weight: 0.8 },
  { source: "document:docs/reference-projects/SEREIN-FUNCTION-MAP.md", target: "document:docs/reference-projects/sources/Serein/docs/architecture.md", type: "documents", direction: "forward", weight: 0.8 },
  { source: "document:docs/reference-projects/SEREIN-FUNCTION-MAP.md", target: "document:docs/reference-projects/sources/Serein/docs/voice.md", type: "documents", direction: "forward", weight: 0.9 },

  // Serein document -> file edges
  { source: "document:docs/reference-projects/SEREIN-FUNCTION-MAP.md", target: "file:docs/reference-projects/sources/Serein/crates/discord-voice/src/transport.rs", type: "documents", direction: "forward", weight: 0.9 },
  { source: "document:docs/reference-projects/SEREIN-FUNCTION-MAP.md", target: "file:docs/reference-projects/sources/Serein/crates/discord-voice/src/video_receive.rs", type: "documents", direction: "forward", weight: 0.9 },
  { source: "document:docs/reference-projects/SEREIN-FUNCTION-MAP.md", target: "file:docs/reference-projects/sources/Serein/crates/discord-voice/src/video.rs", type: "documents", direction: "forward", weight: 0.9 },
  { source: "document:docs/reference-projects/SEREIN-FUNCTION-MAP.md", target: "file:docs/reference-projects/sources/Serein/crates/discord-voice/src/crypto.rs", type: "documents", direction: "forward", weight: 0.9 },
  { source: "document:docs/reference-projects/SEREIN-FUNCTION-MAP.md", target: "file:docs/reference-projects/sources/Serein/crates/discord-voice/src/screen.rs", type: "documents", direction: "forward", weight: 0.9 },
  { source: "document:docs/reference-projects/SEREIN-FUNCTION-MAP.md", target: "file:docs/reference-projects/sources/Serein/crates/discord-voice/src/mixer.rs", type: "documents", direction: "forward", weight: 0.9 },
  { source: "document:docs/reference-projects/SEREIN-FUNCTION-MAP.md", target: "file:docs/reference-projects/sources/Serein/apps/desktop/src/gpu_video.rs", type: "documents", direction: "forward", weight: 0.9 },
  { source: "document:docs/reference-projects/SEREIN-FUNCTION-MAP.md", target: "file:docs/reference-projects/sources/Serein/apps/desktop/src/watch.rs", type: "documents", direction: "forward", weight: 0.9 },

  // LemonCord document -> file edges
  { source: "document:docs/reference-projects/LEMONCORD-FUNCTION-MAP.md", target: "file:docs/reference-projects/sources/LemonCord/src/main.rs", type: "documents", direction: "forward", weight: 0.9 },
  { source: "document:docs/reference-projects/LEMONCORD-FUNCTION-MAP.md", target: "file:docs/reference-projects/sources/LemonCord/src/log.rs", type: "documents", direction: "forward", weight: 0.8 },

  // rust-discord-activity document -> file edges
  { source: "document:docs/reference-projects/RUST-DISCORD-ACTIVITY-MAP.md", target: "file:docs/reference-projects/sources/rust-discord-activity-0.3.1/src/client/ipc.rs", type: "documents", direction: "forward", weight: 0.9 },
  { source: "document:docs/reference-projects/RUST-DISCORD-ACTIVITY-MAP.md", target: "file:docs/reference-projects/sources/rust-discord-activity-0.3.1/src/models/activity.rs", type: "documents", direction: "forward", weight: 0.9 },
  { source: "document:docs/reference-projects/RUST-DISCORD-ACTIVITY-MAP.md", target: "file:docs/reference-projects/sources/rust-discord-activity-0.3.1/src/models/client/payload.rs", type: "documents", direction: "forward", weight: 0.9 },
  { source: "document:docs/reference-projects/RUST-DISCORD-ACTIVITY-MAP.md", target: "file:docs/reference-projects/sources/Serein/apps/desktop/src/game_activity.rs", type: "documents", direction: "forward", weight: 0.9 },

  // File contains class/function edges
  { source: "file:docs/reference-projects/sources/Serein/apps/desktop/src/gpu_video.rs", target: "class:docs/reference-projects/sources/Serein/apps/desktop/src/gpu_video.rs:Converter", type: "contains", direction: "forward", weight: 0.9 },
  { source: "file:docs/reference-projects/sources/Serein/apps/desktop/src/gpu_video.rs", target: "function:docs/reference-projects/sources/Serein/apps/desktop/src/gpu_video.rs:upload_gpu", type: "contains", direction: "forward", weight: 0.9 },
  { source: "file:docs/reference-projects/sources/Serein/apps/desktop/src/watch.rs", target: "function:docs/reference-projects/sources/Serein/apps/desktop/src/watch.rs:store_frame", type: "contains", direction: "forward", weight: 0.9 },
  { source: "file:docs/reference-projects/sources/Serein/apps/desktop/src/watch.rs", target: "function:docs/reference-projects/sources/Serein/apps/desktop/src/watch.rs:Watch::poll", type: "contains", direction: "forward", weight: 0.9 },
  { source: "file:docs/reference-projects/sources/Serein/apps/desktop/src/game_activity.rs", target: "function:docs/reference-projects/sources/Serein/apps/desktop/src/game_activity.rs:serve_ipc", type: "contains", direction: "forward", weight: 0.9 },
  { source: "file:docs/reference-projects/sources/Serein/crates/discord-voice/src/transport.rs", target: "function:docs/reference-projects/sources/Serein/crates/discord-voice/src/transport.rs:run_inner", type: "contains", direction: "forward", weight: 0.9 },
  { source: "file:docs/reference-projects/sources/Serein/crates/discord-voice/src/transport.rs", target: "function:docs/reference-projects/sources/Serein/crates/discord-voice/src/transport.rs:run_stream_inner", type: "contains", direction: "forward", weight: 0.9 },
  { source: "file:docs/reference-projects/sources/Serein/crates/discord-voice/src/transport.rs", target: "function:docs/reference-projects/sources/Serein/crates/discord-voice/src/transport.rs:announce_video", type: "contains", direction: "forward", weight: 0.9 },
  { source: "file:docs/reference-projects/sources/Serein/crates/discord-voice/src/video_receive.rs", target: "class:docs/reference-projects/sources/Serein/crates/discord-voice/src/video_receive.rs:Assembler", type: "contains", direction: "forward", weight: 0.9 },
  { source: "file:docs/reference-projects/sources/Serein/crates/discord-voice/src/video_receive.rs", target: "function:docs/reference-projects/sources/Serein/crates/discord-voice/src/video_receive.rs:Receivers::restore_rtx", type: "contains", direction: "forward", weight: 0.9 },
  { source: "file:docs/reference-projects/sources/Serein/crates/discord-voice/src/video_receive.rs", target: "function:docs/reference-projects/sources/Serein/crates/discord-voice/src/video_receive.rs:spawn_decoder", type: "contains", direction: "forward", weight: 0.9 },
  { source: "file:docs/reference-projects/sources/Serein/crates/discord-voice/src/video.rs", target: "function:docs/reference-projects/sources/Serein/crates/discord-voice/src/video.rs:packetize", type: "contains", direction: "forward", weight: 0.9 },
  { source: "file:docs/reference-projects/sources/Serein/crates/discord-voice/src/crypto.rs", target: "function:docs/reference-projects/sources/Serein/crates/discord-voice/src/crypto.rs:Encryption::open", type: "contains", direction: "forward", weight: 0.9 },
  { source: "file:docs/reference-projects/sources/Serein/crates/discord-voice/src/screen.rs", target: "function:docs/reference-projects/sources/Serein/crates/discord-voice/src/screen.rs:encode_loop", type: "contains", direction: "forward", weight: 0.9 },
  { source: "file:docs/reference-projects/sources/Serein/crates/discord-voice/src/mixer.rs", target: "function:docs/reference-projects/sources/Serein/crates/discord-voice/src/mixer.rs:Mixer::pop_with_volumes", type: "contains", direction: "forward", weight: 0.9 },
  { source: "file:docs/reference-projects/sources/LemonCord/src/main.rs", target: "function:docs/reference-projects/sources/LemonCord/src/main.rs:discord", type: "contains", direction: "forward", weight: 0.9 },
  { source: "file:docs/reference-projects/sources/rust-discord-activity-0.3.1/src/client/ipc.rs", target: "class:docs/reference-projects/sources/rust-discord-activity-0.3.1/src/client/ipc.rs:DiscordClient", type: "contains", direction: "forward", weight: 0.9 },
  { source: "file:docs/reference-projects/sources/rust-discord-activity-0.3.1/src/client/ipc.rs", target: "function:docs/reference-projects/sources/rust-discord-activity-0.3.1/src/client/ipc.rs:DiscordClient::connect", type: "contains", direction: "forward", weight: 0.9 },
  { source: "file:docs/reference-projects/sources/rust-discord-activity-0.3.1/src/client/ipc.rs", target: "function:docs/reference-projects/sources/rust-discord-activity-0.3.1/src/client/ipc.rs:DiscordClient::send_payload", type: "contains", direction: "forward", weight: 0.9 },
  { source: "file:docs/reference-projects/sources/rust-discord-activity-0.3.1/src/client/ipc.rs", target: "function:docs/reference-projects/sources/rust-discord-activity-0.3.1/src/client/ipc.rs:DiscordClient::send", type: "contains", direction: "forward", weight: 0.9 },
  { source: "file:docs/reference-projects/sources/rust-discord-activity-0.3.1/src/client/ipc.rs", target: "function:docs/reference-projects/sources/rust-discord-activity-0.3.1/src/client/ipc.rs:DiscordClient::recv", type: "contains", direction: "forward", weight: 0.9 },

  // Function-to-Function Call Graph edges
  { source: "function:docs/reference-projects/sources/Serein/crates/discord-voice/src/transport.rs:run_stream_inner", target: "function:docs/reference-projects/sources/Serein/crates/discord-voice/src/transport.rs:announce_video", type: "calls", direction: "forward", weight: 0.9 },
  { source: "function:docs/reference-projects/sources/Serein/crates/discord-voice/src/transport.rs:run_stream_inner", target: "function:docs/reference-projects/sources/Serein/crates/discord-voice/src/crypto.rs:Encryption::open", type: "calls", direction: "forward", weight: 0.9 },
  { source: "function:docs/reference-projects/sources/Serein/crates/discord-voice/src/transport.rs:run_stream_inner", target: "function:docs/reference-projects/sources/Serein/crates/discord-voice/src/video_receive.rs:Receivers::restore_rtx", type: "calls", direction: "forward", weight: 0.8 },
  { source: "function:docs/reference-projects/sources/Serein/crates/discord-voice/src/transport.rs:run_stream_inner", target: "function:docs/reference-projects/sources/Serein/crates/discord-voice/src/video_receive.rs:spawn_decoder", type: "calls", direction: "forward", weight: 0.9 },
  { source: "function:docs/reference-projects/sources/Serein/crates/discord-voice/src/video_receive.rs:spawn_decoder", target: "function:docs/reference-projects/sources/Serein/apps/desktop/src/watch.rs:store_frame", type: "calls", direction: "forward", weight: 0.9 },
  { source: "function:docs/reference-projects/sources/Serein/apps/desktop/src/watch.rs:Watch::poll", target: "function:docs/reference-projects/sources/Serein/apps/desktop/src/gpu_video.rs:upload_gpu", type: "calls", direction: "forward", weight: 0.9 },
  { source: "function:docs/reference-projects/sources/Serein/crates/discord-voice/src/screen.rs:encode_loop", target: "function:docs/reference-projects/sources/Serein/crates/discord-voice/src/video.rs:packetize", type: "calls", direction: "forward", weight: 0.9 },
  { source: "function:docs/reference-projects/sources/rust-discord-activity-0.3.1/src/client/ipc.rs:DiscordClient::connect", target: "function:docs/reference-projects/sources/rust-discord-activity-0.3.1/src/client/ipc.rs:DiscordClient::send", type: "calls", direction: "forward", weight: 0.9 },
  { source: "function:docs/reference-projects/sources/rust-discord-activity-0.3.1/src/client/ipc.rs:DiscordClient::connect", target: "function:docs/reference-projects/sources/rust-discord-activity-0.3.1/src/client/ipc.rs:DiscordClient::recv", type: "calls", direction: "forward", weight: 0.9 },
  { source: "function:docs/reference-projects/sources/rust-discord-activity-0.3.1/src/client/ipc.rs:DiscordClient::send_payload", target: "function:docs/reference-projects/sources/rust-discord-activity-0.3.1/src/client/ipc.rs:DiscordClient::send", type: "calls", direction: "forward", weight: 0.9 },
  { source: "function:docs/reference-projects/sources/rust-discord-activity-0.3.1/src/client/ipc.rs:DiscordClient::send_payload", target: "function:docs/reference-projects/sources/rust-discord-activity-0.3.1/src/client/ipc.rs:DiscordClient::recv", type: "calls", direction: "forward", weight: 0.9 },
  { source: "function:docs/reference-projects/sources/rust-discord-activity-0.3.1/src/client/ipc.rs:DiscordClient::send_payload", target: "function:docs/reference-projects/sources/Serein/apps/desktop/src/game_activity.rs:serve_ipc", type: "related", direction: "forward", weight: 0.8 },

  // Cross-project links to discord-lab crates
  { source: "file:docs/reference-projects/sources/Serein/crates/discord-voice/src/transport.rs", target: "file:rust/crates/discord-voice/src/lib.rs", type: "similar_to", direction: "forward", weight: 0.9 },
  { source: "file:docs/reference-projects/sources/Serein/crates/discord-voice/src/video_receive.rs", target: "file:rust/crates/discord-voice/src/lib.rs", type: "similar_to", direction: "forward", weight: 0.9 },
  { source: "file:docs/reference-projects/sources/Serein/crates/discord-voice/src/video.rs", target: "file:rust/crates/discord-voice/src/lib.rs", type: "similar_to", direction: "forward", weight: 0.9 },
  { source: "file:docs/reference-projects/sources/Serein/crates/discord-voice/src/mixer.rs", target: "file:rust/crates/discord-voice/src/lib.rs", type: "similar_to", direction: "forward", weight: 0.9 },
  { source: "file:docs/reference-projects/sources/Serein/crates/discord-gateway/src/voice.rs", target: "file:rust/crates/discord-core/src/gateway.rs", type: "similar_to", direction: "forward", weight: 0.9 },
  { source: "file:docs/reference-projects/sources/Serein/crates/platform/src/video/live_gst.rs", target: "file:rust/crates/discord-media/src/lib.rs", type: "similar_to", direction: "forward", weight: 0.9 },
  { source: "file:docs/reference-projects/sources/Serein/apps/desktop/src/gpu_video.rs", target: "file:rust/crates/discord-ui/src/lib.rs", type: "similar_to", direction: "forward", weight: 0.9 },
  { source: "file:docs/reference-projects/sources/LemonCord/src/main.rs", target: "file:rust/crates/discord-ui/src/lib.rs", type: "similar_to", direction: "forward", weight: 0.8 },
  { source: "file:docs/reference-projects/sources/rust-discord-activity-0.3.1/src/client/ipc.rs", target: "file:rust/crates/discord-platform/src/lib.rs", type: "similar_to", direction: "forward", weight: 0.9 }
];

g.edges.push(...newEdges);

const fileLevelTypes = new Set(["file", "config", "document", "service", "pipeline", "table", "schema", "resource", "endpoint"]);
const refLayerNodeIds = newNodes.filter(n => fileLevelTypes.has(n.type)).map(n => n.id);

g.layers.push({
  id: "layer:reference-projects",
  name: "Эталонные проекты (Serein, LemonCord, rust-discord-activity)",
  description: "Локальная база знаний, исходный код, карты вызовов и пофункционная документация сторонних Rust-проектов Discord (ViceVerse-cz/Serein, japandotorg/LemonCord, rust-discord-activity v0.3.1 DiscordClient), сопоставленная с модулями discord-lab.",
  nodeIds: refLayerNodeIds
});

g.tour.push({
  order: 13,
  title: "Эталонные проекты и карта функций (Serein, LemonCord, rust-discord-activity)",
  description: "Справочный слой docs/reference-projects/ объединяет полный исходный код и подробные пофункционные карты вызовов трёх сторонних Rust-проектов: нативного клиента ViceVerse-cz/Serein (двойной голосовой и Go Live транспорт, сборка H.264/AV1 RTP-пакетов с NACK/PLI/RTX, двухслойный криптоконвейер SRTP + DAVE MLS E2EE, захват через PipeWire/GStreamer и рендер NV12/I420 через шейдеры wgpu), десктопной WebView-оболочки japandotorg/LemonCord (tao + wry с фильтрацией внешних ссылок) и клиента локального RPC-сокета rust-discord-activity (DiscordClient для discord-ipc-0).",
  nodeIds: [
    "document:docs/reference-projects/README.md",
    "document:docs/reference-projects/SEREIN-FUNCTION-MAP.md",
    "document:docs/reference-projects/LEMONCORD-FUNCTION-MAP.md",
    "document:docs/reference-projects/RUST-DISCORD-ACTIVITY-MAP.md",
    "file:docs/reference-projects/sources/Serein/crates/discord-voice/src/transport.rs",
    "file:docs/reference-projects/sources/Serein/crates/discord-voice/src/video_receive.rs",
    "file:docs/reference-projects/sources/Serein/apps/desktop/src/gpu_video.rs",
    "file:docs/reference-projects/sources/LemonCord/src/main.rs",
    "file:docs/reference-projects/sources/rust-discord-activity-0.3.1/src/client/ipc.rs"
  ]
});

g.lastUpdated = new Date().toISOString();
fs.writeFileSync(path, JSON.stringify(g, null, 2) + "\n");
console.log("Updated knowledge-graph.json:", {
  nodes: g.nodes.length,
  edges: g.edges.length,
  layers: g.layers.length,
  tour: g.tour.length
});
