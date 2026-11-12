//! Контрактные тесты крейта `discord-updater` (карточка P8-01).
//!
//! Контракт заморожен в `waves/RW-25/TZ-W26c.md` (§3 — API, §4 — кейсы).
//!
//! Оракулы:
//! - SHA-256 — векторы из NIST/openssl (`sha256sum`), см. `sha256_hex_known_k`;
//! - Ed25519 — вектор RFC 8032 TEST 1 (`rfc8032_test_vector_k`), публичный ключ
//!   для seed `[7u8; 32]` детерминированно выводится через `ring`;
//! - версии, план и staged-замена — сверяются с текстом контракта и фактическим
//!   состоянием файлов в temp-каталоге.
//!
//! Файловые операции выполняются только под `std::env::temp_dir()` с уникальным
//! суффиксом; каталог удаляется в `Drop`.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use chord_updater::manifest::{
    Manifest, ManifestError, VerifiedManifest, Version, parse_manifest, sha256_hex, sign_manifest,
    verify_manifest,
};
use chord_updater::plan::{
    PlanError, PlanStep, UpdatePlan, apply_local_steps, plan_update, rollback,
};
use chord_updater::{
    DEFAULT_UPDATE_BACKOFF_MAX_MS, DEFAULT_UPDATE_ENDPOINT, LinuxPackageFormat,
    NEW_UPDATE_ENDPOINT, ReleaseChannel, SCHEDULED_UPDATE_BACKOFF_MS, ScheduledUpdateBackoff,
    SplashUpdateState, build_linux_download_url, build_linux_feed_url, build_modules_base_url,
    should_disable_updater,
};
use ring::signature::{Ed25519KeyPair, KeyPair};

/// Seed для ключей подписи в тестах (детерминированный, не случайный).
const SEED: [u8; 32] = [7u8; 32];

/// SHA-256 от строки `"hello"` (независимый оракул: `sha256sum`).
const HELLO_SHA256: &str = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";

/// SHA-256 от пустой строки (независимый оракул: `sha256sum`).
const EMPTY_SHA256: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

/// SHA-256 от строки `"abc"` (независимый оракул: `sha256sum`).
const ABC_SHA256: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

/// Seed из RFC 8032 TEST 1.
const RFC8032_SEED: [u8; 32] = [
    0x9d, 0x61, 0xb1, 0x9d, 0xef, 0xfd, 0x5a, 0x60, 0xba, 0x84, 0x4a, 0xf4, 0x92, 0xec, 0x2c, 0xc4,
    0x44, 0x49, 0xc5, 0x69, 0x7b, 0x32, 0x69, 0x19, 0x70, 0x3b, 0xac, 0x03, 0x1c, 0xae, 0x7f, 0x60,
];

/// Публичный ключ RFC 8032 TEST 1 для `RFC8032_SEED`.
const RFC8032_PUBLIC: [u8; 32] = [
    0xd7, 0x5a, 0x98, 0x01, 0x82, 0xb1, 0x0a, 0xb7, 0xd5, 0x4b, 0xfe, 0xd3, 0xc9, 0x64, 0x07, 0x3a,
    0x0e, 0xe1, 0x72, 0xf3, 0xda, 0xa6, 0x23, 0x25, 0xaf, 0x02, 0x1a, 0x68, 0xf7, 0x07, 0x51, 0x1a,
];

/// Подпись RFC 8032 TEST 1 над пустым сообщением.
const RFC8032_SIGNATURE: [u8; 64] = [
    0xe5, 0x56, 0x43, 0x00, 0xc3, 0x60, 0xac, 0x72, 0x90, 0x86, 0xe2, 0xcc, 0x80, 0x6e, 0x82, 0x8a,
    0x84, 0x87, 0x7f, 0x1e, 0xb8, 0xe5, 0xd9, 0x74, 0xd8, 0x73, 0xe0, 0x65, 0x22, 0x49, 0x01, 0x55,
    0x5f, 0xb8, 0x82, 0x15, 0x90, 0xa3, 0x3b, 0xac, 0xc6, 0x1e, 0x39, 0x70, 0x1c, 0xf9, 0xb4, 0x6b,
    0xd2, 0x5b, 0xf5, 0xf0, 0x59, 0x5b, 0xbe, 0x24, 0x65, 0x51, 0x41, 0x43, 0x8e, 0x7a, 0x10, 0x0b,
];

/// Temp-каталог под тест: уникальный суффикс, удаляется в `Drop`.
struct TempDir {
    path: PathBuf,
}

impl TempDir {
    /// Создаёт уникальный каталог под `std::env::temp_dir()`.
    fn new(tag: &str) -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!(
            "discord-updater-{tag}-{}-{nanos}-{seq}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("создать временный каталог");
        Self { path }
    }

    /// Путь внутри временного каталога.
    fn join(&self, name: &str) -> PathBuf {
        self.path.join(name)
    }

    /// Корневой путь временного каталога.
    fn root(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        // Очистка best-effort: ошибка удаления не должна маскировать результат теста.
        let _ = fs::remove_dir_all(&self.path);
    }
}

/// Публичный ключ Ed25519 из seed (детерминированно, без сети).
fn public_key(seed: &[u8; 32]) -> [u8; 32] {
    let pair = Ed25519KeyPair::from_seed_unchecked(seed).expect("ключ из seed");
    let bytes = pair.public_key().as_ref();
    let mut out = [0u8; 32];
    out.copy_from_slice(bytes);
    out
}

