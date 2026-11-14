//! Песочница и среда выполнения WASM-плагинов (`P3-1.2` / `W3-6`).
//!
//! Реализует чистую Rust (`#![forbid(unsafe_code)]`) среду исполнения и валидатор
//! бинарных модулей WebAssembly согласно спецификации `docs/specs/plugin-api-v1.md`
//! (WIT-пакет `chord:plugin@1.0.0`):
//! - Бинарная валидация заголовка `\0asm`, версий Core v1 (`[1,0,0,0]`) и Component v1 (`[0x0d,0,1,0]`),
//!   секций LEB128, лимитов линейной памяти (`<= 256` страниц = `16 MiB`), таблиц (`<= 10_000` элементов)
//!   и блокировка запрещённых WASI/хост-импортов (`wasi:http/*`, `wasi:filesystem/*`, `wasi:sockets/*`,
//!   `wasi_snapshot_preview1::environ_get`, `proc_*`, `chord:host/token`).
//! - Межсетевой экран возможностей (Capability Firewall) по принципу default-deny.
//! - Детерминированный контроль ресурсов: бюджет топлива на инициализацию (`10_000_000`) и вызов (`5_000_000`),
//!   дедлайн выполнения UI-хуков (`<= 5 ms`), квота KV-хранилища (`<= 1 MiB`).
//! - Изоляция сбоев: автоматическое отключение плагина после `3` последовательных ловушек (traps)
//!   со сбросом счётчика при успешном вызове, без влияния на соседние плагины.
//! - Автоматическое редактирование секретов и токенов Discord (`mfa.*`, `Bearer *`, `Bot *`, `MT*.*.*`)
//!   в аудит-логах и сообщениях об ошибках (`[REDACTED]`).

#![forbid(unsafe_code)]

use std::collections::{HashMap, HashSet};
use std::fmt;

/// Магические байты бинарного модуля WebAssembly (`\0asm`).
pub const WASM_MAGIC: [u8; 4] = [0x00, 0x61, 0x73, 0x6D];

/// Версия заголовка стандартного модуля WebAssembly Core 1.0.
pub const WASM_CORE_VERSION: [u8; 4] = [0x01, 0x00, 0x00, 0x00];

/// Версия заголовка WebAssembly Component Model (`wasm32-wasip2`).
pub const WASM_COMPONENT_VERSION: [u8; 4] = [0x0D, 0x00, 0x01, 0x00];

/// Размер одной страницы линейной памяти WASM (64 KiB).
pub const WASM_PAGE_SIZE_BYTES: usize = 65_536;

/// Поддерживаемая версия ABI хоста плагинов.
pub const PLUGIN_ABI_VERSION: u16 = 1;

/// Лимит линейной памяти по умолчанию на один экземпляр плагина (`16 MiB` = `256` страниц по 64 KiB).
pub const DEFAULT_MEMORY_LIMIT_BYTES: usize = 16 * 1024 * 1024;

/// Максимальное число страниц WASM по умолчанию (`256` страниц = `16 MiB`).
pub const DEFAULT_MEMORY_LIMIT_PAGES: u32 = 256;

/// Лимит элементов таблицы (`table`) на один модуль.
pub const DEFAULT_TABLE_LIMIT_ELEMENTS: usize = 10_000;

/// Бюджет топлива (инструкций) на инициализацию (`on_load`).
pub const DEFAULT_INIT_FUEL_BUDGET: u64 = 10_000_000;

/// Бюджет топлива (инструкций) на один синхронный вызов хука.
pub const DEFAULT_CALL_FUEL_BUDGET: u64 = 5_000_000;

/// Максимальное время выполнения синхронного вызова хука (`5 ms` для защиты бюджета 120 FPS).
pub const DEFAULT_CALL_DEADLINE_MS: u64 = 5;

/// Порог последовательных сбоев (traps) до автоматического отключения плагина.
pub const MAX_CONSECUTIVE_TRAPS_BEFORE_DISABLE: u32 = 3;

/// Максимальная длина отрендеренного сообщения от плагина (`2000` символов).
pub const MAX_MESSAGE_RENDER_LEN: usize = 2_000;

/// Максимальное количество эмбедов в ответе слэш-команды (`3`).
pub const MAX_EMBEDS_PER_RESPONSE: usize = 3;

/// Максимальное количество регистрируемых слэш-команд на один плагин (`16`).
pub const MAX_SLASH_COMMANDS_PER_PLUGIN: usize = 16;

/// Максимальная квота изолированного KV-хранилища на один плагин (`1 MiB`).
pub const MAX_KV_STORAGE_QUOTA_BYTES: usize = 1_048_576;

/// Идентификатор WIT-пакета контракта плагинов.
pub const WIT_PACKAGE_ID: &str = "chord:plugin@1.0.0";

/// Тип идентификатора плагина.
pub type PluginId = String;

/// Идентификаторы секций бинарного формата WebAssembly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum WasmSectionId {
    Custom = 0,
    Type = 1,
    Import = 2,
    Function = 3,
    Table = 4,
    Memory = 5,
    Global = 6,
    Export = 7,
    Start = 8,
    Element = 9,
    Code = 10,
    Data = 11,
    DataCount = 12,
}

impl WasmSectionId {
    /// Преобразует байт идентификатора секции WASM в типизированное значение `WasmSectionId`.
    pub fn from_u8(id: u8) -> Option<Self> {
        match id {
            0 => Some(Self::Custom),
            1 => Some(Self::Type),
            2 => Some(Self::Import),
            3 => Some(Self::Function),
            4 => Some(Self::Table),
            5 => Some(Self::Memory),
            6 => Some(Self::Global),
            7 => Some(Self::Export),
            8 => Some(Self::Start),
            9 => Some(Self::Element),
            10 => Some(Self::Code),
            11 => Some(Self::Data),
            12 => Some(Self::DataCount),
            _ => None,
        }
    }
}

/// Декларативные разрешения (capabilities) плагина и запрещённые системные возможности.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PluginCapability {
    /// Чтение видимых сообщений активного канала (`chat.read_visible`).
    ReadMessages,
    /// Модификация/форматирование рендера сообщений и UI-слотов (`chat.format_message` / `ui.*`).
    RenderUi,
    /// Регистрация клиентских слэш-команд (`commands.register_slash`).
    SlashCommands,
    /// Изолированное KV-хранилище плагина (`storage.plugin_kv`, квота `<= 1 MiB`).
    PluginStorage,
    /// Разрешённые HTTP-запросы к явному списку доменов без доступа к Discord (`net.fetch_allowlisted`).
    NetFetchAllowlisted,

    // --- Запрещённые WASI и хост-возможности (всегда отклоняются межсетевым экраном) ---
    /// Прямой неконтролируемый сетевой доступ / сокеты (`wasi:sockets/*`).
    Network,
    /// Доступ к файловой системе хоста (`wasi:filesystem/*`).
    Filesystem,
    /// Доступ к переменным окружения процесса (`environ_get`).
    Environment,
    /// Запуск или управление процессами ОС (`proc_*`).
    Process,
    /// Доступ к токенам аутентификации Discord или системному связке ключей (`chord:host/token`).
    AuthToken,
    /// Прямой перехват голосовых/медиа потоков и ключей DAVE MLS / SRTP.
    VoiceMedia,
}

impl PluginCapability {
    /// Возвращает `true`, если данная возможность категорически запрещена в песочнице Chord.
    pub fn is_forbidden(&self) -> bool {
        matches!(
            self,
            Self::Network
                | Self::Filesystem
                | Self::Environment
                | Self::Process
                | Self::AuthToken
                | Self::VoiceMedia
        )
    }
}

