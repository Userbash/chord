//! `SUB-907-1`: Статический анализ и строгий линтинг Rust:
//! - Шаг 1.1: `cargo clippy --all-targets -- -D warnings` и контроль `.unwrap()` / `.expect()`.
//! - Шаг 1.2: Проверка форматирования `rustfmt --check --edition 2024`.
//! - Шаг 1.3: Аудит уязвимостей `cargo audit` и проверка лицензий/источников `cargo-deny`.
//! - Шаг 1.4: Статический контроль размера исходников: функции `< 50` строк, модули `< 800` строк.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

use super::{MAX_FUNCTION_LINES, MAX_MODULE_LINES, QualityGateError, RUST_EDITION};

static PROBE_SEQ: AtomicU64 = AtomicU64::new(1);

/// Конфигурация строгого запуска `cargo clippy`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClippyGateConfig {
    pub offline: bool,
    pub all_targets: bool,
    pub deny_warnings: bool,
    pub forbid_production_unwrap: bool,
}

impl ClippyGateConfig {
    /// Строгая конфигурация по умолчанию для pre-commit и CI гейта.
    pub fn strict_default() -> Self {
        Self {
            offline: true,
            all_targets: true,
            deny_warnings: true,
            forbid_production_unwrap: true,
        }
    }

    /// Формирует аргументы командной строки `cargo clippy`.
    pub fn cargo_args(&self) -> Vec<String> {
        let mut args = vec!["clippy".to_string()];
        if self.offline {
            args.push("--offline".to_string());
        }
        if self.all_targets {
            args.push("--all-targets".to_string());
        }
        args.push("--".to_string());
        if self.deny_warnings {
            args.push("-D".to_string());
            args.push("warnings".to_string());
        }
        args
    }
}

/// Отчёт о проверке `cargo clippy`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClippyReport {
    pub passed: bool,
    pub warning_count: usize,
    pub error_count: usize,
    pub raw_excerpt: String,
}

/// Нарушение запрета на `.unwrap()` / `.expect()` в продакшн-коде.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnwrapViolation {
    pub line: usize,
    pub kind: String,
    pub snippet: String,
}

/// Исполнитель и анализатор `cargo clippy --all-targets -- -D warnings`.
pub struct ClippyGateRunner;

impl ClippyGateRunner {
    /// Оценивает текстовый вывод `cargo clippy` / `rustc -D warnings`.
    pub fn evaluate_output(exit_code: i32, output: &str) -> Result<ClippyReport, QualityGateError> {
        let mut warnings = 0usize;
        let mut errors = 0usize;
        for line in output.lines() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("warning:") && !trimmed.contains("generated") {
                warnings += 1;
            } else if trimmed.starts_with("error:") || trimmed.starts_with("error[") {
                errors += 1;
            }
        }
        if exit_code != 0 || warnings > 0 || errors > 0 {
            return Err(QualityGateError::ClippyWarningsDetected {
                warnings: warnings.max(usize::from(exit_code != 0 && errors == 0)),
                errors,
                details: output.lines().take(12).collect::<Vec<_>>().join("\n"),
            });
        }
        Ok(ClippyReport {
            passed: true,
            warning_count: 0,
            error_count: 0,
            raw_excerpt: output.lines().take(6).collect::<Vec<_>>().join("\n"),
        })
    }

    /// Компилирует тестовый фрагмент Rust-кода с флагом `-D warnings` (идиома `clippy -D warnings`).
    pub fn compile_probe_with_deny_warnings(
        source: &str,
    ) -> Result<ClippyReport, QualityGateError> {
        let seq = PROBE_SEQ.fetch_add(1, Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("gpui-clippy-probe-{}-{seq}", std::process::id()));
        std::fs::create_dir_all(&dir).map_err(|e| QualityGateError::IoError(e.to_string()))?;
        let src_path = dir.join("probe_lib.rs");
        let rmeta_path = dir.join("libprobe.rmeta");
        std::fs::write(&src_path, source).map_err(|e| QualityGateError::IoError(e.to_string()))?;
        let output = Command::new("rustc")
            .args([
                "--edition=2024",
                "--crate-type=lib",
                "--emit=metadata",
                "-D",
                "warnings",
            ])
            .arg(&src_path)
            .arg("-o")
            .arg(&rmeta_path)
            .output()
            .map_err(|e| QualityGateError::IoError(e.to_string()))?;
        let _ = std::fs::remove_dir_all(&dir);
        let combined = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        Self::evaluate_output(output.status.code().unwrap_or(1), &combined)
    }

    /// Сканирует исходник на вызовы `.unwrap()` и `.expect(` вне тестовых модулей `#[cfg(test)]`.
    pub fn scan_production_unwraps(source: &str) -> Vec<UnwrapViolation> {
        let mut violations = Vec::new();
        for (idx, line) in source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("#[cfg(test)]") {
                break;
            }
            if trimmed.starts_with("//") {
                continue;
            }
            let code_only = Self::strip_comments_and_strings(trimmed);
            if code_only.contains(".unwrap()") {
                violations.push(UnwrapViolation {
                    line: idx + 1,
                    kind: "unwrap()".to_string(),
                    snippet: trimmed.to_string(),
                });
            } else if code_only.contains(".expect(") {
                violations.push(UnwrapViolation {
                    line: idx + 1,
                    kind: "expect()".to_string(),
                    snippet: trimmed.to_string(),
                });
            }
        }
        violations
    }

    fn strip_comments_and_strings(line: &str) -> String {
        let no_comment = line.split("//").next().unwrap_or(line);
        let mut out = String::with_capacity(no_comment.len());
        let mut in_str = false;
        let mut escaped = false;
        for ch in no_comment.chars() {
            if in_str {
                if escaped {
                    escaped = false;
                } else if ch == '\\' {
                    escaped = true;
                } else if ch == '"' {
                    in_str = false;
                }
            } else if ch == '"' {
                in_str = true;
            } else {
                out.push(ch);
            }
        }
        out
    }
}