/// JSON манифеста с одним артефактом `name` и указанным хэшем.
///
/// Манифест собирается через `serde_json`, чтобы в фикстуру можно было подставить
/// в том числе небезопасные имена (`sub\dir`, управляющие символы и т. п.) — экранирование
/// делает сериализатор, а не ручной `format!`.
fn manifest_json(version: &str, name: &str, sha256: &str) -> String {
    serde_json::json!({
        "version": version,
        "released_at": "2026-10-22T00:00:00Z",
        "artifacts": [{
            "name": name,
            "url": "https://example.invalid/app-bin",
            "sha256": sha256,
            "size": 5,
        }],
    })
    .to_string()
}

/// Разобранный (но не подписанный) манифест с одним артефактом `app`.
/// Только для инспекции: планировать по нему нельзя — `plan_update` принимает
/// исключительно `VerifiedManifest`.
fn manifest(version: &str, sha256: &str) -> Manifest {
    let json = manifest_json(version, "app", sha256);
    parse_manifest(json.as_bytes()).expect("валидный манифест")
}

/// Подписанный манифест: тело подписывается тестовым seed'ом и проверяется ключом.
fn signed_manifest(version: &str, sha256: &str) -> VerifiedManifest {
    signed_manifest_named(version, "app", sha256)
}

/// Подписанный манифест с артефактом под заданным именем.
fn signed_manifest_named(version: &str, name: &str, sha256: &str) -> VerifiedManifest {
    let body = manifest_json(version, name, sha256);
    let signed = sign_manifest(body.as_bytes(), &SEED).expect("подпись");
    verify_manifest(&signed, &public_key(&SEED)).expect("подпись должна совпасть")
}

/// Записывает файл с указанным содержимым, создавая родительский каталог.
fn write_file(path: &Path, content: &[u8]) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("создать родительский каталог");
    }
    fs::write(path, content).expect("записать файл");
}

/// Читает файл в строку.
fn read_to_string(path: &Path) -> String {
    fs::read_to_string(path).expect("прочитать файл")
}

#[test]
fn manifest_parse_k() {
    let json = manifest_json("1.2.3", "app", HELLO_SHA256);
    let parsed = parse_manifest(json.as_bytes()).expect("валидный манифест");

    assert_eq!(
        parsed.version,
        Version {
            major: 1,
            minor: 2,
            patch: 3,
            pre: None
        }
    );
    assert_eq!(parsed.released_at, "2026-10-22T00:00:00Z");
    assert_eq!(parsed.artifacts.len(), 1);
    assert_eq!(parsed.artifacts[0].name, "app");
    assert_eq!(parsed.artifacts[0].url, "https://example.invalid/app-bin");
    assert_eq!(parsed.artifacts[0].sha256, HELLO_SHA256);
    assert_eq!(parsed.artifacts[0].size, 5);
}

#[test]
fn manifest_parse_pre_release_k() {
    let json = manifest_json("1.2.3-beta.1", "app", HELLO_SHA256);
    let parsed = parse_manifest(json.as_bytes()).expect("валидный манифест");

    assert_eq!(
        parsed.version,
        Version {
            major: 1,
            minor: 2,
            patch: 3,
            pre: Some("beta.1".to_string())
        }
    );
}

#[test]
fn manifest_invalid_json_k() {
    let err =
        parse_manifest(b"{\"version\": \"1.0.0\", ").expect_err("битый JSON должен отвергаться");
    assert!(
        matches!(err, ManifestError::InvalidJson { .. }),
        "ожидался InvalidJson, получено {err:?}"
    );
}

#[test]
fn manifest_missing_field_k() {
    let json = r#"{"artifacts":[]}"#;
    let err = parse_manifest(json.as_bytes()).expect_err("нет version ⇒ ошибка");
    assert_eq!(err, ManifestError::MissingField { field: "version" });
}

#[test]
fn manifest_invalid_version_k() {
    let json = manifest_json("1.2.x", "app", HELLO_SHA256);
    let err = parse_manifest(json.as_bytes()).expect_err("1.2.x ⇒ ошибка");
    assert_eq!(
        err,
        ManifestError::InvalidVersion {
            value: "1.2.x".to_string()
        }
    );
}

#[test]
fn manifest_invalid_digest_k() {
    let json = manifest_json("1.2.3", "app", "0123456789");
    let err = parse_manifest(json.as_bytes()).expect_err("короткий хэш ⇒ ошибка");
    assert_eq!(
        err,
        ManifestError::InvalidDigest {
            artifact: "app".to_string(),
            value: "0123456789".to_string()
        }
    );
}

#[test]
fn manifest_duplicate_artifact_k() {
    let json = format!(
        r#"{{"version":"1.0.0","artifacts":[{{"name":"app","url":"u","sha256":"{HELLO_SHA256}","size":1}},{{"name":"app","url":"u2","sha256":"{HELLO_SHA256}","size":2}}]}}"#
    );
    let err = parse_manifest(json.as_bytes()).expect_err("дубликат ⇒ ошибка");
    assert_eq!(
        err,
        ManifestError::DuplicateArtifact {
            name: "app".to_string()
        }
    );
}

#[test]
fn signature_ok_k() {
    let body = manifest_json("1.2.3", "app", HELLO_SHA256);
    let signed = sign_manifest(body.as_bytes(), &SEED).expect("подпись");

    assert_eq!(
        signed.len(),
        body.len() + 64,
        "блоб = body || signature[64]"
    );
    assert_eq!(&signed[..body.len()], body.as_bytes(), "тело не изменено");

    let verified = verify_manifest(&signed, &public_key(&SEED)).expect("подпись должна совпасть");
    assert_eq!(verified.version().major, 1);
    assert_eq!(verified.artifacts()[0].name, "app");
}

