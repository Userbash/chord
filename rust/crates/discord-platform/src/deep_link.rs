//! P6-04: разбор `discord://` deep-link и маршрутизация по RE-подтверждённым формам.
//!
//! Оракул: `docs/re/main-arch.md` §9.4 (цитата `extractUrlFromArgs`), строки бандлов
//! `discord://login/one-time?token=…`, `discord://action/oauth2/authorize`,
//! `discord://app/open#…`, `discord://app/settings/nitro`, `discord:///library/<id>/launch`
//! и форма навигации `discord://[-]/channels/...`.
//!
//! Модель ничего не открывает и не ходит в сеть: это чистый разбор входа.
//!
//! Безопасность: `DeepLink::raw`/`query` содержат исходные данные (включая `token`),
//! поэтому в логи должен попадать только [`Route`] — его `Debug` скрывает токен.
//! Токен в [`Route::LoginOneTime`] — сырой срез query (без percent-декодирования),
//! чтобы потребитель сам решил, когда и как его декодировать.

use std::fmt;

/// Максимальная длина deep-link в байтах (включительно).
pub const MAX_DEEP_LINK_LEN: usize = 4096;

/// Ошибки разбора deep-link.
#[derive(Debug, PartialEq, Eq)]
pub enum DeepLinkError {
    /// Пустая строка.
    Empty,
    /// Схема не `discord` (сравнение ASCII-регистронезависимое).
    WrongScheme,
    /// Длина больше [`MAX_DEEP_LINK_LEN`].
    TooLong,
    /// Синтаксис нарушен: нет `//`, пустая схема, пробелы в scheme/host,
    /// control-символы (`< 0x20` или `0x7F`) в исходной строке.
    Malformed,
}

/// Разобранный deep-link; все срезы заимствованы из исходной строки.
///
/// `Debug` намеренно не выводит `raw`: исходная строка и query могут содержать
/// `token` (`discord://login/one-time?token=…`). В диагностике видны `host`/`path`,
/// а `query`/`fragment` печатаются как `<redacted>`.
#[derive(PartialEq, Eq)]
pub struct DeepLink<'a> {
    /// Исходная строка целиком.
    pub raw: &'a str,
    /// Authority (для `discord:///path` — пустая строка, для `discord://-/path` — `-`).
    pub host: &'a str,
    /// Путь с ведущим `/` (может быть пустым).
    pub path: &'a str,
    /// Query без `?`; пустая query → `None`.
    pub query: Option<&'a str>,
    /// Fragment без `#`; пустой fragment → `None`.
    pub fragment: Option<&'a str>,
}

/// Маршрут, подтверждённый RE; всё остальное — [`Route::Unknown`].
#[derive(Clone, PartialEq, Eq)]
pub enum Route {
    /// `discord://login/one-time[?token=…]`.
    LoginOneTime { token: Option<String> },
    /// `discord://action/oauth2/authorize`.
    OAuth2Authorize,
    /// `discord://app/open`.
    AppOpen,
    /// `discord://app/settings/<section>`.
    AppSettings { section: String },
    /// `discord:///library/<id>/launch`.
    Library { id: String },
    /// `discord://[-]/channels/...`; `path` — канонический путь `/channels/...`.
    Channels { path: String },
    /// Форма не подтверждена RE.
    Unknown,
}

impl fmt::Debug for DeepLink<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DeepLink")
            .field("host", &self.host)
            .field("path", &self.path)
            .field("query", &self.query.map(|_| "<redacted>"))
            .field("fragment", &self.fragment.map(|_| "<redacted>"))
            .finish()
    }
}

impl fmt::Debug for Route {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LoginOneTime { token } => {
                let mut debug = f.debug_struct("LoginOneTime");
                match token {
                    Some(_) => {
                        debug.field("token", &Some("<redacted>"));
                    }
                    None => {
                        debug.field("token", &Option::<&str>::None);
                    }
                }
                debug.finish()
            }
            Self::OAuth2Authorize => f.write_str("OAuth2Authorize"),
            Self::AppOpen => f.write_str("AppOpen"),
            Self::AppSettings { section } => f
                .debug_struct("AppSettings")
                .field("section", section)
                .finish(),
            Self::Library { id } => f.debug_struct("Library").field("id", id).finish(),
            Self::Channels { path } => f.debug_struct("Channels").field("path", path).finish(),
            Self::Unknown => f.write_str("Unknown"),
        }
    }
}