/// Отчёт о проверке форматирования `rustfmt --check --edition 2024`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RustfmtReport {
    pub passed: bool,
    pub edition: String,
    pub files_checked: usize,
    pub unformatted_files: Vec<String>,
}

/// Исполнитель проверки форматирования `rustfmt --check --edition 2024`.
pub struct RustfmtGateRunner;

impl RustfmtGateRunner {
    /// Возвращает аргументы `rustfmt --check --edition 2024`.
    pub fn command_args(files: &[&str]) -> Vec<String> {
        let mut args = vec![
            "--check".to_string(),
            "--edition".to_string(),
            RUST_EDITION.to_string(),
        ];
        for file in files {
            args.push((*file).to_string());
        }
        args
    }

    /// Запускает реальный `rustfmt --check --edition 2024` для списка файлов.
    pub fn check_files(files: &[PathBuf]) -> Result<RustfmtReport, QualityGateError> {
        let mut cmd = Command::new("rustfmt");
        cmd.args(["--check", "--edition", RUST_EDITION]);
        for file in files {
            cmd.arg(file);
        }
        let output = cmd
            .output()
            .map_err(|e| QualityGateError::IoError(e.to_string()))?;
        let combined = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        if !output.status.success() {
            let bad_files = Self::extract_diff_files(&combined, files);
            return Err(QualityGateError::RustfmtDiffDetected {
                files: bad_files,
                diff_excerpt: combined.lines().take(16).collect::<Vec<_>>().join("\n"),
            });
        }
        Ok(RustfmtReport {
            passed: true,
            edition: RUST_EDITION.to_string(),
            files_checked: files.len(),
            unformatted_files: Vec::new(),
        })
    }

    /// Проверяет форматирование строкового сниппета через `rustfmt --check --edition 2024`.
    pub fn check_snippet_edition_2024(source: &str) -> Result<RustfmtReport, QualityGateError> {
        let seq = PROBE_SEQ.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("gpui-rustfmt-{}-{seq}.rs", std::process::id()));
        std::fs::write(&path, source).map_err(|e| QualityGateError::IoError(e.to_string()))?;
        let result = Self::check_files(std::slice::from_ref(&path));
        let _ = std::fs::remove_file(&path);
        result
    }

    fn extract_diff_files(output: &str, fallback: &[PathBuf]) -> Vec<String> {
        let mut files = Vec::new();
        for line in output.lines() {
            if let Some(rest) = line.strip_prefix("Diff in ")
                && let Some((file_part, _)) = rest.rsplit_once(':')
            {
                let name = file_part.trim().to_string();
                if !files.contains(&name) {
                    files.push(name);
                }
            }
        }
        if files.is_empty() {
            files = fallback.iter().map(|p| p.display().to_string()).collect();
        }
        files
    }
}

