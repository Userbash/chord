//! Contract tests for Task R4-07: Application Updater Service.
//!
//! Verifies:
//! 1. Startup update verification (disabled, up-to-date, available, invalid manifest signature).
//! 2. Download progress display (monotonic progress events, formatted progress label, failure handling).
//! 3. Snooze and skip-version persistence across service restarts.
//! 4. Cryptographic Ed25519 signature verification of release binary before atomic swap.
//! 5. Atomic binary swap and rollback.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use chord_updater::ReleaseChannel;
use chord_updater::manifest::{Version, sha256_hex, sign_binary, sign_manifest};
use chord_updater::service::{
    DownloadProgress, DownloadedRelease, StartupCheckOutcome, UpdateDownloader, UpdaterConfig,
    UpdaterError, UpdaterService, UpdaterState,
};
use ring::signature::{Ed25519KeyPair, KeyPair};

fn test_keypair(seed_byte: u8) -> ([u8; 32], [u8; 32]) {
    let seed = [seed_byte; 32];
    let pair = Ed25519KeyPair::from_seed_unchecked(&seed).expect("valid ed25519 seed");
    let mut public_key = [0u8; 32];
    public_key.copy_from_slice(pair.public_key().as_ref());
    (seed, public_key)
}

fn temp_test_dir(tag: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "chord_updater_r4_07_{tag}_{}_{}",
        std::process::id(),
        nanos
    ));
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn build_signed_manifest(
    version: &str,
    artifact_name: &str,
    binary_bytes: &[u8],
    seed: &[u8; 32],
) -> Vec<u8> {
    let digest = sha256_hex(binary_bytes);
    let body = serde_json::json!({
        "version": version,
        "released_at": "2026-11-21T12:00:00Z",
        "artifacts": [
            {
                "name": artifact_name,
                "url": format!("https://updates.discord.com/releases/{version}/{artifact_name}"),
                "sha256": digest,
                "size": binary_bytes.len()
            }
        ]
    });
    let raw = serde_json::to_vec(&body).expect("serialize manifest");
    sign_manifest(&raw, seed).expect("sign manifest")
}

struct MockChunkedDownloader {
    payload: Vec<u8>,
    signature: Vec<u8>,
    chunk_size: usize,
    fail_with: Option<String>,
}

impl UpdateDownloader for MockChunkedDownloader {
    fn download(
        &mut self,
        _url: &str,
        dest: &Path,
        expected_total_bytes: u64,
        on_progress: &mut dyn FnMut(DownloadProgress),
    ) -> Result<DownloadedRelease, UpdaterError> {
        if let Some(err) = &self.fail_with {
            return Err(UpdaterError::DownloadFailed {
                message: err.clone(),
            });
        }
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).expect("create download parent");
        }
        let total = if expected_total_bytes > 0 {
            expected_total_bytes
        } else {
            self.payload.len() as u64
        };
        let mut written: u64 = 0;
        let mut out = Vec::with_capacity(self.payload.len());
        for chunk in self.payload.chunks(self.chunk_size.max(1)) {
            out.extend_from_slice(chunk);
            written += chunk.len() as u64;
            on_progress(DownloadProgress::new(written, total));
        }
        fs::write(dest, &out).expect("write downloaded payload");
        Ok(DownloadedRelease {
            file_path: dest.to_path_buf(),
            binary_signature: self.signature.clone(),
        })
    }
}

fn base_config(dir: &Path, public_key: [u8; 32]) -> UpdaterConfig {
    UpdaterConfig {
        current_version: Version::parse("1.0.0").expect("valid version"),
        channel: ReleaseChannel::Stable,
        artifact_name: "chord".to_string(),
        install_dir: dir.join("install"),
        preferences_path: dir.join("updater_prefs.json"),
        public_key,
        disable_updater: false,
        standalone_or_local_modules: false,
    }
}