#[test]
fn signature_bad_k() {
    let body = manifest_json("1.2.3", "app", HELLO_SHA256);
    let mut signed = sign_manifest(body.as_bytes(), &SEED).expect("подпись");
    let last = signed.len() - 1;
    signed[last] ^= 0xff;

    let err = verify_manifest(&signed, &public_key(&SEED)).expect_err("битая подпись ⇒ ошибка");
    assert_eq!(err, ManifestError::InvalidSignature);
}

#[test]
fn signature_wrong_key_k() {
    let body = manifest_json("1.2.3", "app", HELLO_SHA256);
    let signed = sign_manifest(body.as_bytes(), &SEED).expect("подпись");

    let other_seed = [8u8; 32];
    let err = verify_manifest(&signed, &public_key(&other_seed)).expect_err("чужой ключ ⇒ ошибка");
    assert_eq!(err, ManifestError::InvalidSignature);
}

#[test]
fn signature_tampered_body_k() {
    let body = manifest_json("1.2.3", "app", HELLO_SHA256);
    let mut signed = sign_manifest(body.as_bytes(), &SEED).expect("подпись");
    // Подмена первого байта тела: длина блоба сохраняется, подпись перестаёт совпадать.
    signed[0] = b'[';

    let err = verify_manifest(&signed, &public_key(&SEED)).expect_err("подмена тела ⇒ ошибка");
    assert_eq!(err, ManifestError::InvalidSignature);
}

#[test]
fn signature_missing_k() {
    let err = verify_manifest(&[0u8; 10], &public_key(&SEED)).expect_err("короткий блоб ⇒ ошибка");
    assert_eq!(err, ManifestError::SignatureMissing);

    let err = verify_manifest(&[0u8; 63], &public_key(&SEED)).expect_err("63 байта ⇒ ошибка");
    assert_eq!(err, ManifestError::SignatureMissing);
}

#[test]
fn version_order_k() {
    let v = |text: &str| Version::parse(text).expect("валидная версия");

    assert!(v("1.10.0").is_newer_than(&v("1.9.9")));
    assert!(v("2.0.0").is_newer_than(&v("1.99.99")));
    assert!(v("1.0.0").is_newer_than(&v("1.0.0-beta")));
    assert!(v("1.0.0-beta.2").is_newer_than(&v("1.0.0-beta.1")));
    assert!(!v("1.0.0").is_newer_than(&v("1.0.0")));
    assert!(!v("1.0.0-beta").is_newer_than(&v("1.0.0")));
    assert!(!v("1.9.9").is_newer_than(&v("1.10.0")));
    assert!(!v("0.9.9").is_newer_than(&v("1.0.0")));
}

#[test]
fn version_parse_rejects_k() {
    for bad in [
        "", "1.2", "1.2.3.4", "1.2.x", "v1.2.3", "01.2.3", "1.02.3", "1.2.03", "1.2.3-", "1.2.3 ",
        "1.2.-3",
    ] {
        let err = Version::parse(bad).expect_err("невалидная версия должна отвергаться");
        assert_eq!(
            err,
            ManifestError::InvalidVersion {
                value: bad.to_string()
            },
            "версия {bad:?} должна отвергаться"
        );
    }

    for good in ["0.0.1", "1.2.3", "10.20.30", "1.2.3-beta.1", "1.2.3-rc-2"] {
        assert!(
            Version::parse(good).is_ok(),
            "версия {good:?} должна приниматься"
        );
    }
}

#[test]
fn parse_manifest_stays_plain_manifest_k() {
    // `parse_manifest` — только разбор и инспекция: возвращается обычный `Manifest`,
    // из которого нельзя построить план (типы не совпадают).
    let parsed: Manifest = manifest("1.2.3", HELLO_SHA256);
    assert_eq!(parsed.version.major, 1);
    assert_eq!(parsed.artifacts[0].name, "app");
}

#[test]
fn verified_manifest_from_signed_bytes_k() {
    // Подписанный манифест получается только через `verify_manifest`.
    let body = manifest_json("1.2.3", "app", HELLO_SHA256);
    let signed = sign_manifest(body.as_bytes(), &SEED).expect("подпись");

    let verified: VerifiedManifest =
        verify_manifest(&signed, &public_key(&SEED)).expect("подпись должна совпасть");

    assert_eq!(
        verified.version(),
        &Version {
            major: 1,
            minor: 2,
            patch: 3,
            pre: None,
        }
    );
    assert_eq!(verified.artifacts().len(), 1);
    assert_eq!(verified.artifacts()[0].name, "app");
    assert_eq!(
        verified.artifact("app").map(|a| a.sha256.as_str()),
        Some(HELLO_SHA256)
    );
    assert!(verified.artifact("нет-такого").is_none());
}

#[test]
fn verified_manifest_requires_valid_signature_k() {
    // Неверная подпись ⇒ VerifiedManifest не получен вовсе.
    let body = manifest_json("1.2.3", "app", HELLO_SHA256);
    let mut signed = sign_manifest(body.as_bytes(), &SEED).expect("подпись");
    let last = signed.len() - 1;
    signed[last] ^= 0xff;

    let err = verify_manifest(&signed, &public_key(&SEED)).expect_err("битая подпись ⇒ ошибка");
    assert_eq!(err, ManifestError::InvalidSignature);
}

#[test]
fn plan_from_verified_manifest_k() {
    // План строится из подписанного манифеста.
    let dir = TempDir::new("verified-plan");
    let m = signed_manifest("1.2.3", HELLO_SHA256);
    let current = Version::parse("1.0.0").expect("версия");

    let plan = plan_update(&current, &m, "app", dir.root()).expect("план строится");
    assert_eq!(plan.to_version.major, 1);
    assert_eq!(plan.artifact, "app");
}

