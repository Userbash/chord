//! Платформенный D-Bus клиент `org.freedesktop.portal.ScreenCast` (PipeWire fd) и зонд X11 Display/SHM.
//!
//! Оракул: `/usr/share/dbus-1/interfaces/org.freedesktop.portal.ScreenCast.xml` (v6),
//! `org.freedesktop.portal.Request.xml`, `org.freedesktop.portal.Session.xml` и
//! `/home/sanya/re-deep/native-re.md` (`discord_voice.node` -> `capture_linux`).

use std::fmt;
use std::os::fd::RawFd;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::dbus::{BusAddress, Connection, DbusError, Value, fd};

/// Имя D-Bus сервиса XDG Desktop Portal.
pub const PORTAL_BUS_NAME: &str = "org.freedesktop.portal.Desktop";
/// Путь корневого объекта XDG Desktop Portal.
pub const PORTAL_OBJECT_PATH: &str = "/org/freedesktop/portal/desktop";
/// Интерфейс захвата экрана и окон.
pub const SCREENCAST_INTERFACE: &str = "org.freedesktop.portal.ScreenCast";
/// Интерфейс асинхронного запроса портала.
pub const REQUEST_INTERFACE: &str = "org.freedesktop.portal.Request";
/// Интерфейс сессии портала.
pub const SESSION_INTERFACE: &str = "org.freedesktop.portal.Session";

/// Таймаут ожидания ответа пользователя в диалоге портала по умолчанию.
pub const DEFAULT_PORTAL_DIALOG_TIMEOUT: Duration = Duration::from_secs(30);

/// Битовая маска типов источников `org.freedesktop.portal.ScreenCast`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PortalSourceTypes(u32);

impl PortalSourceTypes {
    /// Захват монитора (`1`).
    pub const MONITOR: Self = Self(1);
    /// Захват окна (`2`).
    pub const WINDOW: Self = Self(2);
    /// Виртуальный дисплей (`4`).
    pub const VIRTUAL: Self = Self(4);
    /// Захват мониторов и окон (`1 | 2`).
    pub const MONITOR_AND_WINDOW: Self = Self(1 | 2);

    /// Создаёт маску с усечением неизвестных битов.
    #[must_use]
    pub const fn from_bits_truncate(bits: u32) -> Self {
        Self(bits & 0b111)
    }

    /// Возвращает числовое представление битовой маски.
    #[must_use]
    pub const fn bits(self) -> u32 {
        self.0
    }

    /// Проверяет, что маска пуста.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

/// Режим курсора `org.freedesktop.portal.ScreenCast`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum PortalCursorMode {
    /// Курсор скрыт (`1`).
    Hidden = 1,
    /// Курсор врисован в кадр (`2`).
    Embedded = 2,
    /// Курсор передаётся в метаданных PipeWire (`4`).
    Metadata = 4,
}

/// Режим сохранения разрешения `org.freedesktop.portal.ScreenCast`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum PortalPersistMode {
    /// Не сохранять (`0`).
    DoNotPersist = 0,
    /// Сохранять на время работы приложения (`1`).
    ApplicationRunning = 1,
    /// Сохранять до явного отзыва (`2`).
    ExplicitlyRevoked = 2,
}

/// Ошибки платформенного клиента `ScreenCastPortal` и зонда X11.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PortalRequestError {
    /// Пользователь отклонил запрос в диалоге (`Response == 1`).
    ConsentDenied,
    /// Диалог портала был прерван (`Response == 2`).
    ConsentAborted,
    /// Истекло время ожидания сигнала `Request.Response`.
    Timeout,
    /// Недопустимый токен запроса/сессии (`handle_token` / `session_handle_token`).
    InvalidToken(String),
    /// Недопустимый путь объекта D-Bus.
    InvalidObjectPath(String),
    /// Недопустимая маска типов источников.
    InvalidSourceTypes(u32),
    /// Невалидный файловый дескриптор PipeWire (`fd < 0`).
    InvalidFd(RawFd),
    /// Портал не вернул потоков PipeWire в `Start`.
    EmptyStreams,
    /// Некорректная строка `DISPLAY` X11.
    InvalidX11Display(String),
    /// Ошибка протокола или транспорта D-Bus.
    Dbus(String),
}

