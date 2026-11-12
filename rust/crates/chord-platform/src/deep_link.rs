//! P6-04 / BE-P2-13: разбор `discord://` и `disclip://` deep-link и маршрутизация.
//!
//! Оракул: `docs/re/main-arch.md` §9.4 (цитата `extractUrlFromArgs`), строки бандлов
//! `discord://login/one-time?token=…`, `discord://action/oauth2/authorize`,
//! `discord://app/open#…`, `discord://app/settings/nitro`, `discord:///library/<id>/launch`,
//! навигация `discord://[-]/channels/...`, `discord://guilds/...`,
//! а также привилегированный протокол клипов `disclip://` (`RE/main-arch.json` §9, §10,
//! `DISCORD_CLIP_PROTOCOL = 'disclip'`).
//!
//! Модель ничего не открывает и не ходит в сеть: это чистый разбор входа и очередь
//! ссылок до готовности окна (`openOrQueueUrl`).
//!
//! Безопасность: `DeepLink::raw`/`query` содержат исходные данные (включая `token`),
//! поэтому в логи должен попадать только [`Route`] — его `Debug` скрывает токен.
//! Токен в [`Route::LoginOneTime`] — сырой срез query (без percent-декодирования),
//! чтобы потребитель сам решил, когда и как его декодировать.

use std::collections::VecDeque;
use std::fmt;
use std::path::{Path, PathBuf};

/// Максимальная длина deep-link в байтах (включительно).
pub const MAX_DEEP_LINK_LEN: usize = 4096;

/// Максимальная длина кода инвайта или подарочного кода (`1..=64`).
pub const MAX_INVITE_CODE_LEN: usize = 64;

/// Префикс стандартного протокола приложения Discord.
pub const DISCORD_APP_PROTOCOL: &str = "discord";

/// Префикс привилегированного протокола клипов (`RE/main-arch.json` §9, §10).
pub const DISCORD_CLIP_PROTOCOL: &str = "disclip";

/// Емкость очереди deep links до готовности окна по умолчанию.
pub const DEFAULT_QUEUE_CAPACITY: usize = 32;

/// Hex-представление `DISCORD_UUID` для верификации MP4-клипов в `disclip://`
/// (`/home/sanya/re-deep/main-arch-work/m-core-7635.txt` L145).
pub const DISCORD_CLIP_UUID_HEX: &str = "a1c8529933464db888f083f57a75a5ef";

/// 16-байтовый `DISCORD_UUID` бокса `uuid` в MP4-файлах клипов Discord.
pub const DISCORD_CLIP_UUID_BYTES: [u8; 16] = [
    0xa1, 0xc8, 0x52, 0x99, 0x33, 0x46, 0x4d, 0xb8, 0x88, 0xf0, 0x83, 0xf5, 0x7a, 0x75, 0xa5, 0xef,
];

/// Схемы URL, заблокированные при открытии во внешнем браузере
/// (`/home/sanya/re-deep/asar-extract/core/bundle.js` module `#1331` `BLOCKED_URL_PROTOCOLS`).
pub const BLOCKED_URL_PROTOCOLS: &[&str] = &[
    "file:",
    "javascript:",
    "vbscript:",
    "data:",
    "about:",
    "chrome:",
    "ms-cxh:",
    "ms-cxh-full:",
    "ms-word:",
    "shell:",
    "ms-msdt:",
    "search-ms:",
    "ms-officecmd:",
    "ms-appinstaller:",
];

/// Ошибки разбора deep-link.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeepLinkError {
    /// Пустая строка.
    Empty,
    /// Схема не `discord` и не `disclip` (сравнение ASCII-регистронезависимое).
    WrongScheme,
    /// Длина больше [`MAX_DEEP_LINK_LEN`].
    TooLong,
    /// Синтаксис нарушен: нет `//` (для схем, требующих authority), пустая схема,
    /// пробелы в scheme/host, control-символы (`< 0x20` или `0x7F`) в исходной строке.
    Malformed,
}

/// Разобранный deep-link; все срезы заимствованы из исходной строки.
///
/// `Debug` намеренно не выводит `raw`: исходная строка и query могут содержать
/// `token` (`discord://login/one-time?token=…`). В диагностике видны `scheme`/`host`/`path`,
/// а `query`/`fragment` печатаются как `<redacted>`.
#[derive(PartialEq, Eq)]
pub struct DeepLink<'a> {
    /// Исходная строка целиком.
    pub raw: &'a str,
    /// Схема протокола (`discord` или `disclip`).
    pub scheme: &'a str,
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
    /// `discord://login/handoff` (`BVt.LOGIN_HANDOFF`).
    LoginHandoff,
    /// `discord://action/oauth2/authorize`.
    OAuth2Authorize,
    /// `discord://connect/authorize` (`BVt.CONNECT_AUTHORIZE`).
    ConnectAuthorize,
    /// `discord://app/open`.
    AppOpen,
    /// `discord://app/settings/<section>` или `discord://settings/<section>`.
    AppSettings { section: String },
    /// `discord:///library/<id>/launch`.
    Library { id: String },
    /// `discord://[-]/channels/...`; `path` — канонический путь `/channels/...`.
    Channels { path: String },
    /// `discord://[-]/guilds/<id>[/<channel_id>]` или `discord://guilds/...`.
    Guild {
        id: String,
        channel_id: Option<String>,
    },
    /// `discord://[-]/invite/<code>` или `discord://invite/<code>` (`BVt.INVITE`).
    Invite { code: String },
    /// `discord://[-]/gifts/<code>` или `discord://app/gifts/<code>` (`BVt.GIFT_CODE`).
    GiftCode { code: String },
    /// `discord://[-]/users/<user_id>` (`BVt.USERS`).
    UserProfile { user_id: String },
    /// `discord://[-]/quests/<quest_id>` (`BVt.QUESTS`).
    Quest { quest_id: String },
    /// `disclip://<id>`, `disclip:<id>`, `discord://clips/<id>` (`RE/main-arch.json` §9, §10).
    Clip { id: String },
    /// Форма не подтверждена RE.
    Unknown,
}

impl fmt::Debug for DeepLink<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DeepLink")
            .field("scheme", &self.scheme)
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
            Self::LoginHandoff => f.write_str("LoginHandoff"),
            Self::OAuth2Authorize => f.write_str("OAuth2Authorize"),
            Self::ConnectAuthorize => f.write_str("ConnectAuthorize"),
            Self::AppOpen => f.write_str("AppOpen"),
            Self::AppSettings { section } => f
                .debug_struct("AppSettings")
                .field("section", section)
                .finish(),
            Self::Library { id } => f.debug_struct("Library").field("id", id).finish(),
            Self::Channels { path } => f.debug_struct("Channels").field("path", path).finish(),
            Self::Guild { id, channel_id } => f
                .debug_struct("Guild")
                .field("id", id)
                .field("channel_id", channel_id)
                .finish(),
            Self::Invite { code } => f.debug_struct("Invite").field("code", code).finish(),
            Self::GiftCode { code } => f.debug_struct("GiftCode").field("code", code).finish(),
            Self::UserProfile { user_id } => f
                .debug_struct("UserProfile")
                .field("user_id", user_id)
                .finish(),
            Self::Quest { quest_id } => {
                f.debug_struct("Quest").field("quest_id", quest_id).finish()
            }
            Self::Clip { id } => f.debug_struct("Clip").field("id", id).finish(),
            Self::Unknown => f.write_str("Unknown"),
        }
    }
}

/// Разбирает `discord://` или `disclip://` ссылку.
///
/// Порядок проверок: пустота → длина → control-символы → схема (`discord` / `disclip`)
/// → `//` / authority → host → query/fragment.
/// Для `discord:` схема без `//` требует ведущего `/` (иначе `Malformed`, контракт P6-04).
/// Для `disclip:` поддерживаются формы `disclip://<id>`, `disclip:///path`, `disclip:<id>`.
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
    let is_discord = scheme.eq_ignore_ascii_case(DISCORD_APP_PROTOCOL);
    let is_disclip = scheme.eq_ignore_ascii_case(DISCORD_CLIP_PROTOCOL);
    if !is_discord && !is_disclip {
        return Err(DeepLinkError::WrongScheme);
    }

    let rest = &raw[colon + 1..];
    let (host, remainder) = if let Some(after_slashes) = rest.strip_prefix("//") {
        let host_end = after_slashes
            .find(['/', '?', '#'])
            .unwrap_or(after_slashes.len());
        let host = &after_slashes[..host_end];
        if host.bytes().any(|byte| byte.is_ascii_whitespace()) {
            return Err(DeepLinkError::Malformed);
        }
        (host, &after_slashes[host_end..])
    } else if is_discord {
        // Для discord: без //: если начинается с '/', считаем path (host = ""),
        // иначе (например, discord:app/open) — Malformed (P6-04 контракт).
        if rest.starts_with('/') {
            ("", rest)
        } else {
            return Err(DeepLinkError::Malformed);
        }
    } else {
        // Для disclip: (без //): например disclip:12345 или disclip:/12345
        if rest.is_empty() {
            return Err(DeepLinkError::Malformed);
        }
        if rest.starts_with('/') {
            ("", rest)
        } else {
            let host_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
            let host = &rest[..host_end];
            if host.is_empty() || host.bytes().any(|byte| byte.is_ascii_whitespace()) {
                return Err(DeepLinkError::Malformed);
            }
            (host, &rest[host_end..])
        }
    };

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
        scheme,
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

