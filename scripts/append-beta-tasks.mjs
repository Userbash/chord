#!/usr/bin/env node
// @ts-check
import fs from "node:fs";
import path from "node:path";

const ROOT = "/home/sanya/orca/project";
const BOARD_FILE = path.join(ROOT, "waves/kanban-board.json");
const EXPORT_HTML_FILE = path.join(ROOT, "waves/kanban-board.html");

const kb = JSON.parse(fs.readFileSync(BOARD_FILE, "utf8"));

const newBetaTasks = [
  {
    id: "TASK-951",
    title: "Формирование централизованного манифеста сборки BUILD-005 и подготовка слияния unstable -> testing",
    department: "devops",
    status: "backlog",
    priority: "P0",
    summary: "Консолидация всех 128 завершённых задач в ветке unstable, проверка чистоты рабочего дерева, формирование машиночитаемого манифеста waves/BUILD-005.json и слияние в ветку testing с тегом build-005-candidate.",
    layout: {
      container: "Release Build Manifest & Branch Management",
      dimensions: "Repository Wide (unstable -> testing)",
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
      { element: "GitBranch", default: "unstable", hover: "merge --no-ff", active: "testing" },
      { element: "BuildManifest", default: "BUILD-005.json", hover: "schema validation", active: "tagged build-005-candidate" }
    ],
    subtasks: [
      {
        id: "SUB-951-1",
        title: "Сверка и слияние рабочих веток в unstable",
        micro_steps: [
          "Шаг 1.1: Запустить git status и убедиться в отсутствии незафиксированных артефактов вне .gitignore",
          "Шаг 1.2: Проверить статус всех 128 задач на канбан-доске — все задачи должны быть в статусе done",
          "Шаг 1.3: Выполнить слияние рабочих веток в unstable с флагом --no-ff для сохранения истории ревью"
        ]
      },
      {
        id: "SUB-951-2",
        title: "Формирование манифеста сборки BUILD-005.json",
        micro_steps: [
          "Шаг 2.1: Собрать список всех 128 выполненных задач с указанием ID, названия, отдела и даты закрытия",
          "Шаг 2.2: Зафиксировать SHA-хеш коммита ветки unstable и дату сборки",
          "Шаг 2.3: Записать файл waves/BUILD-005.json и верифицировать его JSON-схему"
        ]
      },
      {
        id: "SUB-951-3",
        title: "Перенос в ветку testing и установка контрольного тега",
        micro_steps: [
          "Шаг 3.1: Переключиться на ветку testing (git checkout testing)",
          "Шаг 3.2: Выполнить слияние unstable -> testing",
          "Шаг 3.3: Выставить легковесный Git-тег build-005-candidate"
        ]
      }
    ],
    acceptance_criteria: [
      "Ветка testing синхронизирована с unstable без конфликтов слияния",
      "Файл waves/BUILD-005.json содержит полный реестр всех 128 задач со статусами и коммитами",
      "Тег build-005-candidate указывает на вершину ветки testing"
    ],
    column: "backlog"
  },
  {
    id: "TASK-952",
    title: "Сквозной интеграционный гейт T2/T3 в ветке testing (scripts/ci.sh & scripts/gate.mjs)",
    department: "g4_qa",
    status: "backlog",
    priority: "P0",
    summary: "Полный безоговорочный прогон всех уровней проверок конвейера в ветке testing: cargo fmt, clippy -D warnings, 391+ тестов, GPUI headless smoke, E2E journeys, SSIM visual regression и live-keyring.",
    layout: {
      container: "Centralized CI & Verification Gate",
      dimensions: "Full Test Harness T0-T4",
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
      { element: "CiRunner", default: "scripts/ci.sh", hover: "--offline", active: "exit code 0" },
      { element: "GateRunner", default: "scripts/gate.mjs --scope release", hover: "failOnSkips: true", active: "passed" }
    ],
    subtasks: [
      {
        id: "SUB-952-1",
        title: "Локальный базовый конвейер (scripts/ci.sh)",
        micro_steps: [
          "Шаг 1.1: Запустить cargo fmt --all -- --check по всему воркспейсу (0 расхождений)",
          "Шаг 1.2: Запустить cargo clippy --workspace --all-targets --all-features -- -D warnings (0 варнингов)",
          "Шаг 1.3: Выполнить bash scripts/ci.sh и зафиксировать зелёный статус всех 391+ тестов"
        ]
      },
      {
        id: "SUB-952-2",
        title: "Релизный гейт с проверкой аппаратного хранилища (scripts/gate.mjs)",
        micro_steps: [
          "Шаг 2.1: Запустить node scripts/gate.mjs --scope release с переменной DISCORD_LAB_KEYRING_LIVE=1",
          "Шаг 2.2: Проверить прохождение всех фаз (gpui-smoke, gpui-e2e, gpui-visual, keyring-live)",
          "Шаг 2.3: Убедиться, что количество пропущенных тестов (skipped) строго равно 0"
        ]
      },
      {
        id: "SUB-952-3",
        title: "Оформление манифеста качества QA-BUILD-005.md",
        micro_steps: [
          "Шаг 3.1: Зафиксировать точное время прогона и потребление памяти",
          "Шаг 3.2: Оформить отчёт waves/RW-44/QUALITY/QA-BUILD-005.md с вердиктом QA Passed"
        ]
      }
    ],
    acceptance_criteria: [
      "Скрипт scripts/ci.sh завершился с кодом 0",
      "scripts/gate.mjs --scope release вернул вердикт passed (без skips)",
      "Подписан документ QA-BUILD-005.md с вердиктом QA Passed"
    ],
    column: "backlog"
  },
  {
    id: "TASK-953",
    title: "Автономная верификация системных библиотек ldd и совместимости с дистрибутивами Linux",
    department: "devops",
    status: "backlog",
    priority: "P1",
    summary: "Инспекция динамических зависимостей бинарника discord-gpui через ldd/objdump, проверка совместимости с glibc <= 2.35 и тестирование холодного старта при программном рендере llvmpipe.",
    layout: {
      container: "Binary Linkage & ABI Verification",
      dimensions: "ELF x86_64, glibc <= 2.35",
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
      { element: "LddInspector", default: "ldd discord-gpui", hover: "GLIBC_2.35 check", active: "0 missing symbols" },
      { element: "LlvmPipeFallback", default: "LIBGL_ALWAYS_SOFTWARE=1", hover: "swrast", active: "render OK" }
    ],
    subtasks: [
      {
        id: "SUB-953-1",
        title: "Инспекция зависимостей через ldd и objdump",
        micro_steps: [
          "Шаг 1.1: Запустить ldd rust/dist/discord-gpui и проверить список разделяемых библиотек",
          "Шаг 1.2: Проверить минимальную требуемую версию GLIBC (objdump -p | grep GLIBC_ <= 2.35)",
          "Шаг 1.3: Убедиться в отсутствии жесткой привязки к проприетарным библиотекам NVIDIA/AMD"
        ]
      },
      {
        id: "SUB-953-2",
        title: "Проверка софтверного рендеринга llvmpipe",
        micro_steps: [
          "Шаг 2.1: Запустить бинарник с LIBGL_ALWAYS_SOFTWARE=1 и MESA_LOADER_DRIVER_OVERRIDE=llvmpipe",
          "Шаг 2.2: Запустить --headless-smoke и убедиться в успешной инициализации WGPU OpenGL/Vulkan"
        ]
      },
      {
        id: "SUB-953-3",
        title: "Формирование отчёта системных требований",
        micro_steps: [
          "Шаг 3.1: Создать документ docs/SYSTEM-REQUIREMENTS.md с перечнем поддерживаемых версий ядра Linux (5.15+) и графических серверов"
        ]
      }
    ],
    acceptance_criteria: [
      "Требование GLIBC <= 2.35 соблюдено (Ubuntu 22.04 LTS, Debian 12, Fedora 38+)",
      "Бинарник успешно стартует на программном драйвере llvmpipe без падений",
      "Документ docs/SYSTEM-REQUIREMENTS.md зафиксирован в репозитории"
    ],
    column: "backlog"
  },
  {
    id: "TASK-954",
    title: "Матричное тестирование захвата экрана и звука под 4 композиторами Wayland/X11",
    department: "g4_qa",
    status: "backlog",
    priority: "P1",
    summary: "Сквозная проверка ScreenShareModal и PipeWire Portal под GNOME Mutter, KDE KWin, Hyprland/wlroots и X11 SHM fallback по контрактам manual_qa_suite_contract.rs.",
    layout: {
      container: "Screen Share & Desktop Portals Matrix",
      dimensions: "GNOME, KDE, Hyprland, X11",
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
      { element: "GnomePortal", default: "xdg-desktop-portal-gnome", hover: "NV12 60 FPS", active: "stream active" },
      { element: "KdePortal", default: "xdg-desktop-portal-kde", hover: "window picker", active: "stream active" },
      { element: "WlrPortal", default: "xdg-desktop-portal-wlr", hover: "pipewire node", active: "stream active" },
      { element: "X11Fallback", default: "XShm / XGetImage", hover: "legacy capture", active: "stream active" }
    ],
    subtasks: [
      {
        id: "SUB-954-1",
        title: "Тест портала GNOME (Mutter)",
        micro_steps: [
          "Шаг 1.1: Проверить вызов org.freedesktop.portal.ScreenCast через D-Bus с сессией GNOME",
          "Шаг 1.2: Убедиться в корректной передаче FD PipeWire и согласовании формата NV12 60 FPS"
        ]
      },
      {
        id: "SUB-954-2",
        title: "Тест портала KDE Plasma 6 (KWin)",
        micro_steps: [
          "Шаг 2.1: Проверить выбор окон и мониторов через KWin Portal диалог",
          "Шаг 2.2: Проверить отсутствие утечек дескрипторов при закрытии стрима"
        ]
      },
      {
        id: "SUB-954-3",
        title: "Тест Hyprland / Sway (wlroots)",
        micro_steps: [
          "Шаг 3.1: Проверить захват экрана через xdg-desktop-portal-wlr"
        ]
      },
      {
        id: "SUB-954-4",
        title: "Тест обратной совместимости с X11",
        micro_steps: [
          "Шаг 4.1: Проверить автоматический fallback на XShm при отсутствии демона Wayland"
        ]
      }
    ],
    acceptance_criteria: [
      "Тест cargo test --test manual_qa_suite_contract test_desktop_matrix проходит на 100% Green",
      "Подтверждена работа DMA-BUF захвата и X11 fallback без падений и артефактов"
    ],
    column: "backlog"
  },
  {
    id: "TASK-955",
    title: "Стресс-тест долговременной стабильности и отсутствия деградации памяти (Soak Test 4h)",
    department: "g4_qa",
    status: "backlog",
    priority: "P1",
    summary: "4-часовой автоматизированный стресс-тест нативного клиента: непрерывный скролл 100k сообщений, 1000 циклов модалок и звонков, мониторинг отсутствия дрейфа VmRSS (>1 МБ/час) и утечек дескрипторов файлов.",
    layout: {
      container: "Long-Running Soak & Leak Test",
      dimensions: "4 Hours Continuous Headless Run",
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
      { element: "SoakRunner", default: "scripts/soak-test.mjs", hover: "4h loop", active: "monitoring" },
      { element: "MemoryProbe", default: "VmRSS < 60 MB", hover: "leak drift < 1MB/h", active: "0 bytes leaked" }
    ],
    subtasks: [
      {
        id: "SUB-955-1",
        title: "Создание сценария долговременной нагрузки (scripts/soak-test.mjs)",
        micro_steps: [
          "Шаг 1.1: Написать скрипт, эмулирующий 10 000 сообщений в минуту, 500 переключений каналов и 50 голосовых звонков",
          "Шаг 1.2: Добавить циклический сброс кэша медиа при достижении лимита"
        ]
      },
      {
        id: "SUB-955-2",
        title: "Мониторинг VmRSS и дескрипторов FD",
        micro_steps: [
          "Шаг 2.1: Считывать /proc/$PID/status каждые 30 секунд",
          "Шаг 2.2: Проверять /proc/$PID/fd на отсутствие утечки открытых сокетов"
        ]
      },
      {
        id: "SUB-955-3",
        title: "Оценка деградации памяти",
        micro_steps: [
          "Шаг 3.1: Убедиться, что дрейф памяти составляет менее 1 МБ в час",
          "Шаг 3.2: Оформить отчёт waves/RW-44/QUALITY/SOAK-TEST-REPORT.md"
        ]
      }
    ],
    acceptance_criteria: [
      "Пиковое потребление памяти не превысило 60 МБ за весь 4-часовой прогон",
      "Утечки сокетов и дескрипторов файлов строго равны 0",
      "Отчёт SOAK-TEST-REPORT.md сформирован и прикреплён к сборке"
    ],
    column: "backlog"
  },
  {
    id: "TASK-956",
    title: "Комплексный аудит безопасности, очистка секретов и генерация SBOM-манифеста",
    department: "devops",
    status: "backlog",
    priority: "P0",
    summary: "Проверка кодовой базы на отсутствие утекших ключей/токенов по 20+ паттернам, аудит маскирования в логах паник и генерация финального SBOM-манифеста по стандарту SPDX 2.3.",
    layout: {
      container: "Security Audit & Software Bill of Materials",
      dimensions: "Repository Wide Security Scan",
      padding: "0px",
      spacing: "0px"
    },
    design_tokens: {
      background: "#1e1f22",
      surface: "#2b2d31",
      text_primary: "#ffffff",
      accent: "#ed4245"
    },
    interactive_states: [
      { element: "SecretScanner", default: "scripts/secret-scan.mjs", hover: "20+ regex patterns", active: "0 leaks" },
      { element: "SbomGenerator", default: "scripts/sbom.mjs --check", hover: "SPDX 2.3", active: "valid" }
    ],
    subtasks: [
      {
        id: "SUB-956-1",
        title: "Статический поиск секретов по регулярным выражениям",
        micro_steps: [
          "Шаг 1.1: Запустить сканирование по паттернам токенов Discord (MFA, Bot, User tokens)",
          "Шаг 1.2: Проверить отсутствие приватных ключей DAVE MLS в исходных кодах"
        ]
      },
      {
        id: "SUB-956-2",
        title: "Аудит маскирования в логах и выводах ошибок",
        micro_steps: [
          "Шаг 2.1: Проверить, что в выводе panic и logcat токены маскируются маской mfa.*** или Bot ***",
          "Шаг 2.2: Проверить zeroize очистку секретов в оперативной памяти перед деаллокацией"
        ]
      },
      {
        id: "SUB-956-3",
        title: "Генерация финального SBOM манифеста",
        micro_steps: [
          "Шаг 3.1: Запустить node scripts/sbom.mjs --check --output rust/dist/sbom.spdx.json",
          "Шаг 3.2: Проверить соответствие лицензий всех 85+ зависимостей политике открытого ПО (MIT / Apache-2.0 / BSD-3)"
        ]
      }
    ],
    acceptance_criteria: [
      "Сканирование кодовой базы на секреты показало 0 находок",
      "Маскирование чувствительных данных подтверждено тестами",
      "Манифест rust/dist/sbom.spdx.json сгенерирован и валиден по стандарту SPDX 2.3"
    ],
    column: "backlog"
  },
  {
    id: "TASK-957",
    title: "Процедура слияния testing -> beta и регламент экстренного отката (Rollback Procedure)",
    department: "devops",
    status: "backlog",
    priority: "P0",
    summary: "Оформление PR слияния testing -> beta, разработка регламента немедленного отката в случае непредвиденных дефектов и проверка целостности локальной базы SQLite WAL при даунгрейде.",
    layout: {
      container: "Promotion Pull Request & Emergency Protocol",
      dimensions: "testing -> beta Branch Lifecycle",
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
      { element: "BetaPR", default: "PR testing -> beta", hover: "review approval", active: "merged" },
      { element: "RollbackPlan", default: "docs/specs/ROLLBACK-PROCEDURE.md", hover: "1-command revert", active: "tested" }
    ],
    subtasks: [
      {
        id: "SUB-957-1",
        title: "Подготовка PR testing -> beta",
        micro_steps: [
          "Шаг 1.1: Сформировать детальное описание PR со списком всех включённых фич, результатами QA и контрольными суммами",
          "Шаг 1.2: Проверить отсутствие конфликтов и получить одобрение Lead Orchestrator"
        ]
      },
      {
        id: "SUB-957-2",
        title: "Разработка регламента отката (Rollback Protocol)",
        micro_steps: [
          "Шаг 2.1: Описать команду быстрого отката коммита в ветке beta (git revert -m 1 <merge-commit>)",
          "Шаг 2.2: Проверить процедуру восстановления предыдущего релиза бинарника из кэша артефактов"
        ]
      },
      {
        id: "SUB-957-3",
        title: "Проверка совместимости схемы SQLite при даунгрейде",
        micro_steps: [
          "Шаг 3.1: Проверить, что база данных, открытая новой сборкой, не портится при возврате на предыдущую версию клиента"
        ]
      }
    ],
    acceptance_criteria: [
      "Документ docs/specs/ROLLBACK-PROCEDURE.md написан и утверждён",
      "База данных SQLite сохраняет целостность при циклическом обновлении/откате",
      "Слияние testing -> beta выполнено успешно без конфликтов"
    ],
    column: "backlog"
  },
  {
    id: "TASK-958",
    title: "Релизный пакет бета-версии, контрольные суммы SHA256SUMS и руководство тестировщика",
    department: "g6_tech_writer",
    status: "backlog",
    priority: "P1",
    summary: "Упаковка бинарника discord-gpui в тарболл, публикация актуального файла SHA256SUMS, составление руководства бета-тестировщика docs/BETA-TESTING-GUIDE.md и релизных заметок.",
    layout: {
      container: "Release Package & Documentation Suite",
      dimensions: "Tarball, SHA256, User Guide",
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
      { element: "ReleaseTarball", default: "discord-gpui-v1.0.0-beta.1-linux-x86_64.tar.gz", hover: "tar -czf", active: "ready" },
      { element: "BetaGuide", default: "docs/BETA-TESTING-GUIDE.md", hover: "read", active: "published" }
    ],
    subtasks: [
      {
        id: "SUB-958-1",
        title: "Сборка релизного архива tar.gz",
        micro_steps: [
          "Шаг 1.1: Упаковать stripped бинарник discord-gpui и лицензионные файлы в архив rust/dist/discord-gpui-v1.0.0-beta.1-linux-x86_64.tar.gz",
          "Шаг 1.2: Пересчитать контрольные суммы и обновить rust/dist/SHA256SUMS"
        ]
      },
      {
        id: "SUB-958-2",
        title: "Создание руководства бета-тестировщика",
        micro_steps: [
          "Шаг 2.1: Написать docs/BETA-TESTING-GUIDE.md: системные требования, запуск в 1 команду, логи RUST_LOG=info",
          "Шаг 2.2: Оформить шаблон репорта о найденной ошибке (версия Linux, GPU, логи)"
        ]
      },
      {
        id: "SUB-958-3",
        title: "Выпуск Release Notes для Beta 1",
        micro_steps: [
          "Шаг 3.1: Создать файл RELEASE-NOTES-v1.0.0-beta.1.md со сводкой всех реализованных возможностей"
        ]
      }
    ],
    acceptance_criteria: [
      "Архив discord-gpui-v1.0.0-beta.1-linux-x86_64.tar.gz готов к раздаче",
      "Файл rust/dist/SHA256SUMS содержит актуальный хеш архива",
      "Документы BETA-TESTING-GUIDE.md и RELEASE-NOTES-v1.0.0-beta.1.md опубликованы"
    ],
    column: "backlog"
  }
];

// Deduplicate by ID
for (const task of newBetaTasks) {
  const existingIdx = kb.cards.findIndex((/** @type {any} */ c) => c.id === task.id);
  if (existingIdx >= 0) {
    kb.cards[existingIdx] = task;
  } else {
    kb.cards.push(task);
  }
}

kb.updated_at = new Date().toISOString();
fs.writeFileSync(BOARD_FILE, JSON.stringify(kb, null, 2));

// Re-export HTML
let html = fs.readFileSync(EXPORT_HTML_FILE, "utf8");
html = html.replace(/const BOARD_DATA = .*/, "const BOARD_DATA = " + JSON.stringify(kb) + ";");
fs.writeFileSync(EXPORT_HTML_FILE, html);

console.log(`[Beta Tasks Ingestion] Added 8 Beta promotion tasks (TASK-951..TASK-958) to ${BOARD_FILE}`);
console.log(`[Beta Tasks Ingestion] Total cards on board now: ${kb.cards.length}`);
console.log(`[Beta Tasks Ingestion] Re-exported HTML board at ${EXPORT_HTML_FILE}`);