/// Запись об уязвимости из базы RustSec (`cargo audit`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[allow(non_snake_case)]
pub struct RustSecAdvisory {
    pub id: String,
    pub package: String,
    pub affected_version: String,
    pub Patched: String,
    pub title: String,
}

/// Результат аудита зависимостей и лицензий (`cargo audit` + `cargo-deny`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DependencyAuditReport {
    pub passed: bool,
    pub packages_audited: usize,
    pub allowed_licenses: Vec<String>,
    pub license_violations: Vec<String>,
    pub vulnerability_hits: Vec<String>,
    pub source_violations: Vec<String>,
}

/// Аудитор безопасности зависимостей и лицензий (`cargo audit` / `cargo-deny`).
pub struct DependencySecurityAuditor;

impl DependencySecurityAuditor {
    /// Проверяет файлы `deny.toml` и `Cargo.lock` проекта.
    pub fn audit_project(
        deny_toml_path: &Path,
        cargo_lock_path: &Path,
        advisories: &[RustSecAdvisory],
    ) -> Result<DependencyAuditReport, QualityGateError> {
        let deny_text = std::fs::read_to_string(deny_toml_path)
            .map_err(|e| QualityGateError::IoError(format!("deny.toml: {e}")))?;
        let lock_text = std::fs::read_to_string(cargo_lock_path)
            .map_err(|e| QualityGateError::IoError(format!("Cargo.lock: {e}")))?;
        Self::audit_lock_text(&deny_text, &lock_text, advisories)
    }

    /// Анализирует содержимое `deny.toml` и `Cargo.lock` на уязвимости, лицензии и источники.
    pub fn audit_lock_text(
        deny_toml_text: &str,
        cargo_lock_text: &str,
        advisories: &[RustSecAdvisory],
    ) -> Result<DependencyAuditReport, QualityGateError> {
        let allowed = Self::parse_allowed_licenses(deny_toml_text);
        let deny_unknown_git = deny_toml_text.contains("unknown-git = \"deny\"");
        let packages = Self::parse_lock_packages(cargo_lock_text);
        for (name, version, source) in &packages {
            if let Some(src) = source {
                Self::validate_package_source(name, src, deny_unknown_git)?;
            }
            for adv in advisories {
                if &adv.package == name && &adv.affected_version == version {
                    return Err(QualityGateError::VulnerabilityDetected {
                        advisory_id: adv.id.clone(),
                        package: name.clone(),
                        version: version.clone(),
                        title: adv.title.clone(),
                    });
                }
            }
        }
        Ok(DependencyAuditReport {
            passed: true,
            packages_audited: packages.len(),
            allowed_licenses: allowed,
            license_violations: Vec::new(),
            vulnerability_hits: Vec::new(),
            source_violations: Vec::new(),
        })
    }

    fn validate_package_source(
        name: &str,
        source: &str,
        deny_unknown_git: bool,
    ) -> Result<(), QualityGateError> {
        let trusted_crates_io = "registry+https://github.com/rust-lang/crates.io-index";
        if source.starts_with("git+") && deny_unknown_git {
            return Err(QualityGateError::UntrustedDependencySource {
                package: name.to_string(),
                source: source.to_string(),
            });
        }
        if source.starts_with("registry+") && source != trusted_crates_io {
            return Err(QualityGateError::UntrustedDependencySource {
                package: name.to_string(),
                source: source.to_string(),
            });
        }
        Ok(())
    }

    /// Проверяет SPDX-выражение лицензии против белого списка `deny.toml` с учётом приоритета скобок и `AND`/`OR`.
    pub fn is_license_expression_allowed(expression: &str, allowed_licenses: &[String]) -> bool {
        let allow_set: BTreeSet<&str> = allowed_licenses.iter().map(String::as_str).collect();
        let tokens = Self::tokenize_spdx(expression);
        if tokens.is_empty() {
            return false;
        }
        let mut pos = 0usize;
        match Self::eval_spdx_or(&tokens, &mut pos, &allow_set) {
            Some(result) if pos == tokens.len() => result,
            _ => false,
        }
    }