/// Извлекает deep link URL из аргументов командной строки по RE-оракулу Electron
/// (`RE/main-arch.json` §9: `extractUrlFromArgs`).
///
/// Поддерживает:
/// - аргумент `--url` перед `--` (классический Squirrel/Electron паттерн);
/// - аргумент `--url <val>` или `--url=<val>` (включая после `--`);
/// - прямой deep-link аргумент в `argv[1..]` (`discord://`, `disclip://`, `discord:`, `disclip:`).
pub fn extract_url_from_args(args: &[String]) -> Option<String> {
    if args.len() <= 1 {
        return None;
    }

    // 1. Electron classic: `--url` appearing before `--`
    let url_index = args.iter().position(|arg| arg == "--url");
    let pass_through_index = args.iter().position(|arg| arg == "--");
    if let (Some(u_idx), Some(p_idx)) = (url_index, pass_through_index)
        && p_idx > u_idx
    {
        return args.get(p_idx + 1).cloned();
    }

    // 2. Check for `--url=<val>` (anywhere in args[1..])
    for arg in args.iter().skip(1) {
        if let Some(val) = arg.strip_prefix("--url=")
            && !val.is_empty()
        {
            return Some(val.to_string());
        }
    }

    // 3. Check for `--url <val>` (anywhere in args[1..])
    for i in 1..args.len() {
        if args[i] == "--url"
            && let Some(next_arg) = args.get(i + 1)
            && next_arg != "--"
            && !next_arg.starts_with("--")
        {
            return Some(next_arg.clone());
        }
    }

    // 4. Direct deep link argument in args[1..]
    for arg in args.iter().skip(1) {
        if is_direct_deep_link(arg) {
            return Some(arg.clone());
        }
    }

    None
}

/// Строит маршрут по разобранной ссылке.
///
/// Host сравнивается ASCII-регистронезависимо (host в URL регистронезависим),
/// путь — с учётом регистра (RE-формы в нижнем регистре).
pub fn route(link: &DeepLink<'_>) -> Route {
    if link.scheme.eq_ignore_ascii_case(DISCORD_CLIP_PROTOCOL) {
        return match clip_id_from_disclip(link) {
            Some(id) => Route::Clip { id },
            None => Route::Unknown,
        };
    }
    if let Some(id) = clip_id_from_discord(link) {
        return Route::Clip { id };
    }

    if host_is(link, "login") {
        if link.path == "/one-time" {
            return Route::LoginOneTime {
                token: query_param(link.query, "token").map(str::to_owned),
            };
        }
        if link.path == "/handoff" {
            return Route::LoginHandoff;
        }
    }
    if host_is_dash_or_empty(link) && link.path == "/login/handoff" {
        return Route::LoginHandoff;
    }
    if host_is(link, "action") && link.path == "/oauth2/authorize" {
        return Route::OAuth2Authorize;
    }
    if (host_is(link, "connect") && link.path == "/authorize")
        || (host_is_dash_or_empty(link) && link.path == "/connect/authorize")
    {
        return Route::ConnectAuthorize;
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
        if let Some(code) = prefixed_code_segment(link.path, "/gifts/") {
            return Route::GiftCode {
                code: code.to_owned(),
            };
        }
    }
    if host_is(link, "settings") {
        return bare_settings_section(link.path).map_or(Route::Unknown, |section| {
            Route::AppSettings {
                section: section.to_owned(),
            }
        });
    }
    if let Some(id) = library_id(link.path).filter(|_| link.host.is_empty()) {
        return Route::Library { id: id.to_owned() };
    }
    if let Some(path) = channels_path(link) {
        return Route::Channels { path };
    }
    if let Some((guild_id, channel_id)) = guild_route(link) {
        return Route::Guild {
            id: guild_id,
            channel_id,
        };
    }
    if let Some(code) = invite_route(link) {
        return Route::Invite { code };
    }
    if let Some(code) = gift_route(link) {
        return Route::GiftCode { code };
    }
    if let Some(user_id) = user_profile_route(link) {
        return Route::UserProfile { user_id };
    }
    if let Some(quest_id) = quest_route(link) {
        return Route::Quest { quest_id };
    }

    Route::Unknown
}

fn is_safe_segment(segment: &str) -> bool {
    if segment.is_empty()
        || segment == "."
        || segment == ".."
        || segment.contains('/')
        || segment.contains('\\')
    {
        return false;
    }
    let bytes = segment.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            if i + 2 >= bytes.len() {
                return false;
            }
            let Some(hi) = hex_nibble(bytes[i + 1]) else {
                return false;
            };
            let Some(lo) = hex_nibble(bytes[i + 2]) else {
                return false;
            };
            let decoded = (hi << 4) | lo;
            if decoded < 0x20
                || decoded == 0x7F
                || decoded == b'/'
                || decoded == b'\\'
                || decoded == b'.'
            {
                return false;
            }
            i += 3;
        } else {
            i += 1;
        }
    }
    true
}

fn hex_nibble(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

fn is_valid_code_segment(segment: &str) -> bool {
    !segment.is_empty()
        && segment.len() <= MAX_INVITE_CODE_LEN
        && is_safe_segment(segment)
        && segment
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

fn parse_snowflake(s: &str) -> Option<u64> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let id = s.parse::<u64>().ok()?;
    if id == 0 {
        return None;
    }
    Some(id)
}

fn clip_id_from_disclip(link: &DeepLink<'_>) -> Option<String> {
    if !link.host.is_empty() && !link.host.eq_ignore_ascii_case("clips") {
        if !link.path.is_empty() && link.path != "/" {
            return None;
        }
        if !is_safe_segment(link.host) {
            return None;
        }
        return Some(link.host.to_owned());
    }
    let rest = link.path.strip_prefix('/')?;
    let id = rest.strip_prefix("clips/").unwrap_or(rest);
    let id = id.strip_suffix('/').unwrap_or(id);
    if !is_safe_segment(id) {
        return None;
    }
    Some(id.to_owned())
}

fn clip_id_from_discord(link: &DeepLink<'_>) -> Option<String> {
    if host_is(link, "clips") {
        let rest = link.path.strip_prefix('/')?;
        let id = rest.strip_suffix('/').unwrap_or(rest);
        if !is_safe_segment(id) {
            return None;
        }
        return Some(id.to_owned());
    }
    if host_is_dash_or_empty(link) {
        let rest = link.path.strip_prefix("/clips/")?;
        let id = rest.strip_suffix('/').unwrap_or(rest);
        if !is_safe_segment(id) {
            return None;
        }
        return Some(id.to_owned());
    }
    None
}

fn bare_settings_section(path: &str) -> Option<&str> {
    let rest = path.strip_prefix('/')?;
    let section = rest.strip_suffix('/').unwrap_or(rest);
    if !is_safe_segment(section) {
        return None;
    }
    Some(section)
}

fn guild_route(link: &DeepLink<'_>) -> Option<(String, Option<String>)> {
    if host_is(link, "guilds") {
        let rest = link.path.strip_prefix('/')?;
        return parse_guild_path_segments(rest);
    }
    if host_is_dash_or_empty(link) {
        let rest = link.path.strip_prefix("/guilds/")?;
        return parse_guild_path_segments(rest);
    }
    None
}

fn parse_guild_path_segments(path: &str) -> Option<(String, Option<String>)> {
    let clean = path.strip_suffix('/').unwrap_or(path);
    if clean.is_empty() {
        return None;
    }
    if let Some((guild, chan)) = clean.split_once('/') {
        if !is_safe_segment(guild) || !is_safe_segment(chan) {
            return None;
        }
        Some((guild.to_owned(), Some(chan.to_owned())))
    } else {
        if !is_safe_segment(clean) {
            return None;
        }
        Some((clean.to_owned(), None))
    }
}

fn invite_route(link: &DeepLink<'_>) -> Option<String> {
    if host_is(link, "invite") || host_is(link, "invites") {
        let rest = link.path.strip_prefix('/')?;
        let code = rest.strip_suffix('/').unwrap_or(rest);
        if is_valid_code_segment(code) {
            return Some(code.to_owned());
        }
        return None;
    }
    if host_is_dash_or_empty(link)
        && let Some(code) = prefixed_code_segment(link.path, "/invite/")
            .or_else(|| prefixed_code_segment(link.path, "/invites/"))
    {
        return Some(code.to_owned());
    }
    None
}

fn gift_route(link: &DeepLink<'_>) -> Option<String> {
    if host_is(link, "gifts") || host_is(link, "gift") {
        let rest = link.path.strip_prefix('/')?;
        let code = rest.strip_suffix('/').unwrap_or(rest);
        if is_valid_code_segment(code) {
            return Some(code.to_owned());
        }
        return None;
    }
    if host_is_dash_or_empty(link)
        && let Some(code) = prefixed_code_segment(link.path, "/gifts/")
            .or_else(|| prefixed_code_segment(link.path, "/gift/"))
    {
        return Some(code.to_owned());
    }
    None
}

fn prefixed_code_segment<'a>(path: &'a str, prefix: &str) -> Option<&'a str> {
    let rest = path.strip_prefix(prefix)?;
    let code = rest.strip_suffix('/').unwrap_or(rest);
    if is_valid_code_segment(code) {
        Some(code)
    } else {
        None
    }
}

