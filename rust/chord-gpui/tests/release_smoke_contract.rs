//! Контрактный и интеграционный тест релизной сборки `discord-gpui` (LTO + strip),
//! headless smoke-прогона на Wayland/X11 и манифеста контрольных сумм `SHA256SUMS`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn crate_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .canonicalize()
        .expect("CARGO_MANIFEST_DIR должен существовать")
}

fn repo_root() -> PathBuf {
    crate_root()
        .join("../..")
        .canonicalize()
        .expect("Корень репозитория должен существовать")
}

fn release_bin_path() -> PathBuf {
    crate_root().join("target/release/discord-gpui")
}

fn dist_dir() -> PathBuf {
    repo_root().join("rust/dist")
}

#[test]
fn test_cargo_toml_release_profile_has_lto_and_strip_contract() {
    let cargo_toml_path = crate_root().join("Cargo.toml");
    let content = fs::read_to_string(&cargo_toml_path)
        .expect("Не удалось прочитать rust/discord-gpui/Cargo.toml");

    assert!(
        content.contains("[profile.release]"),
        "В Cargo.toml должна присутствовать секция [profile.release]"
    );

    let has_lto = content.contains("lto = true")
        || content.contains("lto = \"thin\"")
        || content.contains("lto = \"fat\"");
    assert!(
        has_lto,
        "В [profile.release] должен быть включён LTO (lto = true | \"thin\" | \"fat\")"
    );

    let has_strip = content.contains("strip = true") || content.contains("strip = \"symbols\"");
    assert!(
        has_strip,
        "В [profile.release] должен быть включён strip (strip = true | \"symbols\")"
    );
}

#[test]
fn test_headless_smoke_wayland_and_x11_execution_contract() {
    let bin = env!("CARGO_BIN_EXE_discord-gpui");

    // 1. Позитивный сценарий: Wayland + X11 комбинированный профиль
    let out_wayland = Command::new(bin)
        .arg("--headless-smoke")
        .env("WAYLAND_DISPLAY", "wayland-1")
        .env("DISPLAY", ":0")
        .env("XDG_SESSION_TYPE", "wayland")
        .output()
        .expect("Не удалось запустить discord-gpui --headless-smoke (Wayland)");

    assert!(
        out_wayland.status.success(),
        "Wayland smoke должен завершиться с кодом 0, stderr: {}",
        String::from_utf8_lossy(&out_wayland.stderr)
    );
    let stdout_w = String::from_utf8_lossy(&out_wayland.stdout);
    let smoke_line_w = stdout_w
        .lines()
        .find(|l| l.starts_with("GPUI_SMOKE_OK "))
        .unwrap_or_else(|| panic!("Ожидался маркер GPUI_SMOKE_OK в stdout:\n{stdout_w}"));
    let json_w: serde_json::Value =
        serde_json::from_str(smoke_line_w.trim_start_matches("GPUI_SMOKE_OK "))
            .expect("GPUI_SMOKE_OK должен содержать валидный JSON");

    assert_eq!(json_w["status"], "ok");
    assert_eq!(json_w["display_backend"], "wayland+x11");
    assert_eq!(json_w["window_width"], 1280);
    assert_eq!(json_w["window_height"], 720);
    assert_eq!(json_w["sqlite_wal_ok"], true);
    assert_eq!(json_w["fps_budget_ok"], true);
    assert!(
        json_w["guilds"].as_u64().unwrap_or(0) >= 1,
        "Должна быть инициализирована минимум 1 гильдия"
    );
    let vmrss_mb = json_w["vmrss_mb"].as_f64().unwrap_or(999.0);
    assert!(
        vmrss_mb > 0.1 && vmrss_mb < 60.0,
        "VmRSS ({vmrss_mb:.2} MB) должен укладываться в бюджет < 60 MB"
    );

    // 2. Позитивный сценарий: чистый X11 профиль (без WAYLAND_DISPLAY)
    let out_x11 = Command::new(bin)
        .arg("--headless-smoke")
        .env_remove("WAYLAND_DISPLAY")
        .env("DISPLAY", ":0")
        .env("XDG_SESSION_TYPE", "x11")
        .output()
        .expect("Не удалось запустить discord-gpui --headless-smoke (X11)");

    assert!(out_x11.status.success());
    let stdout_x = String::from_utf8_lossy(&out_x11.stdout);
    let smoke_line_x = stdout_x
        .lines()
        .find(|l| l.starts_with("GPUI_SMOKE_OK "))
        .expect("GPUI_SMOKE_OK для X11");
    let json_x: serde_json::Value =
        serde_json::from_str(smoke_line_x.trim_start_matches("GPUI_SMOKE_OK ")).unwrap();
    assert_eq!(json_x["display_backend"], "x11");

    // 3. Граничный сценарий: чистое headless окружение без DISPLAY и WAYLAND_DISPLAY
    let out_headless = Command::new(bin)
        .env("DISCORD_GPUI_SMOKE", "1")
        .env_remove("WAYLAND_DISPLAY")
        .env_remove("DISPLAY")
        .env_remove("XDG_SESSION_TYPE")
        .output()
        .expect("Не удалось запустить discord-gpui в чистом headless режиме");

    assert!(out_headless.status.success());
    let stdout_h = String::from_utf8_lossy(&out_headless.stdout);
    let smoke_line_h = stdout_h
        .lines()
        .find(|l| l.starts_with("GPUI_SMOKE_OK "))
        .expect("GPUI_SMOKE_OK для headless");
    let json_h: serde_json::Value =
        serde_json::from_str(smoke_line_h.trim_start_matches("GPUI_SMOKE_OK ")).unwrap();
    assert_eq!(json_h["display_backend"], "headless");
}

