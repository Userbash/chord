//! Каталог REST-маршрутов v9 (T-REST-01, волна RW-08 / P1-07; маркировка `[S]`-расширений —
//! RW-40 BE-P3-02).
//!
//! Публичные типы и сигнатуры — замороженный контракт (`waves/RW-08/TZ.md` §4.1). Модуль
//! хранит 49 маршрутов: 47 строк живого захвата D-02 плюс 2 расширения, подтверждённых не
//! D-02, — `POST /channels/{id}/messages` (живой контракт отправки) и
//! `GET /guilds/{id}/messages/search` (сырой `capture-run2.jsonl`). Ищет маршруты по точной
//! паре «метод, шаблон» и подставляет части в плейсхолдеры `{id}`/`{num}`.
//!
//! Оракул: `docs/re/rest-d02.json` (md5 `87d7a05d8cd11f3b8ae37aa5c9811c9e`) — 47 endpoints.
//! Это санитизированная копия лабораторного `/home/sanya/re-deep/rest-d02.json`
//! (md5 `9b450c34a2277b04989830b53b02f012`): отличие только в `reqBodySample`, заменённом
//! на `REDACTED` (`docs/re/README.md`). Выдержка для тестов —
//! `tests/data/rest-routes-47.tsv` (47 строк).
//!
//! D-108: маршруты, подтверждённые не захватом D-02, а отдельным `[S]`-оракулом или сырым
//! захватом, помечает [`RouteSpec::is_static_extension`]. Внутри [`ROUTES`] таких две
//! ([`CAPTURED_D02_ROUTE_COUNT`] + [`STATIC_EXTENSION_ROUTE_COUNT`] = 49); ещё четыре лежат
//! в `EXTENSIONS` и резолвятся [`find`]:
//! - [`TYPING_PATH`] — `POST /channels/{id}/typing` (`NETWORK-CONTRACTS.md` §1.6 /
//!   `G6-REPORT-RW40-02.md` §3.6);
//! - [`INVITE_PATH`] — `GET`/`POST /invites/{code}` (`NETWORK-CONTRACTS.md` §1.2/§1.4 /
//!   `G6-REPORT-RW40-02.md` §3.3). В D-02 invite-ручек нет вообще;
//! - [`CHANNEL_SEARCH_PATH`] — `GET /channels/{id}/messages/search`.

/// HTTP-метод запроса (набор RFC 9110; в захвате D-02 наблюдались GET/POST/PUT/PATCH).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
}

impl HttpMethod {
    /// Имя метода в проводе (`"GET"`, `"POST"`, …).
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            HttpMethod::Get => "GET",
            HttpMethod::Post => "POST",
            HttpMethod::Put => "PUT",
            HttpMethod::Patch => "PATCH",
            HttpMethod::Delete => "DELETE",
        }
    }
}

/// Тип параметра в query-строке запроса.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QueryParamType {
    /// Строковый параметр (UTF-8, экранируется percent-encoding).
    String,
    /// Целочисленный параметр (u32/u64/i64).
    Integer,
    /// Булев параметр ("true" / "false").
    Boolean,
    /// Идентификатор Snowflake Discord (17-20 десятичных цифр).
    Snowflake,
    /// Идентификатор UUID v4 (например, для сессий поиска).
    Uuid,
}

/// Спецификация параметра query-строки маршрута.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueryParamSpec {
    /// Имя параметра в URL (например, "archived", "limit").
    pub name: &'static str,
    /// Обязателен ли параметр для отправки запроса.
    pub required: bool,
    /// Тип параметра.
    pub param_type: QueryParamType,
}

impl QueryParamSpec {
    /// Конструктор обязательного параметра.
    #[must_use]
    pub const fn required(name: &'static str, param_type: QueryParamType) -> Self {
        Self {
            name,
            required: true,
            param_type,
        }
    }

    /// Конструктор опционального параметра.
    #[must_use]
    pub const fn optional(name: &'static str, param_type: QueryParamType) -> Self {
        Self {
            name,
            required: false,
            param_type,
        }
    }
}

/// Маршрут: метод, шаблон пути (относительный, без базы `/api/v9`), ключи тела запроса
/// и спецификация параметров строки запроса.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RouteSpec {
    /// HTTP-метод запроса.
    pub method: HttpMethod,
    /// Шаблон пути относительно базы (без `/api/v9`).
    pub path: &'static str,
    /// Ключи тела запроса дословно из оракула; пусто — тела нет.
    pub body_keys: &'static [&'static str],
    /// Спецификация параметров строки запроса.
    pub query_params: &'static [QueryParamSpec],
}

impl RouteSpec {
    /// `true`, если маршрут подтверждён не захватом D-02, а `[S]`-оракулом или сырым
    /// захватом (`capture-run2.jsonl`).
    ///
    /// Расширения внутри [`ROUTES`] — `POST /channels/{id}/messages` и
    /// `GET /guilds/{id}/messages/search`; в `EXTENSIONS` — typing, invite `GET`/`POST` и
    /// поиск по каналу. Остальные 47 строк [`ROUTES`] — живой захват D-02.
    #[must_use]
    pub fn is_static_extension(&self) -> bool {
        STATIC_EXTENSION_ROUTES
            .iter()
            .any(|(method, path)| *method == self.method && *path == self.path)
    }
}