impl fmt::Display for PortalRequestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConsentDenied => write!(f, "пользователь отклонил захват экрана в портале"),
            Self::ConsentAborted => write!(f, "диалог портала захвата экрана был прерван"),
            Self::Timeout => write!(f, "истекло время ожидания ответа от портала ScreenCast"),
            Self::InvalidToken(t) => write!(f, "недопустимый токен портала: {t:?}"),
            Self::InvalidObjectPath(p) => write!(f, "недопустимый D-Bus путь: {p:?}"),
            Self::InvalidSourceTypes(m) => write!(f, "недопустимая маска типов источников: {m}"),
            Self::InvalidFd(raw) => write!(f, "недопустимый файловый дескриптор PipeWire: {raw}"),
            Self::EmptyStreams => write!(f, "портал не вернул ни одного потока PipeWire"),
            Self::InvalidX11Display(d) => write!(f, "некорректная спецификация X11 DISPLAY: {d:?}"),
            Self::Dbus(msg) => write!(f, "ошибка D-Bus: {msg}"),
        }
    }
}

impl std::error::Error for PortalRequestError {}

impl From<DbusError> for PortalRequestError {
    fn from(err: DbusError) -> Self {
        match err {
            DbusError::Timeout => Self::Timeout,
            DbusError::Remote { ref name, .. }
                if name == "org.freedesktop.portal.Error.Cancelled"
                    || name == "org.freedesktop.portal.Error.NotAllowed" =>
            {
                Self::ConsentDenied
            }
            other => Self::Dbus(format!("{other:?}")),
        }
    }
}

/// Опции запуска сессии `org.freedesktop.portal.ScreenCast`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScreenCastOptions {
    /// Токен запроса (`handle_token`).
    pub handle_token: String,
    /// Токен сессии (`session_handle_token`).
    pub session_handle_token: String,
    /// Типы запрашиваемых источников.
    pub source_types: PortalSourceTypes,
    /// Разрешить множественный выбор.
    pub multiple: bool,
    /// Режим курсора.
    pub cursor_mode: PortalCursorMode,
    /// Режим сохранения токена.
    pub persist_mode: PortalPersistMode,
    /// Предыдущий токен восстановления.
    pub restore_token: Option<String>,
    /// Идентификатор родительского окна.
    pub parent_window: String,
}

impl ScreenCastOptions {
    /// Проверяет корректность токенов и маски типов источников.
    pub fn validate(&self) -> Result<(), PortalRequestError> {
        validate_token(&self.handle_token)?;
        validate_token(&self.session_handle_token)?;
        if self.source_types.is_empty() {
            return Err(PortalRequestError::InvalidSourceTypes(
                self.source_types.bits(),
            ));
        }
        Ok(())
    }
}

/// Описание потока из ответа `Start` (`streams: a(ua{sv})`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortalStreamInfo {
    /// Идентификатор узла PipeWire (`node_id`).
    pub node_id: u32,
    /// Строковый идентификатор источника (`id`).
    pub id: Option<String>,
    /// Код типа источника (`1` = монитор, `2` = окно, `4` = виртуальный).
    pub source_type: u32,
    /// Координаты `(x, y)`.
    pub position: Option<(i32, i32)>,
    /// Размеры `(width, height)`.
    pub size: Option<(u32, u32)>,
    /// Идентификатор сопоставления (`mapping_id`).
    pub mapping_id: Option<String>,
}

/// RAII-обёртка над файловым дескриптором сокета PipeWire remote (`OpenPipeWireRemote`).
///
/// Гарантированно закрывает дескриптор через [`fd::close_fd`] при выходе из области видимости.
#[derive(Debug)]
pub struct OwnedPipeWireFd {
    raw: RawFd,
}

