#![forbid(unsafe_code)]

#[path = "../src/auth/token_store.rs"]
mod token_store;

use token_store::{
    DEFAULT_TOKEN_KEY, MemoryKeyringBackend, SecretBackend, SecretToken, SystemKeyringBackend,
    TokenStore, TokenStoreError,
};

#[test]
fn t01_secret_token_validation_and_trimming() {
    // Valid token with surrounding whitespace gets trimmed
    let token = SecretToken::new("  mfa.synthetic_token_abc123_xyz789  \t").expect("valid token");
    assert_eq!(token.expose_secret(), "mfa.synthetic_token_abc123_xyz789");

    // Direct valid token
    let token2 = SecretToken::new("MTIzNDU2Nzg5MDEyMzQ1Njc4.G1a2b3.c4d5e6f7g8h9").expect("valid");
    assert_eq!(
        token2.expose_secret(),
        "MTIzNDU2Nzg5MDEyMzQ1Njc4.G1a2b3.c4d5e6f7g8h9"
    );

    // Reject empty and whitespace-only
    assert_eq!(SecretToken::new(""), Err(TokenStoreError::InvalidToken));
    assert_eq!(
        SecretToken::new("   \t  "),
        Err(TokenStoreError::InvalidToken)
    );
    assert_eq!(
        SecretToken::new("\n\r\t "),
        Err(TokenStoreError::InvalidToken)
    );

    // Reject interior NUL, CR, LF even if outer whitespace is trimmed
    assert_eq!(
        SecretToken::new("token\0with_nul"),
        Err(TokenStoreError::InvalidToken)
    );
    assert_eq!(
        SecretToken::new("token\nwith_lf"),
        Err(TokenStoreError::InvalidToken)
    );
    assert_eq!(
        SecretToken::new("token\rwith_cr"),
        Err(TokenStoreError::InvalidToken)
    );
    assert_eq!(
        SecretToken::new("  token\nmiddle  "),
        Err(TokenStoreError::InvalidToken)
    );
    assert_eq!(
        SecretToken::new("  token\rmiddle  "),
        Err(TokenStoreError::InvalidToken)
    );
}

#[test]
fn t02_secret_token_debug_and_display_redaction_never_leaks() {
    let raw_secret = "mfa.TOP_SECRET_NEVER_LEAK_THIS_VALUE_987654321";
    let token = SecretToken::new(raw_secret).expect("valid token");

    let debug_str = format!("{token:?}");
    let display_str = format!("{token}");

    assert_eq!(debug_str, "SecretToken([REDACTED])");
    assert_eq!(display_str, "[REDACTED]");
    assert!(!debug_str.contains(raw_secret));
    assert!(!display_str.contains(raw_secret));
    assert!(!debug_str.contains("TOP_SECRET"));
    assert!(!display_str.contains("TOP_SECRET"));

    // Also verify invalid token errors never contain the input string
    let bad_secret = "mfa.BAD_SECRET_WITH_NUL\0LEAK_CHECK";
    let err = SecretToken::new(bad_secret).unwrap_err();
    let err_debug = format!("{err:?}");
    let err_display = format!("{err}");
    assert!(!err_debug.contains("BAD_SECRET"));
    assert!(!err_display.contains("BAD_SECRET"));
}

#[test]
fn t03_active_token_save_load_overwrite_delete_roundtrip() {
    let backend = MemoryKeyringBackend::new();
    assert!(backend.is_empty());
    assert_eq!(backend.len(), 0);
    let mut store = TokenStore::new(backend);

    assert_eq!(DEFAULT_TOKEN_KEY, "chord.active_account");

    // Initially missing -> Ok(None)
    assert_eq!(store.load_active_token(), Ok(None));
    assert_eq!(store.delete_active_token(), Ok(false));

    // Save first token
    let tok1 = SecretToken::new("mfa.first_active_token_111").expect("valid");
    store
        .save_active_token(&tok1)
        .expect("save active token succeeds");
    let loaded1 = store
        .load_active_token()
        .expect("load succeeds")
        .expect("token present");
    assert_eq!(loaded1.expose_secret(), "mfa.first_active_token_111");
    assert_eq!(loaded1, tok1);

    // Overwrite with second token
    let tok2 = SecretToken::new("mfa.second_active_token_222").expect("valid");
    store
        .save_active_token(&tok2)
        .expect("overwrite active token succeeds");
    let loaded2 = store
        .load_active_token()
        .expect("load succeeds")
        .expect("token present");
    assert_eq!(loaded2.expose_secret(), "mfa.second_active_token_222");

    // Delete returns true once, then false
    assert_eq!(store.delete_active_token(), Ok(true));
    assert_eq!(store.load_active_token(), Ok(None));
    assert_eq!(store.delete_active_token(), Ok(false));

    let inner = store.into_backend();
    assert!(inner.is_empty());
}