/// Пары «метод, шаблон» всех маршрутов вне захвата D-02 (`[S]`/сырой захват).
const STATIC_EXTENSION_ROUTES: [(HttpMethod, &str); 7] = [
    (HttpMethod::Post, "/channels/{id}/messages"),
    (HttpMethod::Get, GUILD_SEARCH_PATH),
    (HttpMethod::Post, TYPING_PATH),
    (HttpMethod::Get, INVITE_PATH),
    (HttpMethod::Post, INVITE_PATH),
    (HttpMethod::Get, CHANNEL_SEARCH_PATH),
    (HttpMethod::Post, CHANNEL_ATTACHMENTS_PATH),
];

/// Конструктор элемента каталога (приватный: состав каталога фиксирован оракулом).
const fn route(
    method: HttpMethod,
    path: &'static str,
    body_keys: &'static [&'static str],
) -> RouteSpec {
    RouteSpec {
        method,
        path,
        body_keys,
        query_params: &[],
    }
}

/// Конструктор элемента каталога со спецификацией параметров строки запроса.
const fn route_with_query(
    method: HttpMethod,
    path: &'static str,
    body_keys: &'static [&'static str],
    query_params: &'static [QueryParamSpec],
) -> RouteSpec {
    RouteSpec {
        method,
        path,
        body_keys,
        query_params,
    }
}

/// Спецификация query-параметров выборки сообщений (`GET /channels/{id}/messages`).
pub const MESSAGES_QUERY_PARAMS: &[QueryParamSpec] = &[
    QueryParamSpec::optional("limit", QueryParamType::Integer),
    QueryParamSpec::optional("before", QueryParamType::Snowflake),
    QueryParamSpec::optional("after", QueryParamType::Snowflake),
    QueryParamSpec::optional("around", QueryParamType::Snowflake),
];

/// Спецификация query-параметров списка pins (`GET /channels/{id}/messages/pins`).
pub const PINS_QUERY_PARAMS: &[QueryParamSpec] =
    &[QueryParamSpec::optional("limit", QueryParamType::Integer)];

/// Спецификация query-параметров поиска тредов (`GET /channels/{id}/threads/search`).
pub const THREADS_SEARCH_QUERY_PARAMS: &[QueryParamSpec] = &[
    QueryParamSpec::required("archived", QueryParamType::Boolean),
    QueryParamSpec::optional("sort_by", QueryParamType::String),
    QueryParamSpec::optional("sort_order", QueryParamType::String),
    QueryParamSpec::optional("limit", QueryParamType::Integer),
    QueryParamSpec::optional("tag_setting", QueryParamType::String),
    QueryParamSpec::optional("offset", QueryParamType::Integer),
];

/// Спецификация query-параметров поиска сообщений (`/messages/search`).
pub const MESSAGES_SEARCH_QUERY_PARAMS: &[QueryParamSpec] = &[
    QueryParamSpec::optional("content", QueryParamType::String),
    QueryParamSpec::optional("sort_by", QueryParamType::String),
    QueryParamSpec::optional("sort_order", QueryParamType::String),
    QueryParamSpec::optional("search_session_id", QueryParamType::Uuid),
    QueryParamSpec::optional("search_query_id", QueryParamType::Uuid),
    QueryParamSpec::optional("offset", QueryParamType::Integer),
];

/// Число маршрутов живого захвата D-02 в [`ROUTES`] (47 endpoints `rest-d02.json`).
pub const CAPTURED_D02_ROUTE_COUNT: usize = 47;

/// Число `[S]`/raw-расширений внутри [`ROUTES`]: `POST /channels/{id}/messages` и
/// `GET /guilds/{id}/messages/search`.
pub const STATIC_EXTENSION_ROUTE_COUNT: usize = 2;