impl OwnedPipeWireFd {
    /// Проверяет, что `raw >= 0`, и оборачивает дескриптор в RAII-структуру.
    pub fn from_raw_fd_checked(raw: RawFd) -> Result<Self, PortalRequestError> {
        if raw < 0 {
            return Err(PortalRequestError::InvalidFd(raw));
        }
        Ok(Self { raw })
    }

    /// Возвращает числовое значение файлового дескриптора.
    #[must_use]
    pub const fn as_raw_fd(&self) -> RawFd {
        self.raw
    }
}

impl Drop for OwnedPipeWireFd {
    fn drop(&mut self) {
        if self.raw >= 0 {
            fd::close_fd(self.raw);
        }
    }
}

/// Разобранная спецификация дисплея X11 (переменная `DISPLAY`, например `:0`, `:1.2`, `unix:0.0`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct X11DisplaySpec {
    /// Номер дисплея X11.
    pub display_number: u32,
    /// Номер экрана X11.
    pub screen_number: u32,
    /// Путь к локальному unix-сокету `/tmp/.X11-unix/X<N>`.
    pub unix_socket_path: PathBuf,
}

impl X11DisplaySpec {
    /// Проверяет наличие файла сокета X11 в файловой системе.
    #[must_use]
    pub fn is_socket_present(&self) -> bool {
        self.unix_socket_path.exists()
    }
}

/// Разбирает строку переменной `DISPLAY` (например, `":0"`, `":1.2"`, `"unix:0"`)
/// и возвращает путь к локальному сокету `/tmp/.X11-unix/X<display>`.
pub fn parse_x11_display_spec(display: &str) -> Result<X11DisplaySpec, PortalRequestError> {
    let trimmed = display.trim();
    let rest = if let Some(after_unix) = trimmed.strip_prefix("unix:") {
        after_unix
    } else if let Some(after_colon) = trimmed.strip_prefix(':') {
        after_colon
    } else {
        return Err(PortalRequestError::InvalidX11Display(display.to_string()));
    };

    if rest.is_empty() {
        return Err(PortalRequestError::InvalidX11Display(display.to_string()));
    }

    let (disp_str, screen_str) = match rest.split_once('.') {
        Some((d, s)) => (d, Some(s)),
        None => (rest, None),
    };

    if disp_str.is_empty() || !disp_str.bytes().all(|b| b.is_ascii_digit()) {
        return Err(PortalRequestError::InvalidX11Display(display.to_string()));
    }
    let display_number = disp_str
        .parse::<u32>()
        .map_err(|_| PortalRequestError::InvalidX11Display(display.to_string()))?;

    let screen_number = match screen_str {
        Some(s) => {
            if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
                return Err(PortalRequestError::InvalidX11Display(display.to_string()));
            }
            s.parse::<u32>()
                .map_err(|_| PortalRequestError::InvalidX11Display(display.to_string()))?
        }
        None => 0,
    };

    let unix_socket_path = PathBuf::from(format!("/tmp/.X11-unix/X{display_number}"));
    Ok(X11DisplaySpec {
        display_number,
        screen_number,
        unix_socket_path,
    })
}

/// D-Bus клиент портала `org.freedesktop.portal.ScreenCast`.
pub struct ScreenCastPortal {
    conn: Connection,
    dialog_timeout: Duration,
}

impl ScreenCastPortal {
    /// Подключается к сессионной шине D-Bus с согласованием `NEGOTIATE_UNIX_FD`.
    pub fn connect_session() -> Result<Self, PortalRequestError> {
        let addr = BusAddress::from_env().ok_or_else(|| {
            PortalRequestError::Dbus("DBUS_SESSION_BUS_ADDRESS не задан".to_string())
        })?;
        Self::connect(&addr)
    }

    /// Подключается к указанному адресу шины D-Bus.
    pub fn connect(address: &BusAddress) -> Result<Self, PortalRequestError> {
        let conn = Connection::connect(address)?;
        Ok(Self {
            conn,
            dialog_timeout: DEFAULT_PORTAL_DIALOG_TIMEOUT,
        })
    }

    /// Устанавливает таймаут ожидания сигнала `Request.Response`.
    pub fn set_dialog_timeout(&mut self, timeout: Duration) {
        self.dialog_timeout = timeout;
    }

