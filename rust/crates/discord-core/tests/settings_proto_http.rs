//! BE-P2-11: settings-proto lossless wire encode and REST GET/PATCH /users/@me/settings-proto/{type}.

use std::time::Duration;

use discord_core::rest::base64;
use discord_core::rest::routes::HttpMethod;
use discord_core::settings::{
    build_get_settings_proto_request, build_patch_settings_proto_body,
    parse_settings_proto_response, settings_proto_path, ProtoField, ProtoMessage, ProtoValue,
    SettingsProto, SettingsProtoError, SettingsProtoHttpError,
    FRECENCY_AND_FAVORITES_SETTINGS_TYPE, PRELOADED_USER_SETTINGS_TYPE,
};

const READY: &str = include_str!("data/settings-preloaded-ready-4400.b64");
const UPDATE_8269: &str = include_str!("data/settings-frecency-update-8269.b64");
const UPDATE_8608: &str = include_str!("data/settings-frecency-update-8608.b64");
const UPDATE_13072: &str = include_str!("data/settings-frecency-update-13072.b64");

#[test]
fn t01_wire_encode_roundtrip_all_wire_types() {
    let mut msg = ProtoMessage { fields: Vec::new() };
    msg.fields.push(ProtoField {
        number: 1,
        value: ProtoValue::Varint(42),
    });
    msg.fields.push(ProtoField {
        number: 2,
        value: ProtoValue::Fixed64(0x0123_4567_89AB_CDEF),
    });
    msg.fields.push(ProtoField {
        number: 3,
        value: ProtoValue::LengthDelimited(vec![0xAA, 0xBB, 0xCC, 0xDD]),
    });
    msg.fields.push(ProtoField {
        number: 4,
        value: ProtoValue::Fixed32(0xCAFE_BABE),
    });

    let encoded = msg.encode();
    let decoded = ProtoMessage::decode(&encoded).expect("decode valid wire bytes");
    assert_eq!(decoded.fields, msg.fields);
    assert_eq!(decoded.encode(), encoded);
}

#[test]
fn t02_wire_encode_roundtrip_unknown_fields() {
    let mut msg = ProtoMessage { fields: Vec::new() };
    msg.fields.push(ProtoField {
        number: 9999,
        value: ProtoValue::Varint(9876543210),
    });
    msg.fields.push(ProtoField {
        number: 10000,
        value: ProtoValue::LengthDelimited(b"unknown_custom_wire_payload".to_vec()),
    });

    let encoded = msg.encode();
    let decoded = ProtoMessage::decode(&encoded).expect("decode unknown fields");
    assert_eq!(decoded.fields.len(), 2);
    assert_eq!(decoded.scalar_u64(9999).unwrap(), Some(9876543210));
    assert_eq!(
        decoded.length_delimited(10000).unwrap(),
        Some(b"unknown_custom_wire_payload".as_slice())
    );
    assert_eq!(decoded.encode(), encoded);
}

#[test]
fn t03_set_field_replaces_or_appends() {
    let mut msg = ProtoMessage { fields: Vec::new() };

    // 1. Append new field
    msg.set_field(10, ProtoValue::Varint(100));
    assert_eq!(msg.fields.len(), 1);
    assert_eq!(msg.scalar_u64(10).unwrap(), Some(100));

    // 2. Append another field
    msg.set_field(20, ProtoValue::Fixed32(555));
    assert_eq!(msg.fields.len(), 2);
    assert_eq!(msg.field(20).unwrap().value, ProtoValue::Fixed32(555));

    // 3. Replace existing field
    msg.set_field(10, ProtoValue::Varint(200));
    assert_eq!(msg.fields.len(), 2);
    assert_eq!(msg.scalar_u64(10).unwrap(), Some(200));

    // 4. Duplicate cleanup on replace
    let mut dup_msg = ProtoMessage {
        fields: vec![
            ProtoField {
                number: 1,
                value: ProtoValue::Varint(1),
            },
            ProtoField {
                number: 2,
                value: ProtoValue::Varint(2),
            },
            ProtoField {
                number: 1,
                value: ProtoValue::Varint(3),
            },
        ],
    };
    dup_msg.set_field(1, ProtoValue::Varint(99));
    assert_eq!(dup_msg.fields.len(), 2);
    assert_eq!(dup_msg.scalar_u64(1).unwrap(), Some(99));
    assert_eq!(dup_msg.scalar_u64(2).unwrap(), Some(2));
}

