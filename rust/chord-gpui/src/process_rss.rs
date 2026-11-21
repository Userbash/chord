//! Headless-smoke process RSS measurement. Unavailable metrics are errors.
//! This measures resident process memory, without starting a GPUI window.

use std::fmt;
#[cfg(any(target_os = "linux", test))]
use std::path::Path;
#[cfg(any(target_os = "windows", target_os = "macos"))]
use std::process::Command;

#[derive(Debug)]
pub struct RssMeasurementError(String);

impl RssMeasurementError {
    fn new(detail: impl Into<String>) -> Self {
        Self(detail.into())
    }
}

impl fmt::Display for RssMeasurementError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for RssMeasurementError {}

fn positive_integer(raw: &str) -> Result<u64, RssMeasurementError> {
    let raw = raw.trim();
    if raw.is_empty() || !raw.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(RssMeasurementError::new("RSS is not an unsigned integer"));
    }
    let value = raw
        .parse::<u64>()
        .map_err(|_| RssMeasurementError::new("RSS integer overflow"))?;
    if value == 0 {
        return Err(RssMeasurementError::new("RSS must be greater than zero"));
    }
    Ok(value)
}

/// Parse Linux's kernel-provided VmRSS, whose kB unit means 1024 bytes.
#[cfg(any(target_os = "linux", test))]
pub fn parse_proc_status(status: &str) -> Result<u64, RssMeasurementError> {
    let mut lines = status.lines().filter(|line| line.starts_with("VmRSS:"));
    let line = lines
        .next()
        .ok_or_else(|| RssMeasurementError::new("VmRSS is missing from process status"))?;
    if lines.next().is_some() {
        return Err(RssMeasurementError::new("VmRSS occurs more than once"));
    }
    let mut fields = line.split_whitespace();
    fields.next();
    let value = fields
        .next()
        .ok_or_else(|| RssMeasurementError::new("VmRSS value is missing"))?;
    if fields.next() != Some("kB") || fields.next().is_some() {
        return Err(RssMeasurementError::new("VmRSS requires a single kB value"));
    }
    positive_integer(value)
}

/// A missing procfs mount/file is an environment failure, never a low RSS.
#[cfg(any(target_os = "linux", test))]
pub fn read_proc_status(path: &Path) -> Result<u64, RssMeasurementError> {
    let status = std::fs::read_to_string(path).map_err(|error| {
        RssMeasurementError::new(format!("cannot read {}: {error}", path.display()))
    })?;
    parse_proc_status(&status)
}

#[cfg(any(target_os = "windows", test))]
pub fn working_set_bytes_to_kb(raw: &str) -> Result<u64, RssMeasurementError> {
    positive_integer(raw)?
        .checked_add(1023)
        .map(|bytes| bytes / 1024)
        .ok_or_else(|| RssMeasurementError::new("RSS byte-to-KiB conversion overflow"))
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
fn native_tool_output(mut command: Command) -> Result<String, RssMeasurementError> {
    let program = command.get_program().to_string_lossy().into_owned();
    let output = command
        .output()
        .map_err(|error| RssMeasurementError::new(format!("cannot run {program}: {error}")))?;
    if !output.status.success() {
        return Err(RssMeasurementError::new(format!(
            "{program} exited with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    String::from_utf8(output.stdout)
        .map_err(|_| RssMeasurementError::new(format!("{program} returned invalid RSS text")))
}

#[cfg(target_os = "linux")]
pub fn current_process_rss_kb() -> Result<u64, RssMeasurementError> {
    read_proc_status(Path::new("/proc/self/status"))
}

#[cfg(target_os = "macos")]
pub fn current_process_rss_kb() -> Result<u64, RssMeasurementError> {
    // Darwin ps documents rss in 1024-byte units. The PID is a separate argument.
    let mut command = Command::new("/bin/ps");
    command
        .args(["-o", "rss=", "-p"])
        .arg(std::process::id().to_string());
    positive_integer(&native_tool_output(command)?)
}

#[cfg(target_os = "windows")]
pub fn current_process_rss_kb() -> Result<u64, RssMeasurementError> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const QUERY: &str = concat!(
        "& { param([uint32]$ProcessId) ",
        "$ErrorActionPreference = 'Stop'; ",
        "$targetProcess = Get-Process -Id $ProcessId -ErrorAction Stop; ",
        "$targetProcess.Refresh(); ",
        "[Console]::Out.Write($targetProcess.WorkingSet64.ToString(",
        "[Globalization.CultureInfo]::InvariantCulture)) }"
    );
    let system_root = std::env::var_os("SystemRoot").ok_or_else(|| {
        RssMeasurementError::new("SystemRoot is unavailable for native RSS measurement")
    })?;
    let powershell = std::path::PathBuf::from(system_root)
        .join("System32/WindowsPowerShell/v1.0/powershell.exe");
    if !powershell.is_absolute() {
        return Err(RssMeasurementError::new(
            "SystemRoot must be an absolute path",
        ));
    }
    let mut command = Command::new(powershell);
    command
        .creation_flags(CREATE_NO_WINDOW)
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            QUERY,
        ])
        .arg(std::process::id().to_string());
    working_set_bytes_to_kb(&native_tool_output(command)?)
}

#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
pub fn current_process_rss_kb() -> Result<u64, RssMeasurementError> {
    Err(RssMeasurementError::new(format!(
        "native RSS measurement is unavailable on {}",
        std::env::consts::OS
    )))
}