/// Маршруты захвата D-02 плюс два `[S]`/raw-расширения (`POST` отправки сообщения и поиск
/// по серверу из сырого `capture-run2.jsonl`).
///
/// 47 строк D-02 идут в порядке выдержки `tests/data/rest-routes-47.tsv` (порядок захвата),
/// два расширения вставлены в позиции живого захвата; пути относительные (без `/api/v9`);
/// `body_keys` — дословно из оракула, пусто = тела нет.
pub const ROUTES: [RouteSpec; 49] = [
    route(
        HttpMethod::Post,
        "/channels/{id}/messages",
        &["content", "nonce", "tts"],
    ),
    route_with_query(
        HttpMethod::Get,
        "/channels/{id}/messages",
        &[],
        MESSAGES_QUERY_PARAMS,
    ),
    route(HttpMethod::Get, "/applications/public", &[]),
    route(HttpMethod::Get, "/games/autocomplete", &[]),
    route(
        HttpMethod::Post,
        "/channels/{id}/messages/{id}/ack",
        &["token", "last_viewed"],
    ),
    route(HttpMethod::Get, "/quests/@me", &[]),
    route(HttpMethod::Get, "/users/{id}/profile", &[]),
    route(HttpMethod::Get, "/users/@me/billing/subscriptions", &[]),
    route(HttpMethod::Get, "/content-inventory/users/@me", &[]),
    route(HttpMethod::Get, "/games/detectable/exclusions", &[]),
    route(HttpMethod::Get, "/partner-sdk/storefront-config", &[]),
    route(HttpMethod::Get, "/promotions", &[]),
    route(HttpMethod::Get, "/quests/decision", &[]),
    route(HttpMethod::Get, "/quests/get-decisions", &[]),
    route(HttpMethod::Get, "/users/@me", &[]),
    route(HttpMethod::Get, "/users/@me/achievements/unread-state", &[]),
    route(
        HttpMethod::Get,
        "/users/@me/activities/statistics/applications",
        &[],
    ),
    route(
        HttpMethod::Get,
        "/users/@me/applications/{id}/entitlements",
        &[],
    ),
    route(HttpMethod::Get, "/users/@me/billing/checkout-recovery", &[]),
    route(
        HttpMethod::Post,
        "/users/@me/billing/user-offer",
        &["allow_triggers"],
    ),
    route(HttpMethod::Get, "/users/@me/collectibles-marketing", &[]),
    route(HttpMethod::Get, "/users/@me/entitlements", &[]),
    route(HttpMethod::Get, "/users/@me/library", &[]),
    route(HttpMethod::Get, "/users/@me/mfa/webauthn/credentials", &[]),
    route(HttpMethod::Get, "/users/@me/referrals/eligibility", &[]),
    route(HttpMethod::Get, "/users/@me/saved-messages", &[]),
    route(HttpMethod::Get, "/users/@me/settings-proto/{num}", &[]),
    route(HttpMethod::Get, "/users/@me/virtual-currency/balance", &[]),
    route(HttpMethod::Get, "/widget-configs/developer", &[]),
    route(HttpMethod::Get, "/widget-configs/featured", &[]),
    route(HttpMethod::Get, "/activities/shelf", &[]),
    route(
        HttpMethod::Get,
        "/guilds/{id}/application-command-index",
        &[],
    ),
    route_with_query(
        HttpMethod::Get,
        GUILD_SEARCH_PATH,
        &[],
        MESSAGES_SEARCH_QUERY_PARAMS,
    ),
    route(
        HttpMethod::Put,
        "/guilds/{id}/members/@me",
        &["(empty-object)"],
    ),
    route(HttpMethod::Get, "/soundboard-default-sounds", &[]),
    route(HttpMethod::Get, "/users/{id}/application-identities", &[]),
    route(HttpMethod::Get, "/users/@me/application-command-index", &[]),
    route(HttpMethod::Get, "/users/@me/notes/{id}", &[]),
    route(HttpMethod::Post, "/beaker", &["token", "events"]),
    route(
        HttpMethod::Patch,
        "/channels/{id}/explicit-media",
        &["message_ids"],
    ),
    route(
        HttpMethod::Put,
        "/channels/{id}/messages/{id}/reactions/%F0%9F%91%8B/%40me",
        &[],
    ),
    route_with_query(
        HttpMethod::Get,
        "/channels/{id}/messages/pins",
        &[],
        PINS_QUERY_PARAMS,
    ),
    route_with_query(
        HttpMethod::Get,
        "/channels/{id}/threads/search",
        &[],
        THREADS_SEARCH_QUERY_PARAMS,
    ),
    route(HttpMethod::Get, "/collectibles-products/{id}", &[]),
    route(HttpMethod::Get, "/games", &[]),
    route(HttpMethod::Get, "/storefront/products/skus", &[]),
    route(HttpMethod::Get, "/users/@me/billing/payment-sources", &[]),
    route(
        HttpMethod::Patch,
        "/users/@me/settings-proto/{num}",
        &["(non-json)"],
    ),
    route(HttpMethod::Get, "/users/@me/survey", &[]),
];

/// Поиск маршрута по методу и точному шаблону пути (`find(Get, "/channels/{id}/messages")`).
///
/// Совпадение точное: путь с базой `/api/v9`, префикс или другой метод не подходят.
/// Кроме frozen [`ROUTES`], резолвит `[S]`-расширения, подтверждённые не захватом D-02
/// (сейчас — [`TYPING_PATH`] и [`INVITE_PATH`]).
pub fn find(method: HttpMethod, path: &str) -> Option<&'static RouteSpec> {
    ROUTES
        .iter()
        .find(|spec| spec.method == method && spec.path == path)
        .or_else(|| {
            EXTENSIONS
                .iter()
                .find(|spec| spec.method == method && spec.path == path)
        })
}

/// `POST /channels/{id}/typing` — `[S]`-расширение каталога (D-108).
///
/// Контракт: `NETWORK-CONTRACTS.md` §1.6 и `G6-REPORT-RW40-02.md` §3.6 — тело запроса
/// отсутствует (`body_keys` пусто), успех `200`, ответ
/// `{message_send_cooldown_ms?, thread_create_cooldown_ms?}`. В захвате D-02 маршрута нет,
/// поэтому он не входит в [`ROUTES`] и не размывает оракул 47 live-строк.
pub const TYPING_PATH: &str = "/channels/{id}/typing";

/// `GET`/`POST /invites/{code}` — `[S]`-расширение каталога (G-03, D-108).
///
/// Контракт: `NETWORK-CONTRACTS.md` §1.2/§1.4 и `G6-REPORT-RW40-02.md` §3.3. У `GET`
/// (resolve) тела нет, ответ — invite-объект; `POST` (accept) принимает ровно три
/// nullable-поля [`INVITE_ACCEPT_BODY_KEYS`], ответ — тот же invite-объект. В захвате
/// D-02 invite-ручек нет вообще (47 путей, 0 invite), поэтому маршруты не входят в
/// [`ROUTES`] и оракул live-строк не размывается.
pub const INVITE_PATH: &str = "/invites/{code}";