#[test]
fn plan_not_newer_k() {
    let m = signed_manifest("1.2.3", HELLO_SHA256);
    let current = Version::parse("1.2.3").expect("версия");

    let err = plan_update(&current, &m, "app", Path::new("/tmp"))
        .expect_err("текущая версия не старше ⇒ ошибка");
    assert_eq!(
        err,
        PlanError::NotNewer {
            current: "1.2.3".to_string(),
            candidate: "1.2.3".to_string()
        }
    );

    let err = plan_update(
        &Version::parse("2.0.0").expect("версия"),
        &m,
        "app",
        Path::new("/tmp"),
    )
    .expect_err("текущая версия новее ⇒ ошибка");
    assert!(matches!(err, PlanError::NotNewer { .. }), "{err:?}");
}

#[test]
fn plan_unknown_artifact_k() {
    let m = signed_manifest("1.2.3", HELLO_SHA256);
    let current = Version::parse("1.0.0").expect("версия");

    let err = plan_update(&current, &m, "нет-такого", Path::new("/tmp"))
        .expect_err("нет артефакта ⇒ ошибка");
    assert_eq!(
        err,
        PlanError::UnknownArtifact {
            name: "нет-такого".to_string()
        }
    );
}

#[test]
fn plan_steps_k() {
    let dir = TempDir::new("plan-steps");
    let m = signed_manifest("1.2.3", HELLO_SHA256);
    let current = Version::parse("1.0.0").expect("версия");

    let plan = plan_update(&current, &m, "app", dir.root()).expect("план строится");

    assert_eq!(plan.from_version, current);
    assert_eq!(plan.to_version, *m.version());
    assert_eq!(plan.artifact, "app");

    let kinds: Vec<&str> = plan
        .steps
        .iter()
        .map(|step| match step {
            PlanStep::Download { .. } => "download",
            PlanStep::VerifyDigest { .. } => "verify",
            PlanStep::Stage { .. } => "stage",
            PlanStep::Swap { .. } => "swap",
            PlanStep::Rollback { .. } => "rollback",
            PlanStep::RemoveBackup { .. } => "remove",
        })
        .collect();
    assert_eq!(
        kinds,
        vec!["download", "verify", "stage", "swap", "remove"],
        "порядок шагов по контракту"
    );

    // Все пути плана лежат внутри install_dir.
    for step in &plan.steps {
        let paths: Vec<&Path> = match step {
            PlanStep::Download { to, .. } => vec![to.as_path()],
            PlanStep::Stage { from, to, .. } => vec![from.as_path(), to.as_path()],
            PlanStep::Swap { from, to, backup } => {
                vec![from.as_path(), to.as_path(), backup.as_path()]
            }
            PlanStep::Rollback { backup, to } => vec![backup.as_path(), to.as_path()],
            PlanStep::RemoveBackup { backup } => vec![backup.as_path()],
            PlanStep::VerifyDigest { .. } => vec![],
        };
        for path in paths {
            assert!(
                path.starts_with(dir.root()),
                "путь {path:?} вне install_dir {:?}",
                dir.root()
            );
        }
    }

    // Ожидаемый хэш в шаге проверки совпадает с манифестом.
    let verify = plan
        .steps
        .iter()
        .find_map(|step| match step {
            PlanStep::VerifyDigest {
                artifact,
                expected,
                actual,
            } => Some((artifact.clone(), expected.clone(), actual.clone())),
            _ => None,
        })
        .expect("шаг проверки хэша");
    assert_eq!(verify.0, "app");
    assert_eq!(verify.1, HELLO_SHA256);
}

#[test]
fn digest_mismatch_k() {
    let dir = TempDir::new("digest-mismatch");
    let target = dir.join("app");
    let source = dir.join("downloaded.bin");
    write_file(&target, b"old-content");
    write_file(&source, b"other-content");

    let m = signed_manifest("1.2.3", HELLO_SHA256);
    let current = Version::parse("1.0.0").expect("версия");
    let plan = plan_update(&current, &m, "app", dir.root()).expect("план");

    let err = apply_local_steps(&plan, &source).expect_err("чужое содержимое ⇒ ошибка");
    assert_eq!(
        err,
        PlanError::DigestMismatch {
            artifact: "app".to_string()
        }
    );

    // Цель не тронута, лишних файлов не появилось.
    assert_eq!(read_to_string(&target), "old-content");
    assert!(!dir.join("app.bak").exists(), "backup не создавался");
}

#[test]
fn staged_swap_k() {
    let dir = TempDir::new("staged-swap");
    let target = dir.join("app");
    let source = dir.join("downloaded.bin");
    write_file(&target, b"old-content");
    write_file(&source, b"hello");

    let m = signed_manifest("1.2.3", HELLO_SHA256);
    let current = Version::parse("1.0.0").expect("версия");
    let plan = plan_update(&current, &m, "app", dir.root()).expect("план");

    apply_local_steps(&plan, &source).expect("замена выполняется");

    assert_eq!(
        read_to_string(&target),
        "hello",
        "цель содержит новое содержимое"
    );
    assert_eq!(
        read_to_string(&dir.join("app.bak")),
        "old-content",
        "backup хранит прежнее"
    );
}

#[test]
fn rollback_k() {
    let dir = TempDir::new("rollback");
    let target = dir.join("app");
    let source = dir.join("downloaded.bin");
    write_file(&target, b"old-content");
    write_file(&source, b"hello");

    let m = signed_manifest("1.2.3", HELLO_SHA256);
    let current = Version::parse("1.0.0").expect("версия");
    let plan = plan_update(&current, &m, "app", dir.root()).expect("план");

    apply_local_steps(&plan, &source).expect("замена выполняется");
    rollback(&plan).expect("откат выполняется");

    assert_eq!(
        read_to_string(&target),
        "old-content",
        "прежнее содержимое возвращено"
    );
    assert!(!dir.join("app.bak").exists(), "backup снят");
}