#[test]
fn t04_fixtures_lossless_roundtrip_bytes_and_base64() {
    let fixtures = [
        (PRELOADED_USER_SETTINGS_TYPE, READY.trim()),
        (FRECENCY_AND_FAVORITES_SETTINGS_TYPE, UPDATE_8269.trim()),
        (FRECENCY_AND_FAVORITES_SETTINGS_TYPE, UPDATE_8608.trim()),
        (FRECENCY_AND_FAVORITES_SETTINGS_TYPE, UPDATE_13072.trim()),
    ];

    for (proto_type, b64_text) in fixtures {
        let raw_bytes = base64::decode(b64_text).expect("decode base64");
        let parsed = SettingsProto::parse(proto_type, &raw_bytes).expect("parse SettingsProto");

        let encoded_bytes = parsed.encode_bytes();
        assert_eq!(
            encoded_bytes, raw_bytes,
            "bytes roundtrip failed for proto_type {proto_type}"
        );

        let encoded_b64 = parsed.encode_base64();
        assert_eq!(
            encoded_b64, b64_text,
            "base64 roundtrip failed for proto_type {proto_type}"
        );
    }
}

#[test]
fn t05_settings_proto_path_validation() {
    assert_eq!(
        settings_proto_path(1).unwrap(),
        "/users/@me/settings-proto/1"
    );
    assert_eq!(
        settings_proto_path(2).unwrap(),
        "/users/@me/settings-proto/2"
    );

    assert_eq!(
        settings_proto_path(0).unwrap_err(),
        SettingsProtoHttpError::UnsupportedProtoType(0)
    );
    assert_eq!(
        settings_proto_path(3).unwrap_err(),
        SettingsProtoHttpError::UnsupportedProtoType(3)
    );
    assert_eq!(
        settings_proto_path(999).unwrap_err(),
        SettingsProtoHttpError::UnsupportedProtoType(999)
    );
}

#[test]
fn t06_build_get_settings_proto_request() {
    let req1 = build_get_settings_proto_request(1).expect("request for type 1");
    assert_eq!(req1.method, HttpMethod::Get);
    assert_eq!(req1.target, "/users/@me/settings-proto/1");
    assert!(req1.body.is_empty());

    let req2 = build_get_settings_proto_request(2).expect("request for type 2");
    assert_eq!(req2.method, HttpMethod::Get);
    assert_eq!(req2.target, "/users/@me/settings-proto/2");
    assert!(req2.body.is_empty());

    let err = build_get_settings_proto_request(0).unwrap_err();
    assert_eq!(err, SettingsProtoHttpError::UnsupportedProtoType(0));
}