fn user_profile_route(link: &DeepLink<'_>) -> Option<String> {
    let raw_id = if host_is(link, "users") {
        let rest = link.path.strip_prefix('/')?;
        rest.strip_suffix('/').unwrap_or(rest)
    } else if host_is_dash_or_empty(link) {
        let rest = link.path.strip_prefix("/users/")?;
        rest.strip_suffix('/').unwrap_or(rest)
    } else {
        return None;
    };
    if raw_id == "@me" || parse_snowflake(raw_id).is_some() {
        Some(raw_id.to_owned())
    } else {
        None
    }
}

fn quest_route(link: &DeepLink<'_>) -> Option<String> {
    let raw_id = if host_is(link, "quests") {
        let rest = link.path.strip_prefix('/')?;
        rest.strip_suffix('/').unwrap_or(rest)
    } else if host_is_dash_or_empty(link) {
        let rest = link.path.strip_prefix("/quests/")?;
        rest.strip_suffix('/').unwrap_or(rest)
    } else {
        return None;
    };
    if is_safe_segment(raw_id) {
        Some(raw_id.to_owned())
    } else {
        None
    }
}

fn non_empty(value: &str) -> Option<&str> {
    if value.is_empty() { None } else { Some(value) }
}

fn starts_with_discord_scheme(arg: &str) -> bool {
    arg.get(..10)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("discord://"))
}

fn is_direct_deep_link(arg: &str) -> bool {
    let lower = arg.to_ascii_lowercase();
    lower.starts_with("discord://")
        || lower.starts_with("disclip://")
        || lower.starts_with("discord:")
        || lower.starts_with("disclip:")
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
    if !is_safe_segment(section) {
        return None;
    }
    Some(section)
}

fn library_id(path: &str) -> Option<&str> {
    let rest = path.strip_prefix("/library/")?;
    let (id, tail) = rest.split_once('/')?;
    if !is_safe_segment(id) || tail != "launch" {
        return None;
    }
    Some(id)
}

fn channels_path(link: &DeepLink<'_>) -> Option<String> {
    let rest = if host_is(link, "channels") {
        link.path.strip_prefix('/')?
    } else if host_is_dash_or_empty(link) {
        link.path.strip_prefix("/channels/")?
    } else {
        return None;
    };
    let clean = rest.strip_suffix('/').unwrap_or(rest);
    if clean.is_empty() || !clean.split('/').all(is_safe_segment) {
        return None;
    }
    Some(format!("/channels/{rest}"))
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

/// Канонический путь для передачи в окно рендерера (`MAIN_WINDOW_PATH`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MainWindowPath {
    pub path: String,
    pub query: Option<String>,
}

impl MainWindowPath {
    pub fn new(path: impl Into<String>, query: Option<impl Into<String>>) -> Self {
        Self {
            path: path.into(),
            query: query.map(Into::into),
        }
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn query(&self) -> Option<&str> {
        self.query.as_deref()
    }
}

impl fmt::Display for MainWindowPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.path)?;
        if let Some(ref q) = self.query {
            write!(f, "?{}", q)?;
        }
        Ok(())
    }
}

/// Ошибки вычисления канонического `MAIN_WINDOW_PATH`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MainWindowPathError {
    /// Некорректный синтаксис URL или недопустимая схема.
    InvalidUrl(DeepLinkError),
    /// Неизвестная или неподтверждённая форма маршрута.
    UnknownRoute(String),
}

impl fmt::Display for MainWindowPathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidUrl(err) => write!(f, "недопустимый deep link URL: {err:?}"),
            Self::UnknownRoute(raw) => write!(f, "неизвестный маршрут deep link: {raw}"),
        }
    }
}

impl std::error::Error for MainWindowPathError {}

/// Преобразует URL deep link в канонический путь `MAIN_WINDOW_PATH`
/// (`/channels/...`, `/settings/...`, `/clips/...`) в соответствии с RE-оракулом
/// (`RE/main-arch.json` §9: `webContentsSend("MAIN_WINDOW_PATH", path, query)`).
pub fn main_window_path(url: &str) -> Result<MainWindowPath, MainWindowPathError> {
    let link = parse_deep_link(url).map_err(MainWindowPathError::InvalidUrl)?;
    let r = route(&link);
    let query = link.query.map(str::to_string);
    match r {
        Route::Channels { path } => Ok(MainWindowPath { path, query }),
        Route::Guild {
            id,
            channel_id: Some(ch),
        } => Ok(MainWindowPath {
            path: format!("/channels/{id}/{ch}"),
            query,
        }),
        Route::Guild {
            id,
            channel_id: None,
        } => Ok(MainWindowPath {
            path: format!("/channels/{id}"),
            query,
        }),
        Route::AppSettings { section } => Ok(MainWindowPath {
            path: format!("/settings/{section}"),
            query,
        }),
        Route::Clip { id } => Ok(MainWindowPath {
            path: format!("/clips/{id}"),
            query,
        }),
        Route::LoginOneTime { .. } => Ok(MainWindowPath {
            path: "/one-time".to_string(),
            query,
        }),
        Route::OAuth2Authorize => Ok(MainWindowPath {
            path: "/oauth2/authorize".to_string(),
            query,
        }),
        Route::AppOpen => Ok(MainWindowPath {
            path: "/app/open".to_string(),
            query,
        }),
        Route::Library { id } => Ok(MainWindowPath {
            path: format!("/library/{id}/launch"),
            query,
        }),
        Route::LoginHandoff => Ok(MainWindowPath {
            path: "/login/handoff".to_string(),
            query,
        }),
        Route::ConnectAuthorize => Ok(MainWindowPath {
            path: "/connect/authorize".to_string(),
            query,
        }),
        Route::Invite { code } => Ok(MainWindowPath {
            path: format!("/invite/{code}"),
            query,
        }),
        Route::GiftCode { code } => Ok(MainWindowPath {
            path: format!("/gifts/{code}"),
            query,
        }),
        Route::UserProfile { user_id } => Ok(MainWindowPath {
            path: format!("/users/{user_id}"),
            query,
        }),
        Route::Quest { quest_id } => Ok(MainWindowPath {
            path: format!("/quests/{quest_id}"),
            query,
        }),
        Route::Unknown => Err(MainWindowPathError::UnknownRoute(url.to_string())),
    }
}

/// Структурированная цель навигации внутри `/channels/...` с валидацией границ Snowflake (`u64`, `1..=u64::MAX`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChannelTarget {
    /// Список личных сообщений (`/channels/@me`).
    DirectMessagesHome,
    /// Личный или групповой канал (`/channels/@me/<channel_id>[/<message_id>]`).
    DirectMessage {
        channel_id: u64,
        message_id: Option<u64>,
    },
    /// Канал гильдии (`/channels/<guild_id>[/<channel_id>[/<message_id>]]`).
    GuildChannel {
        guild_id: u64,
        channel_id: Option<u64>,
        message_id: Option<u64>,
    },
    /// Ветка (thread) внутри канала гильдии (`/channels/<guild_id>/<channel_id>/threads/<thread_id>[/<message_id>]`).
    GuildThread {
        guild_id: u64,
        channel_id: u64,
        thread_id: u64,
        message_id: Option<u64>,
    },
}