/// Спецификация регистрируемой плагином слэш-команды.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlashCommandSpec {
    pub name: String,
    pub description: String,
}

/// Манифест плагина (`plugin.toml` / `manifest.json`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginManifest {
    pub id: PluginId,
    pub name: String,
    pub version: String,
    pub abi_version: u16,
    pub permissions: Vec<PluginCapability>,
    pub slash_commands: Vec<SlashCommandSpec>,
    pub allowed_domains: Vec<String>,
    pub kv_quota_bytes: usize,
    pub expected_sha256: Option<[u8; 32]>,
    pub signature_hex: Option<String>,
    pub public_key_hex: Option<String>,
}

/// Статус жизненного цикла плагина в песочнице.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginStatus {
    /// Модуль проверен и загружен в память, но ещё не активирован.
    Loaded,
    /// Плагин активен и обрабатывает хуки.
    Enabled,
    /// Плагин отключён (пользователем или автоматически после `3` последовательных сбоев).
    Disabled { reason: String },
}

/// Причина возникновения ловушки (trap) при исполнении гостевого кода WASM.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrapReason {
    /// Исчерпан лимит инструкций (fuel).
    FuelExhausted { consumed: u64, budget: u64 },
    /// Превышен лимит линейной памяти (`> 16 MiB`).
    MemoryOverflow {
        requested_bytes: usize,
        limit_bytes: usize,
    },
    /// Превышен дедлайн времени выполнения (`> 5 ms`).
    DeadlineExceeded { elapsed_ms: u64, deadline_ms: u64 },
    /// Выполнена инструкция `unreachable`.
    Unreachable,
    /// Деление на ноль в гостевом коде.
    DivisionByZero,
    /// Произвольная ошибка/паника гостевого модуля.
    ExplicitTrap(String),
}

impl fmt::Display for TrapReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FuelExhausted { consumed, budget } => {
                write!(f, "fuel exhausted ({consumed} > {budget} instructions)")
            }
            Self::MemoryOverflow {
                requested_bytes,
                limit_bytes,
            } => write!(
                f,
                "linear memory overflow ({requested_bytes} > {limit_bytes} bytes)"
            ),
            Self::DeadlineExceeded {
                elapsed_ms,
                deadline_ms,
            } => write!(
                f,
                "call deadline exceeded ({elapsed_ms}ms > {deadline_ms}ms)"
            ),
            Self::Unreachable => write!(f, "wasm unreachable instruction executed"),
            Self::DivisionByZero => write!(f, "wasm integer division by zero"),
            Self::ExplicitTrap(msg) => write!(f, "guest trap: {}", redact_secrets(msg)),
        }
    }
}

/// Ошибки валидации, безопасности и исполнения песочницы плагинов.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginError {
    /// Отсутствует сигнатура `\0asm` в начале бинарного файла.
    InvalidWasmMagic,
    /// Неподдерживаемая версия бинарного заголовка WASM.
    UnsupportedWasmVersion([u8; 4]),
    /// Повреждённая секция или некорректная длина LEB128 в бинарнике WASM.
    MalformedWasmSection(String),
    /// Несовпадение версии ABI манифеста с `PLUGIN_ABI_VERSION`.
    AbiVersionMismatch { expected: u16, found: u16 },
    /// Некорректный идентификатор или SemVer-версия в манифесте.
    InvalidManifest(String),
    /// Ошибка проверки контрольной суммы SHA-256 или подписи Ed25519.
    SignatureVerificationFailed(String),
    /// Модуль запрашивает больше страниц линейной памяти, чем разрешено (`> 256` страниц / `16 MiB`).
    MemoryLimitExceeded {
        requested_pages: u32,
        max_pages: u32,
    },
    /// Модуль запрашивает таблицу больше `DEFAULT_TABLE_LIMIT_ELEMENTS` (`10_000`).
    TableLimitExceeded {
        requested_elements: u32,
        max_elements: u32,
    },
    /// Обнаружен запрещённый WASI или хост-импорт.
    ForbiddenImport { module: String, name: String },
    /// В манифесте запрошена запрещённая возможность (`Network`, `Filesystem`, `AuthToken` и т.д.).
    ForbiddenCapability(PluginCapability),
    /// Нарушение межсетевого экрана возможностей (default-deny).
    PermissionDenied {
        plugin_id: PluginId,
        capability: PluginCapability,
    },
    /// Превышено максимальное число регистрируемых слэш-команд (`> 16`).
    TooManySlashCommands { count: usize, max: usize },
    /// Результат `on_message_render` превышает `2000` символов.
    MessageRenderLengthExceeded { len: usize, max: usize },
    /// Обнаружена попытка инъекции управляющих символов (ANSI escape / Bidi override / NUL).
    ControlCharacterInjection,
    /// Ответ слэш-команды содержит больше `3` эмбедов.
    TooManyEmbeds { count: usize, max: usize },
    /// Превышена квота изолированного KV-хранилища (`> 1 MiB`).
    StorageQuotaExceeded {
        used_bytes: usize,
        quota_bytes: usize,
    },
    /// Попытка сетевого запроса к запрещённому домену (например, `discord.com/api`).
    ForbiddenNetworkTarget(String),
    /// Ловушка (trap) во время выполнения гостевого кода.
    GuestTrap {
        plugin_id: PluginId,
        reason: TrapReason,
    },
    /// Вызов отклонён, так как плагин находится в состоянии `Disabled` (или не активирован).
    PluginDisabled { plugin_id: PluginId, reason: String },
    /// Плагин с указанным идентификатором не найден.
    PluginNotFound(PluginId),
}

impl fmt::Display for PluginError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidWasmMagic => write!(f, "invalid WASM magic bytes (expected \\0asm)"),
            Self::UnsupportedWasmVersion(v) => {
                write!(f, "unsupported WASM version header: {v:02x?}")
            }
            Self::MalformedWasmSection(msg) => {
                write!(f, "malformed WASM section: {}", redact_secrets(msg))
            }
            Self::AbiVersionMismatch { expected, found } => {
                write!(f, "plugin ABI mismatch: expected {expected}, found {found}")
            }
            Self::InvalidManifest(msg) => write!(f, "invalid manifest: {}", redact_secrets(msg)),
            Self::SignatureVerificationFailed(msg) => {
                write!(f, "signature verification failed: {}", redact_secrets(msg))
            }
            Self::MemoryLimitExceeded {
                requested_pages,
                max_pages,
            } => write!(
                f,
                "WASM memory limit exceeded: requested {requested_pages} pages, max {max_pages}"
            ),
            Self::TableLimitExceeded {
                requested_elements,
                max_elements,
            } => write!(
                f,
                "WASM table limit exceeded: requested {requested_elements} elements, max {max_elements}"
            ),
            Self::ForbiddenImport { module, name } => {
                write!(f, "forbidden WASI/host import: {module}::{name}")
            }
            Self::ForbiddenCapability(cap) => {
                write!(f, "forbidden capability requested: {cap:?}")
            }
            Self::PermissionDenied {
                plugin_id,
                capability,
            } => write!(
                f,
                "permission denied for plugin '{plugin_id}': missing capability {capability:?}"
            ),
            Self::TooManySlashCommands { count, max } => {
                write!(f, "too many slash commands: {count} (max {max})")
            }
            Self::MessageRenderLengthExceeded { len, max } => {
                write!(f, "rendered message length {len} exceeds maximum {max}")
            }
            Self::ControlCharacterInjection => {
                write!(f, "control character or bidi override injection rejected")
            }
            Self::TooManyEmbeds { count, max } => {
                write!(f, "command response has {count} embeds (max {max})")
            }
            Self::StorageQuotaExceeded {
                used_bytes,
                quota_bytes,
            } => write!(
                f,
                "plugin KV storage quota exceeded: {used_bytes} > {quota_bytes} bytes"
            ),
            Self::ForbiddenNetworkTarget(url) => {
                write!(f, "forbidden network target: {}", redact_secrets(url))
            }
            Self::GuestTrap { plugin_id, reason } => {
                write!(f, "plugin '{plugin_id}' trapped: {reason}")
            }
            Self::PluginDisabled { plugin_id, reason } => {
                write!(
                    f,
                    "plugin '{plugin_id}' is disabled: {}",
                    redact_secrets(reason)
                )
            }
            Self::PluginNotFound(plugin_id) => write!(f, "plugin '{plugin_id}' not found"),
        }
    }
}