#[test]
fn swap_failure_rollback_k() {
    let dir = TempDir::new("swap-failure");
    let target = dir.join("app");
    let source = dir.join("downloaded.bin");
    write_file(&target, b"old-content");
    write_file(&source, b"hello");

    // План со сломанной подменой: staged-файл отсутствует, поэтому rename
    // цели падает и срабатывает откат на backup.
    let plan = UpdatePlan {
        from_version: Version::parse("1.0.0").expect("версия"),
        to_version: Version::parse("1.2.3").expect("версия"),
        artifact: "app".to_string(),
        steps: vec![
            PlanStep::Download {
                url: "https://example.invalid/app-bin".to_string(),
                to: dir.join("stage/app.part"),
            },
            PlanStep::VerifyDigest {
                artifact: "app".to_string(),
                expected: HELLO_SHA256.to_string(),
                actual: String::new(),
            },
            PlanStep::Stage {
                artifact: "app".to_string(),
                from: dir.join("stage/app.part"),
                to: dir.join("stage/app.new"),
            },
            PlanStep::Swap {
                from: dir.join("stage/app.absent"),
                to: dir.join("app"),
                backup: dir.join("app.bak"),
            },
            PlanStep::RemoveBackup {
                backup: dir.join("app.bak"),
            },
        ],
    };

    let err = apply_local_steps(&plan, &source).expect_err("сбой подмены ⇒ ошибка");
    assert!(
        matches!(err, PlanError::Io { .. }),
        "ожидался Io, получено {err:?}"
    );
    assert_eq!(
        read_to_string(&target),
        "old-content",
        "прежнее содержимое цели цело после отката"
    );
}

#[test]
fn sha256_hex_known_k() {
    assert_eq!(sha256_hex(b""), EMPTY_SHA256);
    assert_eq!(sha256_hex(b"abc"), ABC_SHA256);
    assert_eq!(sha256_hex(b"hello"), HELLO_SHA256);

    let hex = sha256_hex("произвольные байты".as_bytes());
    assert_eq!(hex.len(), 64, "hex должен быть ровно из 64 символов");
    assert!(
        hex.chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()),
        "hex должен быть в нижнем регистре: {hex}"
    );
}

#[test]
fn rfc8032_test_vector_k() {
    // Независимый оракул: RFC 8032 TEST 1 (подпись над пустым сообщением).
    let signed = sign_manifest(b"", &RFC8032_SEED).expect("подпись");
    assert_eq!(signed.len(), 64);
    assert_eq!(
        signed.as_slice(),
        RFC8032_SIGNATURE.as_slice(),
        "подпись должна совпасть с вектором RFC 8032"
    );
    assert_eq!(
        public_key(&RFC8032_SEED),
        RFC8032_PUBLIC,
        "публичный ключ должен совпасть с вектором RFC 8032"
    );

    // Тот же вектор должен проверяться и нашей проверкой подписи.
    let body = manifest_json("1.0.0", "app", EMPTY_SHA256);
    let signed = sign_manifest(body.as_bytes(), &RFC8032_SEED).expect("подпись");
    let verified = verify_manifest(&signed, &RFC8032_PUBLIC).expect("подпись должна совпасть");
    assert_eq!(verified.artifacts()[0].sha256, EMPTY_SHA256);
}

/// Имена артефактов, которые обязаны быть отвергнуты при построении плана.
const UNSAFE_ARTIFACT_NAMES: [&str; 9] = [
    "../../outside",
    "../evil",
    "sub/dir",
    "sub\\dir",
    "",
    ".",
    "..",
    "with space",
    "bell\u{7}",
];

/// Все пути плана (включая staging и backup) — список для проверок «внутри install_dir».
fn plan_paths(plan: &UpdatePlan) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    for step in &plan.steps {
        match step {
            PlanStep::Download { to, .. } => paths.push(to.clone()),
            PlanStep::Stage { from, to, .. } => {
                paths.push(from.clone());
                paths.push(to.clone());
            }
            PlanStep::Swap { from, to, backup } => {
                paths.push(from.clone());
                paths.push(to.clone());
                paths.push(backup.clone());
            }
            PlanStep::Rollback { backup, to } => {
                paths.push(backup.clone());
                paths.push(to.clone());
            }
            PlanStep::RemoveBackup { backup } => paths.push(backup.clone()),
            PlanStep::VerifyDigest { .. } => {}
        }
    }
    paths
}

#[test]
fn plan_rejects_unsafe_artifact_name_k() {
    let dir = TempDir::new("unsafe-name");
    let current = Version::parse("1.0.0").expect("версия");

    for bad in UNSAFE_ARTIFACT_NAMES {
        let m = signed_manifest_named("1.2.3", bad, HELLO_SHA256);

        let err = plan_update(&current, &m, bad, dir.root())
            .expect_err("небезопасное имя артефакта должно отвергаться");
        assert_eq!(
            err,
            PlanError::UnsafeArtifactName {
                name: bad.to_string()
            },
            "имя {bad:?} должно отвергаться с UnsafeArtifactName"
        );
    }
}

