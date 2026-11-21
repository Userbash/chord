//! Updater Service for Task R4-07 (Application Updater).
//!
//! Provides:
//! - Startup update verification (with `should_disable_updater`, Ed25519 manifest check,
//!   version comparison, and snooze/skip suppression).
//! - Chunked download progress reporting (`DownloadProgress`, `UpdateDownloader`).
//! - Persistent snooze/skip preferences (`UpdaterPreferences`).
//! - Cryptographic Ed25519 release binary signature verification before atomic swap.
//! - Atomic binary swap and rollback via `crate::plan`.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::manifest::{
    ManifestError, VerifiedManifest, Version, sha256_hex, verify_binary_signature, verify_manifest,
};
use crate::plan::{PlanError, UpdatePlan, apply_local_steps, plan_update, rollback};
use crate::{ReleaseChannel, ScheduledUpdateBackoff, should_disable_updater};

/// Configuration for [`UpdaterService`].
#[derive(Debug, Clone)]
pub struct UpdaterConfig {
    /// Currently running application version.
    pub current_version: Version,
    /// Active release channel.
    pub channel: ReleaseChannel,
    /// Target artifact name inside the manifest and `install_dir`.
    pub artifact_name: String,
    /// Directory where the target binary is installed.
    pub install_dir: PathBuf,
    /// JSON file path where snooze/skip preferences are persisted.
    pub preferences_path: PathBuf,
    /// Trusted Ed25519 public key (32 bytes) for manifest and binary verification.
    pub public_key: [u8; 32],
    /// Explicit disable flag (`disableUpdater`).
    pub disable_updater: bool,
    /// Whether standalone/local modules are active on the development channel.
    pub standalone_or_local_modules: bool,
}

/// Persisted updater user preferences (snooze & skip version).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdaterPreferences {
    /// Unix epoch timestamp (seconds) until which update prompts are snoozed.
    pub snoozed_until_epoch_secs: Option<u64>,
    /// Specific release version string permanently skipped by the user.
    pub skipped_version: Option<String>,
    /// Whether automatic startup verification is enabled.
    pub auto_check_on_startup: bool,
}

impl Default for UpdaterPreferences {
    fn default() -> Self {
        Self {
            snoozed_until_epoch_secs: None,
            skipped_version: None,
            auto_check_on_startup: true,
        }
    }
}

/// Visible download progress snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadProgress {
    /// Bytes downloaded so far.
    pub downloaded_bytes: u64,
    /// Total expected bytes for the release artifact.
    pub total_bytes: u64,
    /// Completion percentage clamped to `0..=100`.
    pub percent: u8,
    /// Human-readable progress label for UI display.
    pub formatted_label: String,
}

impl DownloadProgress {
    /// Constructs a [`DownloadProgress`] snapshot and computes percentage & formatted label.
    pub fn new(downloaded_bytes: u64, total_bytes: u64) -> Self {
        let percent = downloaded_bytes
            .saturating_mul(100)
            .checked_div(total_bytes)
            .map_or(0, |pct| pct.min(100) as u8);
        let formatted_label =
            format!("Downloading {percent}% ({downloaded_bytes} / {total_bytes} B)");
        Self {
            downloaded_bytes,
            total_bytes,
            percent,
            formatted_label,
        }
    }
}

/// Downloaded release artifact and its detached Ed25519 binary signature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadedRelease {
    /// Local path of the downloaded binary.
    pub file_path: PathBuf,
    /// Detached 64-byte Ed25519 signature over the release binary bytes.
    pub binary_signature: Vec<u8>,
}

/// Abstraction for downloading a release artifact with progress callbacks.
pub trait UpdateDownloader {
    /// Downloads `url` to `dest`, reporting progress via `on_progress`.
    fn download(
        &mut self,
        url: &str,
        dest: &Path,
        expected_total_bytes: u64,
        on_progress: &mut dyn FnMut(DownloadProgress),
    ) -> Result<DownloadedRelease, UpdaterError>;
}

/// Outcome of a startup update verification check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartupCheckOutcome {
    /// Updater is disabled by configuration.
    Disabled,
    /// Installed version is already up to date.
    UpToDate,
    /// Candidate version was explicitly skipped by the user.
    Skipped {
        /// Skipped version string.
        version: String,
    },
    /// Update prompt is currently snoozed.
    Snoozed {
        /// Available version string.
        version: String,
        /// Snooze expiration timestamp (Unix epoch seconds).
        until_epoch_secs: u64,
    },
    /// A newer verified update is available for download.
    UpdateAvailable {
        /// Candidate version string.
        version: String,
        /// Expected artifact size in bytes.
        total_bytes: u64,
    },
}