/// Ключи тела `POST /invites/{code}` — дословно из call-site (§1.2).
///
/// Все три nullable: `session_id` = `getSessionId()` (`null` в бандле),
/// `invite_instance_id` может отсутствовать для вставленного кода,
/// `received_installation_id` клиент чистит после успеха. `context` — analytics-объект
/// запроса, **не** поле тела.
pub const INVITE_ACCEPT_BODY_KEYS: [&str; 3] = [
    "session_id",
    "invite_instance_id",
    "received_installation_id",
];

/// `GET /channels/{id}/messages/search` — `[S]`-расширение каталога для поиска сообщений в канале.
pub const CHANNEL_SEARCH_PATH: &str = "/channels/{id}/messages/search";

/// `GET /guilds/{id}/messages/search` — поиск по серверу из сырого `capture-run2.jsonl`.
///
/// В 47 endpoints `rest-d02.json` маршрута нет, поэтому он помечен как `[S]`/raw-расширение
/// ([`RouteSpec::is_static_extension`]), хотя и находится внутри [`ROUTES`].
pub const GUILD_SEARCH_PATH: &str = "/guilds/{id}/messages/search";

/// `POST /channels/{id}/attachments` — выделение pre-signed слотов загрузки вложений в канале (`P2-2.2`).
pub const CHANNEL_ATTACHMENTS_PATH: &str = "/channels/{id}/attachments";

/// Ключи тела `POST /channels/{id}/attachments`.
pub const CHANNEL_ATTACHMENTS_BODY_KEYS: [&str; 1] = ["files"];

/// Максимальное количество вложений в одном сообщении Discord (`10`).
pub const MAX_ATTACHMENTS_PER_MESSAGE: usize = 10;

/// Максимальный размер одного файла вложения для стандартного аккаунта (`25 MiB = 26_214_400` байт).
pub const MAX_ATTACHMENT_SIZE_BYTES: usize = 25 * 1024 * 1024;

/// Статический список `[S]`-расширений: `find` возвращает `&'static RouteSpec`.
static EXTENSIONS: [RouteSpec; 5] = [
    route(HttpMethod::Post, TYPING_PATH, &[]),
    route(HttpMethod::Get, INVITE_PATH, &[]),
    route(HttpMethod::Post, INVITE_PATH, &INVITE_ACCEPT_BODY_KEYS),
    route_with_query(
        HttpMethod::Get,
        CHANNEL_SEARCH_PATH,
        &[],
        MESSAGES_SEARCH_QUERY_PARAMS,
    ),
    route(
        HttpMethod::Post,
        CHANNEL_ATTACHMENTS_PATH,
        &CHANNEL_ATTACHMENTS_BODY_KEYS,
    ),
];

/// Возвращает статическую спецификацию маршрута `POST /channels/{id}/attachments`.
#[must_use]
pub fn attachment_upload_route_spec() -> &'static RouteSpec {
    &EXTENSIONS[4]
}

/// Ошибки рендера пути.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathRenderError {
    /// Число частей не совпало с числом плейсхолдеров `{id}`/`{num}`.
    PartsMismatch {
        /// Сколько плейсхолдеров в шаблоне.
        expected: usize,
        /// Сколько частей передано.
        got: usize,
    },
    /// Часть пустая или содержит `{`/`}`.
    InvalidPart(String),
}

/// Подставляет части в плейсхолдеры `{id}`/`{num}` слева направо.
///
/// Порядок проверок контрактный: сначала число частей, затем допустимость каждой части,
/// и только потом подстановка. Ошибки не поглощаются: любая некорректная часть — `Err`.
pub fn render_path(spec: &RouteSpec, parts: &[&str]) -> Result<String, PathRenderError> {
    let expected = placeholder_count(spec.path);
    if parts.len() != expected {
        return Err(PathRenderError::PartsMismatch {
            expected,
            got: parts.len(),
        });
    }
    for part in parts {
        if part.is_empty() || part.contains('{') || part.contains('}') {
            return Err(PathRenderError::InvalidPart((*part).to_string()));
        }
    }

    let mut rendered =
        String::with_capacity(spec.path.len() + parts.iter().map(|part| part.len()).sum::<usize>());
    let mut rest = spec.path;
    for part in parts {
        if let Some((position, placeholder)) = next_placeholder(rest) {
            rendered.push_str(&rest[..position]);
            rendered.push_str(part);
            rest = &rest[position + placeholder.len()..];
        }
        // Недостижимо: число плейсхолдеров сверено с числом частей выше. Ветка без паники:
        // если инвариант когда-нибудь нарушится, остаток шаблона добавится как есть.
    }
    rendered.push_str(rest);
    Ok(rendered)
}

/// Плейсхолдеры шаблонов пути: `{id}`/`{num}` зафиксированы оракулом D-02,
/// `{code}` — `[S]`-расширение invite (G-03); других нет.
const PLACEHOLDERS: [&str; 3] = ["{id}", "{num}", "{code}"];

/// Число плейсхолдеров `{id}`/`{num}`/`{code}` в шаблоне.
fn placeholder_count(template: &str) -> usize {
    PLACEHOLDERS
        .iter()
        .copied()
        .map(|placeholder| template.matches(placeholder).count())
        .sum()
}

