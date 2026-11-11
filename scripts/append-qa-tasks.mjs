#!/usr/bin/env node
// @ts-check
import fs from "node:fs";
import path from "node:path";
import { execSync } from "node:child_process";

const ROOT = "/home/sanya/orca/project";
const BOARD_FILE = path.join(ROOT, "waves/kanban-board.json");
const EXPORT_HTML_FILE = path.join(ROOT, "waves/kanban-board.html");

const kb = JSON.parse(fs.readFileSync(BOARD_FILE, "utf8"));

const newQaTasks = [
  {
    id: "TASK-901",
    title: "Архитектурный каркас нативного E2E-харнесса (Virtual Wayland/Cage + Mock Gateway Daemon)",
    department: "g4_qa",
    status: "backlog",
    priority: "P0",
    summary: "Создание полностью изолированной среды для сквозных тестов native Rust GPUI без реального экрана и внешних серверов: локальный виртуальный композитор Wayland/Cage, локальный Tokio Mock Gateway с воспроизведением дампов capture-ready.jsonl.",
    layout: {
      container: "E2E Test Harness & Daemon",
      dimensions: "Virtual 1920x1080 Headless Display",
      padding: "0px",
      spacing: "0px"
    },
    design_tokens: {
      background: "#1e1f22",
      surface: "#2b2d31",
      text_primary: "#ffffff",
      accent: "#5865f2"
    },
    interactive_states: [
      { element: "VirtualDisplay", default: "WAYLAND_DISPLAY=wayland-test-0", hover: "cage -d", active: "active" },
      { element: "MockGateway", default: "127.0.0.1:9002 WS/HTTP", hover: "record-replay", active: "streaming ready" }
    ],
    subtasks: [
      {
        id: "SUB-901-1",
        title: "Изолированный композитор Wayland в оперативной памяти",
        micro_steps: [
          "Шаг 1.1: Настройка запуска cage/wlroots без физического монитора через скрипт scripts/run-headless-compositor.sh",
          "Шаг 1.2: Интеграция программного рендерера llvmpipe (LIBGL_ALWAYS_SOFTWARE=1) для стабильного рендеринга на CI машинах без GPU",
          "Шаг 1.3: Автоматический захват и аллокация виртуального сокета wayland-99 с изоляцией от рабочего стола пользователя"
        ]
      },
      {
        id: "SUB-901-2",
        title: "Локальный Mock Discord Gateway Daemon на базе Tokio",
        micro_steps: [
          "Шаг 2.1: Реализация HTTP/2 Mock сервера для REST v9 маршрутов (/users/@me, /guilds, /channels/{id}/messages)",
          "Шаг 2.2: Реализация WebSocket сервера с поддержкой Discord Gateway v9 (Heartbeat ACK, Opcode 2 IDENTIFY, Opcode 4 VOICE_STATE_UPDATE)",
          "Шаг 2.3: Загрузчик эталонных трасс /home/sanya/re-deep/capture-ready.jsonl для 100% реалистичной гидратации состояния"
        ]
      },
      {
        id: "SUB-901-3",
        title: "E2E LifeCycle Runner & Process Orchestrator",
        micro_steps: [
          "Шаг 3.1: Rust крейт тестового раннера tests/e2e/harness/runner.rs с автоматическим стартом и graceful shutdown",
          "Шаг 3.2: Перехват логов stdout/stderr и дамп бэктрейсов при панике бинарника",
          "Шаг 3.3: Проверка отсутствия зависших зомби-процессов и файловых дескрипторов после завершения сценария"
        ]
      }
    ],
    acceptance_criteria: [
      "Запуск полного цикла E2E без физического дисплея и без интернета",
      "Время развертывания виртуального окружения < 300 мс",
      "0 паник, 0 зомби-процессов, детерминированное воспроизведение"
    ]
  },
  {
    id: "TASK-902",
    title: "Семантический драйвер GUI и автоматизация тестирования через D-Bus AT-SPI",
    department: "g4_qa",
    status: "backlog",
    priority: "P0",
    summary: "Автоматизированное управление и инспекция нативного интерфейса GPUI через системное дерево доступности Linux (AT-SPI / org.a11y.Bus) без хрупких координат и привязки к пикселям.",
    layout: {
      container: "Accessibility Tree & Action Driver",
      dimensions: "Full Window Tree Hierarchy",
      padding: "0px",
      spacing: "0px"
    },
    design_tokens: {
      background: "#1e1f22",
      surface: "#2b2d31",
      text_primary: "#ffffff",
      accent: "#23a55a"
    },
    interactive_states: [
      { element: "AtSpiBus", default: "org.a11y.Bus connected", hover: "query nodes", active: "action dispatched" }
    ],
    subtasks: [
      {
        id: "SUB-902-1",
        title: "Экспозиция дерева доступности в компонентах GPUI",
        micro_steps: [
          "Шаг 1.1: Проверка экспозиции Accessible Roles для всех кнопок, инпутов и списков (Role::PushButton, Role::Text, Role::List)",
          "Шаг 1.2: Привязка Accessible Name и Accessible Description из i18n-каталога без хардкода",
          "Шаг 1.3: Поддержка состояний компонентов (State::Focused, State::Selected, State::Checked, State::Disabled)"
        ]
      },
      {
        id: "SUB-902-2",
        title: "Реализация высокоуровневого Page-Object DSL (AppDriver)",
        micro_steps: [
          "Шаг 2.1: Метод driver.find_by_role_and_name('button', 'Войти') с детерминированным ожиданием появления элемента",
          "Шаг 2.2: Метод driver.click(element) через вызов метода D-Bus do_action(0)",
          "Шаг 2.3: Метод driver.type_text(element, 'Текст') с эмуляцией посимвольного ввода и проверкой реактивного редьюсера",
          "Шаг 2.4: Метод driver.press_hotkey('Ctrl+K') для проверки клавиатурных сочетаний"
        ]
      },
      {
        id: "SUB-902-3",
        title: "Автоматизированный аудит доступности WCAG 2.2",
        micro_steps: [
          "Шаг 3.1: Валидация контрастности цветов текста ко всем фонам (минимальный коэффициент 4.5:1 для нормального текста, 7:1 для AAA)",
          "Шаг 3.2: Тестирование сквозной навигации исключительно с клавиатуры (Tab, Shift+Tab, Enter, Space, Escape, Стрелки)",
          "Шаг 3.3: Проверка отсутствия 'клавиатурных ловушек' (Focus Traps) во всех модальных окнах"
        ]
      }
    ],
    acceptance_criteria: [
      "100% ключевых экранов доступны через AT-SPI семантическое дерево",
      "DSL Driver позволяет кликать и вводить текст без использования координат мыши",
      "Соответствие стандарту доступности WCAG 2.2 Level AA/AAA"
    ]
  },
  {
    id: "TASK-903",
    title: "Комплексный фаззинг сетевых протоколов и криптографии DAVE MLS v1 (cargo-fuzz)",
    department: "g4_qa",
    status: "backlog",
    priority: "P0",
    summary: "Внедрение непрерывного фаззинг-тестирования на базе libFuzzer/cargo-fuzz и AddressSanitizer для проверки сетевых парсеров, RTP медиа-декодеров и сквозного шифрования DAVE E2EE на устойчивость к поврежденным и враждебным пакетам.",
    layout: {
      container: "Fuzzing Harness Suite",
      dimensions: "Headless / CLI",
      padding: "0px",
      spacing: "0px"
    },
    design_tokens: {
      background: "#1e1f22",
      surface: "#2b2d31",
      text_primary: "#ffffff",
      accent: "#f0b232"
    },
    interactive_states: [
      { element: "LibFuzzer", default: "corpus loaded", hover: "generating inputs", active: "10M execs / 0 crash" }
    ],
    subtasks: [
      {
        id: "SUB-903-1",
        title: "Фаззинг-мишени для сетевых парсеров Discord Gateway и Voice",
        micro_steps: [
          "Шаг 1.1: Создание fuzz-мишени fuzz_gateway_json_etf для парсера входящих сообщений шлюза",
          "Шаг 1.2: Создание fuzz-мишени fuzz_voice_udp_packets для 74-байтных IP Discovery и 8-байтных Keepalive пакетов",
          "Шаг 1.3: Создание fuzz-мишени fuzz_rtp_header_extensions для парсера заголовков RTP (0xBEDE, One-Byte/Two-Byte Header)"
        ]
      },
      {
        id: "SUB-903-2",
        title: "Фаззинг сквозного шифрования Discord DAVE E2EE MLS v1",
        micro_steps: [
          "Шаг 2.1: Фаззинг дешифратора Aes128GcmTruncated8 на случайных и усеченных байтах с проверкой отсутствия утечек памяти",
          "Шаг 2.2: Проверка обработки поврежденных MLS Commit и Proposal сообщений в DaveVoiceSession",
          "Шаг 2.3: Тестирование защиты от атак повторного воспроизведения (Anti-Replay sliding window) на переполнениях u64"
        ]
      },
      {
        id: "SUB-903-3",
        title: "Автоматизированная интеграция в CI и сбор краш-корпуса",
        micro_steps: [
          "Шаг 3.1: Настройка запуска cargo fuzz run на 1 000 000 итераций в ночном CI пайплайне",
          "Шаг 3.2: Сохранение и минимизация корпуса интересных входных данных в tests/fuzz/corpus/",
          "Шаг 3.3: Автоматический генератор отчёта при обнаружении паники или out-of-bounds чтения"
        ]
      }
    ],
    acceptance_criteria: [
      "Минимум 10 000 000 итераций фаззинга без единого крэша, паники или утечки памяти",
      "AddressSanitizer (ASan) подтверждает чистоту работы с памятью",
      "Все сбои парсинга возвращают явный Err, а не panic!"
    ]
  },
  {
    id: "TASK-904",
    title: "Автоматизация визуальной регрессии GPUI на базе SSIM (>0.99) и маскирования",
    department: "g4_qa",
    status: "backlog",
    priority: "P1",
    summary: "Система автоматического визуального сравнения экранов и компонентов GPUI с эталонными снимками (Goldens) с использованием алгоритма структурного сходства SSIM (Structural Similarity Index) с порогом > 0.99 и маскированием динамических областей.",
    layout: {
      container: "Visual Snapshot & Perceptual Diff",
      dimensions: "1920x1080 & 1280x720 Golden Frames",
      padding: "0px",
      spacing: "0px"
    },
    design_tokens: {
      background: "#1e1f22",
      surface: "#2b2d31",
      text_primary: "#ffffff",
      accent: "#eb459e"
    },
    interactive_states: [
      { element: "SnapshotComparator", default: "baseline loaded", hover: "diffing pixels", active: "SSIM: 0.998 PASS" }
    ],
    subtasks: [
      {
        id: "SUB-904-1",
        title: "Генератор детерминированных скриншотов из FrameBuffer",
        micro_steps: [
          "Шаг 1.1: Снятие RGBA пикселей напрямую из виртуального буфера кадра GPUI/WGPU без сжатия",
          "Шаг 1.2: Принудительное отключение антиалиасинга шрифтов и субпиксельного сглаживания в тестовом режиме для кросс-платформенной стабильности",
          "Шаг 1.3: Фиксация системных часов (Frozen Clock) для устранения рассинхрона времени в сообщениях"
        ]
      },
      {
        id: "SUB-904-2",
        title: "Алгоритм перцептивного сравнения SSIM с масками исключений",
        micro_steps: [
          "Шаг 2.1: Реализация модуля вычисления Structural Similarity Index (SSIM) с порогом допуска 0.995",
          "Шаг 2.2: Определение JSON-масок динамических регионов (прямоугольники таймстемпов, аватарок, индикаторов FPS и пинга)",
          "Шаг 2.3: Генерация наглядных HTML-диффов с наложением красных пикселей расхождений (Visual Diff Report)"
        ]
      },
      {
        id: "SUB-904-3",
        title: "Базовый набор золотых эталонов (Golden Snapshots)",
        micro_steps: [
          "Шаг 3.1: Снятие эталонов главного окна в темной теме Discord Dark при 1280x720 и 1920x1080",
          "Шаг 3.2: Снятие эталонов диалога настроек SettingsDialog для всех 8 вкладок",
          "Шаг 3.3: Снятие эталонов модальных окон CaptchaModal, CreateGuildModal, ScreenShareModal",
          "Шаг 3.4: Снятие эталонов контекстного меню ПКМ и всплывающих тултипов профилей"
        ]
      }
    ],
    acceptance_criteria: [
      "SSIM > 0.99 для всех утвержденных экранов и компонентов",
      "Отсутствие ложных срабатываний из-за часов или динамических аватарок",
      "Генерация интерактивного HTML отчёта с режимами сравнения Slider / Onion Skin"
    ]
  },
  {
    id: "TASK-905",
    title: "Стенд Chaos Engineering: сетевые обрывы, джиттер 20-60мс и аварийное восстановление",
    department: "g4_qa",
    status: "backlog",
    priority: "P1",
    summary: "Стресс-тестирование надежности и отказоустойчивости приложения в экстремальных условиях: внезапные обрывы WebSocket, искусственная потеря 5-30% UDP пакетов, скачки джиттера, отказ PipeWire демона и переключение аудиоустройств на лету.",
    layout: {
      container: "Chaos Engineering Lab",
      dimensions: "Automated Fault Injection Suite",
      padding: "0px",
      spacing: "0px"
    },
    design_tokens: {
      background: "#1e1f22",
      surface: "#2b2d31",
      text_primary: "#ffffff",
      accent: "#f0b232"
    },
    interactive_states: [
      { element: "FaultInjector", default: "monitoring", hover: "dropping packets", active: "reconnect verified" }
    ],
    subtasks: [
      {
        id: "SUB-905-1",
        title: "Сетевой хаос-инжектор (Network Fault Injection)",
        micro_steps: [
          "Шаг 1.1: Эмуляция обрыва TCP соединения шлюза (Socket Reset / EOF) каждые N секунд",
          "Шаг 1.2: Проверка плавного перехода в режим RESUME (Opcode 6) с сохранением seq и сессионного токена",
          "Шаг 1.3: Искусственное внедрение HTTP 429 Too Many Requests со случайным retry_after_ms для проверки экспоненциального бэкоффа",
          "Шаг 1.4: Инжекция 5%, 15% и 30% потерь UDP пакетов голоса для проверки адаптивного битрейта Opus и PLC"
        ]
      },
      {
        id: "SUB-905-2",
        title: "Аппаратный и системный хаос (Audio/Video Driver Faults)",
        micro_steps: [
          "Шаг 2.1: Внезапное удаление активного аудиоустройства в процессе звонка и автоматическое переключение на Default",
          "Шаг 2.2: Имитация аварийного перезапуска демона PipeWire во время активной трансляции экрана",
          "Шаг 2.3: Тестирование нехватки памяти (OOM Injection) и проверка сброса кэша изображений MediaCache"
        ]
      },
      {
        id: "SUB-905-3",
        title: "Метрики живучести и целостности состояния (State Integrity Audit)",
        micro_steps: [
          "Шаг 3.1: Проверка отсутствия дубликатов сообщений в ленте после серии реконнектов",
          "Шаг 3.2: Проверка сохранения черновика текста сообщения при внезапном закрытии модалок",
          "Шаг 3.3: Проверка корректного освобождения файловых дескрипторов и сокетов после сбоев"
        ]
      }
    ],
    acceptance_criteria: [
      "100% автоматическое восстановление после разрыва соединения шлюза без потери сообщений",
      "Аудио-стек продолжает работу при смене устройств ввода/вывода без падения процесса",
      "0 утечек ресурсов и дескрипторов при 100 последовательных циклах сбоя"
    ]
  },
  {
    id: "TASK-906",
    title: "Бенчмаркинг 120 FPS и профилирование утечек памяти в реальном времени (Heap/RSS < 60 MB)",
    department: "g4_qa",
    status: "backlog",
    priority: "P0",
    summary: "Автоматизированные замеры производительности графического конвейера и потребления ресурсов: непрерывный мониторинг времени рендеринга кадра (<8.33 мс для 120 FPS), профилирование аллокаций памяти кучи через jemalloc/valgrind, исключение утечек в длинных сессиях.",
    layout: {
      container: "Performance & Profiling Suite",
      dimensions: "Continuous Perf Telemetry",
      padding: "0px",
      spacing: "0px"
    },
    design_tokens: {
      background: "#1e1f22",
      surface: "#2b2d31",
      text_primary: "#ffffff",
      accent: "#23a55a"
    },
    interactive_states: [
      { element: "PerfMonitor", default: "measuring", hover: "analyzing p95/p99", active: "Frame: 1.2ms | RAM: 18MB" }
    ],
    subtasks: [
      {
        id: "SUB-906-1",
        title: "Непрерывный замер бюджета кадра 120 FPS (<8.33 мс)",
        micro_steps: [
          "Шаг 1.1: Внедрение таймеров высокой точности (High-Resolution Clock) на каждом шаге цикла layout & render",
          "Шаг 1.2: Сбор перцентилей p50, p95, p99 времени кадра при скроллинге чата на 100 000 сообщений",
          "Шаг 1.3: Автоматический фейл теста при превышении времени отрисовки > 8.33 мс более чем в 0.1% кадров"
        ]
      },
      {
        id: "SUB-906-2",
        title: "Профилирование памяти и обнаружение утечек (Memory Leak Detection)",
        micro_steps: [
          "Шаг 2.1: Снятие профилей аллокаций кучи (Heap Profiling) через jemalloc / heaptrack в процессе 24-часового стресс-теста",
          "Шаг 2.2: Проверка стабильности VmRSS (Resident Set Size) под нагрузкой: жесткий лимит < 60 МБ для всего приложения",
          "Шаг 2.3: Тестирование сборки мусора и выгрузки неиспользуемых текстур аватарок и вложений из GPU VRAM"
        ]
      },
      {
        id: "SUB-906-3",
        title: "Бенчмарк скорости холодного старта (Cold Start Time)",
        micro_steps: [
          "Шаг 3.1: Замер времени от старта процесса до отрисовки первого интерактивного кадра First Meaningful Paint",
          "Шаг 3.2: Бюджет холодного старта: < 150 мс при прогретом кэше SQLite, < 300 мс при чистом запуске",
          "Шаг 3.3: Публикация графиков перформанса в артефакты CI сборки"
        ]
      }
    ],
    acceptance_criteria: [
      "99.9% кадров укладываются в бюджет 8.33 мс (120 FPS)",
      "VmRSS стабильно ниже 60 МБ при непрерывной работе",
      "0 растущих утечек памяти при многократном открытии/закрытии модалок и переключении каналов"
    ]
  },
  {
    id: "TASK-907",
    title: "Автоматизированный гейт качества кода: Clippy, ASan/TSan санитайзеры и 5/5 мутационный гейт",
    department: "g4_qa",
    status: "backlog",
    priority: "P0",
    summary: "Строгий интеграционный пайплайн контроля качества кода перед каждым коммитом и PR: линтинг с нулевой терпимостью к предупреждениям, санитайзеры многопоточности и памяти (Address/Thread Sanitizer), мутационное тестирование с обязательным уничтожением 100% мутаций (5/5).",
    layout: {
      container: "CI Quality Gate Pipeline",
      dimensions: "Automated Pre-Commit / Pre-Merge Gate",
      padding: "0px",
      spacing: "0px"
    },
    design_tokens: {
      background: "#1e1f22",
      surface: "#2b2d31",
      text_primary: "#ffffff",
      accent: "#5865f2"
    },
    interactive_states: [
      { element: "QualityGate", default: "inspecting", hover: "running sanitizers", active: "5/5 mutations killed PASS" }
    ],
    subtasks: [
      {
        id: "SUB-907-1",
        title: "Статический анализ и строгий линтинг Rust",
        micro_steps: [
          "Шаг 1.1: Настройка cargo clippy --all-targets -- -D warnings для блокировки любого коммита с варнингами",
          "Шаг 1.2: Проверка форматирования через rustfmt --check --edition 2024",
          "Шаг 1.3: Аудит уязвимостей зависимостей через cargo audit и проверка лицензий через cargo-deny",
          "Шаг 1.4: Статический контроль размера исходников: функции < 50 строк, файлы модулей < 800 строк"
        ]
      },
      {
        id: "SUB-907-2",
        title: "Динамические санитайзеры LLVM (ASan & TSan)",
        micro_steps: [
          "Шаг 2.1: Сборка и прогон контрактных тестов с RUSTFLAGS='-Zsanitizer=address' для поиска use-after-free и buffer overflow",
          "Шаг 2.2: Сборка и прогон с RUSTFLAGS='-Zsanitizer=thread' для выявления гонок данных (Data Races) в многопоточных кольцевых буферах",
          "Шаг 2.3: Проверка отсутствия дедлоков в связке Tokio <-> GPUI Event Loop"
        ]
      },
      {
        id: "SUB-907-3",
        title: "Мутационный гейт тестирования (Mutation Testing Gate)",
        micro_steps: [
          "Шаг 3.1: Автоматическая генерация 5 контрольных мутаций на каждый изменяемый модуль (инверсия условий, замена констант, пропуск валидации)",
          "Шаг 3.2: Запуск таргетированных тестов против мутированного кода",
          "Шаг 3.3: Обязательное условие приёмки: ровно 5 из 5 мутаций должны быть пойманы и убиты (100% kill rate)"
        ]
      }
    ],
    acceptance_criteria: [
      "Clippy: 0 warnings, rustfmt: clean",
      "ASan и TSan подтверждают отсутствие ошибок памяти и гонок потоков",
      "Мутационный гейт: ровно 5/5 мутаций уничтожено"
    ]
  },
  {
    id: "TASK-908",
    title: "Регламент, чек-листы и сценарии ручного исследовательского тестирования (Manual QA Suite)",
    department: "g4_qa",
    status: "backlog",
    priority: "P1",
    summary: "Формализованный регламент ручного тестирования, чек-листы приемки функционала (Definition of Done), сценарии исследовательского тестирования (Exploratory Testing) и процедуры регулярных Bug-bash сессий перед релизом на различных графических окружениях Linux.",
    layout: {
      container: "Manual QA Protocols & Checklists",
      dimensions: "Documentation & Test Scenarios",
      padding: "0px",
      spacing: "0px"
    },
    design_tokens: {
      background: "#1e1f22",
      surface: "#2b2d31",
      text_primary: "#ffffff",
      accent: "#4e5058"
    },
    interactive_states: [
      { element: "ManualQA", default: "checklist ready", hover: "running bug-bash", active: "sign-off complete" }
    ],
    subtasks: [
      {
        id: "SUB-908-1",
        title: "Чек-листы приемочного тестирования (Smoke & Sanity Checklists)",
        micro_steps: [
          "Шаг 1.1: Чек-лист авторизации (вход по паролю, QR-код, капча 40003, сохранение токена, выход из аккаунта)",
          "Шаг 1.2: Чек-лист чата (отправка текста, эмодзи, вложения до 25 МБ, закрепление, редактирование, удаление, цитирование)",
          "Шаг 1.3: Чек-лист голоса (подключение, mute/deafen, переключение аудиоустройств, Push-to-Talk, индикатор громкости)",
          "Шаг 1.4: Чек-лист шаринга экрана (выбор экранов, окон, пресеты 720p/1080p, переключение в процессе стрима)"
        ]
      },
      {
        id: "SUB-908-2",
        title: "Кросс-платформенная матрица тестирования Linux Desktop",
        micro_steps: [
          "Шаг 2.1: Тестирование в GNOME 46/47 Wayland (проверка интеграции с треем AppIndicator и нативными уведомлениями)",
          "Шаг 2.2: Тестирование в KDE Plasma 6 Wayland (проверка PipeWire portal и масштабирования Fractional Scaling 125%/150%)",
          "Шаг 2.3: Тестирование в Hyprland / Sway (тилинговые оконные менеджеры, корректность размеров и ресайза окон)",
          "Шаг 2.4: Тестирование в классической сессии X11 (Xfce / i3) с проверкой fallback захвата экрана X11 SHM"
        ]
      },
      {
        id: "SUB-908-3",
        title: "Сценарии исследовательского тестирования (Exploratory Testing Charters)",
        micro_steps: [
          "Шаг 3.1: Сценарий стрессового ввода: длинные сообщения (2000 символов), спецсимволы Unicode, RTL-текст, битые Markdown теги",
          "Шаг 3.2: Сценарий быстрого переключения серверов и каналов (Monkey Testing мышью со скоростью 10 кликов в секунду)",
          "Шаг 3.3: Сценарий работы с нестабильным интернетом: физическое отключение кабеля и включение через 30 секунд",
          "Шаг 3.4: Фиксация дефектов в баг-трекере с воспроизводимыми шагами, скриншотами, логами GDB и классификацией P0-P3"
        ]
      }
    ],
    acceptance_criteria: [
      "Полный комплект чек-листов покрывает 100% пользовательских сценариев клиента",
      "Проверена совместимость с 4 основными оконными менеджерами Linux (GNOME, KDE, Hyprland, X11)",
      "Процедура Bug-bash сессий стандартизирована и готова к регулярному использованию перед релизами"
    ]
  }
];

// Добавляем новые задачи в cards
for (const task of newQaTasks) {
  // Проверяем, нет ли уже такой карточки
  const idx = kb.cards.findIndex((/** @type {any} */ c) => c.id === task.id);
  if (idx >= 0) {
    kb.cards[idx] = task;
  } else {
    kb.cards.push(task);
  }
}

kb.updated_at = new Date().toISOString();

fs.writeFileSync(BOARD_FILE, JSON.stringify(kb, null, 2), "utf-8");
execSync("node scripts/mcp-kanban-board.mjs --export");

console.log(`[QA Tasks Ingestion] Added ${newQaTasks.length} highly-detailed QA automation tasks to ${BOARD_FILE}`);
console.log(`[QA Tasks Ingestion] Total cards on board now: ${kb.cards.length}`);
console.log(`[QA Tasks Ingestion] Re-exported HTML board at ${EXPORT_HTML_FILE}`);