/// Observable state machine for [`UpdaterService`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdaterState {
    /// Initial idle state.
    Idle,
    /// Updater is disabled.
    Disabled,
    /// Checking signed manifest on startup.
    Checking,
    /// Client is up to date.
    UpToDate,
    /// Update was skipped by user preference.
    Skipped {
        /// Skipped version string.
        version: String,
    },
    /// Update is snoozed until `until_epoch_secs`.
    Snoozed {
        /// Available version string.
        version: String,
        /// Snooze expiration timestamp (Unix epoch seconds).
        until_epoch_secs: u64,
    },
    /// Newer update is available.
    Available {
        /// Available version string.
        version: String,
        /// Artifact size in bytes.
        total_bytes: u64,
    },
    /// Actively downloading release artifact with visible progress.
    Downloading(DownloadProgress),
    /// Download finished; ready to verify binary signature and swap.
    ReadyToInstall {
        /// Downloaded version string.
        version: String,
        /// Local path to downloaded binary.
        downloaded_path: PathBuf,
    },
    /// Verifying cryptographic binary signature and SHA-256 digest before swap.
    Verifying,
    /// Atomic swap completed; application restart required.
    RestartRequired {
        /// Installed version string.
        version: String,
    },
    /// Update was rolled back to the previous binary.
    RolledBack {
        /// Rolled-back target version string.
        version: String,
    },
    /// Error state.
    Error {
        /// Diagnostic error message.
        message: String,
    },
}

/// Errors produced by [`UpdaterService`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdaterError {
    /// Manifest parsing or Ed25519 verification failed.
    Manifest(ManifestError),
    /// Update planning, digest, or atomic swap failed.
    Plan(PlanError),
    /// Release binary Ed25519 signature is missing or invalid.
    BinarySignatureInvalid,
    /// Download transport failed.
    DownloadFailed {
        /// Failure details.
        message: String,
    },
    /// Operation is not valid in the current service state.
    InvalidState {
        /// Explanation of the invalid state transition.
        message: String,
    },
    /// Filesystem I/O or preferences serialization error.
    Io {
        /// Path involved in the error.
        path: String,
        /// Failure details.
        message: String,
    },
}

impl From<ManifestError> for UpdaterError {
    fn from(error: ManifestError) -> Self {
        Self::Manifest(error)
    }
}

impl From<PlanError> for UpdaterError {
    fn from(error: PlanError) -> Self {
        Self::Plan(error)
    }
}

/// Coordinates startup verification, download progress, snooze/skip persistence,
/// cryptographic binary signature validation, atomic swap, and rollback.
#[derive(Debug)]
pub struct UpdaterService {
    config: UpdaterConfig,
    preferences: UpdaterPreferences,
    state: UpdaterState,
    backoff: ScheduledUpdateBackoff,
    verified_manifest: Option<VerifiedManifest>,
    downloaded: Option<DownloadedRelease>,
    last_plan: Option<UpdatePlan>,
}

impl UpdaterService {
    /// Creates a new [`UpdaterService`] and loads persisted preferences if present.
    pub fn new(config: UpdaterConfig) -> Result<Self, UpdaterError> {
        let preferences = load_preferences(&config.preferences_path)?;
        Ok(Self {
            config,
            preferences,
            state: UpdaterState::Idle,
            backoff: ScheduledUpdateBackoff::new(),
            verified_manifest: None,
            downloaded: None,
            last_plan: None,
        })
    }

    /// Current observable updater state.
    pub fn state(&self) -> &UpdaterState {
        &self.state
    }

    /// Current persisted snooze/skip preferences.
    pub fn preferences(&self) -> &UpdaterPreferences {
        &self.preferences
    }

    /// Verifies a signed manifest on startup and evaluates snooze/skip/disable policies.
    pub fn check_startup_updates(
        &mut self,
        signed_manifest: &[u8],
        now_epoch_secs: u64,
    ) -> Result<StartupCheckOutcome, UpdaterError> {
        if should_disable_updater(
            self.config.channel,
            self.config.disable_updater,
            self.config.standalone_or_local_modules,
        ) {
            self.state = UpdaterState::Disabled;
            return Ok(StartupCheckOutcome::Disabled);
        }

        self.state = UpdaterState::Checking;
        let verified = match verify_manifest(signed_manifest, &self.config.public_key) {
            Ok(manifest) => manifest,
            Err(err) => {
                self.state = UpdaterState::Error {
                    message: format!("manifest verification failed: {err:?}"),
                };
                return Err(UpdaterError::Manifest(err));
            }
        };

        if !verified
            .version()
            .is_newer_than(&self.config.current_version)
        {
            self.verified_manifest = None;
            self.state = UpdaterState::UpToDate;
            return Ok(StartupCheckOutcome::UpToDate);
        }

        let version_str = format_version(verified.version());
        let artifact = verified
            .artifact(&self.config.artifact_name)
            .ok_or_else(|| {
                UpdaterError::Plan(PlanError::UnknownArtifact {
                    name: self.config.artifact_name.clone(),
                })
            })?;
        let total_bytes = artifact.size;

        if self.preferences.skipped_version.as_deref() == Some(version_str.as_str()) {
            self.verified_manifest = Some(verified);
            self.state = UpdaterState::Skipped {
                version: version_str.clone(),
            };
            return Ok(StartupCheckOutcome::Skipped {
                version: version_str,
            });
        }

        if let Some(until) = self.preferences.snoozed_until_epoch_secs
            && now_epoch_secs < until
        {
            self.verified_manifest = Some(verified);
            self.state = UpdaterState::Snoozed {
                version: version_str.clone(),
                until_epoch_secs: until,
            };
            return Ok(StartupCheckOutcome::Snoozed {
                version: version_str,
                until_epoch_secs: until,
            });
        }

        self.verified_manifest = Some(verified);
        self.state = UpdaterState::Available {
            version: version_str.clone(),
            total_bytes,
        };
        Ok(StartupCheckOutcome::UpdateAvailable {
            version: version_str,
            total_bytes,
        })
    }