fn parse_nonzero_snowflake(s: &str) -> Option<u64> {
    parse_snowflake(s)
}

fn is_valid_scheme(scheme: &str) -> bool {
    let mut bytes = scheme.bytes();
    let Some(first) = bytes.next() else {
        return false;
    };
    first.is_ascii_alphabetic()
        && bytes.all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'-' | b'.'))
}

/// Разбирает канонический путь `/channels/...` в типизированный [`ChannelTarget`]
/// с проверкой границ `Snowflake` (`1..=u64::MAX`).
#[must_use]
pub fn parse_channel_target(path: &str) -> Option<ChannelTarget> {
    let rest = path.strip_prefix("/channels/")?;
    if rest.is_empty() || rest.ends_with('/') {
        return None;
    }
    let parts: Vec<&str> = rest.split('/').collect();
    if parts.iter().any(|seg| !is_safe_segment(seg)) {
        return None;
    }

    match parts.as_slice() {
        ["@me"] => Some(ChannelTarget::DirectMessagesHome),
        ["@me", ch] => Some(ChannelTarget::DirectMessage {
            channel_id: parse_nonzero_snowflake(ch)?,
            message_id: None,
        }),
        ["@me", ch, msg] => Some(ChannelTarget::DirectMessage {
            channel_id: parse_nonzero_snowflake(ch)?,
            message_id: Some(parse_nonzero_snowflake(msg)?),
        }),
        [guild] => Some(ChannelTarget::GuildChannel {
            guild_id: parse_nonzero_snowflake(guild)?,
            channel_id: None,
            message_id: None,
        }),
        [guild, ch] => Some(ChannelTarget::GuildChannel {
            guild_id: parse_nonzero_snowflake(guild)?,
            channel_id: Some(parse_nonzero_snowflake(ch)?),
            message_id: None,
        }),
        [guild, ch, msg] => Some(ChannelTarget::GuildChannel {
            guild_id: parse_nonzero_snowflake(guild)?,
            channel_id: Some(parse_nonzero_snowflake(ch)?),
            message_id: Some(parse_nonzero_snowflake(msg)?),
        }),
        [guild, ch, "threads", thread] => Some(ChannelTarget::GuildThread {
            guild_id: parse_nonzero_snowflake(guild)?,
            channel_id: parse_nonzero_snowflake(ch)?,
            thread_id: parse_nonzero_snowflake(thread)?,
            message_id: None,
        }),
        [guild, ch, "threads", thread, msg] => Some(ChannelTarget::GuildThread {
            guild_id: parse_nonzero_snowflake(guild)?,
            channel_id: parse_nonzero_snowflake(ch)?,
            thread_id: parse_nonzero_snowflake(thread)?,
            message_id: Some(parse_nonzero_snowflake(msg)?),
        }),
        _ => None,
    }
}

/// Валидированные параметры запроса `/oauth2/authorize`
/// (`/home/sanya/re-deep/decoded/4d2a64cb5df5fc4e_0`: `Kk["/oauth2/authorize"]`).
///
/// Чувствительные поля `state` и `nonce` маскируются в `Debug` (`<redacted>`).
#[derive(Clone, PartialEq, Eq)]
pub struct OAuth2AuthorizeParams {
    pub client_id: u64,
    pub scopes: Vec<String>,
    pub redirect_uri: String,
    pub response_type: String,
    pub state: Option<String>,
    pub nonce: Option<String>,
    pub code_challenge: Option<String>,
    pub code_challenge_method: Option<String>,
}

impl fmt::Debug for OAuth2AuthorizeParams {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OAuth2AuthorizeParams")
            .field("client_id", &self.client_id)
            .field("scopes", &self.scopes)
            .field("redirect_uri", &self.redirect_uri)
            .field("response_type", &self.response_type)
            .field("state", &self.state.as_ref().map(|_| "<redacted>"))
            .field("nonce", &self.nonce.as_ref().map(|_| "<redacted>"))
            .field("code_challenge", &self.code_challenge)
            .field("code_challenge_method", &self.code_challenge_method)
            .finish()
    }
}

/// Ошибки валидации параметров `/oauth2/authorize`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OAuth2AuthorizeError {
    MissingQuery,
    InvalidClientId,
    MissingScope,
    MissingRedirectUri,
    UnsafeRedirectUri,
    MissingResponseType,
    InvalidPercentEncoding,
}

impl fmt::Display for OAuth2AuthorizeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingQuery => write!(f, "отсутствует query-строка OAuth2"),
            Self::InvalidClientId => write!(f, "некорректный client_id OAuth2"),
            Self::MissingScope => write!(f, "отсутствует параметр scope OAuth2"),
            Self::MissingRedirectUri => write!(f, "отсутствует redirect_uri OAuth2"),
            Self::UnsafeRedirectUri => write!(f, "небезопасный redirect_uri OAuth2"),
            Self::MissingResponseType => write!(f, "отсутствует response_type OAuth2"),
            Self::InvalidPercentEncoding => write!(f, "некорректное percent-кодирование в query"),
        }
    }
}

impl std::error::Error for OAuth2AuthorizeError {}

