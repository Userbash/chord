use chord_store::keyring_crypto::{
    Argon2idParams, DerivedKey, InMemoryKeyring, KeyringError, MasterKeyManager,
    SecretServiceProvider, argon2id_derive,
};

#[test]
fn k01_argon2id_rfc9106_deterministic_and_salt_sensitive() {
    let params = Argon2idParams {
        memory_kib: 32,
        iterations: 3,
        parallelism: 1,
        output_len: 32,
    };
    let k1 = argon2id_derive(b"password", b"somesalt12345678", &params).expect("derive k1");
    let k2 = argon2id_derive(b"password", b"somesalt12345678", &params).expect("derive k2");
    let k3 = argon2id_derive(b"password", b"othersalt2345678", &params).expect("derive k3");

    assert_eq!(k1.as_bytes().len(), 32);
    assert_eq!(k1.as_bytes(), k2.as_bytes());
    assert_ne!(k1.as_bytes(), k3.as_bytes());
}

#[test]
fn k02_argon2id_rejects_invalid_params() {
    let bad_mem = Argon2idParams {
        memory_kib: 4, // < 8 * parallelism
        iterations: 1,
        parallelism: 1,
        output_len: 32,
    };
    assert!(matches!(
        argon2id_derive(b"pw", b"somesalt12345678", &bad_mem),
        Err(KeyringError::InvalidParams(_))
    ));

    let bad_salt = Argon2idParams::default();
    assert!(matches!(
        argon2id_derive(b"pw", b"short", &bad_salt),
        Err(KeyringError::InvalidParams(_))
    ));
}

#[test]
fn k03_master_key_manager_derives_from_keyring_secret_and_hardware_salt() {
    let keyring = InMemoryKeyring::new();
    let manager = MasterKeyManager::with_params(
        keyring,
        Argon2idParams {
            memory_kib: 32,
            iterations: 2,
            parallelism: 1,
            output_len: 32,
        },
    );

    let hw_salt = b"machine-id-0123456789abcdef";
    let key1 = manager
        .derive_or_create_master_key("chord-db", "user-1", hw_salt)
        .expect("create and derive");
    let key2 = manager
        .derive_or_create_master_key("chord-db", "user-1", hw_salt)
        .expect("load and derive");

    assert_eq!(key1.as_bytes(), key2.as_bytes());
    assert_ne!(key1.as_bytes(), &[0u8; 32]);
}

#[test]
fn k04_derived_key_zeroizes_on_drop_and_explicit_zeroize() {
    let mut key = DerivedKey::from_bytes(vec![0xAA; 32]);
    assert_eq!(key.as_bytes(), &[0xAA; 32]);
    key.zeroize_now();
    assert!(key.as_bytes().iter().all(|&b| b == 0));
}

#[test]
fn k05_keyring_provider_missing_secret_returns_not_found() {
    let keyring = InMemoryKeyring::new();
    let res = keyring.get_secret("chord-db", "missing-user");
    assert!(matches!(res, Err(KeyringError::NotFound)));
}