/// Ближайший слева плейсхолдер `{id}`/`{num}`: позиция и литерал.
fn next_placeholder(template: &str) -> Option<(usize, &'static str)> {
    PLACEHOLDERS
        .iter()
        .copied()
        .filter_map(|placeholder| {
            template
                .find(placeholder)
                .map(|position| (position, placeholder))
        })
        .min_by_key(|(position, _)| *position)
}

/// 45 имён query-параметров, подтверждённых живым захватом D-02 (`rest-d02.json` queryParams).
pub const CAPTURED_QUERY_PARAMS: [&str; 45] = [
    "application_ids",
    "visible_guild_ids",
    "limit",
    "platform",
    "placement",
    "client_heartbeat_session_id",
    "client_ad_session_id",
    "q",
    "filter_group",
    "locale",
    "country_code",
    "type",
    "with_mutual_guilds",
    "with_mutual_friends",
    "with_mutual_friends_count",
    "with_sku",
    "with_application",
    "entitlement_type",
    "exclude_ended",
    "exclude_consumed",
    "bookmark_ids",
    "include_inactive",
    "exclude_unpaid_statuses",
    "subscription_type",
    "for_game_profile",
    "feature",
    "num_decisions_requested",
    "sku_ids",
    "with_profiles",
    "before",
    "lurker",
    "guild_id",
    "sync_level",
    "with_bundled_skus",
    "include_google_sku_ids",
    "ignore_cache",
    "game_ids",
    "archived",
    "sort_by",
    "sort_order",
    "tag_setting",
    "offset",
    "around",
    "location",
    "disable_auto_seen",
];

/// Ошибки валидации и построения строки параметров запроса (query string).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueryError {
    /// Отсутствует обязательный query-параметр.
    MissingRequiredParam(&'static str),
    /// Значение параметра невалидно.
    InvalidParamValue {
        /// Имя параметра.
        param: &'static str,
        /// Причина невалидности.
        reason: String,
    },
    /// Взаимно исключающие параметры (например, одновременно `before` и `after`).
    MutuallyExclusiveParams {
        /// Первый параметр.
        param1: &'static str,
        /// Второй параметр.
        param2: &'static str,
    },
    /// Ошибка в пути маршрута.
    Path(PathRenderError),
}

impl std::fmt::Display for QueryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            QueryError::MissingRequiredParam(p) => {
                write!(f, "missing required query parameter: {p}")
            }
            QueryError::InvalidParamValue { param, reason } => {
                write!(f, "invalid query parameter '{param}': {reason}")
            }
            QueryError::MutuallyExclusiveParams { param1, param2 } => {
                write!(
                    f,
                    "mutually exclusive query parameters: {param1} and {param2}"
                )
            }
            QueryError::Path(err) => write!(f, "path error: {err:?}"),
        }
    }
}

impl std::error::Error for QueryError {}

impl From<PathRenderError> for QueryError {
    fn from(err: PathRenderError) -> Self {
        QueryError::Path(err)
    }
}

/// RFC 3986 percent-encoding компонента query-параметра (ключ или значение).
///
/// Символы `A-Z`, `a-z`, `0-9`, `-`, `_`, `.`, `~` (unreserved) остаются без изменений.
/// Все остальные байты кодируются в верхнем регистре `%XX`.
/// Экранирует `+` (%2B), `#` (%23), `&` (%26), пробел (%20), кириллицу и другие UTF-8 байты.
#[must_use]
pub fn encode_query_component(input: &str) -> String {
    let mut encoded = String::with_capacity(input.len());
    for &byte in input.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            _ => {
                use std::fmt::Write;
                let _ = write!(encoded, "%{:02X}", byte);
            }
        }
    }
    encoded
}

/// Конструктор query-строки запроса.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct QueryBuilder {
    params: Vec<(&'static str, String)>,
}

impl QueryBuilder {
    /// Создаёт пустой `QueryBuilder`.
    #[must_use]
    pub fn new() -> Self {
        Self { params: Vec::new() }
    }

    /// Добавить параметр со строковым значением (значение экранируется через [`encode_query_component`]).
    pub fn param(&mut self, key: &'static str, value: impl AsRef<str>) -> &mut Self {
        self.params
            .push((key, encode_query_component(value.as_ref())));
        self
    }

    /// Добавить строковый параметр, если значение Some.
    pub fn param_opt(&mut self, key: &'static str, value: Option<impl AsRef<str>>) -> &mut Self {
        if let Some(v) = value {
            self.param(key, v);
        }
        self
    }

    /// Добавить параметр с форматированием через Display (например, числа).
    pub fn param_display(&mut self, key: &'static str, value: impl std::fmt::Display) -> &mut Self {
        self.params.push((key, value.to_string()));
        self
    }

    /// Добавить параметр через Display, если Some.
    pub fn param_display_opt<T: std::fmt::Display>(
        &mut self,
        key: &'static str,
        value: Option<T>,
    ) -> &mut Self {
        if let Some(v) = value {
            self.param_display(key, v);
        }
        self
    }

    /// Добавить булев параметр ("true" или "false").
    pub fn param_bool(&mut self, key: &'static str, value: bool) -> &mut Self {
        self.params.push((
            key,
            if value {
                "true".to_string()
            } else {
                "false".to_string()
            },
        ));
        self
    }