fn percent_decode_form(input: &str) -> Result<String, OAuth2AuthorizeError> {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b'%' => {
                if i + 2 >= bytes.len() {
                    return Err(OAuth2AuthorizeError::InvalidPercentEncoding);
                }
                let hi = (bytes[i + 1] as char)
                    .to_digit(16)
                    .ok_or(OAuth2AuthorizeError::InvalidPercentEncoding)?;
                let lo = (bytes[i + 2] as char)
                    .to_digit(16)
                    .ok_or(OAuth2AuthorizeError::InvalidPercentEncoding)?;
                let byte = ((hi << 4) | lo) as u8;
                if byte < 0x20 || byte == 0x7F {
                    return Err(OAuth2AuthorizeError::InvalidPercentEncoding);
                }
                out.push(byte);
                i += 3;
            }
            b if b < 0x20 || b == 0x7F => {
                return Err(OAuth2AuthorizeError::InvalidPercentEncoding);
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8(out).map_err(|_| OAuth2AuthorizeError::InvalidPercentEncoding)
}

fn is_safe_oauth2_redirect_uri(uri: &str) -> bool {
    if uri.is_empty()
        || uri.len() > MAX_DEEP_LINK_LEN
        || uri
            .bytes()
            .any(|b| b < 0x20 || b == 0x7F || b.is_ascii_whitespace() || b == b'\\')
    {
        return false;
    }
    let Some(colon) = uri.find(':') else {
        return false;
    };
    let scheme = &uri[..colon];
    let rest = &uri[colon + 1..];
    if !is_valid_scheme(scheme) || scheme.len() < 2 {
        return false;
    }
    let scheme_colon = format!("{}:", scheme.to_ascii_lowercase());
    if BLOCKED_URL_PROTOCOLS.contains(&scheme_colon.as_str()) || scheme.eq_ignore_ascii_case("blob")
    {
        return false;
    }

    if scheme.eq_ignore_ascii_case("https") {
        let Some(after_slashes) = rest.strip_prefix("//") else {
            return false;
        };
        let host_end = after_slashes
            .find(['/', '?', '#'])
            .unwrap_or(after_slashes.len());
        let authority = &after_slashes[..host_end];
        return !authority.is_empty()
            && !authority.contains('@')
            && authority.contains('.')
            && !authority.starts_with('.')
            && !authority.ends_with('.');
    }

    if scheme.eq_ignore_ascii_case("http") {
        let Some(after_slashes) = rest.strip_prefix("//") else {
            return false;
        };
        let host_end = after_slashes
            .find(['/', '?', '#'])
            .unwrap_or(after_slashes.len());
        let authority = &after_slashes[..host_end];
        let host = authority.rsplit_once(':').map_or(authority, |(h, _)| h);
        return host.eq_ignore_ascii_case("localhost") || host == "127.0.0.1";
    }

    if scheme.eq_ignore_ascii_case("discord") {
        return parse_deep_link(uri).is_ok();
    }

    false
}

/// Разбирает и валидирует query-параметры `/oauth2/authorize` (`client_id`, `scope`, `redirect_uri`,
/// `response_type`, `state`, `nonce`, `code_challenge`, `code_challenge_method`).
pub fn parse_oauth2_authorize_params(
    query: Option<&str>,
) -> Result<OAuth2AuthorizeParams, OAuth2AuthorizeError> {
    let raw_query = query
        .filter(|q| !q.is_empty())
        .ok_or(OAuth2AuthorizeError::MissingQuery)?;

    let mut client_id_raw = None;
    let mut scope_raw = None;
    let mut redirect_uri_raw = None;
    let mut response_type_raw = None;
    let mut state = None;
    let mut nonce = None;
    let mut code_challenge = None;
    let mut code_challenge_method = None;

    for pair in raw_query.split('&') {
        if pair.is_empty() {
            continue;
        }
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        let decoded_val = percent_decode_form(v)?;
        match k {
            "client_id" => client_id_raw = Some(decoded_val),
            "scope" => scope_raw = Some(decoded_val),
            "redirect_uri" => redirect_uri_raw = Some(decoded_val),
            "response_type" => response_type_raw = Some(decoded_val),
            "state" if !decoded_val.is_empty() => state = Some(decoded_val),
            "nonce" if !decoded_val.is_empty() => nonce = Some(decoded_val),
            "code_challenge" if !decoded_val.is_empty() => code_challenge = Some(decoded_val),
            "code_challenge_method" if !decoded_val.is_empty() => {
                code_challenge_method = Some(decoded_val);
            }
            _ => {}
        }
    }

    let client_id_str = client_id_raw.ok_or(OAuth2AuthorizeError::InvalidClientId)?;
    let client_id =
        parse_nonzero_snowflake(&client_id_str).ok_or(OAuth2AuthorizeError::InvalidClientId)?;

    let scope_str = scope_raw.ok_or(OAuth2AuthorizeError::MissingScope)?;
    let scopes: Vec<String> = scope_str
        .split_whitespace()
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect();
    if scopes.is_empty() {
        return Err(OAuth2AuthorizeError::MissingScope);
    }

    let redirect_uri = redirect_uri_raw
        .filter(|u| !u.is_empty())
        .ok_or(OAuth2AuthorizeError::MissingRedirectUri)?;
    if !is_safe_oauth2_redirect_uri(&redirect_uri) {
        return Err(OAuth2AuthorizeError::UnsafeRedirectUri);
    }

    let response_type = response_type_raw
        .filter(|r| !r.is_empty())
        .ok_or(OAuth2AuthorizeError::MissingResponseType)?;

    Ok(OAuth2AuthorizeParams {
        client_id,
        scopes,
        redirect_uri,
        response_type,
        state,
        nonce,
        code_challenge,
        code_challenge_method,
    })
}

/// Проверяет, безопасно ли открывать внешний URL в системном браузере/ОС
/// (`/home/sanya/re-deep/asar-extract/core/bundle.js` модуль `#1331`: `shouldOpenExternalUrl`).
///
/// Блокирует все схемы из [`BLOCKED_URL_PROTOCOLS`], однобуквенные схемы (пути дисков Windows `C:\...`),
/// управляющие символы, обратные слэши и учётные данные `user:pass@host`.
#[must_use]
pub fn should_open_external_url(external_url: &str) -> bool {
    if external_url.is_empty()
        || external_url.len() > MAX_DEEP_LINK_LEN
        || external_url
            .bytes()
            .any(|b| b < 0x20 || b == 0x7F || b.is_ascii_whitespace() || b == b'\\')
    {
        return false;
    }

    let Some(colon) = external_url.find(':') else {
        return false;
    };
    let scheme = &external_url[..colon];
    let rest = &external_url[colon + 1..];

    // Однобуквенная схема (например `c:/windows/system32/cmd.exe`) блокируется
    if scheme.len() < 2 || !is_valid_scheme(scheme) {
        return false;
    }

    let scheme_with_colon = format!("{}:", scheme.to_ascii_lowercase());
    if BLOCKED_URL_PROTOCOLS.contains(&scheme_with_colon.as_str())
        || scheme.eq_ignore_ascii_case("blob")
    {
        return false;
    }

    if scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https") {
        let Some(after_slashes) = rest.strip_prefix("//") else {
            return false;
        };
        let host_end = after_slashes
            .find(['/', '?', '#'])
            .unwrap_or(after_slashes.len());
        let authority = &after_slashes[..host_end];
        if authority.is_empty() || authority.contains('@') {
            return false;
        }
        let host = authority.rsplit_once(':').map_or(authority, |(h, _)| h);
        return !host.is_empty()
            && host.contains('.')
            && !host.starts_with('.')
            && !host.ends_with('.');
    }

    !rest.is_empty()
}

fn extract_normalized_origin(url: &str) -> Option<(String, String, u16)> {
    if url.is_empty()
        || url.len() > MAX_DEEP_LINK_LEN
        || url
            .bytes()
            .any(|b| b < 0x20 || b == 0x7F || b.is_ascii_whitespace())
    {
        return None;
    }
    let colon = url.find(':')?;
    let scheme = url[..colon].to_ascii_lowercase();
    let default_port = match scheme.as_str() {
        "https" => 443,
        "http" => 80,
        _ => return None,
    };
    let after_slashes = url[colon + 1..].strip_prefix("//")?;
    let host_end = after_slashes
        .find(['/', '?', '#'])
        .unwrap_or(after_slashes.len());
    let authority = &after_slashes[..host_end];
    if authority.is_empty() || authority.contains('@') {
        return None;
    }
    let (host, port) = match authority.rsplit_once(':') {
        Some((h, p)) => {
            if p.is_empty() || !p.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            let port_num = p.parse::<u16>().ok().filter(|&n| n > 0)?;
            (h.to_ascii_lowercase(), port_num)
        }
        None => (authority.to_ascii_lowercase(), default_port),
    };
    if host.is_empty() {
        return None;
    }
    Some((scheme, host, port))
}

/// Проверяет совпадение origin (`scheme + host + port`) двух URL без учёта регистра хоста/схемы
/// и со стандартными портами `80`/`443` (`/home/sanya/re-deep/asar-extract/core/bundle.js` `#1331`: `checkUrlOriginMatches`).
#[must_use]
pub fn check_url_origin_matches(url_a: &str, url_b: &str) -> bool {
    let Some(origin_a) = extract_normalized_origin(url_a) else {
        return false;
    };
    let Some(origin_b) = extract_normalized_origin(url_b) else {
        return false;
    };
    origin_a == origin_b
}

/// Спецификация привилегий кастомной схемы до готовности окна
/// (`/home/sanya/re-deep/main-arch-work/m-app-46312.txt`: `beforeReadyProtocolRegistration`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolPrivilegeSpec {
    pub scheme: &'static str,
    pub standard: bool,
    pub secure: bool,
    pub cors_enabled: bool,
    pub support_fetch_api: bool,
    pub stream: bool,
}

/// Возвращает список привилегированных протоколов, регистрируемых до `app.whenReady()`
/// (`disclip` со `standard`, `secure`, `corsEnabled`, `supportFetchAPI`, `stream`).
#[must_use]
pub fn before_ready_protocol_registration() -> Vec<ProtocolPrivilegeSpec> {
    vec![ProtocolPrivilegeSpec {
        scheme: "disclip",
        standard: true,
        secure: true,
        cors_enabled: true,
        support_fetch_api: true,
        stream: true,
    }]
}

/// Ошибки генерации системных манифестов регистрации протокола (`XDG .desktop` / реестр Windows).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtocolRegistrationError {
    EmptyArgument,
    UnsafeCharacters,
    InvalidProtocolName,
}

impl fmt::Display for ProtocolRegistrationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyArgument => write!(f, "пустой аргумент регистрации протокола"),
            Self::UnsafeCharacters => write!(f, "недопустимые символы в аргументе регистрации"),
            Self::InvalidProtocolName => write!(f, "некорректное имя протокола"),
        }
    }
}

impl std::error::Error for ProtocolRegistrationError {}

fn validate_registration_token(s: &str) -> Result<(), ProtocolRegistrationError> {
    if s.is_empty() {
        return Err(ProtocolRegistrationError::EmptyArgument);
    }
    if s.bytes()
        .any(|b| b < 0x20 || b == 0x7F || matches!(b, b'"' | b'\'' | b';' | b'`' | b'$' | b'|'))
    {
        return Err(ProtocolRegistrationError::UnsafeCharacters);
    }
    Ok(())
}

/// Формирует содержимое XDG `.desktop`-файла для регистрации обработчика `discord://` и `disclip://` на Linux.
pub fn desktop_protocol_handler_entry(
    app_name: &str,
    exec_path: &str,
) -> Result<String, ProtocolRegistrationError> {
    validate_registration_token(app_name)?;
    validate_registration_token(exec_path)?;
    Ok(format!(
        "[Desktop Entry]\nType=Application\nName={app_name}\nExec={exec_path} --url -- %u\nTerminal=false\nNoDisplay=true\nMimeType=x-scheme-handler/discord;x-scheme-handler/disclip;\n"
    ))
}