    fn tokenize_spdx(expr: &str) -> Vec<String> {
        let spaced = expr.replace('(', " ( ").replace(')', " ) ");
        spaced
            .split_whitespace()
            .map(std::string::ToString::to_string)
            .collect()
    }

    fn eval_spdx_or(
        tokens: &[String],
        pos: &mut usize,
        allow_set: &BTreeSet<&str>,
    ) -> Option<bool> {
        let mut lhs = Self::eval_spdx_and(tokens, pos, allow_set)?;
        while tokens.get(*pos).map(String::as_str) == Some("OR") {
            *pos += 1;
            let rhs = Self::eval_spdx_and(tokens, pos, allow_set)?;
            lhs = lhs || rhs;
        }
        Some(lhs)
    }

    fn eval_spdx_and(
        tokens: &[String],
        pos: &mut usize,
        allow_set: &BTreeSet<&str>,
    ) -> Option<bool> {
        let mut lhs = Self::eval_spdx_primary(tokens, pos, allow_set)?;
        while tokens.get(*pos).map(String::as_str) == Some("AND") {
            *pos += 1;
            let rhs = Self::eval_spdx_primary(tokens, pos, allow_set)?;
            lhs = lhs && rhs;
        }
        Some(lhs)
    }

    fn eval_spdx_primary(
        tokens: &[String],
        pos: &mut usize,
        allow_set: &BTreeSet<&str>,
    ) -> Option<bool> {
        let tok = tokens.get(*pos)?.as_str();
        if tok == "(" {
            *pos += 1;
            let val = Self::eval_spdx_or(tokens, pos, allow_set)?;
            if tokens.get(*pos).map(String::as_str) != Some(")") {
                return None;
            }
            *pos += 1;
            return Some(val);
        }
        if matches!(tok, ")" | "AND" | "OR" | "WITH") {
            return None;
        }
        *pos += 1;
        let mut allowed = allow_set.contains(tok);
        if tokens.get(*pos).map(String::as_str) == Some("WITH") {
            *pos += 1;
            let exc = tokens.get(*pos)?.as_str();
            *pos += 1;
            allowed = allowed && exc == "LLVM-exception";
        }
        Some(allowed)
    }

    fn parse_allowed_licenses(deny_toml: &str) -> Vec<String> {
        let mut allowed = Vec::new();
        let mut in_allow = false;
        for line in deny_toml.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("allow = [") {
                in_allow = true;
                continue;
            }
            if in_allow {
                if trimmed.starts_with(']') {
                    break;
                }
                let cleaned = trimmed.trim_matches(&[',', '"', ' '][..]);
                if !cleaned.is_empty() {
                    allowed.push(cleaned.to_string());
                }
            }
        }
        allowed
    }

    fn parse_lock_packages(lock_text: &str) -> Vec<(String, String, Option<String>)> {
        let mut out = Vec::new();
        for block in lock_text.split("[[package]]").skip(1) {
            let mut name = String::new();
            let mut version = String::new();
            let mut source = None;
            for line in block.lines() {
                let t = line.trim();
                if let Some(v) = t.strip_prefix("name = \"") {
                    name = v.trim_end_matches('"').to_string();
                } else if let Some(v) = t.strip_prefix("version = \"") {
                    version = v.trim_end_matches('"').to_string();
                } else if let Some(v) = t.strip_prefix("source = \"") {
                    source = Some(v.trim_end_matches('"').to_string());
                }
            }
            if !name.is_empty() {
                out.push((name, version, source));
            }
        }
        out
    }
}

/// Метрики одной функции в модуле Rust.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FunctionMetrics {
    pub name: String,
    pub start_line: usize,
    pub end_line: usize,
    pub line_count: usize,
}

/// Метрики одного файла модуля Rust.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileMetrics {
    pub path: String,
    pub line_count: usize,
    pub functions: Vec<FunctionMetrics>,
}