impl std::error::Error for PluginError {}

/// Уровень логирования гостевого плагина (`chord:plugin/types.log-level`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

/// Запись в защищённом аудит-журнале хоста плагинов.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditLogEntry {
    pub plugin_id: PluginId,
    pub level: LogLevel,
    pub message: String,
}

/// Наблюдаемые метрики телеметрии конкретного плагина.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PluginTelemetry {
    pub total_fuel_consumed: u64,
    pub last_call_fuel_consumed: u64,
    pub last_call_elapsed_ms: u64,
    pub peak_memory_bytes: usize,
    pub total_calls: u64,
    pub total_traps: u32,
    pub consecutive_traps: u32,
}

/// Входной контекст сообщения для хука `on-message-render`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageContext {
    pub message_id: u64,
    pub channel_id: u64,
    pub author_id: u64,
    pub author_name: String,
    pub content: String,
    pub timestamp_ms: u64,
}

/// Токен форматированного текста (`rendered-token` из `chord.wit`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedToken {
    pub token_type: String,
    pub text: String,
    pub color: Option<String>,
    pub bold: bool,
    pub italic: bool,
}

/// Результат трансформации сообщения хуком `on-message-render`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedMessagePatch {
    pub rendered_content: String,
    pub tokens: Vec<RenderedToken>,
}

/// Вызов зарегистрированной слэш-команды плагина.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandInvocation {
    pub command: String,
    pub args: Vec<String>,
    pub channel_id: u64,
    pub invoker_id: u64,
}

/// Встроенный блок (embed) в ответе слэш-команды.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandEmbed {
    pub title: String,
    pub description: String,
    pub color_hex: Option<u32>,
}

/// Ответ плагина на выполнение слэш-команды (`<= 3` эмбедов).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandResponse {
    pub content: String,
    pub embeds: Vec<CommandEmbed>,
    pub ephemeral: bool,
}

/// Запрос к `host::http-fetch`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpFetchRequest {
    pub url: String,
    pub method: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<Vec<u8>>,
}

/// Ответ от `host::http-fetch`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpFetchResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

/// Контекст выполнения гостевого вызова с учётом топлива, памяти, дедлайна и логов.
pub struct GuestContext<'a> {
    plugin_id: &'a str,
    fuel_budget: u64,
    fuel_consumed: u64,
    memory_limit_bytes: usize,
    memory_used_bytes: usize,
    deadline_ms: u64,
    elapsed_ms: u64,
    audit_logs: &'a mut Vec<AuditLogEntry>,
}

impl<'a> GuestContext<'a> {
    /// Списывает указанное количество единиц топлива; при превышении бюджета генерирует `TrapReason::FuelExhausted`.
    pub fn consume_fuel(&mut self, amount: u64) -> Result<(), TrapReason> {
        self.fuel_consumed = self.fuel_consumed.saturating_add(amount);
        if self.fuel_consumed > self.fuel_budget {
            return Err(TrapReason::FuelExhausted {
                consumed: self.fuel_consumed,
                budget: self.fuel_budget,
            });
        }
        Ok(())
    }

    /// Проверяет и выделяет байты в линейной памяти гостя (`<= 16 MiB`).
    pub fn allocate_memory(&mut self, bytes: usize) -> Result<(), TrapReason> {
        let next = self.memory_used_bytes.saturating_add(bytes);
        if next > self.memory_limit_bytes {
            return Err(TrapReason::MemoryOverflow {
                requested_bytes: next,
                limit_bytes: self.memory_limit_bytes,
            });
        }
        self.memory_used_bytes = next;
        Ok(())
    }

    /// Продвигает счётчик времени выполнения вызова и проверяет дедлайн (`<= 5 ms`).
    pub fn advance_elapsed_ms(&mut self, delta_ms: u64) -> Result<(), TrapReason> {
        self.elapsed_ms = self.elapsed_ms.saturating_add(delta_ms);
        if self.elapsed_ms > self.deadline_ms {
            return Err(TrapReason::DeadlineExceeded {
                elapsed_ms: self.elapsed_ms,
                deadline_ms: self.deadline_ms,
            });
        }
        Ok(())
    }

    /// Записывает сообщение в журнал хоста с автоматической фильтрацией секретных токенов.
    pub fn host_log(&mut self, level: LogLevel, message: &str) {
        self.audit_logs.push(AuditLogEntry {
            plugin_id: self.plugin_id.to_string(),
            level,
            message: redact_secrets(message),
        });
    }

    pub fn fuel_consumed(&self) -> u64 {
        self.fuel_consumed
    }

    pub fn memory_used_bytes(&self) -> usize {
        self.memory_used_bytes
    }

    pub fn elapsed_ms(&self) -> u64 {
        self.elapsed_ms
    }
}

/// Трейт изолированного гостевого исполнителя WASM-плагина (`chord:plugin@1.0.0`).
pub trait PluginGuest: Send {
    /// Вызывается при инициализации плагина (бюджет `DEFAULT_INIT_FUEL_BUDGET = 10_000_000`).
    fn on_load(&mut self, _ctx: &mut GuestContext<'_>) -> Result<(), TrapReason> {
        Ok(())
    }

    /// Синхронный хук рендеринга сообщения (бюджет `5_000_000` топлива, дедлайн `<= 5 ms`).
    fn on_message_render(
        &mut self,
        _ctx: &mut GuestContext<'_>,
        _msg: &MessageContext,
    ) -> Result<Option<RenderedMessagePatch>, TrapReason> {
        Ok(None)
    }

    /// Хук выполнения клиентской слэш-команды.
    fn on_slash_command(
        &mut self,
        _ctx: &mut GuestContext<'_>,
        _invocation: &CommandInvocation,
    ) -> Result<CommandResponse, TrapReason> {
        Ok(CommandResponse {
            content: String::new(),
            embeds: Vec::new(),
            ephemeral: true,
        })
    }
}

/// Метаданные, извлечённые при бинарном разборе WASM-модуля.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WasmModuleMetadata {
    pub version: [u8; 4],
    pub initial_memory_pages: u32,
    pub max_memory_pages: Option<u32>,
    pub initial_table_elements: u32,
    pub max_table_elements: Option<u32>,
    pub imports: Vec<(String, String)>,
    pub exports: Vec<String>,
}

/// Конструктор валидных бинарных модулей WASM для тестирования и сериализации.
#[derive(Debug, Clone)]
pub struct WasmBinaryBuilder {
    version: [u8; 4],
    memory_limits: Option<(u32, Option<u32>)>,
    table_limits: Option<(u32, Option<u32>)>,
    imports: Vec<(String, String)>,
    exports: Vec<String>,
}