#[test]
fn plan_keeps_all_paths_inside_install_dir_k() {
    let dir = TempDir::new("inside-install-dir");
    let current = Version::parse("1.0.0").expect("версия");
    let m = signed_manifest("1.2.3", HELLO_SHA256);

    let plan = plan_update(&current, &m, "app", dir.root()).expect("план строится");

    let paths = plan_paths(&plan);
    assert!(!paths.is_empty(), "в плане должны быть пути");
    for path in paths {
        assert!(
            path.starts_with(dir.root()),
            "путь {path:?} вне install_dir {:?}",
            dir.root()
        );
    }
}

#[test]
fn plan_accepts_safe_artifact_name_k() {
    let dir = TempDir::new("safe-name");
    let current = Version::parse("1.0.0").expect("версия");

    for good in ["app", "discord-1.2.3.bin", "app_2026-10-22", "a.b.c"] {
        let m = signed_manifest_named("1.2.3", good, HELLO_SHA256);
        let plan = plan_update(&current, &m, good, dir.root())
            .unwrap_or_else(|e| panic!("валидное имя {good:?} должно приниматься: {e:?}"));
        for path in plan_paths(&plan) {
            assert!(
                path.starts_with(dir.root()),
                "путь {path:?} вне install_dir {:?}",
                dir.root()
            );
        }
    }
}

#[test]
fn rollback_replaces_existing_target_k() {
    // Цель существует (новая версия), backup существует (прежняя версия).
    // Откат должен вернуть прежнее содержимое цели, убрать backup и не оставить
    // временных файлов.
    let dir = TempDir::new("rollback-existing");
    let target = dir.join("app");
    let source = dir.join("downloaded.bin");
    write_file(&target, b"old-content");
    write_file(&source, b"hello");

    let m = signed_manifest("1.2.3", HELLO_SHA256);
    let current = Version::parse("1.0.0").expect("версия");
    let plan = plan_update(&current, &m, "app", dir.root()).expect("план");

    apply_local_steps(&plan, &source).expect("замена выполняется");
    assert_eq!(read_to_string(&target), "hello");

    rollback(&plan).expect("откат выполняется");

    assert_eq!(
        read_to_string(&target),
        "old-content",
        "прежнее содержимое возвращено"
    );
    assert!(!dir.join("app.bak").exists(), "backup снят");
    assert!(
        !dir.join("app.failed").exists(),
        "временный файл отката не должен остаться"
    );
}

#[test]
fn rollback_refuses_occupied_temp_name_k() {
    // Временное имя `<имя>.failed` уже занято ⇒ откат не выполняется:
    // цель и backup остаются нетронутыми, backup не расходуется.
    let dir = TempDir::new("rollback-occupied");
    let target = dir.join("app");
    let source = dir.join("downloaded.bin");
    write_file(&target, b"old-content");
    write_file(&source, b"hello");

    let m = signed_manifest("1.2.3", HELLO_SHA256);
    let current = Version::parse("1.0.0").expect("версия");
    let plan = plan_update(&current, &m, "app", dir.root()).expect("план");

    apply_local_steps(&plan, &source).expect("замена выполняется");
    // Занятое временное имя: каталог, который нельзя перезаписать файлом.
    fs::create_dir_all(dir.join("app.failed")).expect("создать каталог-препятствие");

    let err = rollback(&plan).expect_err("занятое временное имя ⇒ отказ");
    assert!(
        matches!(err, PlanError::RollbackFailed { .. }),
        "ожидался RollbackFailed, получено {err:?}"
    );
    assert_eq!(read_to_string(&target), "hello", "цель не тронута");
    assert!(dir.join("app.bak").exists(), "backup не тронут");
}

#[test]
fn rollback_missing_backup_keeps_target_k() {
    // Backup недоступен ⇒ отказ, исходная цель не уничтожена.
    let dir = TempDir::new("rollback-missing");
    let target = dir.join("app");
    let source = dir.join("downloaded.bin");
    write_file(&target, b"old-content");
    write_file(&source, b"hello");

    let m = signed_manifest("1.2.3", HELLO_SHA256);
    let current = Version::parse("1.0.0").expect("версия");
    let plan = plan_update(&current, &m, "app", dir.root()).expect("план");

    apply_local_steps(&plan, &source).expect("замена выполняется");
    fs::remove_file(dir.join("app.bak")).expect("убрать backup");

    let err = rollback(&plan).expect_err("backup отсутствует ⇒ отказ");
    assert!(
        matches!(err, PlanError::RollbackFailed { .. }),
        "ожидался RollbackFailed, получено {err:?}"
    );
    assert_eq!(
        read_to_string(&target),
        "hello",
        "исходная цель не уничтожена"
    );
    assert!(
        !dir.join("app.failed").exists(),
        "временный файл не создавался"
    );
}

#[test]
fn rollback_rejects_corrupted_backup_k() {
    // Backup повреждён (сбой питания в момент swap) ⇒ отказ до восстановления:
    // цель не заменена, backup не тронут.
    let dir = TempDir::new("rollback-corrupted");
    let target = dir.join("app");
    let source = dir.join("downloaded.bin");
    write_file(&target, b"old-content");
    write_file(&source, b"hello");

    let m = signed_manifest("1.2.3", HELLO_SHA256);
    let current = Version::parse("1.0.0").expect("версия");
    let plan = plan_update(&current, &m, "app", dir.root()).expect("план");

    apply_local_steps(&plan, &source).expect("замена выполняется");
    // Порча backup: содержимое перестаёт соответствовать зафиксированному хэшу.
    write_file(&dir.join("app.bak"), b"corrupted-bytes");

    let err = rollback(&plan).expect_err("повреждённый backup ⇒ отказ");
    assert!(
        matches!(err, PlanError::BackupCorrupted { .. }),
        "ожидался BackupCorrupted, получено {err:?}"
    );
    assert_eq!(read_to_string(&target), "hello", "цель не заменена");
    assert_eq!(
        read_to_string(&dir.join("app.bak")),
        "corrupted-bytes",
        "backup не тронут"
    );
}