#[test]
fn test_release_binary_stripped_and_sha256sums_manifest_oracle_contract() {
    let rel_bin = release_bin_path();
    assert!(
        rel_bin.exists(),
        "Релизный бинарник {} должен существовать",
        rel_bin.display()
    );

    // 1. Проверка ELF-заголовка и отсутствия таблицы символов .symtab (strip oracle)
    let file_out = Command::new("file")
        .arg(&rel_bin)
        .output()
        .expect("Вызов утилиты file");
    assert!(file_out.status.success());
    let file_stdout = String::from_utf8_lossy(&file_out.stdout);
    assert!(
        file_stdout.contains("ELF 64-bit LSB"),
        "Бинарник должен быть 64-битным ELF: {file_stdout}"
    );
    assert!(
        file_stdout.contains("stripped") && !file_stdout.contains("not stripped"),
        "Релизный бинарник обязан быть stripped: {file_stdout}"
    );

    // 2. Прямой запуск релизного бинарника в режиме --headless-smoke
    let smoke_out = Command::new(&rel_bin)
        .arg("--headless-smoke")
        .env("WAYLAND_DISPLAY", "wayland-1")
        .env("DISPLAY", ":0")
        .output()
        .expect("Запуск target/release/discord-gpui --headless-smoke");
    assert!(
        smoke_out.status.success(),
        "Релизный бинарник упал при smoke-прогоне: {}",
        String::from_utf8_lossy(&smoke_out.stderr)
    );
    assert!(
        String::from_utf8_lossy(&smoke_out.stdout).contains("GPUI_SMOKE_OK "),
        "Релизный бинарник должен выводить GPUI_SMOKE_OK"
    );

    // 3. Проверка манифеста rust/dist/SHA256SUMS независимым оракулом sha256sum -c
    let dist = dist_dir();
    let dist_bin = dist.join("discord-gpui");
    let dist_sha = dist.join("SHA256SUMS");
    assert!(
        dist_bin.exists(),
        "Бинарник в rust/dist/discord-gpui должен существовать"
    );
    assert!(
        dist_sha.exists(),
        "Манифест rust/dist/SHA256SUMS должен существовать"
    );

    let sha_content = fs::read_to_string(&dist_sha).expect("Чтение rust/dist/SHA256SUMS");
    assert!(
        sha_content
            .lines()
            .any(|line| line.ends_with("  discord-gpui") || line.ends_with(" discord-gpui")),
        "В rust/dist/SHA256SUMS должна быть запись для discord-gpui:\n{sha_content}"
    );

    let sha_check = Command::new("sha256sum")
        .args(["-c", "SHA256SUMS"])
        .current_dir(&dist)
        .output()
        .expect("Запуск sha256sum -c SHA256SUMS в rust/dist");
    assert!(
        sha_check.status.success(),
        "sha256sum -c в rust/dist завершился с ошибкой:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&sha_check.stdout),
        String::from_utf8_lossy(&sha_check.stderr)
    );

    // 4. Проверка локального манифеста target/release/SHA256SUMS
    let rel_dir = crate_root().join("target/release");
    let rel_sha = rel_dir.join("SHA256SUMS");
    assert!(
        rel_sha.exists(),
        "Манифест target/release/SHA256SUMS должен существовать"
    );
    let rel_sha_check = Command::new("sha256sum")
        .args(["-c", "SHA256SUMS"])
        .current_dir(&rel_dir)
        .output()
        .expect("Запуск sha256sum -c в target/release");
    assert!(
        rel_sha_check.status.success(),
        "sha256sum -c в target/release завершился с ошибкой:\n{}",
        String::from_utf8_lossy(&rel_sha_check.stderr)
    );
}