    /// Вызывает `CreateSession` и ожидает `session_handle` из сигнала `Request.Response`.
    pub fn create_session(
        &mut self,
        options: &ScreenCastOptions,
    ) -> Result<String, PortalRequestError> {
        options.validate()?;
        let dict = Value::Dict(vec![
            (
                Value::String("handle_token".to_string()),
                Value::Variant(Box::new(Value::String(options.handle_token.clone()))),
            ),
            (
                Value::String("session_handle_token".to_string()),
                Value::Variant(Box::new(Value::String(
                    options.session_handle_token.clone(),
                ))),
            ),
        ]);

        let reply = self.conn.call_typed(
            PORTAL_BUS_NAME,
            PORTAL_OBJECT_PATH,
            SCREENCAST_INTERFACE,
            "CreateSession",
            "a{sv}",
            &[dict],
        )?;
        let request_path = extract_object_path(&reply)?;
        let results = self.await_request_response(&request_path)?;

        if let Some(session_handle) = lookup_dict_string(&results, "session_handle") {
            validate_object_path(&session_handle)?;
            Ok(session_handle)
        } else {
            Err(PortalRequestError::Dbus(
                "в ответе CreateSession отсутствует поле session_handle".to_string(),
            ))
        }
    }

    /// Вызывает `SelectSources` для активной сессии портала.
    pub fn select_sources(
        &mut self,
        session_handle: &str,
        options: &ScreenCastOptions,
    ) -> Result<(), PortalRequestError> {
        validate_object_path(session_handle)?;
        options.validate()?;

        let mut entries = vec![
            (
                Value::String("handle_token".to_string()),
                Value::Variant(Box::new(Value::String(options.handle_token.clone()))),
            ),
            (
                Value::String("types".to_string()),
                Value::Variant(Box::new(Value::U32(options.source_types.bits()))),
            ),
            (
                Value::String("multiple".to_string()),
                Value::Variant(Box::new(Value::Bool(options.multiple))),
            ),
            (
                Value::String("cursor_mode".to_string()),
                Value::Variant(Box::new(Value::U32(options.cursor_mode as u32))),
            ),
            (
                Value::String("persist_mode".to_string()),
                Value::Variant(Box::new(Value::U32(options.persist_mode as u32))),
            ),
        ];
        if let Some(ref restore) = options.restore_token
            && !restore.is_empty()
        {
            entries.push((
                Value::String("restore_token".to_string()),
                Value::Variant(Box::new(Value::String(restore.clone()))),
            ));
        }

        let reply = self.conn.call_typed(
            PORTAL_BUS_NAME,
            PORTAL_OBJECT_PATH,
            SCREENCAST_INTERFACE,
            "SelectSources",
            "oa{sv}",
            &[
                Value::String(session_handle.to_string()),
                Value::Dict(entries),
            ],
        )?;
        let request_path = extract_object_path(&reply)?;
        let _ = self.await_request_response(&request_path)?;
        Ok(())
    }

    /// Вызывает `Start` и разбирает согласованные потоки `streams: a(ua{sv})` и `restore_token`.
    pub fn start(
        &mut self,
        session_handle: &str,
        options: &ScreenCastOptions,
    ) -> Result<(Vec<PortalStreamInfo>, Option<String>), PortalRequestError> {
        validate_object_path(session_handle)?;
        options.validate()?;

        let dict = Value::Dict(vec![(
            Value::String("handle_token".to_string()),
            Value::Variant(Box::new(Value::String(options.handle_token.clone()))),
        )]);

        let reply = self.conn.call_typed(
            PORTAL_BUS_NAME,
            PORTAL_OBJECT_PATH,
            SCREENCAST_INTERFACE,
            "Start",
            "osa{sv}",
            &[
                Value::String(session_handle.to_string()),
                Value::String(options.parent_window.clone()),
                dict,
            ],
        )?;
        let request_path = extract_object_path(&reply)?;
        let results = self.await_request_response(&request_path)?;
        Self::parse_start_results(&results)
    }