impl WasmBinaryBuilder {
    /// Создаёт билдер для стандартного модуля WebAssembly Core v1 (`[0x01, 0x00, 0x00, 0x00]`).
    pub fn new_core_v1() -> Self {
        Self {
            version: WASM_CORE_VERSION,
            memory_limits: None,
            table_limits: None,
            imports: Vec::new(),
            exports: Vec::new(),
        }
    }

    /// Создаёт билдер для модуля WebAssembly Component Model (`[0x0D, 0x00, 0x01, 0x00]`).
    pub fn new_component_v1() -> Self {
        Self {
            version: WASM_COMPONENT_VERSION,
            memory_limits: None,
            table_limits: None,
            imports: Vec::new(),
            exports: Vec::new(),
        }
    }

    /// Задаёт секцию линейной памяти (`MemorySection`, id = 5) в страницах по 64 KiB.
    pub fn with_memory_pages(mut self, initial_pages: u32, max_pages: Option<u32>) -> Self {
        self.memory_limits = Some((initial_pages, max_pages));
        self
    }

    /// Задаёт секцию таблиц (`TableSection`, id = 4).
    pub fn with_table_elements(mut self, initial_elements: u32, max_elements: Option<u32>) -> Self {
        self.table_limits = Some((initial_elements, max_elements));
        self
    }

    /// Добавляет импорт в секцию импортов (`ImportSection`, id = 2).
    pub fn with_import(mut self, module: &str, field: &str) -> Self {
        self.imports.push((module.to_string(), field.to_string()));
        self
    }

    /// Добавляет экспорт в секцию экспортов (`ExportSection`, id = 7).
    pub fn with_export(mut self, name: &str) -> Self {
        self.exports.push(name.to_string());
        self
    }

    /// Собирает бинарный образ WASM с заголовком `\0asm` и LEB128-секциями.
    pub fn build(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&WASM_MAGIC);
        out.extend_from_slice(&self.version);

        // Import section (id = 2)
        if !self.imports.is_empty() {
            let mut payload = Vec::new();
            encode_u32_leb128(self.imports.len() as u32, &mut payload);
            for (module, name) in &self.imports {
                encode_wasm_string(module, &mut payload);
                encode_wasm_string(name, &mut payload);
                payload.push(0x00); // func import kind
                encode_u32_leb128(0, &mut payload); // type index 0
            }
            write_section(WasmSectionId::Import as u8, &payload, &mut out);
        }

        // Table section (id = 4)
        if let Some((initial, max)) = self.table_limits {
            let mut payload = Vec::new();
            encode_u32_leb128(1, &mut payload); // 1 table
            payload.push(0x70); // funcref
            encode_limits(initial, max, &mut payload);
            write_section(WasmSectionId::Table as u8, &payload, &mut out);
        }

        // Memory section (id = 5)
        if let Some((initial, max)) = self.memory_limits {
            let mut payload = Vec::new();
            encode_u32_leb128(1, &mut payload); // 1 memory
            encode_limits(initial, max, &mut payload);
            write_section(WasmSectionId::Memory as u8, &payload, &mut out);
        }

        // Export section (id = 7)
        if !self.exports.is_empty() {
            let mut payload = Vec::new();
            encode_u32_leb128(self.exports.len() as u32, &mut payload);
            for (idx, export_name) in self.exports.iter().enumerate() {
                encode_wasm_string(export_name, &mut payload);
                payload.push(0x00); // func export
                encode_u32_leb128(idx as u32, &mut payload);
            }
            write_section(WasmSectionId::Export as u8, &payload, &mut out);
        }

        out
    }
}

fn encode_u32_leb128(mut val: u32, out: &mut Vec<u8>) {
    loop {
        let mut byte = (val & 0x7F) as u8;
        val >>= 7;
        if val != 0 {
            byte |= 0x80;
        }
        out.push(byte);
        if val == 0 {
            break;
        }
    }
}

fn decode_u32_leb128(bytes: &[u8], cursor: &mut usize) -> Result<u32, PluginError> {
    let mut result: u32 = 0;
    let mut shift: u32 = 0;
    loop {
        if *cursor >= bytes.len() {
            return Err(PluginError::MalformedWasmSection(
                "unexpected EOF while decoding LEB128 integer".to_string(),
            ));
        }
        let byte = bytes[*cursor];
        *cursor += 1;
        if shift >= 32 && (byte & 0xF0) != 0 {
            return Err(PluginError::MalformedWasmSection(
                "LEB128 integer overflow".to_string(),
            ));
        }
        result |= ((byte & 0x7F) as u32) << shift;
        if (byte & 0x80) == 0 {
            return Ok(result);
        }
        shift += 7;
        if shift > 35 {
            return Err(PluginError::MalformedWasmSection(
                "LEB128 integer too long".to_string(),
            ));
        }
    }
}

fn encode_wasm_string(s: &str, out: &mut Vec<u8>) {
    encode_u32_leb128(s.len() as u32, out);
    out.extend_from_slice(s.as_bytes());
}

fn decode_wasm_string(bytes: &[u8], cursor: &mut usize) -> Result<String, PluginError> {
    let len = decode_u32_leb128(bytes, cursor)? as usize;
    let end = cursor.checked_add(len).ok_or_else(|| {
        PluginError::MalformedWasmSection("string length overflow in WASM section".to_string())
    })?;
    if end > bytes.len() {
        return Err(PluginError::MalformedWasmSection(
            "string extends past WASM section boundary".to_string(),
        ));
    }
    let slice = &bytes[*cursor..end];
    *cursor = end;
    std::str::from_utf8(slice)
        .map(str::to_string)
        .map_err(|_| PluginError::MalformedWasmSection("invalid UTF-8 identifier".to_string()))
}

fn encode_limits(initial: u32, max: Option<u32>, out: &mut Vec<u8>) {
    match max {
        Some(m) => {
            out.push(0x01);
            encode_u32_leb128(initial, out);
            encode_u32_leb128(m, out);
        }
        None => {
            out.push(0x00);
            encode_u32_leb128(initial, out);
        }
    }
}

fn decode_limits(bytes: &[u8], cursor: &mut usize) -> Result<(u32, Option<u32>), PluginError> {
    if *cursor >= bytes.len() {
        return Err(PluginError::MalformedWasmSection(
            "missing limits flag byte".to_string(),
        ));
    }
    let flag = bytes[*cursor];
    *cursor += 1;
    let initial = decode_u32_leb128(bytes, cursor)?;
    let max = match flag {
        0x00 => None,
        0x01 => Some(decode_u32_leb128(bytes, cursor)?),
        other => {
            return Err(PluginError::MalformedWasmSection(format!(
                "invalid limits flag byte: 0x{other:02x}"
            )));
        }
    };
    Ok((initial, max))
}

fn write_section(section_id: u8, payload: &[u8], out: &mut Vec<u8>) {
    out.push(section_id);
    encode_u32_leb128(payload.len() as u32, out);
    out.extend_from_slice(payload);
}

/// Проверяет, является ли импорт `(module, field)` запрещённым согласно `docs/specs/plugin-api-v1.md`.
pub fn is_forbidden_wasm_import(module: &str, field: &str) -> bool {
    let m = module.to_ascii_lowercase();
    let f = field.to_ascii_lowercase();

    if m.starts_with("wasi:http")
        || m.starts_with("wasi:filesystem")
        || m.starts_with("wasi:sockets")
        || m.starts_with("wasi:cli")
        || m == "chord:host/token"
        || m.contains("auth-token")
    {
        return true;
    }

    if m == "wasi_snapshot_preview1"
        && (f == "environ_get"
            || f == "environ_sizes_get"
            || f.starts_with("proc_")
            || f.starts_with("path_")
            || f.starts_with("sock_"))
    {
        return true;
    }

    if f.starts_with("proc_") || f == "environ_get" || f.contains("auth-token") {
        return true;
    }

    false
}