    /// Собрать query-строку (например, `?archived=true&limit=25`).
    /// Если параметров нет, возвращает пустую строку `""`.
    #[must_use]
    pub fn build(&self) -> String {
        if self.params.is_empty() {
            String::new()
        } else {
            let mut s = String::from("?");
            for (idx, (k, v)) in self.params.iter().enumerate() {
                if idx > 0 {
                    s.push('&');
                }
                s.push_str(k);
                s.push('=');
                s.push_str(v);
            }
            s
        }
    }

    /// Собрать путь с query-строкой, добавив к базовому пути.
    #[must_use]
    pub fn build_with_path(&self, base_path: &str) -> String {
        let query = self.build();
        format!("{base_path}{query}")
    }

    /// Проверить соответствие параметров спецификации [`RouteSpec`].
    pub fn validate_spec(&self, spec: &RouteSpec) -> Result<(), QueryError> {
        for param_spec in spec.query_params {
            if param_spec.required && !self.params.iter().any(|(k, _)| *k == param_spec.name) {
                return Err(QueryError::MissingRequiredParam(param_spec.name));
            }
        }
        Ok(())
    }
}

/// Дефолтный лимит для pins (`?limit=25` из `rest-static.json` PINS.cacheEvidence).
pub const DEFAULT_PINS_LIMIT: u32 = 25;

/// Максимальный лимит для pins (50 в Discord).
pub const MAX_PINS_LIMIT: u32 = 50;

/// Собрать URL пути для списка закреплённых сообщений (`GET /channels/{id}/messages/pins`).
///
/// Сверяется с эталоном `https://discord.com/api/v9/channels/1058348665342808104/messages/pins?limit=25`.
/// Если `limit` не указан (`None`), используется дефолтный `DEFAULT_PINS_LIMIT` (25).
/// Если указан `limit`, валидируется диапазон `1..=MAX_PINS_LIMIT`.
pub fn build_channel_pins_path(channel_id: &str, limit: Option<u32>) -> Result<String, QueryError> {
    let spec =
        find(HttpMethod::Get, "/channels/{id}/messages/pins").expect("pins route in catalog");
    let base = render_path(spec, &[channel_id])?;

    let effective_limit = limit.unwrap_or(DEFAULT_PINS_LIMIT);
    if effective_limit == 0 || effective_limit > MAX_PINS_LIMIT {
        return Err(QueryError::InvalidParamValue {
            param: "limit",
            reason: format!(
                "pins limit must be between 1 and {MAX_PINS_LIMIT}, got {effective_limit}"
            ),
        });
    }

    let mut qb = QueryBuilder::new();
    qb.param_display("limit", effective_limit);
    Ok(qb.build_with_path(&base))
}

/// Параметры поиска тредов в канале (`GET /channels/{id}/threads/search`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ThreadSearchParams<'a> {
    /// Обязательный флаг архивации тредов (`archived`).
    pub archived: Option<bool>,
    /// Поле сортировки (например, "last_message_time").
    pub sort_by: Option<&'a str>,
    /// Порядок сортировки ("desc" / "asc").
    pub sort_order: Option<&'a str>,
    /// Лимит результатов (обычно 25).
    pub limit: Option<u32>,
    /// Настройка тегов ("match_some" / "match_all").
    pub tag_setting: Option<&'a str>,
    /// Смещение (offset).
    pub offset: Option<u32>,
}

impl<'a> ThreadSearchParams<'a> {
    /// Создаёт параметры с обязательным флагом `archived` и дефолтами из оракула
    /// `THREAD_SEARCH.cacheEvidence` (`sort_by=last_message_time`, `sort_order=desc`, `limit=25`, `tag_setting=match_some`, `offset=0`).
    #[must_use]
    pub fn new(archived: bool) -> Self {
        Self {
            archived: Some(archived),
            sort_by: Some("last_message_time"),
            sort_order: Some("desc"),
            limit: Some(25),
            tag_setting: Some("match_some"),
            offset: Some(0),
        }
    }

    /// Пустые параметры (для тестирования отсутствия обязательных полей).
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }
}

/// Собрать URL пути для поиска тредов (`GET /channels/{id}/threads/search`).
///
/// Сверяется с эталоном `https://discord.com/api/v9/channels/1058348665342808104/threads/search?archived=true&sort_by=last_message_time&sort_order=desc&limit=25&tag_setting=match_some&offset=0`.
/// Обязательный параметр `archived`: возвращает `QueryError::MissingRequiredParam("archived")` если отсутствует.
pub fn build_thread_search_path(
    channel_id: &str,
    params: &ThreadSearchParams<'_>,
) -> Result<String, QueryError> {
    let spec = find(HttpMethod::Get, "/channels/{id}/threads/search")
        .expect("thread search route in catalog");
    let base = render_path(spec, &[channel_id])?;

    let archived = params
        .archived
        .ok_or(QueryError::MissingRequiredParam("archived"))?;

    if let Some(limit) = params.limit {
        let is_invalid = limit == 0 || limit > 100;
        if is_invalid {
            return Err(QueryError::InvalidParamValue {
                param: "limit",
                reason: format!("thread search limit must be between 1 and 100, got {limit}"),
            });
        }
    }

    let mut qb = QueryBuilder::new();
    qb.param_bool("archived", archived);
    qb.param_opt("sort_by", params.sort_by);
    qb.param_opt("sort_order", params.sort_order);
    qb.param_display_opt("limit", params.limit);
    qb.param_opt("tag_setting", params.tag_setting);
    qb.param_display_opt("offset", params.offset);

    Ok(qb.build_with_path(&base))
}