#[test]
fn t04_per_account_token_roundtrip_and_isolation() {
    let mut store = TokenStore::new(MemoryKeyringBackend::new());

    let tok_alice = SecretToken::new("token_alice_001").expect("valid");
    let tok_bob = SecretToken::new("token_bob_002").expect("valid");
    let tok_active = SecretToken::new("token_active_main").expect("valid");

    store
        .save_account_token("123456789012345678", &tok_alice)
        .expect("save alice");
    store
        .save_account_token("user_bob-2.test", &tok_bob)
        .expect("save bob");
    store.save_active_token(&tok_active).expect("save active");

    assert_eq!(
        store.load_account_token("123456789012345678"),
        Ok(Some(tok_alice))
    );
    assert_eq!(
        store.load_account_token("user_bob-2.test"),
        Ok(Some(tok_bob.clone()))
    );
    assert_eq!(store.load_active_token(), Ok(Some(tok_active.clone())));
    assert_eq!(store.load_account_token("missing_account"), Ok(None));

    // Verify underlying key naming in backend ("chord.account.<account_id>" and "chord.active_account")
    assert_eq!(
        store
            .backend()
            .get_secret("chord.account.123456789012345678"),
        Ok(Some("token_alice_001".to_string()))
    );
    assert_eq!(
        store.backend().get_secret(DEFAULT_TOKEN_KEY),
        Ok(Some("token_active_main".to_string()))
    );

    // Deleting Alice does not affect Bob or active token
    assert_eq!(store.delete_account_token("123456789012345678"), Ok(true));
    assert_eq!(store.load_account_token("123456789012345678"), Ok(None));
    assert_eq!(store.delete_account_token("123456789012345678"), Ok(false));
    assert_eq!(
        store.load_account_token("user_bob-2.test"),
        Ok(Some(tok_bob))
    );
    assert_eq!(store.load_active_token(), Ok(Some(tok_active)));
}

#[test]
fn t05_account_id_validation_boundaries() {
    let mut store = TokenStore::new(MemoryKeyringBackend::new());
    let tok = SecretToken::new("valid_secret_token").expect("valid");

    // Boundary: 1 char and 128 chars allowed
    let id_1 = "a";
    let id_128 = "a".repeat(128);
    assert_eq!(store.save_account_token(id_1, &tok), Ok(()));
    assert_eq!(store.save_account_token(&id_128, &tok), Ok(()));
    assert_eq!(store.load_account_token(&id_128), Ok(Some(tok.clone())));

    // Boundary: 129 chars rejected
    let id_129 = "a".repeat(129);
    assert_eq!(
        store.save_account_token(&id_129, &tok),
        Err(TokenStoreError::InvalidAccountId)
    );
    assert_eq!(
        store.load_account_token(&id_129),
        Err(TokenStoreError::InvalidAccountId)
    );
    assert_eq!(
        store.delete_account_token(&id_129),
        Err(TokenStoreError::InvalidAccountId)
    );

    // Invalid account IDs: empty, spaces, slashes, colons, NUL, non-ASCII
    let invalid_ids = [
        "",
        " ",
        "user name",
        "user/name",
        "user:name",
        "user\0name",
        "user\nname",
        "пользователь",
        "user@domain",
    ];
    for bad_id in invalid_ids {
        assert_eq!(
            store.save_account_token(bad_id, &tok),
            Err(TokenStoreError::InvalidAccountId),
            "expected InvalidAccountId for {bad_id:?}"
        );
        assert_eq!(
            store.load_account_token(bad_id),
            Err(TokenStoreError::InvalidAccountId),
            "expected InvalidAccountId for {bad_id:?}"
        );
        assert_eq!(
            store.delete_account_token(bad_id),
            Err(TokenStoreError::InvalidAccountId),
            "expected InvalidAccountId for {bad_id:?}"
        );
    }
}

