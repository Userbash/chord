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

/// Максимальная длина deep-link в байтах (включительно).
pub const MAX_DEEP_LINK_LEN: usize = 4096;

/// Префикс стандартного протокола приложения Discord.
pub const DISCORD_APP_PROTOCOL: &str = "discord";

/// Префикс привилегированного протокола клипов (`RE/main-arch.json` §9, §10).
pub const DISCORD_CLIP_PROTOCOL: &str = "disclip";

/// Емкость очереди deep links до готовности окна по умолчанию.
pub const DEFAULT_QUEUE_CAPACITY: usize = 32;

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
    /// `discord://action/oauth2/authorize`.
    OAuth2Authorize,
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
            Self::OAuth2Authorize => f.write_str("OAuth2Authorize"),
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

    Route::Unknown
}

fn clip_id_from_disclip(link: &DeepLink<'_>) -> Option<String> {
    if !link.host.is_empty() && !link.host.eq_ignore_ascii_case("clips") {
        return Some(link.host.to_owned());
    }
    let rest = link.path.strip_prefix('/')?;
    let id = rest.strip_prefix("clips/").unwrap_or(rest);
    let id = id.strip_suffix('/').unwrap_or(id);
    if id.is_empty() || id.contains('/') {
        return None;
    }
    Some(id.to_owned())
}

fn clip_id_from_discord(link: &DeepLink<'_>) -> Option<String> {
    if host_is(link, "clips") {
        let rest = link.path.strip_prefix('/')?;
        let id = rest.strip_suffix('/').unwrap_or(rest);
        if id.is_empty() || id.contains('/') {
            return None;
        }
        return Some(id.to_owned());
    }
    if host_is_dash_or_empty(link) {
        let rest = link.path.strip_prefix("/clips/")?;
        let id = rest.strip_suffix('/').unwrap_or(rest);
        if id.is_empty() || id.contains('/') {
            return None;
        }
        return Some(id.to_owned());
    }
    None
}

fn bare_settings_section(path: &str) -> Option<&str> {
    let rest = path.strip_prefix('/')?;
    let section = rest.strip_suffix('/').unwrap_or(rest);
    if section.is_empty() || section.contains('/') {
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
        if guild.is_empty() || chan.is_empty() || chan.contains('/') {
            return None;
        }
        Some((guild.to_owned(), Some(chan.to_owned())))
    } else {
        Some((clean.to_owned(), None))
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
        Route::Unknown => Err(MainWindowPathError::UnknownRoute(url.to_string())),
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
