//! T-HARN-06: контракт локального CI и release-скрипта (RW-22 / P8-03).
//!
//! Проверяем именно исполняемый контракт: dry-run не запускает cargo/сеть, план содержит
//! fmt/clippy/test/deny/bench/release, а shell-скрипты синтаксически корректны. Реальные
//! тяжёлые прогоны выполняет `scripts/ci.sh` в integration-лейне.

use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .expect("repo root")
}

fn run(script: &str, args: &[&str]) -> std::process::Output {
    Command::new("bash")
        .arg(script)
        .args(args)
        .current_dir(repo_root())
        .output()
        .expect("bash available")
}

#[test]
fn ci_dry_run_prints_full_plan_without_running_tools() {
    let output = run("scripts/ci.sh", &["--dry-run"]);
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    for expected in [
        "cargo fmt",
        "cargo clippy",
        "cargo test",
        "cargo deny",
        "etf_bench",
        "release.sh",
    ] {
        assert!(
            stdout.contains(expected),
            "в плане нет {expected:?}:\n{stdout}"
        );
    }
    assert!(
        !stdout.contains("curl"),
        "dry-run не должен планировать сеть"
    );
    assert!(!stdout.contains("wget"));
}

#[test]
fn release_dry_run_is_offline_and_writes_dist_only() {
    let output = run("scripts/release.sh", &["--dry-run"]);
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("cargo build --offline --release -p chord-app"));
    assert!(stdout.contains("dist"));
    assert!(stdout.contains("sha256sum"));
    assert!(!stdout.contains("curl"));
    assert!(!stdout.contains("scp"));
}

#[test]
fn scripts_are_syntactically_valid_and_do_not_use_network_clients() {
    let root = repo_root();
    for script in ["scripts/ci.sh", "scripts/release.sh"] {
        let output = Command::new("bash")
            .args(["-n", script])
            .current_dir(&root)
            .output()
            .expect("bash -n");
        assert!(
            output.status.success(),
            "{script} syntax: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let text = std::fs::read_to_string(root.join(script)).unwrap();
        for forbidden in [
            "curl ",
            "wget ",
            "nc ",
            "ssh ",
            "scp ",
            "git push",
            "git commit",
        ] {
            assert!(!text.contains(forbidden), "{script} содержит {forbidden:?}");
        }
    }
}

#[test]
fn workflow_uses_the_same_ci_contract() {
    let workflow = repo_root().join(".github/workflows/ci.yml");
    let text = std::fs::read_to_string(workflow).expect("workflow");
    assert!(text.contains("scripts/ci.sh"));
    assert!(text.contains("scripts/release.sh"));
    assert!(!text.contains("DISCORD_TOKEN"));
}
