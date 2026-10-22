//! T-HARN-07: offline SBOM and license contract for RW-23 / W24b.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .expect("repo root")
}

fn run_node(args: &[&str]) -> Output {
    Command::new("node")
        .args(args)
        .current_dir(repo_root())
        .output()
        .expect("node available")
}

fn workspace_package_names(root: &Path) -> Vec<String> {
    std::fs::read_dir(root.join("rust/crates"))
        .expect("workspace crates")
        .map(|entry| {
            let manifest =
                std::fs::read_to_string(entry.expect("crate entry").path().join("Cargo.toml"))
                    .expect("crate manifest");
            manifest
                .lines()
                .find_map(|line| {
                    line.strip_prefix("name = \"")
                        .and_then(|value| value.strip_suffix('\"'))
                })
                .expect("package name")
                .to_owned()
        })
        .collect()
}

fn lock_package_count(root: &Path) -> usize {
    std::fs::read_to_string(root.join("rust/Cargo.lock"))
        .expect("Cargo.lock")
        .lines()
        .filter(|line| line.trim() == "[[package]]")
        .count()
}

#[test]
fn generates_spdx_23_with_locked_packages_and_licenses() {
    let root = repo_root();
    let output_path = std::env::temp_dir().join(format!(
        "discord-lab-sbom-{}-{}.json",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ));
    let output = run_node(&[
        "scripts/sbom.mjs",
        "--output",
        output_path.to_str().unwrap(),
    ]);
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let mut validation_command = Command::new("node");
    validation_command
        .args([
            "-e",
            "const d=JSON.parse(require('fs').readFileSync(process.argv[1]));\
             const assert=require('node:assert/strict');\
             assert.equal(d.spdxVersion,'SPDX-2.3');\
             assert.equal(d.dataLicense,'CC0-1.0');\
             assert.equal(d.packages.length,Number(process.argv[2]));\
             assert.ok(d.packages.every(p=>typeof p.licenseDeclared==='string'&&typeof p.SPDXID==='string'&&p.filesAnalyzed===false));\
             assert.ok(d.packages.every(p=>p.licenseDeclared!=='NOASSERTION'));\
             const ids=new Set(d.packages.map(p=>p.SPDXID));\
             assert.ok(d.relationships.some(r=>r.relationshipType==='DESCRIBES'));\
             assert.ok(d.relationships.some(r=>r.relationshipType==='DEPENDS_ON'));\
             assert.ok(d.relationships.every(r=>r.spdxElementId==='SPDXRef-DOCUMENT'||ids.has(r.spdxElementId)));\
             assert.ok(d.relationships.every(r=>r.relatedSpdxElement==='SPDXRef-DOCUMENT'||ids.has(r.relatedSpdxElement)));\
             const workspace=process.argv.slice(3);\
             assert.equal(d.relationships.filter(r=>r.relationshipType==='DESCRIBES').length,workspace.length);\
             const names=new Set(d.packages.map(p=>p.name));\
             for(const name of workspace) assert.ok(names.has(name),`workspace package missing: ${name}`);\
             assert.match(d.documentNamespace,/^https:\\/\\/discord-lab\\.local\\/spdx\\/[0-9a-f]{64}$/);",
            output_path.to_str().unwrap(),
            &lock_package_count(&root).to_string(),
        ]);
    let workspace_names = workspace_package_names(&root);
    validation_command.args(&workspace_names);
    let validation = validation_command.output().expect("node available");
    assert!(
        validation.status.success(),
        "SPDX validation failed: {}",
        String::from_utf8_lossy(&validation.stderr)
    );
    let second_path = std::env::temp_dir().join(format!(
        "discord-lab-sbom-repeat-{}-{}.json",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ));
    let second = run_node(&[
        "scripts/sbom.mjs",
        "--output",
        second_path.to_str().unwrap(),
    ]);
    assert!(
        second.status.success(),
        "repeat stderr={}",
        String::from_utf8_lossy(&second.stderr)
    );
    assert_eq!(
        std::fs::read(&output_path).unwrap(),
        std::fs::read(&second_path).unwrap(),
        "SBOM must be byte-for-byte reproducible"
    );
    let _ = std::fs::remove_file(&second_path);
    let _ = std::fs::remove_file(&output_path);
    assert!(root.join("rust/Cargo.lock").is_file());
}

#[test]
fn checks_allowlist_and_rejects_forbidden_license() {
    let allowed = run_node(&["scripts/sbom.mjs", "--check"]);
    assert!(
        allowed.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&allowed.stderr)
    );

    let forbidden = run_node(&["scripts/sbom.mjs", "--check-license", "GPL-3.0-only"]);
    assert_eq!(forbidden.status.code(), Some(1));
}

#[test]
fn license_expression_precedence_and_with_exceptions_are_checked() {
    for (expression, expected_code) in [
        ("(MIT OR Apache-2.0) AND GPL-3.0-only", 1),
        ("MIT OR GPL-3.0-only", 0),
        ("Apache-2.0 WITH LLVM-exception", 0),
        ("Apache-2.0 WITH Classpath-exception-2.0", 1),
        ("MIT !!!", 1),
        ("MIT OR", 1),
        ("LLVM-exception", 1),
        ("CDLA-Permissive-2.0", 0),
    ] {
        let output = run_node(&["scripts/sbom.mjs", "--check-license", expression]);
        assert_eq!(
            output.status.code(),
            Some(expected_code),
            "expression={expression:?}, stderr={}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn ci_and_release_dry_runs_keep_their_contracts() {
    let root = repo_root();
    let ci = Command::new("bash")
        .args(["scripts/ci.sh", "--dry-run"])
        .current_dir(&root)
        .output()
        .expect("bash available");
    assert!(
        ci.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&ci.stderr)
    );
    assert!(
        String::from_utf8_lossy(&ci.stdout)
            .to_lowercase()
            .contains("sbom")
    );
    let ci_source = std::fs::read_to_string(root.join("scripts/ci.sh")).expect("ci script");
    assert!(
        ci_source.contains("node \"$ROOT/scripts/sbom.mjs\" --check"),
        "CI должен реально вызывать SBOM, а не только печатать шаг в dry-run"
    );

    let release = Command::new("bash")
        .args(["scripts/release.sh", "--dry-run"])
        .current_dir(&root)
        .output()
        .expect("bash available");
    assert!(
        release.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&release.stderr)
    );
    let release_source =
        std::fs::read_to_string(root.join("scripts/release.sh")).expect("release script");
    assert!(
        release_source.contains("-ef"),
        "release должен сравнивать source/destination по inode"
    );
    assert!(
        release_source.contains("SHA256SUMS"),
        "release должен включать SBOM в checksum"
    );
}

#[test]
fn sbom_and_hooks_stay_offline_and_use_no_network_clients() {
    let root = repo_root();
    let sbom = std::fs::read_to_string(root.join("scripts/sbom.mjs")).expect("sbom script");
    assert!(sbom.contains("metadata"));
    assert!(sbom.contains("--offline"));

    for path in ["scripts/sbom.mjs", "scripts/ci.sh", "scripts/release.sh"] {
        let source = std::fs::read_to_string(root.join(path)).expect("script");
        for forbidden in ["curl ", "wget ", "fetch(", "http.request", "https.request"] {
            assert!(!source.contains(forbidden), "{path} содержит {forbidden:?}");
        }
    }
}
