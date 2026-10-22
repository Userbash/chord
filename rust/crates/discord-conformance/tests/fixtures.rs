//! T-HARN-01: загрузчик фикстур и контракт путей (P0-04, B02).
//!
//! Курированные фикстуры обязательны; lab-файлы опциональны — при отсутствии лаборатории
//! тест печатает причину и завершается skip-ом, а не падением.

use discord_conformance::{
    ConformanceError, FixtureSource, fixtures_root, lab_root, load_bytes, load_text, manifest_path,
    manifest_text, resolve,
};
use std::fs;
use std::path::{Path, PathBuf};

/// Lab-файл опционален для обычного прогона, но `RE_REQUIRE_LAB=1` превращает его
/// отсутствие в ошибку: acceptance не должен молча засчитывать skip как pass.
fn lab_file_or_skip(path: &Path) -> bool {
    if path.is_file() {
        return true;
    }
    if std::env::var("RE_REQUIRE_LAB").is_ok_and(|value| value == "1") {
        panic!(
            "RE_REQUIRE_LAB задан, но lab-файл отсутствует: {} (RE_LAB_ROOT={})",
            path.display(),
            lab_root().display()
        );
    }
    eprintln!(
        "skip: нет lab-файла {} (RE_LAB_ROOT={})",
        path.display(),
        lab_root().display()
    );
    false
}

#[test]
fn manifest_is_readable_and_has_versioned_provenance() {
    let path = manifest_path();
    assert_eq!(
        path,
        fixtures_root().join("MANIFEST.json"),
        "манифест лежит в корне фикстур, а не в подкаталоге"
    );
    assert!(
        path.ends_with("fixtures/MANIFEST.json"),
        "ожидался <repo>/fixtures/MANIFEST.json, получен {}",
        path.display()
    );
    assert!(path.is_file(), "нет манифеста фикстур {}", path.display());
    let text = manifest_text().expect("манифест читается");
    assert!(text.contains("\"labRoot\""), "манифест фиксирует labRoot");
    assert!(text.contains("\"fixtures\""), "манифест содержит список");
    assert!(
        text.matches("\"md5\"").count() >= 12,
        "ожидается ≥12 записей с md5"
    );
    assert!(
        fixtures_root().is_dir(),
        "корень фикстур отсутствует: {}",
        fixtures_root().display()
    );
}

#[test]
fn repository_fixture_resolves() {
    let fixture = resolve("gateway/ready-stats.json").expect("READY-статистика есть в fixtures/");
    assert_eq!(fixture.source, FixtureSource::Repository);
    assert_eq!(fixture.name, "gateway/ready-stats.json");
    assert!(fixture.path.is_file());
}

#[test]
fn load_helpers_match_direct_read() {
    let fixture = resolve("gateway/ready-stats.json").expect("READY-статистика есть в fixtures/");
    let direct = fs::read(&fixture.path).expect("прямое чтение");
    assert!(!direct.is_empty());
    let via_fixture = fixture.read_bytes().expect("Fixture::read_bytes");
    let via_helper = load_bytes("gateway/ready-stats.json").expect("load_bytes");
    assert_eq!(via_fixture, direct);
    assert_eq!(via_helper, direct);
    let text = load_text("gateway/ready-stats.json").expect("load_text");
    assert_eq!(text.as_bytes(), direct.as_slice());
}

#[test]
fn lab_only_fixture_resolves_when_lab_is_present() {
    let lab_file = lab_root().join("gw-proto/zstd-frames-95974_1389.hexl");
    if !lab_file_or_skip(&lab_file) {
        return;
    }
    let fixture = resolve("gw-proto/zstd-frames-95974_1389.hexl").expect("lab-фикстура");
    assert_eq!(fixture.source, FixtureSource::Lab);
    assert_eq!(fixture.path, lab_file);
    let bytes = fixture.read_bytes().expect("lab read");
    assert!(bytes.len() > 100_000, "lab-поток непустой");
}

#[test]
fn invalid_names_and_missing_fixtures_are_typed_errors() {
    for name in ["", "../etc/passwd", "/etc/passwd"] {
        assert!(
            matches!(
                resolve(name),
                Err(ConformanceError::InvalidName { name: ref found }) if found == name
            ),
            "ожидался InvalidName для {name:?}"
        );
    }
    match resolve("__definitely_missing_fixture__") {
        Err(ConformanceError::NotFound {
            name,
            repository,
            lab,
        }) => {
            assert_eq!(name, "__definitely_missing_fixture__");
            assert!(repository.ends_with("__definitely_missing_fixture__"));
            assert!(lab.ends_with("__definitely_missing_fixture__"));
        }
        other => panic!("ожидался NotFound, получено {other:?}"),
    }
}

#[test]
fn roots_honor_environment_overrides_without_mutation() {
    if let Ok(value) = std::env::var("RE_FIXTURES_ROOT") {
        assert_eq!(fixtures_root(), PathBuf::from(value));
    } else {
        assert!(
            fixtures_root().ends_with("fixtures"),
            "по умолчанию корень — <repo>/fixtures: {}",
            fixtures_root().display()
        );
    }
    if let Ok(value) = std::env::var("RE_LAB_ROOT") {
        assert_eq!(lab_root(), PathBuf::from(value));
    } else {
        assert_eq!(lab_root(), PathBuf::from("/home/sanya/re-deep"));
    }
}