/// Разбирает `discord://`-ссылку.
///
/// Порядок проверок: пустота → длина → control-символы → схема → `//` → host →
/// query/fragment. `discord://app/settings/nitro` → host `app`, path `/settings/nitro`;
/// `discord:///library/42/launch` → host `""`, path `/library/42/launch`.
pub fn parse_deep_link(raw: &str) -> Result<DeepLink<'_>, DeepLinkError> {
    if raw.is_empty() {
        return Err(DeepLinkError::Empty);
    }
    if raw.len() > MAX_DEEP_LINK_LEN {
        return Err(DeepLinkError::TooLong);
    }
    if raw.bytes().any(|byte| byte < 0x20 || byte == 0x7F) {
        return Err(DeepLinkError::Malformed);
    }

    let colon = raw.find(':').ok_or(DeepLinkError::Malformed)?;
    let scheme = &raw[..colon];
    if scheme.is_empty() || scheme.bytes().any(|byte| byte.is_ascii_whitespace()) {
        return Err(DeepLinkError::Malformed);
    }
    if !scheme.eq_ignore_ascii_case("discord") {
        return Err(DeepLinkError::WrongScheme);
    }

    let rest = &raw[colon + 1..];
    let after_slashes = rest.strip_prefix("//").ok_or(DeepLinkError::Malformed)?;

    let host_end = after_slashes
        .find(['/', '?', '#'])
        .unwrap_or(after_slashes.len());
    let host = &after_slashes[..host_end];
    if host.bytes().any(|byte| byte.is_ascii_whitespace()) {
        return Err(DeepLinkError::Malformed);
    }

    let remainder = &after_slashes[host_end..];
    let (before_fragment, fragment) = match remainder.find('#') {
        Some(index) => (&remainder[..index], non_empty(&remainder[index + 1..])),
        None => (remainder, None),
    };
    let (path, query) = match before_fragment.find('?') {
        Some(index) => (
            &before_fragment[..index],
            non_empty(&before_fragment[index + 1..]),
        ),
        None => (before_fragment, None),
    };

    Ok(DeepLink {
        raw,
        host,
        path,
        query,
        fragment,
    })
}

/// Повторяет RE-семантику `extractUrlFromArgs` (`docs/re/main-arch.md` §9.4).
///
/// Если в args есть `--url`: требуется `--` с индексом не меньше индекса `--url`,
/// результат — аргумент сразу после первого `--` (содержимое не валидируется).
/// Иначе — первый аргумент с индекса 1, начинающийся (ASCII-регистронезависимо)
/// с `discord://`.
pub fn extract_deep_link(args: &[String]) -> Option<&str> {
    if let Some(url_index) = args.iter().position(|arg| arg == "--url") {
        let pass_through_index = args.iter().position(|arg| arg == "--")?;
        if pass_through_index < url_index {
            return None;
        }
        return args.get(pass_through_index + 1).map(String::as_str);
    }

    args.iter()
        .skip(1)
        .find(|arg| starts_with_discord_scheme(arg))
        .map(String::as_str)
}

/// Строит маршрут по разобранной ссылке.
///
/// Host сравнивается ASCII-регистронезависимо (host в URL регистронезависим),
/// путь — с учётом регистра (RE-формы в нижнем регистре).
pub fn route(link: &DeepLink<'_>) -> Route {
    if host_is(link, "login") && link.path == "/one-time" {
        return Route::LoginOneTime {
            token: query_param(link.query, "token").map(str::to_owned),
        };
    }
    if host_is(link, "action") && link.path == "/oauth2/authorize" {
        return Route::OAuth2Authorize;
    }
    if host_is(link, "app") {
        if link.path == "/open" {
            return Route::AppOpen;
        }
        if let Some(section) = app_settings_section(link.path) {
            return Route::AppSettings {
                section: section.to_owned(),
            };
        }
    }
    if let Some(id) = library_id(link.path).filter(|_| link.host.is_empty()) {
        return Route::Library { id: id.to_owned() };
    }
    if let Some(path) = channels_path(link) {
        return Route::Channels { path };
    }
    Route::Unknown
}

fn non_empty(value: &str) -> Option<&str> {
    if value.is_empty() { None } else { Some(value) }
}

fn starts_with_discord_scheme(arg: &str) -> bool {
    arg.get(..10)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("discord://"))
}

fn host_is(link: &DeepLink<'_>, expected: &str) -> bool {
    link.host.eq_ignore_ascii_case(expected)
}

fn host_is_dash_or_empty(link: &DeepLink<'_>) -> bool {
    link.host.is_empty() || link.host == "-"
}

fn app_settings_section(path: &str) -> Option<&str> {
    let rest = path.strip_prefix("/settings/")?;
    let section = rest.strip_suffix('/').unwrap_or(rest);
    if section.is_empty() || section.contains('/') {
        return None;
    }
    Some(section)
}

fn library_id(path: &str) -> Option<&str> {
    let rest = path.strip_prefix("/library/")?;
    let (id, tail) = rest.split_once('/')?;
    if id.is_empty() || tail != "launch" {
        return None;
    }
    Some(id)
}

fn channels_path(link: &DeepLink<'_>) -> Option<String> {
    if host_is(link, "channels") {
        let rest = link.path.strip_prefix('/')?;
        if rest.is_empty() {
            return None;
        }
        return Some(format!("/channels/{rest}"));
    }
    if host_is_dash_or_empty(link) {
        let rest = link.path.strip_prefix("/channels/")?;
        if rest.is_empty() {
            return None;
        }
        return Some(link.path.to_owned());
    }
    None
}

fn query_param<'a>(query: Option<&'a str>, key: &str) -> Option<&'a str> {
    let query = query?;
    for pair in query.split('&') {
        let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
        if name == key && !value.is_empty() {
            return Some(value);
        }
    }
    None
}