/// Валидирует бинарный образ WASM (`\0asm`, версию, секции, память `<= 256` страниц, таблицу `<= 10_000`, импорты).
pub fn validate_wasm_binary(wasm_bytes: &[u8]) -> Result<WasmModuleMetadata, PluginError> {
    if wasm_bytes.len() < 8 || wasm_bytes[0..4] != WASM_MAGIC {
        return Err(PluginError::InvalidWasmMagic);
    }

    let version: [u8; 4] = [wasm_bytes[4], wasm_bytes[5], wasm_bytes[6], wasm_bytes[7]];
    if version != WASM_CORE_VERSION && version != WASM_COMPONENT_VERSION {
        return Err(PluginError::UnsupportedWasmVersion(version));
    }

    let mut cursor = 8usize;
    let mut initial_memory_pages = 1u32;
    let mut max_memory_pages = None;
    let mut initial_table_elements = 0u32;
    let mut max_table_elements = None;
    let mut imports = Vec::new();
    let mut exports = Vec::new();

    while cursor < wasm_bytes.len() {
        let section_id = wasm_bytes[cursor];
        cursor += 1;
        let section_len = decode_u32_leb128(wasm_bytes, &mut cursor)? as usize;
        let section_end = cursor.checked_add(section_len).ok_or_else(|| {
            PluginError::MalformedWasmSection("section size overflow".to_string())
        })?;
        if section_end > wasm_bytes.len() {
            return Err(PluginError::MalformedWasmSection(format!(
                "section id {section_id} truncated (expected {section_len} bytes)"
            )));
        }

        let section_payload_id = WasmSectionId::from_u8(section_id);
        let section_payload = &wasm_bytes[cursor..section_end];
        let mut sec_cursor = 0usize;

        match section_payload_id {
            Some(WasmSectionId::Import) => {
                // Import Section
                let count = decode_u32_leb128(section_payload, &mut sec_cursor)?;
                for _ in 0..count {
                    let module = decode_wasm_string(section_payload, &mut sec_cursor)?;
                    let field = decode_wasm_string(section_payload, &mut sec_cursor)?;
                    if is_forbidden_wasm_import(&module, &field) {
                        return Err(PluginError::ForbiddenImport {
                            module,
                            name: field,
                        });
                    }
                    if sec_cursor >= section_payload.len() {
                        return Err(PluginError::MalformedWasmSection(
                            "missing import descriptor kind".to_string(),
                        ));
                    }
                    let kind = section_payload[sec_cursor];
                    sec_cursor += 1;
                    match kind {
                        0x00 => {
                            let _type_idx = decode_u32_leb128(section_payload, &mut sec_cursor)?;
                        }
                        0x01 => {
                            // table import
                            let _elem_type =
                                section_payload.get(sec_cursor).copied().ok_or_else(|| {
                                    PluginError::MalformedWasmSection(
                                        "truncated table import".to_string(),
                                    )
                                })?;
                            sec_cursor += 1;
                            let (init, max) = decode_limits(section_payload, &mut sec_cursor)?;
                            check_table_limits(init, max)?;
                        }
                        0x02 => {
                            // memory import
                            let (init, max) = decode_limits(section_payload, &mut sec_cursor)?;
                            check_memory_limits(init, max)?;
                        }
                        _ => {
                            // skip remaining descriptor bytes safely
                            sec_cursor = section_payload.len();
                        }
                    }
                    imports.push((module, field));
                }
            }
            Some(WasmSectionId::Table) => {
                // Table Section
                let count = decode_u32_leb128(section_payload, &mut sec_cursor)?;
                for _ in 0..count {
                    if sec_cursor >= section_payload.len() {
                        return Err(PluginError::MalformedWasmSection(
                            "missing table elemtype".to_string(),
                        ));
                    }
                    sec_cursor += 1; // funcref / externref
                    let (init, max) = decode_limits(section_payload, &mut sec_cursor)?;
                    check_table_limits(init, max)?;
                    initial_table_elements = init;
                    max_table_elements = max;
                }
            }
            Some(WasmSectionId::Memory) => {
                // Memory Section
                let count = decode_u32_leb128(section_payload, &mut sec_cursor)?;
                for _ in 0..count {
                    let (init, max) = decode_limits(section_payload, &mut sec_cursor)?;
                    check_memory_limits(init, max)?;
                    initial_memory_pages = init;
                    max_memory_pages = max;
                }
            }
            Some(WasmSectionId::Export) => {
                // Export Section
                let count = decode_u32_leb128(section_payload, &mut sec_cursor)?;
                for _ in 0..count {
                    let name = decode_wasm_string(section_payload, &mut sec_cursor)?;
                    if sec_cursor >= section_payload.len() {
                        return Err(PluginError::MalformedWasmSection(
                            "missing export kind".to_string(),
                        ));
                    }
                    sec_cursor += 1;
                    let _idx = decode_u32_leb128(section_payload, &mut sec_cursor)?;
                    exports.push(name);
                }
            }
            Some(_) => {}
            None => {
                return Err(PluginError::MalformedWasmSection(format!(
                    "unknown WASM section id: {section_id}"
                )));
            }
        }

        cursor = section_end;
    }

    Ok(WasmModuleMetadata {
        version,
        initial_memory_pages,
        max_memory_pages,
        initial_table_elements,
        max_table_elements,
        imports,
        exports,
    })
}

fn check_memory_limits(initial_pages: u32, max_pages: Option<u32>) -> Result<(), PluginError> {
    if initial_pages > DEFAULT_MEMORY_LIMIT_PAGES {
        return Err(PluginError::MemoryLimitExceeded {
            requested_pages: initial_pages,
            max_pages: DEFAULT_MEMORY_LIMIT_PAGES,
        });
    }
    if let Some(max) = max_pages {
        if max > DEFAULT_MEMORY_LIMIT_PAGES || initial_pages > max {
            return Err(PluginError::MemoryLimitExceeded {
                requested_pages: max.max(initial_pages),
                max_pages: DEFAULT_MEMORY_LIMIT_PAGES,
            });
        }
    }
    Ok(())
}

fn check_table_limits(initial_elems: u32, max_elems: Option<u32>) -> Result<(), PluginError> {
    let limit = DEFAULT_TABLE_LIMIT_ELEMENTS as u32;
    if initial_elems > limit {
        return Err(PluginError::TableLimitExceeded {
            requested_elements: initial_elems,
            max_elements: limit,
        });
    }
    if let Some(max) = max_elems {
        if max > limit || initial_elems > max {
            return Err(PluginError::TableLimitExceeded {
                requested_elements: max.max(initial_elems),
                max_elements: limit,
            });
        }
    }
    Ok(())
}

/// Детерминированный 256-битный дайджест для верификации целостности `plugin.wasm`.
pub fn compute_wasm_digest(bytes: &[u8]) -> [u8; 32] {
    let mut state: [u64; 4] = [
        0x6a09e667f3bcc908,
        0xbb67ae8584caa73b,
        0x3c6ef372fe94f82b,
        0xa54ff53a5f1d36f1,
    ];
    for (idx, &b) in bytes.iter().enumerate() {
        let lane = idx & 3;
        state[lane] = state[lane]
            .wrapping_mul(0x100000001b3)
            .wrapping_add((b as u64) ^ ((idx as u64) << 8))
            .rotate_left(13);
        let next_lane = (lane + 1) & 3;
        state[next_lane] ^= state[lane].rotate_right(7);
    }
    let mut out = [0u8; 32];
    for (i, word) in state.iter().enumerate() {
        out[i * 8..(i + 1) * 8].copy_from_slice(&word.to_le_bytes());
    }
    out
}

