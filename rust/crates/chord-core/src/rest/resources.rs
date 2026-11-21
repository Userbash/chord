//! Read-only Discord REST resources confirmed by D-02.

use std::fmt;

use crate::API_V9_BASE;

use super::headers::AUTH_HEADER_NAMES;
use super::routes::{
    self, CAPTURED_D02_ROUTE_COUNT, CHANNEL_ATTACHMENTS_PATH, CHANNEL_SEARCH_PATH, HttpMethod,
    INVITE_PATH, ROUTES, RouteSpec, STATIC_EXTENSION_ROUTE_COUNT, TYPING_PATH,
};
use super::super_properties::field_names;

/// Supported read-only resource categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResourceKind {
    Quest,
    QuestDecision,
    QuestGetDecisions,
    Promotion,
    Entitlements,
    ApplicationEntitlements,
    Library,
    SavedMessages,
}

/// A confirmed GET resource and its observed response statuses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReadOnlyResource {
    pub kind: ResourceKind,
    pub path_template: &'static str,
    pub observed_statuses: &'static [u16],
}

/// Confirmed read-only resources from the D-02 capture.
pub const RESOURCES: [ReadOnlyResource; 8] = [
    ReadOnlyResource {
        kind: ResourceKind::Quest,
        path_template: "/quests/@me",
        observed_statuses: &[200],
    },
    ReadOnlyResource {
        kind: ResourceKind::QuestDecision,
        path_template: "/quests/decision",
        observed_statuses: &[200],
    },
    ReadOnlyResource {
        kind: ResourceKind::QuestGetDecisions,
        path_template: "/quests/get-decisions",
        observed_statuses: &[200],
    },
    ReadOnlyResource {
        kind: ResourceKind::Promotion,
        path_template: "/promotions",
        observed_statuses: &[200],
    },
    ReadOnlyResource {
        kind: ResourceKind::Entitlements,
        path_template: "/users/@me/entitlements",
        observed_statuses: &[200],
    },
    ReadOnlyResource {
        kind: ResourceKind::ApplicationEntitlements,
        path_template: "/users/@me/applications/{id}/entitlements",
        observed_statuses: &[200],
    },
    ReadOnlyResource {
        kind: ResourceKind::Library,
        path_template: "/users/@me/library",
        observed_statuses: &[200],
    },
    ReadOnlyResource {
        kind: ResourceKind::SavedMessages,
        path_template: "/users/@me/saved-messages",
        observed_statuses: &[404],
    },
];

/// Failure while validating or rendering a resource path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceError {
    MissingId,
    InvalidId,
    UnexpectedId,
    RouteNotFound,
    RouteRender,
}

impl fmt::Display for ResourceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::MissingId => "missing application id",
            Self::InvalidId => "invalid application id",
            Self::UnexpectedId => "unexpected application id",
            Self::RouteNotFound => "resource GET route not found",
            Self::RouteRender => "resource path could not be rendered",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for ResourceError {}

/// Returns the static catalog entry for `kind`.
#[must_use]
pub fn resource(kind: ResourceKind) -> &'static ReadOnlyResource {
    match kind {
        ResourceKind::Quest => &RESOURCES[0],
        ResourceKind::QuestDecision => &RESOURCES[1],
        ResourceKind::QuestGetDecisions => &RESOURCES[2],
        ResourceKind::Promotion => &RESOURCES[3],
        ResourceKind::Entitlements => &RESOURCES[4],
        ResourceKind::ApplicationEntitlements => &RESOURCES[5],
        ResourceKind::Library => &RESOURCES[6],
        ResourceKind::SavedMessages => &RESOURCES[7],
    }
}

/// Builds a relative route path, validating the sole supported path parameter.
pub fn path(kind: ResourceKind, application_id: Option<&str>) -> Result<String, ResourceError> {
    let entry = resource(kind);
    let parts = match (kind, application_id) {
        (ResourceKind::ApplicationEntitlements, None) => return Err(ResourceError::MissingId),
        (ResourceKind::ApplicationEntitlements, Some(id)) => {
            if id.is_empty() || id.len() > 20 || !id.bytes().all(|byte| byte.is_ascii_digit()) {
                return Err(ResourceError::InvalidId);
            }
            vec![id]
        }
        (_, Some(_)) => return Err(ResourceError::UnexpectedId),
        (_, None) => Vec::new(),
    };

    let route =
        routes::find(HttpMethod::Get, entry.path_template).ok_or(ResourceError::RouteNotFound)?;
    routes::render_path(route, &parts).map_err(|_| ResourceError::RouteRender)
}

/// Functional domain classification of a REST API v9 endpoint in the D-02 catalog.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RestRouteDomain {
    /// User identity, `/users/@me`, MFA WebAuthn credentials, and session/auth endpoints.
    Authentication,
    /// Message history, message creation, message ACK, reactions, and pinned messages.
    Messaging,
    /// Thread search, guild message search, channel message search, and autocomplete search.
    Search,
    /// Channel-scoped operations (`/channels/{id}/*`).
    Channels,
    /// Guild-scoped operations (`/guilds/{id}/*`, `/guilds`).
    Guilds,
    /// Client telemetry endpoints (`/beaker`, `/science`).
    Telemetry,
    /// User profile, billing, quests, entitlements, storefront, and ecosystem resources.
    Resources,
}