#[test]
fn r4_07_startup_verification_available_uptodate_disabled_and_bad_sig() {
    let dir = temp_test_dir("startup");
    let (seed, public_key) = test_keypair(7);
    let binary_v110 = b"chord-binary-v1.1.0".to_vec();
    let signed_v110 = build_signed_manifest("1.1.0", "chord", &binary_v110, &seed);

    // 1. Newer version available
    let mut service = UpdaterService::new(base_config(&dir, public_key)).expect("init service");
    let outcome = service
        .check_startup_updates(&signed_v110, 1_700_000_000)
        .expect("startup check");
    assert_eq!(
        outcome,
        StartupCheckOutcome::UpdateAvailable {
            version: "1.1.0".to_string(),
            total_bytes: binary_v110.len() as u64,
        }
    );
    assert!(matches!(service.state(), UpdaterState::Available { .. }));

    // 2. Up-to-date version
    let mut cfg_current = base_config(&dir, public_key);
    cfg_current.current_version = Version::parse("1.1.0").expect("valid version");
    let mut up_to_date_svc = UpdaterService::new(cfg_current).expect("init service");
    let outcome_same = up_to_date_svc
        .check_startup_updates(&signed_v110, 1_700_000_000)
        .expect("startup check");
    assert_eq!(outcome_same, StartupCheckOutcome::UpToDate);
    assert_eq!(up_to_date_svc.state(), &UpdaterState::UpToDate);

    // 3. Disabled updater
    let mut cfg_disabled = base_config(&dir, public_key);
    cfg_disabled.disable_updater = true;
    let mut disabled_svc = UpdaterService::new(cfg_disabled).expect("init service");
    let outcome_disabled = disabled_svc
        .check_startup_updates(&signed_v110, 1_700_000_000)
        .expect("startup check");
    assert_eq!(outcome_disabled, StartupCheckOutcome::Disabled);
    assert_eq!(disabled_svc.state(), &UpdaterState::Disabled);

    // 4. Tampered manifest signature must fail
    let mut tampered = signed_v110.clone();
    tampered[0] ^= 0xFF;
    let err = service
        .check_startup_updates(&tampered, 1_700_000_000)
        .expect_err("tampered manifest must fail");
    assert!(matches!(err, UpdaterError::Manifest(_)));
    assert!(matches!(service.state(), UpdaterState::Error { .. }));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn r4_07_snooze_and_skip_flags_persist_across_restarts() {
    let dir = temp_test_dir("persist");
    let (seed, public_key) = test_keypair(11);
    let binary_v120 = b"chord-binary-v1.2.0".to_vec();
    let signed_v120 = build_signed_manifest("1.2.0", "chord", &binary_v120, &seed);
    let binary_v130 = b"chord-binary-v1.3.0".to_vec();
    let signed_v130 = build_signed_manifest("1.3.0", "chord", &binary_v130, &seed);

    let now = 1_700_000_000_u64;
    {
        let mut service = UpdaterService::new(base_config(&dir, public_key)).expect("init service");
        service.snooze_for(now, 3_600).expect("persist snooze");
        service.skip_version("1.2.0").expect("persist skip");
        assert!(service.skip_version("not-a-version").is_err());
    }

    // Reload from disk in a fresh UpdaterService instance
    let mut reloaded = UpdaterService::new(base_config(&dir, public_key)).expect("reload service");
    assert_eq!(
        reloaded.preferences().snoozed_until_epoch_secs,
        Some(now + 3_600)
    );
    assert_eq!(
        reloaded.preferences().skipped_version.as_deref(),
        Some("1.2.0")
    );

    // 1.2.0 is skipped
    let res_120 = reloaded
        .check_startup_updates(&signed_v120, now + 100)
        .expect("check 1.2.0");
    assert_eq!(
        res_120,
        StartupCheckOutcome::Skipped {
            version: "1.2.0".to_string()
        }
    );

    // 1.3.0 is NOT skipped, but IS snoozed until now + 3600
    let res_130_snoozed = reloaded
        .check_startup_updates(&signed_v130, now + 1_800)
        .expect("check 1.3.0 snoozed");
    assert_eq!(
        res_130_snoozed,
        StartupCheckOutcome::Snoozed {
            version: "1.3.0".to_string(),
            until_epoch_secs: now + 3_600,
        }
    );

    // After snooze expires, 1.3.0 becomes available
    let res_130_available = reloaded
        .check_startup_updates(&signed_v130, now + 3_600)
        .expect("check 1.3.0 after snooze");
    assert_eq!(
        res_130_available,
        StartupCheckOutcome::UpdateAvailable {
            version: "1.3.0".to_string(),
            total_bytes: binary_v130.len() as u64,
        }
    );

    // Clear flags and verify persistence
    reloaded.clear_snooze().expect("clear snooze");
    reloaded.clear_skipped_version().expect("clear skip");
    let reloaded_again =
        UpdaterService::new(base_config(&dir, public_key)).expect("reload cleared");
    assert_eq!(reloaded_again.preferences().snoozed_until_epoch_secs, None);
    assert_eq!(reloaded_again.preferences().skipped_version, None);

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn r4_07_download_displays_visible_progress_and_handles_failure() {
    let dir = temp_test_dir("progress");
    let (seed, public_key) = test_keypair(23);
    let binary_v200 = vec![0x42u8; 1024];
    let binary_sig = sign_binary(&binary_v200, &seed).expect("sign binary");
    let signed_v200 = build_signed_manifest("2.0.0", "chord", &binary_v200, &seed);

    let mut service = UpdaterService::new(base_config(&dir, public_key)).expect("init service");
    service
        .check_startup_updates(&signed_v200, 1_700_000_000)
        .expect("check startup");

    let mut downloader = MockChunkedDownloader {
        payload: binary_v200.clone(),
        signature: binary_sig,
        chunk_size: 256,
        fail_with: None,
    };

    let mut observed: Vec<DownloadProgress> = Vec::new();
    let downloaded_path = service
        .download_update(&mut downloader, &mut |progress| {
            observed.push(progress);
        })
        .expect("download update");

    assert!(downloaded_path.exists());
    assert_eq!(observed.len(), 4);
    assert_eq!(observed[0].percent, 25);
    assert_eq!(observed[1].percent, 50);
    assert_eq!(observed[2].percent, 75);
    assert_eq!(observed[3].percent, 100);
    assert!(observed[1].formatted_label.contains("50%"));
    assert!(matches!(
        service.state(),
        UpdaterState::ReadyToInstall { .. }
    ));

    // Failure case transitions to Error
    let mut failing_svc = UpdaterService::new(base_config(&dir, public_key)).expect("init");
    failing_svc
        .check_startup_updates(&signed_v200, 1_700_000_000)
        .expect("check");
    let mut failing_dl = MockChunkedDownloader {
        payload: vec![],
        signature: vec![],
        chunk_size: 64,
        fail_with: Some("connection reset".to_string()),
    };
    let err = failing_svc
        .download_update(&mut failing_dl, &mut |_| {})
        .expect_err("download should fail");
    assert!(matches!(err, UpdaterError::DownloadFailed { .. }));
    assert!(matches!(failing_svc.state(), UpdaterState::Error { .. }));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn r4_07_release_binary_signature_verified_before_atomic_swap_and_rollback() {
    let dir = temp_test_dir("swap");
    let (seed, public_key) = test_keypair(31);
    let (wrong_seed, _) = test_keypair(99);
    let cfg = base_config(&dir, public_key);
    fs::create_dir_all(&cfg.install_dir).expect("create install dir");
    let target_bin = cfg.install_dir.join("chord");
    let original_bytes = b"original-v1.0.0-binary";
    fs::write(&target_bin, original_bytes).expect("write original binary");

    let new_bytes = b"updated-v1.5.0-binary-payload".to_vec();
    let signed_manifest = build_signed_manifest("1.5.0", "chord", &new_bytes, &seed);

    // 1. Wrong binary signature must be rejected BEFORE swap; original binary stays untouched!
    let wrong_sig = sign_binary(&new_bytes, &wrong_seed).expect("sign with wrong key");
    let mut svc_bad_sig = UpdaterService::new(cfg.clone()).expect("init");
    svc_bad_sig
        .check_startup_updates(&signed_manifest, 1_700_000_000)
        .expect("check");
    let mut dl_bad_sig = MockChunkedDownloader {
        payload: new_bytes.clone(),
        signature: wrong_sig,
        chunk_size: 16,
        fail_with: None,
    };
    svc_bad_sig
        .download_update(&mut dl_bad_sig, &mut |_| {})
        .expect("download");
    let err = svc_bad_sig
        .install_and_swap()
        .expect_err("wrong binary signature must block swap");
    assert!(matches!(err, UpdaterError::BinarySignatureInvalid));
    assert_eq!(fs::read(&target_bin).expect("read target"), original_bytes);

    // 2. Missing/truncated binary signature must be rejected BEFORE swap!
    let mut svc_missing_sig = UpdaterService::new(cfg.clone()).expect("init");
    svc_missing_sig
        .check_startup_updates(&signed_manifest, 1_700_000_000)
        .expect("check");
    let mut dl_missing_sig = MockChunkedDownloader {
        payload: new_bytes.clone(),
        signature: vec![1, 2, 3],
        chunk_size: 16,
        fail_with: None,
    };
    svc_missing_sig
        .download_update(&mut dl_missing_sig, &mut |_| {})
        .expect("download");
    let err_missing = svc_missing_sig
        .install_and_swap()
        .expect_err("short binary signature must block swap");
    assert!(matches!(err_missing, UpdaterError::BinarySignatureInvalid));
    assert_eq!(fs::read(&target_bin).expect("read target"), original_bytes);

    // 3. Valid binary signature succeeds, swaps atomically, and supports rollback
    let valid_sig = sign_binary(&new_bytes, &seed).expect("sign with valid key");
    let mut svc_ok = UpdaterService::new(cfg.clone()).expect("init");
    svc_ok
        .check_startup_updates(&signed_manifest, 1_700_000_000)
        .expect("check");
    let mut dl_ok = MockChunkedDownloader {
        payload: new_bytes.clone(),
        signature: valid_sig,
        chunk_size: 16,
        fail_with: None,
    };
    svc_ok
        .download_update(&mut dl_ok, &mut |_| {})
        .expect("download");
    svc_ok.install_and_swap().expect("atomic swap succeeds");

    assert_eq!(fs::read(&target_bin).expect("read swapped"), new_bytes);
    assert_eq!(
        svc_ok.state(),
        &UpdaterState::RestartRequired {
            version: "1.5.0".to_string()
        }
    );

    // Rollback restores original binary
    svc_ok.rollback_last_update().expect("rollback succeeds");
    assert_eq!(
        fs::read(&target_bin).expect("read rolled back"),
        original_bytes
    );
    assert_eq!(
        svc_ok.state(),
        &UpdaterState::RolledBack {
            version: "1.5.0".to_string()
        }
    );

    let _ = fs::remove_dir_all(&dir);
}