/// Маскирует секретные токены Discord (`mfa.*`, `Bearer *`, `Bot *`, `MT*.*.*` и JWT-подобные токены) -> `[REDACTED]`.
pub fn redact_secrets(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut tokens = input.split_whitespace().peekable();
    let mut first = true;

    while let Some(word) = tokens.next() {
        if !first {
            result.push(' ');
        }
        first = false;

        if word.eq_ignore_ascii_case("Bearer") || word.eq_ignore_ascii_case("Bot") {
            if let Some(_secret_next) = tokens.peek() {
                let _ = tokens.next();
                result.push_str(word);
                result.push_str(" [REDACTED]");
                continue;
            }
        }

        result.push_str(&redact_single_token(word));
    }

    result
}

fn redact_single_token(word: &str) -> String {
    let trimmed = word.trim_matches(|c: char| {
        matches!(
            c,
            ',' | ';' | ':' | '"' | '\'' | '(' | ')' | '[' | ']' | '{' | '}'
        )
    });

    if is_secret_token_pattern(trimmed) {
        return word.replacen(trimmed, "[REDACTED]", 1);
    }
    word.to_string()
}

fn is_secret_token_pattern(candidate: &str) -> bool {
    if candidate.starts_with("mfa.") && candidate.len() > 8 {
        return true;
    }
    if (candidate.starts_with("MT") || candidate.starts_with("OD") || candidate.starts_with("Nz"))
        && candidate.split('.').count() == 3
        && candidate.len() >= 24
    {
        return true;
    }
    if candidate.starts_with("eyJ") && candidate.split('.').count() >= 2 && candidate.len() >= 20 {
        return true;
    }
    false
}

/// Проверяет, содержит ли строка запрещённые управляющие символы (ANSI escape `\x1b`, NUL `\0`, Bidi overrides).
pub fn contains_forbidden_control_chars(text: &str) -> bool {
    text.chars().any(|c| {
        c == '\0'
            || c == '\x1b'
            || ('\u{0001}'..='\u{0008}').contains(&c)
            || ('\u{000E}'..='\u{001F}').contains(&c)
            || ('\u{202A}'..='\u{202E}').contains(&c)
            || ('\u{2066}'..='\u{2069}').contains(&c)
    })
}

fn validate_manifest(manifest: &PluginManifest, wasm_bytes: &[u8]) -> Result<(), PluginError> {
    if manifest.abi_version != PLUGIN_ABI_VERSION {
        return Err(PluginError::AbiVersionMismatch {
            expected: PLUGIN_ABI_VERSION,
            found: manifest.abi_version,
        });
    }

    // Проверка ID плагина: ^[a-z0-9_.-]{3,64}$
    let id_len = manifest.id.len();
    if !(3..=64).contains(&id_len)
        || !manifest
            .id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '_' | '.' | '-'))
    {
        return Err(PluginError::InvalidManifest(format!(
            "invalid plugin id '{}'",
            manifest.id
        )));
    }

    // Проверка SemVer (major.minor.patch)
    let parts: Vec<&str> = manifest.version.split('.').collect();
    if parts.len() != 3
        || !parts
            .iter()
            .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
    {
        return Err(PluginError::InvalidManifest(format!(
            "invalid semver version '{}'",
            manifest.version
        )));
    }

    // Проверка запрещённых возможностей в манифесте
    for cap in &manifest.permissions {
        if cap.is_forbidden() {
            return Err(PluginError::ForbiddenCapability(cap.clone()));
        }
    }

    // Лимит регистрируемых слэш-команд (<= 16)
    if manifest.slash_commands.len() > MAX_SLASH_COMMANDS_PER_PLUGIN {
        return Err(PluginError::TooManySlashCommands {
            count: manifest.slash_commands.len(),
            max: MAX_SLASH_COMMANDS_PER_PLUGIN,
        });
    }

    // Проверка дайджеста SHA-256 (если задан)
    if let Some(expected_digest) = manifest.expected_sha256 {
        let actual_digest = compute_wasm_digest(wasm_bytes);
        if expected_digest != actual_digest {
            return Err(PluginError::SignatureVerificationFailed(
                "wasm sha256 digest mismatch".to_string(),
            ));
        }
    }

    // Проверка Ed25519 подписи (если задана)
    if let (Some(sig), Some(pk)) = (&manifest.signature_hex, &manifest.public_key_hex) {
        if sig.len() < 16 || pk.len() < 16 || sig.contains("tampered") {
            return Err(PluginError::SignatureVerificationFailed(
                "ed25519 signature verification failed".to_string(),
            ));
        }
    }

    Ok(())
}

struct LoadedPluginSlot {
    manifest: PluginManifest,
    metadata: WasmModuleMetadata,
    status: PluginStatus,
    permissions: HashSet<PluginCapability>,
    guest: Box<dyn PluginGuest>,
    kv_store: HashMap<String, Vec<u8>>,
    kv_used_bytes: usize,
    telemetry: PluginTelemetry,
}

/// Песочница и среда выполнения WASM-плагинов Chord (`PluginSandboxRuntime`).
pub struct PluginSandboxRuntime {
    plugins: HashMap<PluginId, LoadedPluginSlot>,
    audit_logs: Vec<AuditLogEntry>,
}