/// Возвращает пары `(mime_type, desktop_file)` для регистрации в `mimeapps.list` (`xdg-mime`).
pub fn mimeapps_scheme_entries(
    desktop_file: &str,
) -> Result<Vec<(String, String)>, ProtocolRegistrationError> {
    validate_registration_token(desktop_file)?;
    if !desktop_file.ends_with(".desktop")
        || desktop_file.contains('/')
        || desktop_file.contains('\\')
    {
        return Err(ProtocolRegistrationError::UnsafeCharacters);
    }
    Ok(vec![
        (
            "x-scheme-handler/discord".to_string(),
            desktop_file.to_string(),
        ),
        (
            "x-scheme-handler/disclip".to_string(),
            desktop_file.to_string(),
        ),
    ])
}

/// Формирует 4 команды `reg.exe add` для регистрации протокола в `HKCU\Software\Classes\<protocol>`
/// в точном соответствии с `/home/sanya/re-deep/main-arch-work/m-app-27251.txt` (`installProtocol`).
pub fn windows_protocol_registry_commands(
    protocol: &str,
    exec_path: &str,
) -> Result<Vec<Vec<String>>, ProtocolRegistrationError> {
    validate_registration_token(protocol)?;
    validate_registration_token(exec_path)?;
    if !protocol
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'-')
    {
        return Err(ProtocolRegistrationError::InvalidProtocolName);
    }
    let base_key = format!("HKCU\\Software\\Classes\\{protocol}");
    Ok(vec![
        vec![
            base_key.clone(),
            "/ve".to_string(),
            "/d".to_string(),
            format!("URL:{protocol} Protocol"),
        ],
        vec![
            base_key.clone(),
            "/v".to_string(),
            "URL Protocol".to_string(),
        ],
        vec![
            format!("{base_key}\\DefaultIcon"),
            "/ve".to_string(),
            "/d".to_string(),
            format!("\"{exec_path}\",-1"),
        ],
        vec![
            format!("{base_key}\\shell\\open\\command"),
            "/ve".to_string(),
            "/d".to_string(),
            format!("\"{exec_path}\" --url -- \"%1\""),
        ],
    ])
}

/// Причины отклонения клипа протокола `disclip://`
/// (`/home/sanya/re-deep/main-arch-work/m-core-7635.txt`: `DiscordMP4ErrorReason`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscordMp4ErrorReason {
    NotMp4Extension,
    NotMp4Signature,
    MalformedBox,
    NoDiscordUuidBox,
    PathOutsideClipsRoot,
    IoError,
}

impl DiscordMp4ErrorReason {
    /// Строковый код ошибки из RE-оракула (`m-core-7635.txt`).
    #[must_use]
    pub const fn as_re_str(self) -> &'static str {
        match self {
            Self::NotMp4Extension => "not-mp4-extension",
            Self::NotMp4Signature => "not-mp4-signature",
            Self::MalformedBox => "malformed-box",
            Self::NoDiscordUuidBox => "no-discord-uuid-box",
            Self::PathOutsideClipsRoot => "path-outside-clips-root",
            Self::IoError => "io-error",
        }
    }
}

impl fmt::Display for DiscordMp4ErrorReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_re_str())
    }
}

impl std::error::Error for DiscordMp4ErrorReason {}

/// Проверяет, что буфер является валидным MP4-файлом клипа Discord
/// (`/home/sanya/re-deep/main-arch-work/m-core-7635.txt`: `verifyDiscordMP4`):
/// 1. Расширение имени файла `.mp4` (без учёта регистра).
/// 2. Сигнатура ISO BMFF `ftypisom` в байтах `4..12`.
/// 3. Корректная структура атомов (box size >= 8 или 64-bit extended size == 1) без выхода за границы.
/// 4. Наличие атома `uuid` с 16-байтным идентификатором [`DISCORD_CLIP_UUID_BYTES`].
pub fn verify_discord_mp4_bytes(filename: &str, bytes: &[u8]) -> Result<(), DiscordMp4ErrorReason> {
    let has_mp4_ext = Path::new(filename)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("mp4"));
    if !has_mp4_ext {
        return Err(DiscordMp4ErrorReason::NotMp4Extension);
    }

    if bytes.len() < 12 || &bytes[4..12] != b"ftypisom" {
        return Err(DiscordMp4ErrorReason::NotMp4Signature);
    }

    let mut offset = 0usize;
    let mut found_discord_uuid = false;

    while offset + 8 <= bytes.len() {
        let box_size_32 = u32::from_be_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
        ]) as usize;
        let box_type = &bytes[offset + 4..offset + 8];

        let (actual_size, header_size) = if box_size_32 == 1 {
            if offset + 16 > bytes.len() {
                return Err(DiscordMp4ErrorReason::MalformedBox);
            }
            let ext_size = u64::from_be_bytes([
                bytes[offset + 8],
                bytes[offset + 9],
                bytes[offset + 10],
                bytes[offset + 11],
                bytes[offset + 12],
                bytes[offset + 13],
                bytes[offset + 14],
                bytes[offset + 15],
            ]);
            let Ok(sz) = usize::try_from(ext_size) else {
                return Err(DiscordMp4ErrorReason::MalformedBox);
            };
            if sz < 16 {
                return Err(DiscordMp4ErrorReason::MalformedBox);
            }
            (sz, 16usize)
        } else if box_size_32 >= 8 {
            (box_size_32, 8usize)
        } else {
            return Err(DiscordMp4ErrorReason::MalformedBox);
        };

        let Some(next_offset) = offset.checked_add(actual_size) else {
            return Err(DiscordMp4ErrorReason::MalformedBox);
        };
        if next_offset > bytes.len() {
            return Err(DiscordMp4ErrorReason::MalformedBox);
        }

        if box_type == b"uuid" {
            if actual_size < header_size + 16 {
                return Err(DiscordMp4ErrorReason::MalformedBox);
            }
            let uuid_slice = &bytes[offset + header_size..offset + header_size + 16];
            if uuid_slice == DISCORD_CLIP_UUID_BYTES {
                found_discord_uuid = true;
            }
        }

        offset = next_offset;
    }

    if offset != bytes.len() {
        return Err(DiscordMp4ErrorReason::MalformedBox);
    }

    if found_discord_uuid {
        Ok(())
    } else {
        Err(DiscordMp4ErrorReason::NoDiscordUuidBox)
    }
}

/// Проверяет файл клипа на диске: защищает от выхода за пределы `clips_root` (canonicalize + `starts_with`)
/// и валидирует структуру MP4 + `DISCORD_CLIP_UUID_BYTES`.
pub fn verify_discord_mp4_file(
    clips_root: &Path,
    requested_path: &Path,
) -> Result<PathBuf, DiscordMp4ErrorReason> {
    let has_mp4_ext = requested_path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("mp4"));
    if !has_mp4_ext {
        return Err(DiscordMp4ErrorReason::NotMp4Extension);
    }

    let canonical_root =
        std::fs::canonicalize(clips_root).map_err(|_| DiscordMp4ErrorReason::IoError)?;
    let canonical_file = std::fs::canonicalize(requested_path)
        .map_err(|_| DiscordMp4ErrorReason::PathOutsideClipsRoot)?;

    if !canonical_file.starts_with(&canonical_root) {
        return Err(DiscordMp4ErrorReason::PathOutsideClipsRoot);
    }

    let bytes = std::fs::read(&canonical_file).map_err(|_| DiscordMp4ErrorReason::IoError)?;
    let filename = canonical_file
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("clip.mp4");
    verify_discord_mp4_bytes(filename, &bytes)?;
    Ok(canonical_file)
}

/// HTTP-ответ обработчика протокола `disclip://` (`200 OK`, `206 Partial Content`, `416 Range Not Satisfiable`, `404 Not Found`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisclipResponse {
    pub status: u16,
    pub content_type: String,
    pub accept_ranges: Option<String>,
    pub content_range: Option<String>,
    pub content_length: Option<usize>,
    pub body: Vec<u8>,
}

impl DisclipResponse {
    #[must_use]
    pub fn not_found() -> Self {
        Self {
            status: 404,
            content_type: "text/plain".to_string(),
            accept_ranges: None,
            content_range: None,
            content_length: Some(0),
            body: Vec::new(),
        }
    }
}

fn parse_http_byte_range(
    range_header: &str,
    file_size: usize,
) -> Option<Result<(usize, usize), ()>> {
    let spec = range_header.trim().strip_prefix("bytes=")?;
    let (start_str, end_str) = spec.split_once('-')?;
    if file_size == 0 {
        return Some(Err(()));
    }
    let start = if start_str.is_empty() {
        0usize
    } else {
        let Ok(s) = start_str.parse::<usize>() else {
            return Some(Err(()));
        };
        s
    };
    let end = if end_str.is_empty() {
        file_size - 1
    } else {
        let Ok(e) = end_str.parse::<usize>() else {
            return Some(Err(()));
        };
        e
    };
    if start >= file_size || end >= file_size || start > end {
        return Some(Err(()));
    }
    Some(Ok((start, end)))
}

