//! Каталог REST-маршрутов v9 (T-REST-01, волна RW-08 / P1-07).
//!
//! Публичные типы и сигнатуры — замороженный контракт (`waves/RW-08/TZ.md` §4.1). Модуль
//! хранит 47 маршрутов живого захвата D-02, ищет их по точной паре «метод, шаблон» и
//! подставляет части в плейсхолдеры `{id}`/`{num}`.
//!
//! Оракул: `docs/re/rest-d02.json` (md5 `87d7a05d8cd11f3b8ae37aa5c9811c9e`) — 47 живых
//! маршрутов; выдержка для тестов — `tests/data/rest-routes-47.tsv` (47 строк).

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

/// Маршрут: метод, шаблон пути (относительный, без базы `/api/v9`) и ключи тела запроса.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RouteSpec {
    /// HTTP-метод запроса.
    pub method: HttpMethod,
    /// Шаблон пути относительно базы (без `/api/v9`).
    pub path: &'static str,
    /// Ключи тела запроса дословно из оракула; пусто — тела нет.
    pub body_keys: &'static [&'static str],
}

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
    }
}

/// 47 маршрутов из захвата D-02 (`docs/re/rest-d02.json`).
///
/// Порядок совпадает с порядком выдержки `tests/data/rest-routes-47.tsv` (порядок захвата);
/// пути относительные (без `/api/v9`); `body_keys` — дословно из оракула, пусто = тела нет.
pub const ROUTES: [RouteSpec; 47] = [
    route(HttpMethod::Get, "/channels/{id}/messages", &[]),
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
    route(HttpMethod::Get, "/channels/{id}/messages/pins", &[]),
    route(HttpMethod::Get, "/channels/{id}/threads/search", &[]),
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
pub fn find(method: HttpMethod, path: &str) -> Option<&'static RouteSpec> {
    ROUTES
        .iter()
        .find(|spec| spec.method == method && spec.path == path)
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

/// Плейсхолдеры шаблонов пути (набор зафиксирован оракулом D-02).
const PLACEHOLDERS: [&str; 2] = ["{id}", "{num}"];

/// Число плейсхолдеров `{id}`/`{num}` в шаблоне.
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