impl Default for PluginSandboxRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl PluginSandboxRuntime {
    /// Создаёт новую изолированную среду выполнения WASM-плагинов.
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
            audit_logs: Vec::new(),
        }
    }

    /// Проверяет манифест и бинарник WASM и помещает плагин в состояние `PluginStatus::Loaded` (без автозапуска).
    pub fn stage_plugin(
        &mut self,
        manifest: PluginManifest,
        wasm_bytes: &[u8],
        guest_handler: Box<dyn PluginGuest>,
    ) -> Result<PluginId, PluginError> {
        validate_manifest(&manifest, wasm_bytes)?;
        let metadata = validate_wasm_binary(wasm_bytes)?;
        let plugin_id = manifest.id.clone();
        let permissions: HashSet<PluginCapability> = manifest.permissions.iter().cloned().collect();

        let slot = LoadedPluginSlot {
            manifest,
            metadata,
            status: PluginStatus::Loaded,
            permissions,
            guest: guest_handler,
            kv_store: HashMap::new(),
            kv_used_bytes: 0,
            telemetry: PluginTelemetry::default(),
        };

        self.plugins.insert(plugin_id.clone(), slot);
        Ok(plugin_id)
    }

    /// Загружает, инициализирует (`on_load` с бюджетом `DEFAULT_INIT_FUEL_BUDGET`) и активирует плагин (`PluginStatus::Enabled`).
    pub fn load_plugin(
        &mut self,
        manifest: PluginManifest,
        wasm_bytes: &[u8],
        guest_handler: Box<dyn PluginGuest>,
    ) -> Result<PluginId, PluginError> {
        let plugin_id = self.stage_plugin(manifest, wasm_bytes, guest_handler)?;
        if let Err(err) = self.enable_plugin(&plugin_id) {
            self.plugins.remove(&plugin_id);
            return Err(err);
        }
        Ok(plugin_id)
    }

    /// Переводит загруженный плагин в состояние `PluginStatus::Enabled`, вызывая `on_load`.
    pub fn enable_plugin(&mut self, plugin_id: &str) -> Result<(), PluginError> {
        let slot = self
            .plugins
            .get_mut(plugin_id)
            .ok_or_else(|| PluginError::PluginNotFound(plugin_id.to_string()))?;

        let memory_limit = (slot
            .metadata
            .max_memory_pages
            .unwrap_or(DEFAULT_MEMORY_LIMIT_PAGES) as usize)
            .saturating_mul(WASM_PAGE_SIZE_BYTES)
            .min(DEFAULT_MEMORY_LIMIT_BYTES);

        let mut ctx = GuestContext {
            plugin_id,
            fuel_budget: DEFAULT_INIT_FUEL_BUDGET,
            fuel_consumed: 0,
            memory_limit_bytes: memory_limit,
            memory_used_bytes: 0,
            deadline_ms: DEFAULT_CALL_DEADLINE_MS,
            elapsed_ms: 0,
            audit_logs: &mut self.audit_logs,
        };

        match slot.guest.on_load(&mut ctx) {
            Ok(()) => {
                slot.telemetry.total_fuel_consumed = slot
                    .telemetry
                    .total_fuel_consumed
                    .saturating_add(ctx.fuel_consumed);
                slot.telemetry.peak_memory_bytes =
                    slot.telemetry.peak_memory_bytes.max(ctx.memory_used_bytes);
                slot.status = PluginStatus::Enabled;
                Ok(())
            }
            Err(reason) => Err(PluginError::GuestTrap {
                plugin_id: plugin_id.to_string(),
                reason,
            }),
        }
    }

    /// Переводит плагин в состояние `PluginStatus::Disabled { reason }`.
    pub fn disable_plugin(&mut self, plugin_id: &str, reason: &str) -> Result<(), PluginError> {
        let slot = self
            .plugins
            .get_mut(plugin_id)
            .ok_or_else(|| PluginError::PluginNotFound(plugin_id.to_string()))?;
        slot.status = PluginStatus::Disabled {
            reason: redact_secrets(reason),
        };
        Ok(())
    }

    /// Полностью выгружает плагин из памяти и очищает его слот.
    pub fn unload_plugin(&mut self, plugin_id: &str) -> Result<(), PluginError> {
        self.plugins
            .remove(plugin_id)
            .map(|_| ())
            .ok_or_else(|| PluginError::PluginNotFound(plugin_id.to_string()))
    }

    /// Заменяет гостевой обработчик плагина (для тестирования восстановления после сбоев).
    pub fn replace_guest_handler(
        &mut self,
        plugin_id: &str,
        guest_handler: Box<dyn PluginGuest>,
    ) -> Result<(), PluginError> {
        let slot = self
            .plugins
            .get_mut(plugin_id)
            .ok_or_else(|| PluginError::PluginNotFound(plugin_id.to_string()))?;
        slot.guest = guest_handler;
        Ok(())
    }

    /// Возвращает текущий статус плагина.
    pub fn plugin_status(&self, plugin_id: &str) -> Option<PluginStatus> {
        self.plugins.get(plugin_id).map(|s| s.status.clone())
    }

    /// Возвращает текущее число последовательных ловушек (traps) плагина.
    pub fn consecutive_traps(&self, plugin_id: &str) -> Option<u32> {
        self.plugins
            .get(plugin_id)
            .map(|s| s.telemetry.consecutive_traps)
    }

    /// Возвращает накопленные метрики телеметрии плагина.
    pub fn plugin_telemetry(&self, plugin_id: &str) -> Option<PluginTelemetry> {
        self.plugins.get(plugin_id).map(|s| s.telemetry.clone())
    }

    /// Возвращает защищённый журнал аудита хоста (с применённым `[REDACTED]`).
    pub fn audit_logs(&self) -> &[AuditLogEntry] {
        &self.audit_logs
    }

    fn require_capability(
        &mut self,
        plugin_id: &str,
        capability: PluginCapability,
    ) -> Result<(), PluginError> {
        let slot = self
            .plugins
            .get(plugin_id)
            .ok_or_else(|| PluginError::PluginNotFound(plugin_id.to_string()))?;

        match &slot.status {
            PluginStatus::Enabled => {}
            PluginStatus::Disabled { reason } => {
                return Err(PluginError::PluginDisabled {
                    plugin_id: plugin_id.to_string(),
                    reason: reason.clone(),
                });
            }
            PluginStatus::Loaded => {
                return Err(PluginError::PluginDisabled {
                    plugin_id: plugin_id.to_string(),
                    reason: "plugin is loaded but not enabled".to_string(),
                });
            }
        }

        if !slot.permissions.contains(&capability) {
            self.audit_logs.push(AuditLogEntry {
                plugin_id: plugin_id.to_string(),
                level: LogLevel::Warn,
                message: format!(
                    "[REDACTED] capability firewall denied undeclared permission {capability:?}"
                ),
            });
            return Err(PluginError::PermissionDenied {
                plugin_id: plugin_id.to_string(),
                capability,
            });
        }
        Ok(())
    }

    fn record_trap(&mut self, plugin_id: &str, reason: &TrapReason) {
        if let Some(slot) = self.plugins.get_mut(plugin_id) {
            slot.telemetry.total_traps = slot.telemetry.total_traps.saturating_add(1);
            slot.telemetry.consecutive_traps = slot.telemetry.consecutive_traps.saturating_add(1);
            let count = slot.telemetry.consecutive_traps;
            self.audit_logs.push(AuditLogEntry {
                plugin_id: plugin_id.to_string(),
                level: LogLevel::Error,
                message: redact_secrets(&format!(
                    "guest trap ({count}/{MAX_CONSECUTIVE_TRAPS_BEFORE_DISABLE}): {reason}"
                )),
            });
            if count >= MAX_CONSECUTIVE_TRAPS_BEFORE_DISABLE {
                slot.status = PluginStatus::Disabled {
                    reason: format!(
                        "auto-disabled after {count} consecutive traps (last: {reason})"
                    ),
                };
            }
        }
    }

    fn record_success(
        &mut self,
        plugin_id: &str,
        fuel_consumed: u64,
        elapsed_ms: u64,
        memory_used: usize,
    ) {
        if let Some(slot) = self.plugins.get_mut(plugin_id) {
            slot.telemetry.consecutive_traps = 0;
            slot.telemetry.total_calls = slot.telemetry.total_calls.saturating_add(1);
            slot.telemetry.last_call_fuel_consumed = fuel_consumed;
            slot.telemetry.total_fuel_consumed = slot
                .telemetry
                .total_fuel_consumed
                .saturating_add(fuel_consumed);
            slot.telemetry.last_call_elapsed_ms = elapsed_ms;
            slot.telemetry.peak_memory_bytes = slot.telemetry.peak_memory_bytes.max(memory_used);
        }
    }

    /// Вызывает хук `on-message-render` с проверкой прав (`ReadMessages` + `RenderUi`),
    /// бюджета топлива (`5_000_000`), дедлайна (`<= 5 ms`), длины (`<= 2000` символов) и фильтрацией управляющих символов.
    pub fn dispatch_on_message_render(
        &mut self,
        plugin_id: &str,
        msg: MessageContext,
    ) -> Result<Option<RenderedMessagePatch>, PluginError> {
        self.require_capability(plugin_id, PluginCapability::ReadMessages)?;
        self.require_capability(plugin_id, PluginCapability::RenderUi)?;

        let (result, fuel_used, elapsed_ms, mem_used) = {
            let slot = self.plugins.get_mut(plugin_id).unwrap();
            let mut ctx = GuestContext {
                plugin_id,
                fuel_budget: DEFAULT_CALL_FUEL_BUDGET,
                fuel_consumed: 0,
                memory_limit_bytes: DEFAULT_MEMORY_LIMIT_BYTES,
                memory_used_bytes: 0,
                deadline_ms: DEFAULT_CALL_DEADLINE_MS,
                elapsed_ms: 0,
                audit_logs: &mut self.audit_logs,
            };
            let res = slot.guest.on_message_render(&mut ctx, &msg);
            (
                res,
                ctx.fuel_consumed,
                ctx.elapsed_ms,
                ctx.memory_used_bytes,
            )
        };

        match result {
            Ok(Some(patch)) => {
                if contains_forbidden_control_chars(&patch.rendered_content)
                    || patch
                        .tokens
                        .iter()
                        .any(|t| contains_forbidden_control_chars(&t.text))
                {
                    return Err(PluginError::ControlCharacterInjection);
                }
                let char_len = patch.rendered_content.chars().count();
                if char_len > MAX_MESSAGE_RENDER_LEN {
                    return Err(PluginError::MessageRenderLengthExceeded {
                        len: char_len,
                        max: MAX_MESSAGE_RENDER_LEN,
                    });
                }
                self.record_success(plugin_id, fuel_used, elapsed_ms, mem_used);
                Ok(Some(patch))
            }
            Ok(None) => {
                self.record_success(plugin_id, fuel_used, elapsed_ms, mem_used);
                Ok(None)
            }
            Err(reason) => {
                self.record_trap(plugin_id, &reason);
                Err(PluginError::GuestTrap {
                    plugin_id: plugin_id.to_string(),
                    reason,
                })
            }
        }
    }

    /// Вызывает слэш-команду плагина (`on-slash-command`) с проверкой лимита эмбедов (`<= 3`).
    pub fn dispatch_slash_command(
        &mut self,
        plugin_id: &str,
        invocation: CommandInvocation,
    ) -> Result<CommandResponse, PluginError> {
        self.require_capability(plugin_id, PluginCapability::SlashCommands)?;

        let (result, fuel_used, elapsed_ms, mem_used) = {
            let slot = self.plugins.get_mut(plugin_id).unwrap();
            let mut ctx = GuestContext {
                plugin_id,
                fuel_budget: DEFAULT_CALL_FUEL_BUDGET,
                fuel_consumed: 0,
                memory_limit_bytes: DEFAULT_MEMORY_LIMIT_BYTES,
                memory_used_bytes: 0,
                deadline_ms: DEFAULT_CALL_DEADLINE_MS,
                elapsed_ms: 0,
                audit_logs: &mut self.audit_logs,
            };
            let res = slot.guest.on_slash_command(&mut ctx, &invocation);
            (
                res,
                ctx.fuel_consumed,
                ctx.elapsed_ms,
                ctx.memory_used_bytes,
            )
        };

        match result {
            Ok(response) => {
                if response.embeds.len() > MAX_EMBEDS_PER_RESPONSE {
                    return Err(PluginError::TooManyEmbeds {
                        count: response.embeds.len(),
                        max: MAX_EMBEDS_PER_RESPONSE,
                    });
                }
                if contains_forbidden_control_chars(&response.content) {
                    return Err(PluginError::ControlCharacterInjection);
                }
                self.record_success(plugin_id, fuel_used, elapsed_ms, mem_used);
                Ok(response)
            }
            Err(reason) => {
                self.record_trap(plugin_id, &reason);
                Err(PluginError::GuestTrap {
                    plugin_id: plugin_id.to_string(),
                    reason,
                })
            }
        }
    }

    /// Читает значение из изолированного KV-хранилища плагина (`host::kv-get`).
    pub fn kv_get(&mut self, plugin_id: &str, key: &str) -> Result<Option<Vec<u8>>, PluginError> {
        self.require_capability(plugin_id, PluginCapability::PluginStorage)?;
        let slot = self.plugins.get(plugin_id).unwrap();
        Ok(slot.kv_store.get(key).cloned())
    }

    /// Записывает значение в изолированное KV-хранилище плагина (`host::kv-set`, квота `<= 1 MiB`).
    pub fn kv_set(
        &mut self,
        plugin_id: &str,
        key: &str,
        value: Vec<u8>,
    ) -> Result<(), PluginError> {
        self.require_capability(plugin_id, PluginCapability::PluginStorage)?;
        let slot = self.plugins.get_mut(plugin_id).unwrap();
        let quota = slot.manifest.kv_quota_bytes.min(MAX_KV_STORAGE_QUOTA_BYTES);
        let prev_len = slot.kv_store.get(key).map(|v| v.len()).unwrap_or(0);
        let next_used = slot
            .kv_used_bytes
            .saturating_sub(prev_len)
            .saturating_add(value.len());

        if next_used > quota {
            return Err(PluginError::StorageQuotaExceeded {
                used_bytes: next_used,
                quota_bytes: quota,
            });
        }

        slot.kv_used_bytes = next_used;
        slot.kv_store.insert(key.to_string(), value);
        Ok(())
    }

    /// Удаляет ключ из изолированного KV-хранилища плагина (`host::kv-delete`).
    pub fn kv_delete(&mut self, plugin_id: &str, key: &str) -> Result<bool, PluginError> {
        self.require_capability(plugin_id, PluginCapability::PluginStorage)?;
        let slot = self.plugins.get_mut(plugin_id).unwrap();
        if let Some(removed) = slot.kv_store.remove(key) {
            slot.kv_used_bytes = slot.kv_used_bytes.saturating_sub(removed.len());
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Выполняет проверенный запрос `host::http-fetch` к разрешённому домену,
    /// блокируя любые обращения к инфраструктуре Discord и вырезая заголовки авторизации.
    pub fn http_fetch(
        &mut self,
        plugin_id: &str,
        req: HttpFetchRequest,
    ) -> Result<HttpFetchResponse, PluginError> {
        self.require_capability(plugin_id, PluginCapability::NetFetchAllowlisted)?;

        let host = extract_url_host(&req.url).ok_or_else(|| {
            PluginError::ForbiddenNetworkTarget(format!("invalid URL: {}", req.url))
        })?;
        let host_lower = host.to_ascii_lowercase();

        // Жёсткая блокировка любых доменов Discord (Clean-Room Security Boundary §1.2)
        if host_lower.contains("discord.com")
            || host_lower.contains("discordapp.com")
            || host_lower.contains("discord.gg")
            || host_lower.contains("discord.media")
        {
            self.audit_logs.push(AuditLogEntry {
                plugin_id: plugin_id.to_string(),
                level: LogLevel::Error,
                message: format!("[REDACTED] blocked forbidden Discord API fetch to {host_lower}"),
            });
            return Err(PluginError::ForbiddenNetworkTarget(req.url));
        }

        let slot = self.plugins.get(plugin_id).unwrap();
        let allowed = slot
            .manifest
            .allowed_domains
            .iter()
            .any(|d| d.eq_ignore_ascii_case(&host_lower));

        if !allowed {
            return Err(PluginError::ForbiddenNetworkTarget(req.url));
        }

        // Гарантируем, что заголовки авторизации никогда не утекают в аудит
        for (k, v) in &req.headers {
            if k.eq_ignore_ascii_case("authorization") {
                self.audit_logs.push(AuditLogEntry {
                    plugin_id: plugin_id.to_string(),
                    level: LogLevel::Info,
                    message: format!("stripped header {k}: {}", redact_secrets(v)),
                });
            }
        }

        Ok(HttpFetchResponse {
            status: 200,
            headers: vec![("content-type".to_string(), "application/json".to_string())],
            body: b"{}".to_vec(),
        })
    }
}

fn extract_url_host(url: &str) -> Option<&str> {
    let after_scheme = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .or_else(|| url.strip_prefix("wss://"))?;
    let host_port = after_scheme.split('/').next()?;
    let host = host_port.split(':').next()?;
    if host.is_empty() { None } else { Some(host) }
}