/// Aggregate summary of the D-02 REST API v9 route catalog for the Network BFF.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RestDomainCatalogSummary {
    /// Canonical REST API v9 base URL (`https://discord.com/api/v9`).
    pub api_base: &'static str,
    /// Number of empirically captured D-02 routes (47).
    pub captured_d02_routes: usize,
    /// Number of static extension routes inside `ROUTES` (2).
    pub static_extension_routes: usize,
    /// Number of mandatory authentication headers in the request envelope (6).
    pub auth_header_count: usize,
    /// Number of fields in `X-Super-Properties` (22).
    pub super_properties_field_count: usize,
    /// Number of D-02 routes carrying a request body schema (6).
    pub d02_payload_route_count: usize,
    /// Number of routes covering authentication and user identity.
    pub authentication_routes: usize,
    /// Number of routes covering messaging operations.
    pub messaging_routes: usize,
    /// Number of routes covering search and autocomplete operations.
    pub search_routes: usize,
    /// Number of routes targeting `/channels/{id}/*`.
    pub channel_routes: usize,
    /// Number of routes targeting `/guilds/{id}/*`.
    pub guild_routes: usize,
}

/// All catalogued route specs combining `ROUTES` (49) and the 5 `EXTENSIONS` resolved via `routes::find`.
fn all_catalog_routes() -> Vec<&'static RouteSpec> {
    let mut all: Vec<&'static RouteSpec> = ROUTES.iter().collect();
    for (method, ext_path) in [
        (HttpMethod::Post, TYPING_PATH),
        (HttpMethod::Get, INVITE_PATH),
        (HttpMethod::Post, INVITE_PATH),
        (HttpMethod::Get, CHANNEL_SEARCH_PATH),
        (HttpMethod::Post, CHANNEL_ATTACHMENTS_PATH),
    ] {
        if let Some(spec) = routes::find(method, ext_path) {
            all.push(spec);
        }
    }
    all
}

/// Classifies a REST v9 route `(method, path)` into its primary [`RestRouteDomain`].
#[must_use]
pub fn classify_route_domain(_method: HttpMethod, path: &str) -> RestRouteDomain {
    if path == "/users/@me"
        || path.starts_with("/users/@me/mfa/")
        || path.starts_with("/auth/")
        || path == "/users/@me/auth/sessions"
    {
        RestRouteDomain::Authentication
    } else if path.ends_with("/search") || path == "/games/autocomplete" {
        RestRouteDomain::Search
    } else if path.starts_with("/channels/{id}/messages") {
        RestRouteDomain::Messaging
    } else if path.starts_with("/channels/") {
        RestRouteDomain::Channels
    } else if path.starts_with("/guilds") {
        RestRouteDomain::Guilds
    } else if path == "/beaker" || path == "/science" {
        RestRouteDomain::Telemetry
    } else {
        RestRouteDomain::Resources
    }
}

/// Returns all catalogued [`RouteSpec`] entries matching `domain`.
///
/// Note: `RestRouteDomain::Channels` includes all `/channels/*` endpoints (including messaging
/// and channel/thread search), and `RestRouteDomain::Guilds` includes all `/guilds/*` endpoints
/// (including guild search), while `classify_route_domain` assigns the most specific domain.
#[must_use]
pub fn routes_by_domain(domain: RestRouteDomain) -> Vec<&'static RouteSpec> {
    all_catalog_routes()
        .into_iter()
        .filter(|spec| match domain {
            RestRouteDomain::Channels => spec.path.starts_with("/channels/"),
            RestRouteDomain::Guilds => spec.path.starts_with("/guilds/"),
            other => classify_route_domain(spec.method, spec.path) == other,
        })
        .collect()
}

/// Computes the D-02 REST API v9 catalog summary across authentication, messaging, search,
/// channels, guilds, request payload schemas, and auth headers.
#[must_use]
pub fn d02_domain_catalog_summary() -> RestDomainCatalogSummary {
    let d02_payload_route_count = ROUTES
        .iter()
        .filter(|spec| !spec.is_static_extension() && !spec.body_keys.is_empty())
        .count();

    RestDomainCatalogSummary {
        api_base: API_V9_BASE,
        captured_d02_routes: CAPTURED_D02_ROUTE_COUNT,
        static_extension_routes: STATIC_EXTENSION_ROUTE_COUNT,
        auth_header_count: AUTH_HEADER_NAMES.len(),
        super_properties_field_count: field_names().len(),
        d02_payload_route_count,
        authentication_routes: routes_by_domain(RestRouteDomain::Authentication).len(),
        messaging_routes: routes_by_domain(RestRouteDomain::Messaging).len(),
        search_routes: routes_by_domain(RestRouteDomain::Search).len(),
        channel_routes: routes_by_domain(RestRouteDomain::Channels).len(),
        guild_routes: routes_by_domain(RestRouteDomain::Guilds).len(),
    }
}