    /// Snoozes update notifications until `until_epoch_secs` and persists to disk.
    pub fn snooze_until(&mut self, until_epoch_secs: u64) -> Result<(), UpdaterError> {
        self.preferences.snoozed_until_epoch_secs = Some(until_epoch_secs);
        save_preferences(&self.config.preferences_path, &self.preferences)
    }

    /// Snoozes update notifications for `duration_secs` starting from `now_epoch_secs`.
    pub fn snooze_for(
        &mut self,
        now_epoch_secs: u64,
        duration_secs: u64,
    ) -> Result<(), UpdaterError> {
        let until = now_epoch_secs.saturating_add(duration_secs);
        self.snooze_until(until)
    }

    /// Clears any active snooze timestamp and persists to disk.
    pub fn clear_snooze(&mut self) -> Result<(), UpdaterError> {
        self.preferences.snoozed_until_epoch_secs = None;
        save_preferences(&self.config.preferences_path, &self.preferences)
    }

    /// Marks `version` as skipped and persists to disk.
    pub fn skip_version(&mut self, version: &str) -> Result<(), UpdaterError> {
        Version::parse(version)?;
        self.preferences.skipped_version = Some(version.to_string());
        save_preferences(&self.config.preferences_path, &self.preferences)
    }

    /// Clears any skipped version and persists to disk.
    pub fn clear_skipped_version(&mut self) -> Result<(), UpdaterError> {
        self.preferences.skipped_version = None;
        save_preferences(&self.config.preferences_path, &self.preferences)
    }

    /// Downloads the verified update artifact while reporting live [`DownloadProgress`].
    pub fn download_update(
        &mut self,
        downloader: &mut impl UpdateDownloader,
        on_progress: &mut impl FnMut(DownloadProgress),
    ) -> Result<PathBuf, UpdaterError> {
        let verified =
            self.verified_manifest
                .as_ref()
                .ok_or_else(|| UpdaterError::InvalidState {
                    message: "no verified manifest available for download".to_string(),
                })?;

        let plan = plan_update(
            &self.config.current_version,
            verified,
            &self.config.artifact_name,
            &self.config.install_dir,
        )?;
        let artifact = verified
            .artifact(&self.config.artifact_name)
            .ok_or_else(|| {
                UpdaterError::Plan(PlanError::UnknownArtifact {
                    name: self.config.artifact_name.clone(),
                })
            })?;
        let dest = self
            .config
            .install_dir
            .join("stage")
            .join(format!("{}.download", artifact.name));

        let initial = DownloadProgress::new(0, artifact.size);
        self.state = UpdaterState::Downloading(initial);

        let mut forward = |progress: DownloadProgress| {
            self.state = UpdaterState::Downloading(progress.clone());
            on_progress(progress);
        };

        match downloader.download(&artifact.url, &dest, artifact.size, &mut forward) {
            Ok(release) => {
                self.backoff.succeed();
                let version_str = format_version(verified.version());
                let path = release.file_path.clone();
                self.downloaded = Some(release);
                self.last_plan = Some(plan);
                self.state = UpdaterState::ReadyToInstall {
                    version: version_str,
                    downloaded_path: path.clone(),
                };
                Ok(path)
            }
            Err(err) => {
                let _delay_ms = self.backoff.next_delay_ms();
                self.state = UpdaterState::Error {
                    message: format!("download failed: {err:?}"),
                };
                Err(err)
            }
        }
    }