#[test]
fn t07_build_patch_settings_proto_body() {
    let valid_b64 = READY.trim();

    // 1. Without required_data_version
    let body_none =
        build_patch_settings_proto_body(valid_b64, None).expect("patch body without version");
    let json_str_none = std::str::from_utf8(&body_none).expect("utf-8");
    assert_eq!(json_str_none, format!("{{\"settings\":\"{valid_b64}\"}}"));

    // 2. With required_data_version
    let body_some =
        build_patch_settings_proto_body(valid_b64, Some(3027)).expect("patch body with version");
    let json_str_some = std::str::from_utf8(&body_some).expect("utf-8");
    assert_eq!(
        json_str_some,
        format!("{{\"required_data_version\":3027,\"settings\":\"{valid_b64}\"}}")
    );

    // 3. Invalid base64 rejects before JSON
    let err_b64 = build_patch_settings_proto_body("not-valid-b64!@#$", None).unwrap_err();
    match err_b64 {
        SettingsProtoHttpError::Proto(SettingsProtoError::Base64(_)) => {}
        other => panic!("expected Base64 error, got {other:?}"),
    }

    // 4. Valid base64 but invalid protobuf wire (truncated varint 0x80)
    let bad_wire = base64::encode(&[0x80]);
    let err_wire = build_patch_settings_proto_body(&bad_wire, None).unwrap_err();
    match err_wire {
        SettingsProtoHttpError::Proto(SettingsProtoError::UnexpectedEnd { .. }) => {}
        other => panic!("expected UnexpectedEnd proto error, got {other:?}"),
    }
}

#[test]
fn t08_parse_settings_proto_response_200_ok() {
    let json_body = format!("{{\"settings\":\"{}\"}}", READY.trim()).into_bytes();
    let proto = parse_settings_proto_response(1, 200, &[], &json_body).expect("parse 200 response");
    assert_eq!(proto.proto_type(), 1);
    let SettingsProto::Preloaded(p) = proto else {
        panic!("expected preloaded");
    };
    assert_eq!(p.raw.fields.len(), 18);

    // Missing "settings" field
    let bad_json = b"{\"other_field\":123}";
    let err = parse_settings_proto_response(1, 200, &[], bad_json).unwrap_err();
    assert!(matches!(err, SettingsProtoHttpError::InvalidResponse(_)));

    // Non-JSON body
    let not_json = b"<html>502 Bad Gateway</html>";
    let err2 = parse_settings_proto_response(1, 200, &[], not_json).unwrap_err();
    assert!(matches!(err2, SettingsProtoHttpError::InvalidResponse(_)));
}

#[test]
fn t09_parse_settings_proto_response_errors() {
    // 401 Unauthorized
    let err_401 =
        parse_settings_proto_response(1, 401, &[], b"{\"message\":\"401: Unauthorized\"}")
            .unwrap_err();
    assert_eq!(err_401, SettingsProtoHttpError::Unauthorized);

    // 429 RateLimited
    let headers = vec![
        ("retry-after".to_string(), "2.5".to_string()),
        ("x-ratelimit-global".to_string(), "true".to_string()),
    ];
    let err_429 = parse_settings_proto_response(
        1,
        429,
        &headers,
        b"{\"message\":\"You are being rate limited.\"}",
    )
    .unwrap_err();
    match err_429 {
        SettingsProtoHttpError::RateLimited(info) => {
            assert_eq!(info.retry_after, Duration::from_millis(2500));
            assert!(info.global);
        }
        other => panic!("expected RateLimited, got {other:?}"),
    }

    // 5xx Server errors
    let err_500 = parse_settings_proto_response(1, 500, &[], b"Internal Error").unwrap_err();
    assert_eq!(err_500, SettingsProtoHttpError::Server { status: 500 });
    let err_503 = parse_settings_proto_response(2, 503, &[], b"Service Unavailable").unwrap_err();
    assert_eq!(err_503, SettingsProtoHttpError::Server { status: 503 });

    // 4xx Client errors
    let err_400 = parse_settings_proto_response(1, 400, &[], b"Bad Request").unwrap_err();
    assert_eq!(err_400, SettingsProtoHttpError::Client { status: 400 });
    let err_403 = parse_settings_proto_response(2, 403, &[], b"Forbidden").unwrap_err();
    assert_eq!(err_403, SettingsProtoHttpError::Client { status: 403 });
    let err_404 = parse_settings_proto_response(1, 404, &[], b"Not Found").unwrap_err();
    assert_eq!(err_404, SettingsProtoHttpError::Client { status: 404 });
}