/// Обрабатывает запрос `disclip://` в соответствии с `/home/sanya/re-deep/main-arch-work/m-core-7635.txt` (`setupClipsProtocol`):
/// валидирует путь внутри `clips_root`, проверяет `DISCORD_UUID` и обслуживает `Range: bytes=start-end` (`200`/`206`/`416`/`404`).
#[must_use]
pub fn serve_disclip_request(
    clips_root: &Path,
    request_url: &str,
    range_header: Option<&str>,
) -> DisclipResponse {
    let Ok(link) = parse_deep_link(request_url) else {
        return DisclipResponse::not_found();
    };
    if !link.scheme.eq_ignore_ascii_case(DISCORD_CLIP_PROTOCOL) {
        return DisclipResponse::not_found();
    }

    let raw_rel = match (link.host, link.path.trim_matches('/')) {
        ("clips", rest) if !rest.is_empty() => rest.to_string(),
        ("", rest) if !rest.is_empty() => rest.strip_prefix("clips/").unwrap_or(rest).to_string(),
        (host, "") if !host.is_empty() => host.to_string(),
        (host, rest) if !host.is_empty() => format!("{host}/{rest}"),
        _ => return DisclipResponse::not_found(),
    };

    if raw_rel.split('/').any(|seg| !is_safe_segment(seg)) {
        return DisclipResponse::not_found();
    }

    let candidate_path = clips_root.join(&raw_rel);
    let Ok(canonical_file) = verify_discord_mp4_file(clips_root, &candidate_path) else {
        return DisclipResponse::not_found();
    };
    let Ok(bytes) = std::fs::read(&canonical_file) else {
        return DisclipResponse::not_found();
    };
    let file_size = bytes.len();

    if let Some(range_raw) = range_header
        && let Some(range_res) = parse_http_byte_range(range_raw, file_size)
    {
        match range_res {
            Ok((start, end)) => {
                let chunk = bytes[start..=end].to_vec();
                let chunk_len = chunk.len();
                return DisclipResponse {
                    status: 206,
                    content_type: "video/mp4".to_string(),
                    accept_ranges: Some("bytes".to_string()),
                    content_range: Some(format!("bytes {start}-{end}/{file_size}")),
                    content_length: Some(chunk_len),
                    body: chunk,
                };
            }
            Err(()) => {
                return DisclipResponse {
                    status: 416,
                    content_type: "video/mp4".to_string(),
                    accept_ranges: None,
                    content_range: Some(format!("bytes */{file_size}")),
                    content_length: Some(0),
                    body: Vec::new(),
                };
            }
        }
    }

    DisclipResponse {
        status: 200,
        content_type: "video/mp4".to_string(),
        accept_ranges: Some("bytes".to_string()),
        content_range: None,
        content_length: Some(file_size),
        body: bytes,
    }
}

/// Единый безопасный обработчик кастомных протоколов (`discord://` и `disclip://`) с очередью до готовности
/// главного окна и счётчиками наблюдаемости (`accepted_count`, `queued_count`, `blocked_count`).
#[derive(Debug, Clone)]
pub struct CustomProtocolHandler {
    queue: DeepLinkQueue,
    accepted_count: usize,
    blocked_count: usize,
}

impl CustomProtocolHandler {
    #[must_use]
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_QUEUE_CAPACITY)
    }

    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            queue: DeepLinkQueue::with_capacity(capacity),
            accepted_count: 0,
            blocked_count: 0,
        }
    }

    #[must_use]
    pub fn is_window_ready(&self) -> bool {
        self.queue.is_window_ready()
    }

    #[must_use]
    pub fn accepted_count(&self) -> usize {
        self.accepted_count
    }

    #[must_use]
    pub fn queued_count(&self) -> usize {
        self.queue.len()
    }

    #[must_use]
    pub fn blocked_count(&self) -> usize {
        self.blocked_count
    }

    /// Обрабатывает входящий `open-url` / single-instance вызов:
    /// - При ошибке схемы/маршрута/traversal увеличивает `blocked_count` и возвращает `Err`.
    /// - До готовности главного окна помещает канонический [`MainWindowPath`] в очередь (`Ok(None)`).
    /// - После готовности окна увеличивает `accepted_count` и возвращает `Ok(Some(path))`.
    pub fn handle_open_url(
        &mut self,
        url: &str,
    ) -> Result<Option<MainWindowPath>, MainWindowPathError> {
        match self.queue.open_or_queue_url(url) {
            Ok(Some(path)) => {
                self.accepted_count += 1;
                Ok(Some(path))
            }
            Ok(None) => Ok(None),
            Err(err) => {
                self.blocked_count += 1;
                Err(err)
            }
        }
    }

    /// Переводит главное окно в состояние готовности и извлекает все накопленные маршруты,
    /// учитывая их в `accepted_count`.
    pub fn mark_window_ready_and_drain(&mut self) -> Vec<MainWindowPath> {
        self.queue.mark_window_ready();
        let drained = self.queue.drain_queued();
        self.accepted_count += drained.len();
        drained
    }

    /// Классифицирует навигацию WebView и учитывает заблокированные переходы в `blocked_count`.
    pub fn handle_navigation(&mut self, url: &str) -> NavigationDecision {
        let decision = classify_webview_navigation(url);
        if decision == NavigationDecision::Block {
            self.blocked_count += 1;
        }
        decision
    }

    /// Обслуживает запрос `disclip://` и учитывает отклонённые (`404`/`416`) запросы в `blocked_count`.
    pub fn handle_disclip(
        &mut self,
        clips_root: &Path,
        request_url: &str,
        range_header: Option<&str>,
    ) -> DisclipResponse {
        let resp = serve_disclip_request(clips_root, request_url, range_header);
        if resp.status >= 400 {
            self.blocked_count += 1;
        } else {
            self.accepted_count += 1;
        }
        resp
    }
}

impl Default for CustomProtocolHandler {
    fn default() -> Self {
        Self::new()
    }
}

/// Очередь deep links, поступивших до готовности главного окна
/// (`RE/main-arch.json` §9: `openOrQueueUrl`, `drain_queued`).
///
/// До `mark_window_ready` ссылки валидируются и ставятся в ограниченную FIFO-очередь.
/// При переполнении старые ссылки вытесняются (bounded FIFO).
/// После готовности окна `open_or_queue_url` возвращает `Ok(Some(path))`.
#[derive(Debug, Clone)]
pub struct DeepLinkQueue {
    window_ready: bool,
    capacity: usize,
    queue: VecDeque<MainWindowPath>,
}

impl DeepLinkQueue {
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_QUEUE_CAPACITY)
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            window_ready: false,
            capacity: capacity.max(1),
            queue: VecDeque::new(),
        }
    }

    pub fn is_window_ready(&self) -> bool {
        self.window_ready
    }

    pub fn mark_window_ready(&mut self) {
        self.window_ready = true;
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn len(&self) -> usize {
        self.queue.len()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    pub fn main_window_path(&self, url: &str) -> Result<MainWindowPath, MainWindowPathError> {
        main_window_path(url)
    }

    /// Принимает URL: если окно готово — возвращает разрешённый путь сразу;
    /// если не готово — валидирует, помещает в ограниченную FIFO-очередь и возвращает `Ok(None)`.
    /// При неизвестном маршруте или невалидном URL возвращает типизированную ошибку без паники.
    pub fn open_or_queue_url(
        &mut self,
        url: &str,
    ) -> Result<Option<MainWindowPath>, MainWindowPathError> {
        let path = main_window_path(url)?;
        if self.window_ready {
            Ok(Some(path))
        } else {
            if self.queue.len() >= self.capacity {
                self.queue.pop_front();
            }
            self.queue.push_back(path);
            Ok(None)
        }
    }

    /// Извлекает все накопившиеся ссылки в порядке поступления (FIFO).
    pub fn drain_queued(&mut self) -> Vec<MainWindowPath> {
        self.queue.drain(..).collect()
    }
}

impl Default for DeepLinkQueue {
    fn default() -> Self {
        Self::new()
    }
}

/// Решение классификатора навигации WebView (`P2.7`, эталон `LemonCord/src/main.rs` и `Serein/crates/platform/src/lib.rs`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigationDecision {
    /// Разрешить навигацию внутри WebView (локальные маршруты приложения, `discord://`/`disclip://`,
    /// доверенные домены Discord / OAuth2 / hCaptcha).
    AllowInternal,
    /// Перехватить навигацию и открыть ссылку во внешнем браузере ОС (`OpenExternalBrowser`).
    OpenExternalBrowser,
    /// Заблокировать переход (опасная схема `javascript:` / `data:` / `file:`, управляющие символы,
    /// подмена учётных данных или понижение протокола `http://` на домене Discord/Captcha).
    Block,
}

