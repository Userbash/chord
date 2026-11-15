//! Native Tauri jobs exercise the actual core OS entropy implementation.
//! These checks require fresh identities; the process-wide cached identity is stable by design.

use chord_core::rest::identity::{uuid_v4, ClientIdentity, IdentityConfig, SystemSources};
use std::collections::HashSet;

fn assert_uuid_shape(value: &str) {
    let groups: Vec<&str> = value.split('-').collect();
    assert_eq!(
        groups.iter().map(|group| group.len()).collect::<Vec<_>>(),
        [8, 4, 4, 4, 12]
    );
    assert_eq!(value.len(), 36);
    assert!(groups.iter().all(|group| group
        .bytes()
        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))));
    assert_eq!(groups[2].as_bytes()[0], b'4');
    assert!(matches!(groups[3].as_bytes()[0], b'8' | b'9' | b'a' | b'b'));
    assert_ne!(value, "00000000-0000-4000-8000-000000000000");
}

#[test]
fn fresh_system_uuids_are_valid_and_distinct() {
    let sources = SystemSources::new();
    let mut seen = HashSet::new();
    for _ in 0..256 {
        let value = uuid_v4(&sources);
        assert_uuid_shape(&value);
        assert!(
            seen.insert(value),
            "fresh OS entropy repeated an identity UUID"
        );
    }
    assert_eq!(seen.len(), 256);
}

#[test]
fn fresh_identity_builds_have_fresh_telemetry_ids() {
    let sources = SystemSources::new();
    let mut seen = HashSet::new();
    for _ in 0..64 {
        let identity = ClientIdentity::build(&sources, &IdentityConfig::default());
        let properties = identity.super_properties();
        for value in [
            &properties.client_launch_id,
            &properties.launch_signature,
            &properties.client_heartbeat_session_id,
        ] {
            assert_uuid_shape(value);
            assert!(
                seen.insert(value.clone()),
                "fresh identity reused a telemetry UUID"
            );
        }
    }
    assert_eq!(seen.len(), 192);
}

#[test]
fn cached_session_identity_keeps_its_original_telemetry_ids() {
    let first = ClientIdentity::rw_client();
    for _ in 0..8 {
        assert_eq!(ClientIdentity::rw_client(), first);
    }
}