    /// Verifies the downloaded release binary's Ed25519 signature and SHA-256 digest,
    /// then performs an atomic binary swap.
    pub fn install_and_swap(&mut self) -> Result<(), UpdaterError> {
        let verified =
            self.verified_manifest
                .as_ref()
                .ok_or_else(|| UpdaterError::InvalidState {
                    message: "no verified manifest available for install".to_string(),
                })?;
        let downloaded = self
            .downloaded
            .as_ref()
            .ok_or_else(|| UpdaterError::InvalidState {
                message: "no downloaded release available for install".to_string(),
            })?;

        self.state = UpdaterState::Verifying;

        let binary_bytes = fs::read(&downloaded.file_path).map_err(|error| UpdaterError::Io {
            path: downloaded.file_path.display().to_string(),
            message: format!("failed to read downloaded binary: {error}"),
        })?;

        if verify_binary_signature(
            &binary_bytes,
            &downloaded.binary_signature,
            &self.config.public_key,
        )
        .is_err()
        {
            self.state = UpdaterState::Error {
                message: "release binary Ed25519 signature verification failed".to_string(),
            };
            return Err(UpdaterError::BinarySignatureInvalid);
        }

        let artifact = verified
            .artifact(&self.config.artifact_name)
            .ok_or_else(|| {
                UpdaterError::Plan(PlanError::UnknownArtifact {
                    name: self.config.artifact_name.clone(),
                })
            })?;
        let actual_digest = sha256_hex(&binary_bytes);
        if !actual_digest.eq_ignore_ascii_case(&artifact.sha256) {
            return Err(UpdaterError::Plan(PlanError::DigestMismatch {
                artifact: artifact.name.clone(),
            }));
        }

        let plan = plan_update(
            &self.config.current_version,
            verified,
            &self.config.artifact_name,
            &self.config.install_dir,
        )?;
        apply_local_steps(&plan, &downloaded.file_path)?;

        let target_path = self.config.install_dir.join(&self.config.artifact_name);
        ensure_executable_permissions(&target_path)?;

        let version_str = format_version(verified.version());
        self.last_plan = Some(plan);
        self.state = UpdaterState::RestartRequired {
            version: version_str,
        };
        Ok(())
    }

    /// Rolls back the most recent atomic swap using the stored backup.
    pub fn rollback_last_update(&mut self) -> Result<(), UpdaterError> {
        let plan = self
            .last_plan
            .as_ref()
            .ok_or_else(|| UpdaterError::InvalidState {
                message: "no update plan available to roll back".to_string(),
            })?;
        rollback(plan)?;
        let version_str = format_version(&plan.to_version);
        self.state = UpdaterState::RolledBack {
            version: version_str,
        };
        Ok(())
    }
}

fn format_version(version: &Version) -> String {
    match &version.pre {
        Some(pre) => format!(
            "{}.{}.{}-{pre}",
            version.major, version.minor, version.patch
        ),
        None => format!("{}.{}.{}", version.major, version.minor, version.patch),
    }
}

fn load_preferences(path: &Path) -> Result<UpdaterPreferences, UpdaterError> {
    match fs::read_to_string(path) {
        Ok(content) => serde_json::from_str(&content).map_err(|error| UpdaterError::Io {
            path: path.display().to_string(),
            message: format!("failed to parse updater preferences: {error}"),
        }),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(UpdaterPreferences::default())
        }
        Err(error) => Err(UpdaterError::Io {
            path: path.display().to_string(),
            message: format!("failed to read updater preferences: {error}"),
        }),
    }
}

fn save_preferences(path: &Path, prefs: &UpdaterPreferences) -> Result<(), UpdaterError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| UpdaterError::Io {
            path: parent.display().to_string(),
            message: format!("failed to create preferences directory: {error}"),
        })?;
    }
    let json = serde_json::to_string_pretty(prefs).map_err(|error| UpdaterError::Io {
        path: path.display().to_string(),
        message: format!("failed to serialize updater preferences: {error}"),
    })?;
    let tmp_path = path.with_extension("json.tmp");
    fs::write(&tmp_path, json).map_err(|error| UpdaterError::Io {
        path: tmp_path.display().to_string(),
        message: format!("failed to write temp preferences: {error}"),
    })?;
    fs::rename(&tmp_path, path).map_err(|error| UpdaterError::Io {
        path: path.display().to_string(),
        message: format!("failed to atomically persist preferences: {error}"),
    })?;
    Ok(())
}

#[cfg(unix)]
fn ensure_executable_permissions(path: &Path) -> Result<(), UpdaterError> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = fs::metadata(path)
        .map_err(|error| UpdaterError::Io {
            path: path.display().to_string(),
            message: format!("failed to read target metadata: {error}"),
        })?
        .permissions();
    perms.set_mode(0o755);
    fs::set_permissions(path, perms).map_err(|error| UpdaterError::Io {
        path: path.display().to_string(),
        message: format!("failed to set executable permissions: {error}"),
    })
}

#[cfg(not(unix))]
fn ensure_executable_permissions(_path: &Path) -> Result<(), UpdaterError> {
    Ok(())
}