impl NavigationDecision {
    /// Возвращает `true`, если переход разрешён внутри WebView.
    #[must_use]
    pub const fn is_internal(self) -> bool {
        matches!(self, Self::AllowInternal)
    }

    /// Возвращает `true`, если URL должен быть открыт во внешнем браузере.
    #[must_use]
    pub const fn should_open_external(self) -> bool {
        matches!(self, Self::OpenExternalBrowser)
    }
}

/// Классифицирует целевой URL навигации WebView (`P2.7`):
/// - Разрешает внутреннюю навигацию ([`NavigationDecision::AllowInternal`]) только для локальных
///   маршрутов приложения (`/channels/...`, `/settings/...`, `tauri://localhost/...`, валидных
///   deep-link `discord://`/`disclip://`) и доверенных HTTPS-доменов Discord / OAuth2 / Captcha
///   (`discord.com`, `ptb.discord.com`, `canary.discord.com`, `hcaptcha.com`, `*.hcaptcha.com`).
/// - Для любых внешних `http://` и `https://` URL возвращает [`NavigationDecision::OpenExternalBrowser`].
/// - Для невалидных или опасных схем (`javascript:`, `data:`, `file:`, `vbscript:`) возвращает [`NavigationDecision::Block`].
#[must_use]
pub fn classify_webview_navigation(url: &str) -> NavigationDecision {
    if url.is_empty()
        || url.len() > MAX_DEEP_LINK_LEN
        || url
            .bytes()
            .any(|b| b < 0x20 || b == 0x7F || b.is_ascii_whitespace())
    {
        return NavigationDecision::Block;
    }

    // 1. Относительные локальные маршруты приложения (`/app`, `/channels/...`, `/settings/...`, `/login`, `/oauth2/...`)
    if url.starts_with('/') {
        if url.starts_with("//") || url.contains("..") {
            return NavigationDecision::Block;
        }
        return NavigationDecision::AllowInternal;
    }

    // 2. Deep-link протоколы `discord:` и `disclip:`
    if is_direct_deep_link(url) {
        return if main_window_path(url).is_ok() {
            NavigationDecision::AllowInternal
        } else {
            NavigationDecision::Block
        };
    }

    let Some(colon) = url.find(':') else {
        return NavigationDecision::Block;
    };
    let scheme = &url[..colon];
    let rest = &url[colon + 1..];

    // 3. Опасные схемы блокируются безусловно
    if scheme.eq_ignore_ascii_case("javascript")
        || scheme.eq_ignore_ascii_case("data")
        || scheme.eq_ignore_ascii_case("file")
        || scheme.eq_ignore_ascii_case("vbscript")
        || scheme.eq_ignore_ascii_case("blob")
    {
        return NavigationDecision::Block;
    }

    // 4. Локальные custom-схемы Tauri / Wry (`tauri://localhost/...`, `wry://localhost/...`, `app://localhost/...`)
    if scheme.eq_ignore_ascii_case("tauri")
        || scheme.eq_ignore_ascii_case("wry")
        || scheme.eq_ignore_ascii_case("app")
    {
        let Some(after_slashes) = rest.strip_prefix("//") else {
            return NavigationDecision::Block;
        };
        let host_end = after_slashes
            .find(['/', '?', '#'])
            .unwrap_or(after_slashes.len());
        let authority = &after_slashes[..host_end];
        if authority.eq_ignore_ascii_case("localhost") {
            return NavigationDecision::AllowInternal;
        }
        return NavigationDecision::Block;
    }

    // 5. HTTP / HTTPS навигация
    let is_https = scheme.eq_ignore_ascii_case("https");
    let is_http = scheme.eq_ignore_ascii_case("http");
    if !is_https && !is_http {
        return NavigationDecision::Block;
    }

    let Some(after_slashes) = rest.strip_prefix("//") else {
        return NavigationDecision::Block;
    };
    let host_end = after_slashes
        .find(['/', '?', '#'])
        .unwrap_or(after_slashes.len());
    let authority = &after_slashes[..host_end];
    if authority.is_empty() || authority.contains('@') {
        return NavigationDecision::Block;
    }

    let (host, port) = match authority.rsplit_once(':') {
        Some((h, p)) => {
            if p.is_empty() || !p.bytes().all(|b| b.is_ascii_digit()) {
                return NavigationDecision::Block;
            }
            let Ok(port_num) = p.parse::<u16>() else {
                return NavigationDecision::Block;
            };
            if port_num == 0 {
                return NavigationDecision::Block;
            }
            (h, Some(port_num))
        }
        None => (authority, None),
    };

    if host.is_empty() {
        return NavigationDecision::Block;
    }

    // Локальный dev-сервер Tauri (`localhost`, `127.0.0.1`, `tauri.localhost`)
    if host.eq_ignore_ascii_case("localhost")
        || host == "127.0.0.1"
        || host.eq_ignore_ascii_case("tauri.localhost")
    {
        return NavigationDecision::AllowInternal;
    }

    let is_trusted_discord_or_captcha = is_trusted_internal_domain(host);
    if is_trusted_discord_or_captcha {
        // Для доверенных доменов Discord и Captcha разрешён только HTTPS на стандартном порту 443.
        // Попытка перейти по HTTP или на нестандартный порт блокируется (защита от downgrade/MITM).
        if !is_https || port.is_some_and(|p| p != 443) {
            return NavigationDecision::Block;
        }
        return NavigationDecision::AllowInternal;
    }

    // Любой другой валидный внешний домен открывается в системном браузере
    if !host.contains('.') || host.starts_with('.') || host.ends_with('.') {
        return NavigationDecision::Block;
    }
    NavigationDecision::OpenExternalBrowser
}

fn is_trusted_internal_domain(host: &str) -> bool {
    let lower = host.to_ascii_lowercase();
    matches!(
        lower.as_str(),
        "discord.com"
            | "ptb.discord.com"
            | "canary.discord.com"
            | "discordapp.com"
            | "cdn.discordapp.com"
            | "media.discordapp.net"
            | "hcaptcha.com"
            | "recaptcha.net"
            | "www.recaptcha.net"
    ) || lower.ends_with(".hcaptcha.com")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_webview_navigation_allows_local_routes_and_trusted_domains_and_opens_external() {
        // Локальные маршруты и deep links -> AllowInternal
        for internal in [
            "/app",
            "/channels/@me",
            "/channels/123/456",
            "/settings/voice",
            "/login",
            "/oauth2/authorize?client_id=123",
            "discord://-/channels/1/2",
            "disclip://clip-42",
            "tauri://localhost/index.html",
            "https://tauri.localhost/channels/@me",
            "http://localhost:1420/index.html",
            "http://127.0.0.1:1420/",
            "https://discord.com/app",
            "https://discord.com/login",
            "https://discord.com/oauth2/authorize?client_id=1&scope=identify",
            "https://ptb.discord.com/channels/@me",
            "https://canary.discord.com/app",
            "https://hcaptcha.com/1/api.js",
            "https://newassets.hcaptcha.com/captcha/v1/abc/static/hcaptcha.html",
        ] {
            assert_eq!(
                classify_webview_navigation(internal),
                NavigationDecision::AllowInternal,
                "ожидался AllowInternal для {internal:?}"
            );
        }

        // Внешние ссылки -> OpenExternalBrowser
        for external in [
            "https://example.com",
            "https://github.com/ViceVerse-cz/Serein",
            "https://discord.com.evil.test/login",
            "https://evil-discord.com/app",
            "https://hcaptcha.com.evil.test/captcha",
            "https://evilhcaptcha.com/",
            "http://example.com/news",
        ] {
            assert_eq!(
                classify_webview_navigation(external),
                NavigationDecision::OpenExternalBrowser,
                "ожидался OpenExternalBrowser для {external:?}"
            );
        }

        // Опасные или поддельные URL -> Block
        for blocked in [
            "",
            "   ",
            "javascript:alert(1)",
            "data:text/html,<script>alert(1)</script>",
            "file:///etc/passwd",
            "vbscript:msgbox(1)",
            "//evil.test/phishing",
            "http://discord.com/login",
            "http://hcaptcha.com/",
            "https://user:pass@discord.com/login",
            "https://discord.com:8443/login",
        ] {
            assert_eq!(
                classify_webview_navigation(blocked),
                NavigationDecision::Block,
                "ожидался Block для {blocked:?}"
            );
        }
    }
}
