# Детальный отчет: Причины падения CI на ядре (Workflow Run #43631030655)

- **Репозиторий:** `Userbash/chord` & `Userbash/discord-lab`
- **Workflow:** `Cross-platform native`
- **Run ID:** `43631030655`
- **Ветка:** `release/developer-alpha-preview`
- **Коммит:** `97a46aed`

---

## 1. Падение на `core (ubuntu-24.04)` (Ядро Linux)

### Результаты шагов:
- `Verify GPUI dependency without writes`: **PASS**
- `Bootstrap and workflow contracts`: **PASS**
- `Core formatting`: **FAIL (exit code 1)**
- `Core lint`: **PASS**
- `Core tests`: **PASS (все 1485+ тестов пройдены успешно!)**

### Лог ошибки шага `Core formatting`:
```bash
cargo fmt --manifest-path rust/Cargo.toml --check
Diff in /home/runner/work/chord/chord/rust/crates/chord-core/src/ws.rs:6049:
         let deadline = std::time::Instant::now() + std::time::Duration::from_secs(180);
         let mut attached = false;
         while std::time::Instant::now() < deadline {
-            std::thread::sleep(std::time::Duration::from_millis(if attached { 2000 } else { 300 }));
+            std::thread::sleep(std::time::Duration::from_millis(if attached {
+                2000
+            } else {
+                300
+            }));
             if let Ok(out) = std::process::Command::new("pactl")
                 .args(["list", "source-outputs"])
                 .stdin(std::process::Stdio::null())
Process completed with exit code 1.
```

### Причина:
1. В файле [rust/crates/chord-core/src/ws.rs:6049](file:///home/sanya/orca/project/rust/crates/chord-core/src/ws.rs#L6049) вызов `std::thread::sleep` в вотчдоге Soundshare требует форматирования переноса строк по стандарту `rustfmt`.
2. В файле workflow [.github/workflows/cross-platform.yml:109](file:///home/sanya/orca/project/.github/workflows/cross-platform.yml#L109) вызов `cargo fmt --manifest-path rust/Cargo.toml --check` должен содержать флаг `--all` для виртуального воркспейса Cargo.

---

## 2. Падение на `core (macos-15)` (Ядро macOS)

### Результаты шагов:
- `Core formatting`: **FAIL (exit code 1)** — аналогично шагу выше.
- `Core lint`: **PASS**
- `Core tests`: **FAIL (26 passed, 4 failed)**

### Лог паники тестов `crates/chord-platform/tests/dbus_contract.rs`:
```text
---- a06_connect_missing_abstract_returns_error_without_panic stdout ----
thread 'a06_connect_missing_abstract_returns_error_without_panic' panicked at crates/chord-platform/tests/dbus_contract.rs:1298:23:
ожидалась DbusError::Io, получено Address

---- f05_out_of_range_index_is_protocol_and_fds_are_closed stdout ----
thread 'f05_out_of_range_index_is_protocol_and_fds_are_closed' panicked at crates/chord-platform/tests/dbus_contract.rs:1635:5:
assertion `left == right` failed: подготовлен fd (left: 0, right: 1)

---- f17_single_receive_eof_keeps_owned_fds_but_exact_eof_closes_them stdout ----
thread 'f17_single_receive_eof_keeps_owned_fds_but_exact_eof_closes_them' panicked at crates/chord-platform/tests/dbus_contract.rs:1981:5:
assertion `left == right` failed (left: 0, right: 1)

---- f19_single_receive_truncation_closes_every_delivered_fd stdout ----
thread 'f19_single_receive_truncation_closes_every_delivered_fd' panicked at crates/chord-platform/tests/dbus_contract.rs:2042:5:
assertion failed: matches!(result, Err(DbusError::Protocol("control data truncated")))

failures:
    a06_connect_missing_abstract_returns_error_without_panic
    f05_out_of_range_index_is_protocol_and_fds_are_closed
    f17_single_receive_eof_keeps_owned_fds_but_exact_eof_closes_them
    f19_single_receive_truncation_closes_every_delivered_fd
test result: FAILED. 26 passed; 4 failed. Process completed with exit code 101.
```

### Причина:
D-Bus и абстрактные Unix-сокеты (`abstract=...`) являются технологией Linux. Ядро macOS (XNU/Darwin) не поддерживает абстрактное пространство имен сокетов Unix (`AF_UNIX`) и специфику `SCM_RIGHTS` Linux. В тестах `dbus_contract.rs` эти сценарии должны быть ограничены атрибутом `#[cfg(target_os = "linux")]`.

---

## 3. Статус удаления Actions

- В репозитории `Userbash/chord` удалены все 99+ устаревших запусков (dependabot и merge bot).
- В репозитории `Userbash/discord-lab` удалены все 55+ устаревших запусков.
- Сохранен **только один** последний актуальный запуск:
  - `Userbash/chord`: run `43631030655`
  - `Userbash/discord-lab`: run `43631030656`