// ---------------------------------------------------------------------------
// BE-P3-07 (task-179): эндпоинты автообновления, расписание backoff и
// состояния сплэш-экрана. Оракул — `/home/sanya/re-deep/main-arch.md` §5:
// `app #19192 @3231` (эндпоинты), `app #55405 @26169` (disableUpdater),
// `app #53006 @97270..@99200` (сплэш-автомат и backoff), `app #2278`/`#47924`
// (Linux: `download/{channel}?platform=linux&format=`, feed, `modules/{channel}`).
// ---------------------------------------------------------------------------

#[test]
fn updater_endpoint_constants_k() {
    assert_eq!(NEW_UPDATE_ENDPOINT, "https://updates.discord.com/");
    assert_eq!(DEFAULT_UPDATE_ENDPOINT, "https://discord.com/api");
    assert_eq!(DEFAULT_UPDATE_BACKOFF_MAX_MS, 30_000);
    assert_eq!(
        SCHEDULED_UPDATE_BACKOFF_MS,
        [
            2_000, 3_000, 3_000, 4_000, 5_000, 7_000, 10_000, 12_000, 15_000
        ],
        "ScheduledBackoff([2e3,3e3,3e3,4e3,5e3,7e3,1e4,12e3,15e3], !0)"
    );
}

#[test]
fn release_channel_as_str_k() {
    assert_eq!(ReleaseChannel::Stable.as_str(), "stable");
    assert_eq!(ReleaseChannel::Ptb.as_str(), "ptb");
    assert_eq!(ReleaseChannel::Canary.as_str(), "canary");
    assert_eq!(ReleaseChannel::Development.as_str(), "development");
}

#[test]
fn should_disable_updater_k() {
    // app #55405 @26169:
    // disableUpdater || ("development" === releaseChannel && (standaloneModules || localModulesRoot))
    let channels = [
        ReleaseChannel::Stable,
        ReleaseChannel::Ptb,
        ReleaseChannel::Canary,
        ReleaseChannel::Development,
    ];
    for channel in channels {
        assert!(
            should_disable_updater(channel, true, false),
            "{channel:?}: явный disableUpdater всегда отключает апдейтер"
        );
        assert!(
            should_disable_updater(channel, true, true),
            "{channel:?}: явный disableUpdater всегда отключает апдейтер"
        );
    }
    assert!(
        should_disable_updater(ReleaseChannel::Development, false, true),
        "development + standalone/local modules ⇒ апдейтер отключён"
    );
    assert!(
        !should_disable_updater(ReleaseChannel::Development, false, false),
        "development без standalone/local modules ⇒ апдейтер работает"
    );
    assert!(!should_disable_updater(ReleaseChannel::Stable, false, true));
    assert!(!should_disable_updater(ReleaseChannel::Ptb, false, true));
    assert!(!should_disable_updater(ReleaseChannel::Canary, false, true));
}

#[test]
fn linux_package_format_query_value_k() {
    assert_eq!(LinuxPackageFormat::Deb.as_query_value(), "deb");
    assert_eq!(LinuxPackageFormat::TarGz.as_query_value(), "tar.gz");
}

#[test]
fn linux_download_url_k() {
    // splashScreenPreload.js: LINUX_DOWNLOAD_URL_BASE =
    // "https://discord.com/api/download/<channel>?platform=linux&format="
    assert_eq!(
        build_linux_download_url(ReleaseChannel::Stable, LinuxPackageFormat::Deb),
        "https://discord.com/api/download/stable?platform=linux&format=deb"
    );
    assert_eq!(
        build_linux_download_url(ReleaseChannel::Canary, LinuxPackageFormat::TarGz),
        "https://discord.com/api/download/canary?platform=linux&format=tar.gz"
    );
    assert_eq!(
        build_linux_download_url(ReleaseChannel::Ptb, LinuxPackageFormat::Deb),
        "https://discord.com/api/download/ptb?platform=linux&format=deb"
    );
}

#[test]
fn linux_feed_url_k() {
    // app #47924 @62552: feedURL = <endpoint>/updates/<channel>?platform=linux&version=<cur>
    let current = Version::parse("1.0.9035").expect("версия");
    assert_eq!(
        build_linux_feed_url(DEFAULT_UPDATE_ENDPOINT, ReleaseChannel::Stable, &current)
            .expect("feed собирается"),
        "https://discord.com/api/updates/stable?platform=linux&version=1.0.9035"
    );

    // Хвостовой слэш и пробелы по краям не дают двойного разделителя.
    assert_eq!(
        build_linux_feed_url(
            "  https://updates.discord.com/  ",
            ReleaseChannel::Development,
            &current
        )
        .expect("feed собирается"),
        "https://updates.discord.com/updates/development?platform=linux&version=1.0.9035"
    );

    // Pre-release суффикс в version-параметр не попадает: только major.minor.patch.
    let pre = Version::parse("1.2.3-beta.1").expect("версия");
    assert_eq!(
        build_linux_feed_url(DEFAULT_UPDATE_ENDPOINT, ReleaseChannel::Ptb, &pre)
            .expect("feed собирается"),
        "https://discord.com/api/updates/ptb?platform=linux&version=1.2.3"
    );
}