/// Параметры выборки истории сообщений канала (`GET /channels/{id}/messages`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ChannelMessagesParams<'a> {
    /// Лимит сообщений (1..=100).
    pub limit: Option<u32>,
    /// Курсор сообщений до указанного ID.
    pub before: Option<&'a str>,
    /// Курсор сообщений после указанного ID.
    pub after: Option<&'a str>,
    /// Курсор сообщений вокруг указанного ID.
    pub around: Option<&'a str>,
}

fn validate_snowflake_cursor(param: &'static str, value: &str) -> Result<(), QueryError> {
    if value.is_empty()
        || !value.bytes().all(|b| b.is_ascii_digit())
        || value.parse::<u64>().map_or(true, |v| v == 0)
    {
        return Err(QueryError::InvalidParamValue {
            param,
            reason: format!("cursor must be a valid positive numeric snowflake, got '{value}'"),
        });
    }
    Ok(())
}

/// Собрать URL пути для выборки сообщений (`GET /channels/{id}/messages`).
///
/// Сверяется с эталоном `https://discord.com/api/v9/channels/1056563701764857896/messages?limit=12`.
/// Проверяет:
/// - `limit`: 1..=100.
/// - взаимную исключаемость курсоров `before`, `after`, `around`.
/// - валидность snowflake-курсоров (нечисловые, пустые и "0" отвергаются).
pub fn build_channel_messages_path(
    channel_id: &str,
    params: &ChannelMessagesParams<'_>,
) -> Result<String, QueryError> {
    let spec = find(HttpMethod::Get, "/channels/{id}/messages").expect("messages route in catalog");
    let base = render_path(spec, &[channel_id])?;

    if let Some(limit) = params.limit {
        let is_invalid = !(1..=100).contains(&limit);
        if is_invalid {
            return Err(QueryError::InvalidParamValue {
                param: "limit",
                reason: format!("messages limit must be between 1 and 100, got {limit}"),
            });
        }
    }

    let has_before = params.before.is_some();
    let has_after = params.after.is_some();
    let has_around = params.around.is_some();

    if has_before && has_after {
        return Err(QueryError::MutuallyExclusiveParams {
            param1: "before",
            param2: "after",
        });
    }
    if has_before && has_around {
        return Err(QueryError::MutuallyExclusiveParams {
            param1: "before",
            param2: "around",
        });
    }
    if has_after && has_around {
        return Err(QueryError::MutuallyExclusiveParams {
            param1: "after",
            param2: "around",
        });
    }

    if let Some(before) = params.before {
        validate_snowflake_cursor("before", before)?;
    }
    if let Some(after) = params.after {
        validate_snowflake_cursor("after", after)?;
    }
    if let Some(around) = params.around {
        validate_snowflake_cursor("around", around)?;
    }

    let mut qb = QueryBuilder::new();
    qb.param_display_opt("limit", params.limit);
    qb.param_opt("before", params.before);
    qb.param_opt("after", params.after);
    qb.param_opt("around", params.around);

    Ok(qb.build_with_path(&base))
}

/// Параметры поиска сообщений на сервере или в канале (`/messages/search`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MessageSearchParams<'a> {
    /// Текстовый поисковый запрос (`content` или `q`).
    pub content: Option<&'a str>,
    /// Поле сортировки (например, "timestamp").
    pub sort_by: Option<&'a str>,
    /// Порядок сортировки ("desc" / "asc").
    pub sort_order: Option<&'a str>,
    /// Идентификатор сессии поиска (search_session_id UUID v4).
    pub search_session_id: Option<&'a str>,
    /// Идентификатор запроса поиска (search_query_id UUID v4).
    pub search_query_id: Option<&'a str>,
    /// Смещение (offset, обычно кратно 25).
    pub offset: Option<u32>,
    /// Фильтр по каналу.
    pub channel_id: Option<&'a str>,
    /// Фильтр по автору.
    pub author_id: Option<&'a str>,
    /// Фильтр по вложениям (link, embed, file, video, image, sound).
    pub has: Option<&'a str>,
    /// Минимальный Snowflake сообщения.
    pub min_id: Option<&'a str>,
    /// Максимальный Snowflake сообщения.
    pub max_id: Option<&'a str>,
}

/// Собрать URL пути для серверного поиска сообщений (`GET /guilds/{id}/messages/search`).
///
/// Сверяется с живой трассой `capture-run2.jsonl`:
/// `/guilds/1014537851418980372/messages/search?content=%23%F0%9F%92%A5%E2%94%83%D0%BF%D1%80%D0%B0%D0%B2%D0%B8%D0%BB%D0%B0%D1%82%D0%B5%D1%81%D1%82&sort_by=timestamp&sort_order=desc&search_session_id=d6183157-1379-4323-9321-2c4743e835a5&search_query_id=cfb6f5e3-11e3-44be-91b2-8360807ed063&offset=0`.
pub fn build_guild_search_path(
    guild_id: &str,
    params: &MessageSearchParams<'_>,
) -> Result<String, QueryError> {
    let spec = find(HttpMethod::Get, "/guilds/{id}/messages/search")
        .expect("guild search route in catalog");
    let base = render_path(spec, &[guild_id])?;

    let mut qb = QueryBuilder::new();
    qb.param_opt("content", params.content);
    qb.param_opt("sort_by", params.sort_by);
    qb.param_opt("sort_order", params.sort_order);
    qb.param_opt("search_session_id", params.search_session_id);
    qb.param_opt("search_query_id", params.search_query_id);
    qb.param_display_opt("offset", params.offset);
    qb.param_opt("channel_id", params.channel_id);
    qb.param_opt("author_id", params.author_id);
    qb.param_opt("has", params.has);
    qb.param_opt("min_id", params.min_id);
    qb.param_opt("max_id", params.max_id);

    Ok(qb.build_with_path(&base))
}

