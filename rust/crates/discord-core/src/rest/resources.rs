//! Read-only Discord REST resources confirmed by D-02.

use std::fmt;

use super::routes::{self, HttpMethod};

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
