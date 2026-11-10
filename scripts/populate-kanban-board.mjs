#!/usr/bin/env node
// @ts-check
import fs from 'node:fs';
import path from 'node:path';

const ROOT = '/home/sanya/orca/project';
const BOARD_FILE = path.join(ROOT, 'waves/kanban-board.json');
const EXPORT_HTML_FILE = path.join(ROOT, 'waves/kanban-board.html');

/** @type {any} */
const board = {
  project: 'Discord GPUI Client (Rust + WGPU)',
  wave: 'RW-42: Native GPUI Migration & Design Parity',
  updated_at: new Date().toISOString(),
  columns: [
    { id: 'backlog', title: '📋 Бэклог задач', color: '#4e5058' },
    { id: 'design_specs', title: '🎨 Спецификации и Дизайн', color: '#5865f2' },
    { id: 'in_dev', title: '⚡ В разработке', color: '#f0b232' },
    { id: 'in_review', title: '🧪 Тестирование и Ревью', color: '#eb459e' },
    { id: 'done', title: '✅ Завершено и Принято', color: '#23a55a' }
  ],
  cards: [
    {
      id: 'TASK-701',
      title: 'Каркас приложения AppLayout и WGPU GPU-рендеринг',
      status: 'done',
      priority: 'P0',
      department: 'G2 Frontend',
      summary: 'Трехпанельный макет Discord Dark: левая серверная панель (72px), средний список каналов (240px), центральный чат (flex-1) и правый сайдбар участников (240px). 120 FPS без лагов WebKit.',
      layout: {
        container: 'Three-column flex layout',
        dimensions: '1280x720 min, 100vw x 100vh',
        padding: '0px',
        spacing: '0px'
      },
      design_tokens: {
        background: '#1e1f22',
        surface: '#2b2d31',
        text_primary: '#ffffff',
        accent: '#5865f2',
        border: '#3f4147'
      },
      interactive_states: [
        { element: 'ServerRail', default: 'bg: #1e1f22, w: 72px', hover: 'w: 72px, cursor: default', active: 'focus: outline-none' },
        { element: 'ChannelSidebar', default: 'bg: #2b2d31, w: 240px', hover: 'border-r: 1px solid #1e1f22', active: 'selected-item' },
        { element: 'ChatView', default: 'bg: #313338, flex: 1', hover: 'smooth-scroll', active: 'input-focused' }
      ],
      acceptance_criteria: [
        'Сетка не расползается при изменении размеров окна (min-width 900px)',
        'Отсутствуют DOM-коллизии и утечки памяти',
        '61/61 контрактных тестов пройдены успешно'
      ]
    },
    {
      id: 'TASK-707',
      title: 'Панель серверов ServerRail (72px) и Индикаторы Активности',
      status: 'done',
      priority: 'P1',
      department: 'G2 Frontend',
      summary: 'Вертикальный рейл серверов Discord с кнопкой Home, разделителем, иконками серверов с анимацией скругления (круг 24px -> скругленный квадрат rx=16px при наведении), индикатором-пилюлей активности слева и кнопками Добавить сервер (+) и Исследовать (🧭).',
      layout: {
        container: 'Vertical Flex column',
        dimensions: 'Width: 72px, Height: 100%',
        padding: '12px 0px',
        spacing: '8px'
      },
      design_tokens: {
        background: '#1e1f22',
        surface: '#313338',
        text_primary: '#ffffff',
        text_muted: '#dbdee1',
        accent: '#5865f2',
        border: 'none'
      },
      interactive_states: [
        { element: 'Home Button (Discord)', default: 'rx: 24px (круг), bg: #313338', hover: 'rx: 16px, bg: #5865f2', active: 'scale: 0.96' },
        { element: 'Server Icon', default: 'rx: 24px, bg: #313338', hover: 'rx: 16px, bg: #5865f2, белый индикатор 20px', active: 'rx: 16px, bg: #5865f2, белый индикатор 40px' },
        { element: 'Add Server (+)', default: 'rx: 24px, bg: #313338, text: #23a55a', hover: 'rx: 16px, bg: #23a55a, text: #ffffff', active: 'scale: 0.94' },
        { element: 'Explore (🧭)', default: 'rx: 24px, bg: #313338, text: #23a55a', hover: 'rx: 16px, bg: #23a55a, text: #ffffff', active: 'scale: 0.94' }
      ],
      acceptance_criteria: [
        'Активный сервер подсвечивается вертикальной белой пилюлей слева (высота 40px, ширина 4px)',
        'Непрочитанные сообщения отображаются красным бейджем с белым числом в нижнем правом углу иконки',
        'Кнопка + открывает модальное окно CreateGuildModal'
      ]
    },
    {
      id: 'TASK-708',
      title: 'Список каналов ChannelSidebar и Панель профиля (240px)',
      status: 'done',
      priority: 'P1',
      department: 'G2 Frontend',
      summary: 'Иерархический список каналов гильдии с категориями (▾ ТЕКСТОВЫЕ КАНАЛЫ, ▾ ГОЛОСОВЫЕ КАНАЛЫ), плашка текущего статуса звонка VoicePanel (52px flex-none) и фиксированная нижняя панель профиля пользователя UserProfileBar (52px flex-none).',
      layout: {
        container: 'Vertical Flex column with scrollable channel list and fixed bottom bars',
        dimensions: 'Width: 240px, Height: 100%',
        padding: '0px',
        spacing: '2px'
      },
      design_tokens: {
        background: '#2b2d31',
        surface: '#232428',
        text_primary: '#ffffff',
        text_muted: '#949ba4',
        accent: '#5865f2',
        border: '1px solid rgba(255, 255, 255, 0.05)'
      },
      interactive_states: [
        { element: 'Категория каналов', default: 'text: #949ba4, 11px uppercase bold', hover: 'text: #dbdee1, cursor: pointer', active: 'toggle collapse' },
        { element: 'Текстовый канал (#)', default: 'text: #949ba4, bg: transparent', hover: 'bg: rgba(255,255,255,0.04), text: #dbdee1', active: 'bg: #35373c, text: #ffffff, font-bold' },
        { element: 'Голосовой канал (🔊)', default: 'text: #949ba4, bg: transparent', hover: 'bg: rgba(255,255,255,0.04), text: #23a55a', active: 'connect_voice() -> VoicePanel open' },
        { element: 'Кнопки микрофона/звука', default: 'text: #dbdee1', hover: 'bg: rgba(255,255,255,0.08), rounded 4px', active: 'toggle mute/deafen (red slash)' }
      ],
      acceptance_criteria: [
        'Жесткая фиксация высоты UserProfileBar = 52px и VoicePanel = 52px',
        'Голосовая панель полностью скрывается при отключенном звонке (voice_session = None)',
        'Исключено перекрытие элементов профиля пользователя'
      ]
    },
    {
      id: 'TASK-716',
      title: 'Нативное контекстное меню ПКМ (Context Menu)',
      status: 'done',
      priority: 'P1',
      department: 'G2 Frontend',
      summary: 'Всплывающее меню по правому клику мыши на сообщении: «Закрепить/Открепить сообщение», «Ответить», «Копировать текст», «Удалить сообщение». Мгновенный оптимистичный UI с асинхронным вызовом Discord REST API v9.',
      layout: {
        container: 'Floating absolute card with blur backdrop overlay',
        dimensions: 'Width: 220px, Height: auto',
        padding: '6px',
        spacing: '2px'
      },
      design_tokens: {
        background: '#2b2d31',
        surface: '#313338',
        text_primary: '#ffffff',
        text_muted: '#dbdee1',
        accent: '#5865f2',
        border: '1px solid #3f4147'
      },
      interactive_states: [
        { element: 'Пункт «Закрепить» (📌)', default: 'bg: transparent, text: #dbdee1', hover: 'bg: #5865f2, text: #ffffff', active: 'pin_live_message_blocking' },
        { element: 'Пункт «Ответить» (💬)', default: 'bg: transparent, text: #dbdee1', hover: 'bg: #5865f2, text: #ffffff', active: 'focus message bar with reply badge' },
        { element: 'Пункт «Копировать» (📋)', default: 'bg: transparent, text: #dbdee1', hover: 'bg: #5865f2, text: #ffffff', active: 'copy to system clipboard' },
        { element: 'Пункт «Удалить» (🗑️)', default: 'bg: transparent, text: #f23f43', hover: 'bg: #da373c, text: #ffffff', active: 'delete_live_message_blocking' }
      ],
      acceptance_criteria: [
        'Клик вне меню мгновенно закрывает всплывающее окно',
        'Динамическая плашка «Открепить сообщение» если сообщение уже закреплено',
        'Отправка реального DELETE /channels/{id}/messages/{id} при наличии токена'
      ]
    },
    {
      id: 'TASK-741',
      title: 'Реальные REST API действия сообщений (Удаление, Закрепление, Открепление)',
      status: 'done',
      priority: 'P1',
      department: 'G1 Backend',
      summary: 'Интеграция официальных эндпоинтов Discord REST API v9: DELETE /channels/{id}/messages/{id}, PUT /channels/{id}/pins/{id}, DELETE /channels/{id}/pins/{id} через discord-core и reqwest с защитой от 429 Too Many Requests.',
      layout: {
        container: 'Backend Network Stack & Threadpool Worker',
        dimensions: 'Asynchronous Tokio block_on execution',
        padding: 'N/A',
        spacing: 'N/A'
      },
      design_tokens: {
        background: 'N/A',
        surface: 'N/A',
        text_primary: 'N/A',
        accent: '#5865f2'
      },
      interactive_states: [
        { element: 'delete_live_message', default: 'Idle', hover: 'Optimistic UI local delete', active: 'HTTP 204 No Content' },
        { element: 'pin_live_message', default: 'Idle', hover: 'Toggle is_pinned locally', active: 'HTTP 204 No Content' }
      ],
      acceptance_criteria: [
        'Локальное состояние сообщений в ленте обновляется мгновенно за 0 мс',
        'Сетевые запросы исполняются в фоновом потоке без блокировки GUI',
        'Покрыто контрактом test_rest_message_actions_endpoints_contract'
      ]
    },
    {
      id: 'TASK-742',
      title: 'Прямой Voice UDP Media Streamer (tokio::net::UdpSocket + Opus + AES-256-GCM)',
      status: 'done',
      priority: 'P1',
      department: 'G5 Voice/Platform',
      summary: 'Реализация VoiceUdpStreamer по спецификации Serein transport.rs: прямое UDP-соединение к голосовому шлюзу, IP Discovery (74B), периодические Keepalive (8B 0x1337cafe LE), сборка 12B RTP заголовка и шифрование 20мс Opus-пакетов AES-256-GCM.',
      layout: {
        container: 'Realtime Voice UDP Pipeline',
        dimensions: '48 kHz Stereo, 20ms frames = 960 samples',
        padding: '12B RTP header + ciphertext + 16B GCM tag + 4B LE counter',
        spacing: '5000ms keepalive interval'
      },
      design_tokens: {
        background: '#232428',
        surface: '#23a55a',
        text_primary: '#ffffff',
        accent: '#23a55a'
      },
      interactive_states: [
        { element: 'Voice Connection', default: 'Disconnected', hover: 'Connecting (IP Discovery)', active: 'Connected (Voice Ping 18ms)' },
        { element: 'Push-to-Talk / VAD', default: 'Silent (No UDP audio)', hover: 'VAD Trigger', active: 'Streaming encrypted Opus packets' }
      ],
      acceptance_criteria: [
        'Keepalive пакет ровно 8 байт (0x1337cafe LE + sequence)',
        'Шифрование RTP медиа-кадров по эталону voice-deep.md §2.3',
        'Покрыто контрактом test_voice_udp_streamer_lifecycle_contract'
      ]
    },
    {
      id: 'TASK-743',
      title: 'Демонстрация экрана PipeWire через Linux XDG Desktop Portal',
      status: 'done',
      priority: 'P2',
      department: 'G5 Voice/Platform',
      summary: 'ScreenCastManager на базе org.freedesktop.portal.ScreenCast (D-Bus / ashpd): открытие диалога выбора мониторов и окон, согласование курсора (Embedded/Metadata), получение PipeWire remote fd и передача кадров в GPU WGPU видео-пайплайн.',
      layout: {
        container: 'Screen Share Modal (600px) with 2x2 grid of display sources',
        dimensions: 'Width: 600px, Max-height: 520px',
        padding: '20px',
        spacing: '16px'
      },
      design_tokens: {
        background: '#2b2d31',
        surface: '#313338',
        text_primary: '#ffffff',
        accent: '#5865f2',
        border: '1px solid #3f4147'
      },
      interactive_states: [
        { element: 'Карточка экрана/окна', default: 'bg: #313338, border: 1px solid transparent', hover: 'border: 1px solid #5865f2, bg: #35373c', active: 'border: 2px solid #5865f2' },
        { element: 'Выбор разрешения (720p/1080p)', default: 'bg: #1e1f22, text: #949ba4', hover: 'text: #ffffff', active: 'bg: #5865f2, text: #ffffff' },
        { element: 'Выбор частоты (30/60 FPS)', default: 'bg: #1e1f22, text: #949ba4', hover: 'text: #ffffff', active: 'bg: #5865f2, text: #ffffff' }
      ],
      acceptance_criteria: [
        'Проверка доступности D-Bus сессии через is_portal_available()',
        'Сборка корректных параметров PortalSourceTypes::MONITOR_AND_WINDOW',
        'Покрыто контрактом test_screencast_portal_options_contract'
      ]
    },
    {
      id: 'TASK-744',
      title: 'Нативные D-Bus уведомления рабочего стола и системный трей Linux',
      status: 'done',
      priority: 'P2',
      department: 'G5 Voice/Platform',
      summary: 'Интеграция freedesktop D-Bus уведомлений (org.freedesktop.Notifications) с кнопками «Открыть» и «Прочитано», а также системного трея Linux (TrayController) с 6-пунктовым меню Discord и бейджем количества непрочитанных сообщений 1..99+.',
      layout: {
        container: 'System D-Bus Notifications & Linux StatusNotifierItem Tray',
        dimensions: 'Standard desktop notification popup (320px) + Tray Menu (6 items)',
        padding: 'N/A',
        spacing: 'N/A'
      },
      design_tokens: {
        background: '#2b2d31',
        surface: '#1e1f22',
        text_primary: '#ffffff',
        accent: '#5865f2'
      },
      interactive_states: [
        { element: 'Иконка трея', default: 'discord-tray (Normal)', hover: 'discord-tray-unread (Unread)', active: 'discord-tray-speaking (InCall)' },
        { element: 'Кнопка «Открыть» в уведомлении', default: 'Default action', hover: 'Highlight', active: 'Focus main GPUI window' }
      ],
      acceptance_criteria: [
        'Уведомление содержит app_name = "Discord", заголовок "Автор (#канал)" и текст сообщения',
        'Бейдж лаунчера отображает точное число до 99 и "99+" при превышении',
        'Покрыто контрактом test_desktop_notification_formatting_contract'
      ]
    },
    {
      id: 'TASK-745',
      title: 'Сервер локального бинарного протокола Discord Local RPC IPC',
      status: 'done',
      priority: 'P2',
      department: 'G1 Backend',
      summary: 'Поддержка Unix domain socket $XDG_RUNTIME_DIR/discord-ipc-0..9 для интеграции с внешними играми и приложениями по эталону rust-discord-activity и Serein. Разбор рукопожатия Handshake (Opcode 0) и команды SET_ACTIVITY (Opcode 1).',
      layout: {
        container: 'Local Unix Domain Socket Listener',
        dimensions: 'Header 8 bytes [Opcode u32 LE][Length u32 LE] + JSON body up to 16 KiB',
        padding: 'N/A',
        spacing: 'N/A'
      },
      design_tokens: {
        background: 'N/A',
        surface: 'N/A',
        text_primary: 'N/A',
        accent: '#5865f2'
      },
      interactive_states: [
        { element: 'Game connect', default: 'Listening discord-ipc-0', hover: 'Handshake v=1', active: 'DISPATCH READY' },
        { element: 'SET_ACTIVITY', default: 'Idle', hover: 'Update rich presence details', active: 'User status updated' }
      ],
      acceptance_criteria: [
        'Парсинг бинарного фрейма и JSON-пейлоада SET_ACTIVITY',
        'Обновление полей игры (state, details, timestamps) в профиле пользователя',
        'Покрыто контрактом test_local_rpc_ipc_frame_contract'
      ]
    },
    {
      id: 'TASK-746',
      title: 'Автоматизированный регрессионный контрактный харнесс 61/61',
      status: 'done',
      priority: 'P0',
      department: 'G4 QA',
      summary: 'Полная регрессионная верификация всей кодовой базы: 61/61 контрактных тестов GPUI (REST API, Voice UDP, Screencast, D-Bus, SQLite, Audio CPAL, Design tokens, Escape hierarchy, Virtual chat) выполняются за 2.50 секунды со 100% успехом.',
      layout: {
        container: 'Headless GPUI Test Harness',
        dimensions: 'Fast headless execution in 2.5s',
        padding: '0ms sleep delays in mock mode',
        spacing: 'Zero-flake deterministic assertions'
      },
      design_tokens: {
        background: '#1e1f22',
        surface: '#23a55a',
        text_primary: '#ffffff',
        accent: '#23a55a'
      },
      interactive_states: [
        { element: 'cargo test --test gpui_contract_test', default: 'Running 61 tests', hover: 'Compiling', active: 'test result: ok. 61 passed; 0 failed' }
      ],
      acceptance_criteria: [
        '100% зеленых тестов (64/64)',
        '0 падений и регрессий',
        'Скомпилирован рабочий релизный бинарник target/release/discord-gpui (55 МБ)'
      ]
    },
    {
      id: 'TASK-747',
      title: 'Паритет стилей Discord Dark Design Polish и Hover-эффекты сообщений',
      status: 'done',
      priority: 'P1',
      department: 'G3 Design',
      summary: 'Стилизация ховеров строк сообщений ленты чата (rgba(255,255,255,0.03)), плавное скругление иконок серверов и индикаторной пилюли, тени контекстного меню по эталону официального Discord.',
      layout: {
        container: 'Chat message row',
        dimensions: '100% width, min-height 48px',
        padding: '6px 16px',
        spacing: '16px'
      },
      design_tokens: {
        background: '#313338',
        surface: 'rgba(255,255,255,0.04)',
        text_primary: '#ffffff',
        accent: '#5865f2'
      },
      interactive_states: [
        { element: 'Message Row', default: 'transparent', hover: 'bg: rgba(255,255,255,0.04)', active: 'highlight' },
        { element: 'Server Icon', default: 'rx: 24px', hover: 'rx: 16px, bg: #5865f2', active: 'scale: 0.96' }
      ],
      acceptance_criteria: [
        'Плавный фон при наведении мыши',
        'Скругление аватарок серверов при ховере'
      ]
    },
    {
      id: 'TASK-748',
      title: 'Аппаратный GPU-конвейер PipeWire кадра и захват экрана',
      status: 'done',
      priority: 'P1',
      department: 'G5 Voice/Platform',
      summary: 'Связывание D-Bus сессии XDG Desktop Portal ScreenCast и конвейера GpuVideoPipeline: получение remote fd PipeWire и node_id, валидация формата Rgba8 и отрисовка кадра на GPU без лишних копий.',
      layout: {
        container: 'PipeWire Stream Pipe & GpuTextureDescriptor',
        dimensions: '1920x1080 60 FPS',
        padding: '0ms delay',
        spacing: 'Zero-copy DMA'
      },
      design_tokens: {
        background: '#1e1f22',
        surface: '#2b2d31',
        text_primary: '#ffffff',
        accent: '#5865f2'
      },
      interactive_states: [
        { element: 'PipeWire FD Stream', default: 'Unconnected', hover: 'Portal selected', active: 'Connected fd=42 node_id=101' }
      ],
      acceptance_criteria: [
        'Успешная привязка fd и node_id в GpuVideoPipeline',
        'Покрыто тестом test_gpu_video_pipeline_pipewire_connector_contract'
      ]
    },
    {
      id: 'TASK-749',
      title: 'Плавающий бар быстрых действий над сообщением (Floating Action Bar)',
      status: 'done',
      priority: 'P1',
      department: 'G2 Frontend',
      summary: 'Плавающий тулбар быстрых действий в правом углу каждого сообщения при наведении: быстрая реакция 😀 (добавляет/снимает 👍), быстрый ответ 💬 (подставляет @Author: ), закрепление 📌 (инвертирует статус и шлёт REST запрос), кнопка «⋯» для открытия контекстного меню.',
      layout: {
        container: 'Floating flex action bar',
        dimensions: 'auto x 26px',
        padding: '2px 4px',
        spacing: '2px'
      },
      design_tokens: {
        background: '#2b2d31',
        surface: '#383a40',
        text_primary: '#ffffff',
        accent: '#5865f2',
        border: '1px solid rgba(255,255,255,0.08)'
      },
      interactive_states: [
        { element: 'btn-react', default: '😀', hover: 'bg: #35373c', active: 'toggle 👍 reaction' },
        { element: 'btn-reply', default: '💬', hover: 'bg: #35373c', active: 'draft: @Author:' },
        { element: 'btn-pin', default: '📌', hover: 'bg: #35373c', active: 'toggle is_pinned' },
        { element: 'btn-more', default: '⋯', hover: 'bg: #35373c', active: 'open context menu' }
      ],
      acceptance_criteria: [
        'Реализован тулбар в ChatView',
        'Клик на реакцию и пин покрыт тестом test_floating_message_action_bar_and_quick_reaction_contract'
      ]
    },
    {
      id: 'TASK-750',
      title: 'Шлюз Discord Gateway Opcode 4 (Voice State Update)',
      status: 'done',
      priority: 'P0',
      department: 'G1 Backend',
      summary: 'Автоматическая генерация и отправка пакетов шлюза Discord Opcode 4 (Voice State Update) при подключении к голосовому каналу (guild_id, channel_id, self_mute, self_deaf) и при отключении (channel_id = null).',
      layout: {
        container: 'Gateway WebSocket JSON packet',
        dimensions: 'Standard Discord Gateway protocol frame',
        padding: 'N/A',
        spacing: 'N/A'
      },
      design_tokens: {
        background: 'N/A',
        surface: 'N/A',
        text_primary: 'N/A',
        accent: '#5865f2'
      },
      interactive_states: [
        { element: 'Connect to Voice', default: 'Disconnected', hover: 'Click voice channel', active: 'Send Opcode 4 with channel_id' },
        { element: 'Disconnect', default: 'Connected', hover: 'Click disconnect 📞✕', active: 'Send Opcode 4 with channel_id: null' }
      ],
      acceptance_criteria: [
        'Генерация валидного JSON payload Opcode 4',
        'Покрыто тестом test_voice_state_update_opcode_4_gateway_contract'
      ]
    },
    {
      id: 'TASK-751',
      title: 'G1 Backend + G7 REA: Устойчивый парсер read_state.last_acked_id (Snowflake String/Integer/Null)',
      status: 'in_dev',
      priority: 'P1',
      department: 'G1 Backend',
      summary: 'Устранение критической ошибки парсинга read_state, обнаруженной живым GDB-трейсером: поле last_acked_id в entries[31] пакета READY приходит как integer/null/string. Замена жесткого требования строки на гибкий десериализатор Snowflake по эталону re-deep/gateway-schema.md.',
      layout: {
        container: 'Channel Read State Marker & Unread Badge',
        dimensions: 'JSON Snowflake u64 or String id',
        padding: 'N/A',
        spacing: 'N/A'
      },
      design_tokens: {
        background: '#2b2d31',
        surface: '#1e1f22',
        text_primary: '#ffffff',
        accent: '#f23f43'
      },
      interactive_states: [
        { element: 'openChannel click', default: 'Load read_state', hover: 'Parse Snowflake', active: 'Badge cleared without error alert' },
        { element: 'chat-ack-note alert', default: 'Hidden when valid', hover: 'N/A', active: 'No false-positive error banner' }
      ],
      acceptance_criteria: [
        'read_state успешно парсит entries с целочисленным и строковым last_acked_id',
        'Красный баннер «Не удалось прочитать состояние канала» не отображается',
        'Покрыто контрактным тестом на фикстуре 002-READY.json'
      ]
    },
    {
      id: 'TASK-752',
      title: 'G2 Frontend: Оптимизация задержки загрузки сообщений messages_page (< 300 мс)',
      status: 'in_dev',
      priority: 'P2',
      department: 'G2 Frontend',
      summary: 'Устранение задержки 1236 мс, зафиксированной живой телеметрией при клике на личный диалог ShadowByte. Асинхронное чтение из кэша памяти до блокирующего чтения с диска, виртуализация чанками по 50 сообщений.',
      layout: {
        container: 'Virtual Chat Message List',
        dimensions: 'Width: 100%, Height: 100%',
        padding: '16px',
        spacing: '16px'
      },
      design_tokens: {
        background: '#313338',
        surface: '#2b2d31',
        text_primary: '#ffffff',
        accent: '#5865f2'
      },
      interactive_states: [
        { element: 'Click on Channel / DM', default: 'Skeleton loader', hover: 'Preload cache', active: 'Render messages < 300ms' }
      ],
      acceptance_criteria: [
        'Время выполнения messages_page сокращено с 1236 мс до < 300 мс',
        'Плавная прокрутка без рывков интерфейса'
      ]
    }
  ]
};

// Экспорт в JSON и HTML
fs.mkdirSync(path.dirname(BOARD_FILE), { recursive: true });
fs.writeFileSync(BOARD_FILE, JSON.stringify(board, null, 2), 'utf-8');

// Вызов функции экспорта HTML из mcp-kanban-board.mjs
import { execSync } from 'node:child_process';
execSync('node scripts/mcp-kanban-board.mjs --export');
console.log(`[Kanban Population] Successfully loaded ${board.cards.length} structured design-first cards into ${BOARD_FILE}`);
console.log(`[Kanban Population] Generated interactive visual HTML board at ${EXPORT_HTML_FILE}`);