    /// Вызывает `OpenPipeWireRemote` и возвращает RAII-дескриптор сокета PipeWire (`SCM_RIGHTS`).
    pub fn open_pipewire_remote(
        &mut self,
        session_handle: &str,
    ) -> Result<OwnedPipeWireFd, PortalRequestError> {
        validate_object_path(session_handle)?;
        let (values, fds) = self.conn.call_typed_with_fds(
            PORTAL_BUS_NAME,
            PORTAL_OBJECT_PATH,
            SCREENCAST_INTERFACE,
            "OpenPipeWireRemote",
            "oa{sv}",
            &[
                Value::String(session_handle.to_string()),
                Value::Dict(Vec::new()),
            ],
        )?;

        let raw_fd = match values.first() {
            Some(Value::UnixFd(fd_val)) => *fd_val,
            _ => {
                fd::close_all(&fds);
                return Err(PortalRequestError::Dbus(
                    "ответ OpenPipeWireRemote не содержит дескриптора h".to_string(),
                ));
            }
        };

        // Закрываем все побочные fd, если сервер прислал больше одного
        for extra in fds.into_iter().filter(|f| *f != raw_fd) {
            fd::close_fd(extra);
        }

        OwnedPipeWireFd::from_raw_fd_checked(raw_fd)
    }

    /// Закрывает сессию портала (`org.freedesktop.portal.Session.Close`).
    pub fn close_session(&mut self, session_handle: &str) -> Result<(), PortalRequestError> {
        validate_object_path(session_handle)?;
        let _ = self.conn.call_typed(
            PORTAL_BUS_NAME,
            session_handle,
            SESSION_INTERFACE,
            "Close",
            "",
            &[],
        )?;
        Ok(())
    }

    /// Отменяет активный запрос (`org.freedesktop.portal.Request.Close`).
    pub fn cancel_request(&mut self, request_handle: &str) -> Result<(), PortalRequestError> {
        validate_object_path(request_handle)?;
        let _ = self.conn.call_typed(
            PORTAL_BUS_NAME,
            request_handle,
            REQUEST_INTERFACE,
            "Close",
            "",
            &[],
        )?;
        Ok(())
    }

    /// Разбирает словарь `results` (`a{sv}`) из ответа `Start`, извлекая `streams: a(ua{sv})` и `restore_token`.
    pub fn parse_start_results(
        results: &Value,
    ) -> Result<(Vec<PortalStreamInfo>, Option<String>), PortalRequestError> {
        let restore_token = lookup_dict_string(results, "restore_token");
        let Some(streams_val) = lookup_dict_value(results, "streams") else {
            return Err(PortalRequestError::EmptyStreams);
        };
        let Value::Array(items) = streams_val else {
            return Err(PortalRequestError::Dbus(
                "поле streams имеет неверный тип (ожидался массив a(ua{sv}))".to_string(),
            ));
        };

        let mut streams = Vec::with_capacity(items.len());
        for item in items {
            let Value::Struct(fields) = item else {
                return Err(PortalRequestError::Dbus(
                    "элемент streams не является структурой (ua{sv})".to_string(),
                ));
            };
            let Some(Value::U32(node_id)) = fields.first() else {
                return Err(PortalRequestError::Dbus(
                    "первое поле структуры потока не u32 node_id".to_string(),
                ));
            };
            let props = fields.get(1).ok_or_else(|| {
                PortalRequestError::Dbus("отсутствует словарь свойств потока".to_string())
            })?;

            let id = lookup_dict_string(props, "id");
            let source_type = lookup_dict_u32(props, "source_type").unwrap_or(1);
            let position = lookup_dict_i32_pair(props, "position");
            let size = lookup_dict_i32_pair(props, "size").and_then(|(w, h)| {
                let w_u = u32::try_from(w).ok()?;
                let h_u = u32::try_from(h).ok()?;
                Some((w_u, h_u))
            });
            let mapping_id = lookup_dict_string(props, "mapping_id");

            streams.push(PortalStreamInfo {
                node_id: *node_id,
                id,
                source_type,
                position,
                size,
                mapping_id,
            });
        }

        if streams.is_empty() {
            return Err(PortalRequestError::EmptyStreams);
        }

        Ok((streams, restore_token))
    }