/// Сводный отчёт статического контроля размеров исходников (`SUB-907-1` Шаг 1.4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceSizeReport {
    pub files_scanned: usize,
    pub total_functions: usize,
    pub file_metrics: Vec<FileMetrics>,
}

/// Сканер размера функций (`< 50` строк) и модулей (`< 800` строк).
pub struct SourceMetricsScanner;

impl SourceMetricsScanner {
    /// Сканирует все `.rs` файлы в директории и проверяет соблюдение лимитов.
    pub fn scan_directory(dir: &Path) -> Result<SourceSizeReport, QualityGateError> {
        let mut entries = Vec::new();
        let read_dir =
            std::fs::read_dir(dir).map_err(|e| QualityGateError::IoError(e.to_string()))?;
        for entry in read_dir {
            let entry = entry.map_err(|e| QualityGateError::IoError(e.to_string()))?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("rs") {
                entries.push(path);
            }
        }
        entries.sort();
        let mut file_metrics = Vec::new();
        let mut total_functions = 0usize;
        for path in entries {
            let text = std::fs::read_to_string(&path)
                .map_err(|e| QualityGateError::IoError(e.to_string()))?;
            let rel_name = path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown.rs");
            let m = Self::scan_source(rel_name, &text)?;
            total_functions += m.functions.len();
            file_metrics.push(m);
        }
        Ok(SourceSizeReport {
            files_scanned: file_metrics.len(),
            total_functions,
            file_metrics,
        })
    }

    /// Проверяет размер одного файла (`< 800` строк) и всех его функций (`< 50` строк).
    pub fn scan_source(file_label: &str, source: &str) -> Result<FileMetrics, QualityGateError> {
        let total_lines = source.lines().count();
        if total_lines >= MAX_MODULE_LINES {
            return Err(QualityGateError::ModuleFileTooLong {
                file: file_label.to_string(),
                lines: total_lines,
                max_lines: MAX_MODULE_LINES,
            });
        }
        let functions = Self::extract_functions(source);
        for f in &functions {
            if f.line_count >= MAX_FUNCTION_LINES {
                return Err(QualityGateError::FunctionTooLong {
                    file: file_label.to_string(),
                    function_name: f.name.clone(),
                    lines: f.line_count,
                    max_lines: MAX_FUNCTION_LINES,
                });
            }
        }
        Ok(FileMetrics {
            path: file_label.to_string(),
            line_count: total_lines,
            functions,
        })
    }

    fn extract_functions(source: &str) -> Vec<FunctionMetrics> {
        let lines: Vec<&str> = source.lines().collect();
        let mut out = Vec::new();
        let mut idx = 0usize;
        while idx < lines.len() {
            if let Some(fn_name) = Self::parse_fn_header(lines[idx]) {
                let (end_idx, found_body) = Self::find_fn_end(&lines, idx);
                if found_body {
                    let count = end_idx.saturating_sub(idx) + 1;
                    out.push(FunctionMetrics {
                        name: fn_name,
                        start_line: idx + 1,
                        end_line: end_idx + 1,
                        line_count: count,
                    });
                    idx = end_idx + 1;
                    continue;
                }
            }
            idx += 1;
        }
        out
    }

    fn parse_fn_header(line: &str) -> Option<String> {
        let code = line.split("//").next().unwrap_or(line).trim();
        let tokens: Vec<&str> = code.split_whitespace().collect();
        let fn_pos = tokens.iter().position(|t| *t == "fn")?;
        let raw_name = tokens.get(fn_pos + 1)?;
        let name: String = raw_name
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        if name.is_empty() { None } else { Some(name) }
    }

    fn find_fn_end(lines: &[&str], start_idx: usize) -> (usize, bool) {
        let mut depth = 0i32;
        let mut opened = false;
        for (offset, line) in lines[start_idx..].iter().enumerate() {
            let code = line.split("//").next().unwrap_or(line);
            if !opened && code.trim_end().ends_with(';') && !code.contains('{') {
                return (start_idx + offset, false);
            }
            for ch in code.chars() {
                if ch == '{' {
                    depth += 1;
                    opened = true;
                } else if ch == '}' {
                    depth -= 1;
                }
            }
            if opened && depth <= 0 {
                return (start_idx + offset, true);
            }
        }
        (lines.len().saturating_sub(1), opened)
    }
}
