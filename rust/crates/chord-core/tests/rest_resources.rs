use chord_core::rest::resources::{RESOURCES, ResourceError, ResourceKind, path, resource};
use chord_core::rest::routes::{self, HttpMethod};
use chord_json::JsonValue;
use std::path::Path;

const EXPECTED: [(ResourceKind, &str, &[u16]); 8] = [
    (ResourceKind::Quest, "/quests/@me", &[200]),
    (ResourceKind::QuestDecision, "/quests/decision", &[200]),
    (
        ResourceKind::QuestGetDecisions,
        "/quests/get-decisions",
        &[200],
    ),
    (ResourceKind::Promotion, "/promotions", &[200]),
    (
        ResourceKind::Entitlements,
        "/users/@me/entitlements",
        &[200],
    ),
    (
        ResourceKind::ApplicationEntitlements,
        "/users/@me/applications/{id}/entitlements",
        &[200],
    ),
    (ResourceKind::Library, "/users/@me/library", &[200]),
    (
        ResourceKind::SavedMessages,
        "/users/@me/saved-messages",
        &[404],
    ),
];

#[test]
fn catalog_matches_d02_and_get_routes() {
    assert_eq!(RESOURCES.len(), EXPECTED.len());
    for (kind, template, statuses) in EXPECTED {
        let record = resource(kind);
        assert_eq!(record.kind, kind);
        assert_eq!(record.path_template, template);
        assert_eq!(record.observed_statuses, statuses);
        let matching_routes: Vec<_> = routes::ROUTES
            .iter()
            .filter(|route| route.path == template)
            .collect();
        assert_eq!(matching_routes.len(), 1);
        assert_eq!(matching_routes[0].method, HttpMethod::Get);
        assert!(!record.path_template.contains('?'));
        assert!(!record.path_template.contains("://"));
    }

    for (index, left) in RESOURCES.iter().enumerate() {
        assert!(
            !RESOURCES[index + 1..]
                .iter()
                .any(|right| right.kind == left.kind)
        );
        assert!(matches!(
            left.kind,
            ResourceKind::Quest
                | ResourceKind::QuestDecision
                | ResourceKind::QuestGetDecisions
                | ResourceKind::Promotion
                | ResourceKind::Entitlements
                | ResourceKind::ApplicationEntitlements
                | ResourceKind::Library
                | ResourceKind::SavedMessages
        ));
    }
}

#[test]
fn path_renders_only_the_application_id() {
    assert_eq!(
        path(
            ResourceKind::ApplicationEntitlements,
            Some("12345678901234567890")
        ),
        Ok("/users/@me/applications/12345678901234567890/entitlements".to_owned())
    );
    for (kind, template, _) in EXPECTED {
        if kind == ResourceKind::ApplicationEntitlements {
            continue;
        }
        assert_eq!(path(kind, None), Ok(template.to_owned()));
        assert_eq!(path(kind, Some("1")), Err(ResourceError::UnexpectedId));
    }
    for invalid in ["", "abc", "1/2", "123456789012345678901", "１２"] {
        assert_eq!(
            path(ResourceKind::ApplicationEntitlements, Some(invalid)),
            Err(ResourceError::InvalidId)
        );
    }
    assert_eq!(
        path(ResourceKind::ApplicationEntitlements, None),
        Err(ResourceError::MissingId)
    );
}

#[test]
fn saved_messages_404_is_not_a_success_payload() {
    assert_eq!(
        resource(ResourceKind::SavedMessages).observed_statuses,
        &[404]
    );
    assert_eq!(resource(ResourceKind::Quest).observed_statuses, &[200]);
    assert!(path(ResourceKind::SavedMessages, None).is_ok());
}

#[test]
fn errors_are_stable_and_do_not_disclose_input() {
    for (error, expected) in [
        (ResourceError::MissingId, "missing application id"),
        (ResourceError::InvalidId, "invalid application id"),
        (ResourceError::UnexpectedId, "unexpected application id"),
    ] {
        assert_eq!(error.to_string(), expected);
        assert!(!error.to_string().contains("secret"));
    }
}

#[test]
fn catalog_matches_independent_d02_json_oracle() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let oracle_path = root.join("docs/re/rest-d02.json");
    let text = std::fs::read_to_string(&oracle_path)
        .unwrap_or_else(|error| panic!("нет D-02 оракула {}: {error}", oracle_path.display()));
    let oracle = chord_json::parse(&text).expect("D-02 JSON");
    let endpoints = oracle
        .get("endpoints")
        .and_then(JsonValue::as_array)
        .expect("endpoints");
    for (kind, template, expected_statuses) in EXPECTED {
        let absolute = format!("/api/v9{template}");
        let endpoint = endpoints
            .iter()
            .find(|endpoint| endpoint.get("path").and_then(JsonValue::as_str) == Some(&absolute))
            .unwrap_or_else(|| panic!("D-02 не содержит {absolute} для {kind:?}"));
        assert_eq!(
            endpoint.get("method").and_then(JsonValue::as_str),
            Some("GET"),
            "{absolute}"
        );
        let statuses: Vec<u16> = endpoint
            .get("statuses")
            .and_then(JsonValue::as_array)
            .expect("statuses")
            .iter()
            .map(|status| status.as_u64().expect("numeric status") as u16)
            .collect();
        assert_eq!(statuses, expected_statuses, "{absolute}");
    }
}

#[test]
fn catalog_and_paths_are_deterministic() {
    assert!(!RESOURCES.is_empty());
    for (kind, _, _) in EXPECTED {
        assert_eq!(resource(kind), resource(kind));
        assert_eq!(
            path(
                kind,
                if kind == ResourceKind::ApplicationEntitlements {
                    Some("42")
                } else {
                    None
                }
            ),
            path(
                kind,
                if kind == ResourceKind::ApplicationEntitlements {
                    Some("42")
                } else {
                    None
                }
            )
        );
    }
}