    fn await_request_response(&mut self, request_path: &str) -> Result<Value, PortalRequestError> {
        let match_rule = format!(
            "type='signal',interface='{REQUEST_INTERFACE}',member='Response',path='{request_path}'"
        );
        self.conn.add_match(&match_rule)?;

        let deadline = Instant::now() + self.dialog_timeout;
        loop {
            let now = Instant::now();
            if now >= deadline {
                let _ = self.cancel_request(request_path);
                return Err(PortalRequestError::Timeout);
            }
            let remaining = deadline.saturating_duration_since(now);
            let signal = match self.conn.next_signal(remaining) {
                Ok(sig) => sig,
                Err(DbusError::Timeout) => {
                    let _ = self.cancel_request(request_path);
                    return Err(PortalRequestError::Timeout);
                }
                Err(err) => return Err(PortalRequestError::from(err)),
            };

            if signal.interface == REQUEST_INTERFACE
                && signal.member == "Response"
                && signal.path == request_path
            {
                let code = match signal.body.first() {
                    Some(Value::U32(c)) => *c,
                    _ => {
                        return Err(PortalRequestError::Dbus(
                            "сигнал Request.Response не содержит u32 кода".to_string(),
                        ));
                    }
                };
                match code {
                    0 => {
                        let results = signal
                            .body
                            .get(1)
                            .cloned()
                            .unwrap_or_else(|| Value::Dict(Vec::new()));
                        return Ok(results);
                    }
                    1 => return Err(PortalRequestError::ConsentDenied),
                    _ => return Err(PortalRequestError::ConsentAborted),
                }
            }
        }
    }
}

fn validate_token(token: &str) -> Result<(), PortalRequestError> {
    if token.is_empty()
        || !token
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_')
    {
        return Err(PortalRequestError::InvalidToken(token.to_string()));
    }
    Ok(())
}

fn validate_object_path(path: &str) -> Result<(), PortalRequestError> {
    if path == "/" {
        return Ok(());
    }
    if !path.starts_with('/') || path.ends_with('/') {
        return Err(PortalRequestError::InvalidObjectPath(path.to_string()));
    }
    for seg in path[1..].split('/') {
        if seg.is_empty() || !seg.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') {
            return Err(PortalRequestError::InvalidObjectPath(path.to_string()));
        }
    }
    Ok(())
}

fn extract_object_path(reply: &[Value]) -> Result<String, PortalRequestError> {
    match reply.first() {
        Some(Value::String(path)) => {
            validate_object_path(path)?;
            Ok(path.clone())
        }
        _ => Err(PortalRequestError::Dbus(
            "ответ метода портала не содержит ObjectPath".to_string(),
        )),
    }
}

fn unwrap_variant(val: &Value) -> &Value {
    match val {
        Value::Variant(inner) => inner.as_ref(),
        other => other,
    }
}

fn lookup_dict_value<'a>(dict: &'a Value, key: &str) -> Option<&'a Value> {
    let Value::Dict(entries) = dict else {
        return None;
    };
    for (k, v) in entries {
        if let Value::String(k_str) = k
            && k_str == key
        {
            return Some(unwrap_variant(v));
        }
    }
    None
}

fn lookup_dict_string(dict: &Value, key: &str) -> Option<String> {
    match lookup_dict_value(dict, key)? {
        Value::String(s) => Some(s.clone()),
        _ => None,
    }
}

fn lookup_dict_u32(dict: &Value, key: &str) -> Option<u32> {
    match lookup_dict_value(dict, key)? {
        Value::U32(n) => Some(*n),
        _ => None,
    }
}

fn lookup_dict_i32_pair(dict: &Value, key: &str) -> Option<(i32, i32)> {
    let Value::Struct(items) = lookup_dict_value(dict, key)? else {
        return None;
    };
    let (Some(Value::I32(a)), Some(Value::I32(b))) = (items.first(), items.get(1)) else {
        return None;
    };
    Some((*a, *b))
}