#[test]
fn t06_backend_unavailable_and_storage_failure_simulation_and_redaction() {
    let mut backend = MemoryKeyringBackend::new();
    backend.set_unavailable(true);
    let mut store = TokenStore::new(backend);

    let raw_secret = "mfa.SENSITIVE_TOKEN_DO_NOT_LEAK_ON_ERROR_555";
    let tok = SecretToken::new(raw_secret).expect("valid");

    let err_save = store.save_active_token(&tok).unwrap_err();
    assert!(matches!(err_save, TokenStoreError::BackendUnavailable(_)));
    assert!(!format!("{err_save:?}").contains(raw_secret));
    assert!(!format!("{err_save}").contains(raw_secret));

    let err_load = store.load_active_token().unwrap_err();
    assert!(matches!(err_load, TokenStoreError::BackendUnavailable(_)));

    let err_del = store.delete_active_token().unwrap_err();
    assert!(matches!(err_del, TokenStoreError::BackendUnavailable(_)));

    // Switch to storage failure mode
    store.backend_mut().set_unavailable(false);
    store.backend_mut().set_fail_storage(true);

    let err_storage = store.save_active_token(&tok).unwrap_err();
    assert!(matches!(err_storage, TokenStoreError::StorageFailure(_)));
    assert!(!format!("{err_storage:?}").contains(raw_secret));
    assert!(!format!("{err_storage}").contains(raw_secret));

    // Recover backend and verify normal operation resumes
    store.backend_mut().set_fail_storage(false);
    store.save_active_token(&tok).expect("recovered save");
    assert_eq!(store.load_active_token(), Ok(Some(tok)));

    // Verify MemoryKeyringBackend Debug never leaks stored secrets
    let backend_debug = format!("{:?}", store.backend());
    assert!(!backend_debug.contains(raw_secret));
    assert!(!backend_debug.contains("SENSITIVE_TOKEN"));
}

#[test]
fn t07_corrupted_stored_token_rejected_on_load_without_leaking() {
    let mut backend = MemoryKeyringBackend::new();
    // Inject a corrupted value directly via SecretBackend trait
    let corrupted = "corrupted_token_with_newline\nsecret_part";
    backend
        .set_secret(DEFAULT_TOKEN_KEY, corrupted)
        .expect("raw backend insert");

    let store = TokenStore::new(backend);
    let err = store.load_active_token().unwrap_err();
    assert_eq!(err, TokenStoreError::InvalidToken);
    assert!(!format!("{err:?}").contains("secret_part"));
    assert!(!format!("{err}").contains("secret_part"));
}

#[test]
fn t08_system_keyring_backend_error_mapping_and_redaction() {
    use chord_platform::keyring::KeyringError;

    let secret_sample = "mfa.SECRET_IN_SYSTEM_KEYRING_999";

    let e_key = SystemKeyringBackend::map_keyring_error(KeyringError::InvalidKey);
    assert!(matches!(e_key, TokenStoreError::StorageFailure(_)));

    let e_sec = SystemKeyringBackend::map_keyring_error(KeyringError::InvalidSecret);
    assert_eq!(e_sec, TokenStoreError::InvalidToken);

    let e_enc = SystemKeyringBackend::map_keyring_error(KeyringError::Encoding);
    assert!(matches!(e_enc, TokenStoreError::StorageFailure(_)));

    let e_dbus = SystemKeyringBackend::map_keyring_error(KeyringError::Secret {
        code: 42,
        message: format!("dbus error while storing {secret_sample}"),
    });
    // Even if an upstream GError message had arbitrary text, verify map_keyring_error strips it and never leaks secret_sample!
    let _: &dyn std::error::Error = &e_dbus;
    assert!(matches!(e_dbus, TokenStoreError::StorageFailure(_)));
    assert!(!format!("{e_dbus:?}").contains(secret_sample));
    assert!(!format!("{e_dbus}").contains(secret_sample));

    let e_conn = SystemKeyringBackend::map_connect_error(KeyringError::Secret {
        code: 7,
        message: format!("connect failed {secret_sample}"),
    });
    assert!(matches!(e_conn, TokenStoreError::BackendUnavailable(_)));
    assert!(!format!("{e_conn:?}").contains(secret_sample));
    assert!(!format!("{e_conn}").contains(secret_sample));

    // Verify function pointer signatures for SystemKeyringBackend constructors
    let _connect_fn: fn() -> Result<SystemKeyringBackend, TokenStoreError> =
        SystemKeyringBackend::connect;
    let _from_fn: fn(chord_platform::keyring::Keyring) -> SystemKeyringBackend =
        SystemKeyringBackend::from_keyring;
}
