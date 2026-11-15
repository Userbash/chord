//! Real Windows Credential Manager contract using synthetic, uniquely named
//! secrets. This is not a mock store; failure to access the native store fails.
#![cfg(windows)]
use chord_platform::keyring::{Keyring, KeyringError, SCHEMA_NAME};
use keyring::credential::CredentialApi;
use keyring::windows::WinCredential;

#[test]
fn native_credentials_persist_across_handles_and_match_independent_provider() {
    let key = format!(
        "chord-test-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let native = WinCredential::new_with_target(None, SCHEMA_NAME, &key).unwrap();
    struct Cleanup(WinCredential);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = self.0.delete_credential();
        }
    }
    let cleanup = Cleanup(native);
    {
        let mut first = Keyring::connect().unwrap();
        assert_eq!(first.lookup(&key).unwrap(), None);
        assert!(!first.clear(&key).unwrap());
        assert_eq!(first.store("", "synthetic"), Err(KeyringError::InvalidKey));
        assert_eq!(
            first.store(&key, "synthetic\0secret"),
            Err(KeyringError::InvalidSecret)
        );
        first.store(&key, "synthetic native secret: тест").unwrap();
    }
    assert_eq!(
        cleanup.0.get_secret().unwrap(),
        "synthetic native secret: тест".as_bytes()
    );
    let mut second = Keyring::connect().unwrap();
    assert_eq!(
        second.lookup(&key).unwrap().as_deref(),
        Some("synthetic native secret: тест")
    );
    cleanup.0.set_secret(&[0xff, 0xfe]).unwrap();
    assert_eq!(second.lookup(&key), Err(KeyringError::Encoding));
    assert!(second.clear(&key).unwrap());
    assert!(!second.clear(&key).unwrap());
    assert_eq!(second.lookup(&key).unwrap(), None);
}