/// Собрать URL пути для поиска сообщений в канале (`GET /channels/{id}/messages/search`).
pub fn build_channel_search_path(
    channel_id: &str,
    params: &MessageSearchParams<'_>,
) -> Result<String, QueryError> {
    let spec =
        find(HttpMethod::Get, CHANNEL_SEARCH_PATH).expect("channel search route in extensions");
    let base = render_path(spec, &[channel_id])?;

    let mut qb = QueryBuilder::new();
    qb.param_opt("content", params.content);
    qb.param_opt("sort_by", params.sort_by);
    qb.param_opt("sort_order", params.sort_order);
    qb.param_opt("search_session_id", params.search_session_id);
    qb.param_opt("search_query_id", params.search_query_id);
    qb.param_display_opt("offset", params.offset);
    qb.param_opt("author_id", params.author_id);
    qb.param_opt("has", params.has);
    qb.param_opt("min_id", params.min_id);
    qb.param_opt("max_id", params.max_id);

    Ok(qb.build_with_path(&base))
}

/// Собрать URL пути для выделения слотов загрузки вложений (`POST /channels/{id}/attachments`).
///
/// Валидирует `channel_id` как непустой положительный Snowflake (`1..=u64::MAX`, только ASCII-цифры).
pub fn build_channel_attachments_path(channel_id: &str) -> Result<String, QueryError> {
    let spec = attachment_upload_route_spec();
    let base = render_path(spec, &[channel_id])?;
    if !channel_id.bytes().all(|b| b.is_ascii_digit())
        || channel_id.parse::<u64>().map_or(true, |v| v == 0)
    {
        return Err(QueryError::InvalidParamValue {
            param: "channel_id",
            reason: format!(
                "channel_id must be a valid positive numeric snowflake, got '{channel_id}'"
            ),
        });
    }
    Ok(base)
}

/// Шаблон маршрута сообщений канала (`GET`/`POST /channels/{id}/messages`).
pub const CHANNEL_MESSAGES_PATH: &str = "/channels/{id}/messages";

/// Дефолтный лимит выборки истории сообщений канала (`?limit=50`).
pub const DEFAULT_MESSAGES_HISTORY_LIMIT: u32 = 50;

/// Максимальный лимит выборки истории сообщений канала (`?limit=100`).
pub const MAX_MESSAGES_HISTORY_LIMIT: u32 = 100;

/// Возвращает статическую спецификацию маршрута отправки сообщения (`POST /channels/{id}/messages`).
#[must_use]
pub fn channel_messages_post_route_spec() -> &'static RouteSpec {
    &ROUTES[0]
}

/// Возвращает статическую спецификацию маршрута истории сообщений (`GET /channels/{id}/messages`).
#[must_use]
pub fn channel_messages_get_route_spec() -> &'static RouteSpec {
    &ROUTES[1]
}

fn validate_channel_snowflake(channel_id: &str) -> Result<(), QueryError> {
    if channel_id.is_empty()
        || !channel_id.bytes().all(|b| b.is_ascii_digit())
        || channel_id.parse::<u64>().map_or(true, |v| v == 0)
    {
        return Err(QueryError::InvalidParamValue {
            param: "channel_id",
            reason: format!(
                "channel_id must be a valid positive numeric snowflake, got '{channel_id}'"
            ),
        });
    }
    Ok(())
}

/// Собрать URL пути для отправки сообщения в канал (`POST /channels/{id}/messages`).
///
/// Валидирует `channel_id` как непустой положительный Snowflake (`1..=u64::MAX`, только ASCII-цифры).
pub fn build_channel_messages_send_path(channel_id: &str) -> Result<String, QueryError> {
    let spec = channel_messages_post_route_spec();
    let base = render_path(spec, &[channel_id])?;
    validate_channel_snowflake(channel_id)?;
    Ok(base)
}

/// Собрать URL пути для выборки истории сообщений канала (`GET /channels/{id}/messages?limit={limit}&before={before}`).
///
/// По умолчанию использует `limit = 50` ([`DEFAULT_MESSAGES_HISTORY_LIMIT`]); проверяет `channel_id`
/// и опциональный курсор `before` как положительные Snowflake (`1..=u64::MAX`), а `limit` — в диапазоне `1..=100`.
pub fn build_channel_history_path(
    channel_id: &str,
    before: Option<&str>,
    limit: Option<u32>,
) -> Result<String, QueryError> {
    validate_channel_snowflake(channel_id)?;
    let effective_limit = limit.unwrap_or(DEFAULT_MESSAGES_HISTORY_LIMIT);
    let params = ChannelMessagesParams {
        limit: Some(effective_limit),
        before,
        after: None,
        around: None,
    };
    build_channel_messages_path(channel_id, &params)
}