#[test]
fn url_builders_reject_bad_endpoint_k() {
    let current = Version::parse("1.0.0").expect("версия");
    let bad_endpoints = [
        "",
        "   ",
        "updates.discord.com",
        "ftp://updates.discord.com",
        "https://",
        "https:///",
        "https://a b",
    ];
    for bad in bad_endpoints {
        let err = build_linux_feed_url(bad, ReleaseChannel::Stable, &current)
            .expect_err("невалидный эндпоинт должен быть отвергнут");
        assert!(
            matches!(err, ManifestError::InvalidEndpoint { .. }),
            "{bad:?}: ожидался InvalidEndpoint, получено {err:?}"
        );

        let err = build_modules_base_url(bad, ReleaseChannel::Stable)
            .expect_err("невалидный эндпоинт должен быть отвергнут");
        assert!(
            matches!(err, ManifestError::InvalidEndpoint { .. }),
            "{bad:?}: ожидался InvalidEndpoint, получено {err:?}"
        );
    }
}

#[test]
fn modules_base_url_k() {
    // app #47924 @61321: remoteBaseURL = <endpoint>/modules/<channel>
    assert_eq!(
        build_modules_base_url("https://updates.discord.com/", ReleaseChannel::Canary)
            .expect("modules собирается"),
        "https://updates.discord.com/modules/canary"
    );
    assert_eq!(
        build_modules_base_url(DEFAULT_UPDATE_ENDPOINT, ReleaseChannel::Stable)
            .expect("modules собирается"),
        "https://discord.com/api/modules/stable"
    );
    assert_eq!(
        build_modules_base_url("https://discord.com/api///", ReleaseChannel::Ptb)
            .expect("modules собирается"),
        "https://discord.com/api/modules/ptb"
    );
}

#[test]
fn scheduled_backoff_sequence_k() {
    let mut backoff = ScheduledUpdateBackoff::new();
    let delays: Vec<u64> = (0..11).map(|_| backoff.next_delay_ms()).collect();
    assert_eq!(
        delays,
        vec![
            2_000, 3_000, 3_000, 4_000, 5_000, 7_000, 10_000, 12_000, 15_000, 15_000, 15_000
        ],
        "шаг по массиву с clamp на последнем значении"
    );

    backoff.succeed();
    assert_eq!(
        backoff.next_delay_ms(),
        2_000,
        "succeed() сбрасывает счётчик неудач"
    );
}

#[test]
fn scheduled_backoff_clamps_to_max_k() {
    // updateBackoffMaxMs = DESKTOP_TTI_UPDATE_BACKOFF_MAX_MS (по умолчанию 3e4).
    let mut backoff = ScheduledUpdateBackoff::default();
    for _ in 0..SCHEDULED_UPDATE_BACKOFF_MS.len() + 2 {
        assert!(
            backoff.next_delay_ms() <= DEFAULT_UPDATE_BACKOFF_MAX_MS,
            "задержка не превышает updateBackoffMaxMs"
        );
    }

    let mut tight = ScheduledUpdateBackoff::with_max_ms(2_500);
    assert_eq!(tight.next_delay_ms(), 2_000);
    assert_eq!(tight.next_delay_ms(), 2_500, "задержка усекается по max_ms");
    assert_eq!(tight.next_delay_ms(), 2_500, "усечение повторяется");

    assert_eq!(
        ScheduledUpdateBackoff::with_max_ms(0).next_delay_ms(),
        0,
        "max_ms = 0 ⇒ повторов без задержки"
    );
}

#[test]
fn scheduled_backoff_retry_seconds_k() {
    // app #53006 @98275: splashState.seconds = Math.round(delayMs / 1e3)
    let mut backoff = ScheduledUpdateBackoff::new();
    let seconds: Vec<u32> = (0..10).map(|_| backoff.retry_seconds()).collect();
    assert_eq!(seconds, vec![2, 3, 3, 4, 5, 7, 10, 12, 15, 15]);

    // Math.round(2500 / 1000) = 3 — проверка округления, а не усечения.
    let mut tight = ScheduledUpdateBackoff::with_max_ms(2_500);
    assert_eq!(tight.retry_seconds(), 2);
    assert_eq!(tight.retry_seconds(), 3, "Math.round(2500 / 1000) = 3");

    tight.succeed();
    assert_eq!(tight.retry_seconds(), 2, "succeed() сбрасывает расписание");
}

#[test]
fn splash_update_state_event_names_k() {
    // app/bundle.js #53006 updateSplashState(...) + splashScreenPreload.js.
    assert_eq!(
        SplashUpdateState::CheckingForUpdates.event_name(),
        "checking-for-updates"
    );
    assert_eq!(
        SplashUpdateState::DownloadingUpdates.event_name(),
        "downloading-updates"
    );
    assert_eq!(
        SplashUpdateState::InstallingUpdates.event_name(),
        "installing-updates"
    );
    assert_eq!(
        SplashUpdateState::UpdateFailure { seconds: 30 }.event_name(),
        "update-failure"
    );
    assert_eq!(
        SplashUpdateState::UpdateManually {
            latest_version: "1.0.9035".to_string()
        }
        .event_name(),
        "update-manually"
    );
    assert_eq!(SplashUpdateState::Launching.event_name(), "launching");

    // Payload-состояния обязаны сохранять значение: сплэш показывает таймер и версию.
    match (SplashUpdateState::UpdateFailure { seconds: 7 }) {
        SplashUpdateState::UpdateFailure { seconds } => assert_eq!(seconds, 7),
        other => panic!("ожидался UpdateFailure, получено {other:?}"),
    }
    match (SplashUpdateState::UpdateManually {
        latest_version: "2.0.0".to_string(),
    }) {
        SplashUpdateState::UpdateManually { latest_version } => assert_eq!(latest_version, "2.0.0"),
        other => panic!("ожидался UpdateManually, получено {other:?}"),
    }
}
