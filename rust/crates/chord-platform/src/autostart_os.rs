//! Cross-platform OS autostart backends (Linux XDG/Portal, Windows Run, macOS LaunchAgents)
//! and `--start-minimized` flicker-free startup policy (R4-04).

#![forbid(unsafe_code)]

use std::fs;
use std::path::{Path, PathBuf};

use super::{desktop_entry, AutostartEntry, AutostartError};

pub const CLI_FLAG_START_MINIMIZED: &str = "--start-minimized";
pub const CLI_FLAG_START_MINIMIZED_SHORT: &str = "-m";
pub const CLI_FLAG_START_INACTIVE: &str = "--start-inactive";

pub const PORTAL_BUS_NAME: &str = "org.freedesktop.portal.Desktop";
pub const PORTAL_OBJECT_PATH: &str = "/org/freedesktop/portal/desktop";
pub const PORTAL_BACKGROUND_INTERFACE: &str = "org.freedesktop.portal.Background";
pub const PORTAL_BACKGROUND_METHOD: &str = "RequestBackground";

pub const WINDOWS_RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetOs {
    Linux,
    Windows,
    MacOs,
}

impl TargetOs {
    /// Returns the compile-time target OS.
    pub const fn current() -> Self {
        if cfg!(target_os = "windows") {
            Self::Windows
        } else if cfg!(target_os = "macos") {
            Self::MacOs
        } else {
            Self::Linux
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutostartConfig {
    pub app_name: String,
    pub app_id: String,
    pub exec_path: String,
    pub extra_args: Vec<String>,
    pub start_minimized: bool,
    pub enabled: bool,
}

impl AutostartConfig {
    pub fn validate(&self) -> Result<(), AutostartError> {
        if self.app_name.trim().is_empty() {
            return Err(AutostartError::EmptyName);
        }
        if self.exec_path.trim().is_empty() {
            return Err(AutostartError::EmptyExec);
        }
        if has_ctrl(&self.app_name) || has_ctrl(&self.app_id) || has_ctrl(&self.exec_path) {
            return Err(AutostartError::ControlCharacter);
        }
        for arg in &self.extra_args {
            if has_ctrl(arg) {
                return Err(AutostartError::ControlCharacter);
            }
        }
        Ok(())
    }

    fn effective_args(&self) -> Vec<String> {
        let mut args: Vec<String> = self
            .extra_args
            .iter()
            .filter(|a| a.as_str() != CLI_FLAG_START_MINIMIZED)
            .cloned()
            .collect();
        if self.start_minimized {
            args.push(CLI_FLAG_START_MINIMIZED.to_string());
        }
        args
    }
}

fn has_ctrl(s: &str) -> bool {
    s.bytes().any(|b| matches!(b, b'\n' | b'\r' | 0))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortalBackgroundSpec {
    pub bus_name: &'static str,
    pub object_path: &'static str,
    pub interface: &'static str,
    pub method: &'static str,
    pub autostart: bool,
    pub commandline: Vec<String>,
}

/// Builds a FreeDesktop Background Portal `RequestBackground` specification.
pub fn build_portal_background_spec(
    app_id: &str,
    autostart: bool,
    start_minimized: bool,
) -> PortalBackgroundSpec {
    let mut commandline = vec![app_id.to_string()];
    if start_minimized {
        commandline.push(CLI_FLAG_START_MINIMIZED.to_string());
    }
    PortalBackgroundSpec {
        bus_name: PORTAL_BUS_NAME,
        object_path: PORTAL_OBJECT_PATH,
        interface: PORTAL_BACKGROUND_INTERFACE,
        method: PORTAL_BACKGROUND_METHOD,
        autostart,
        commandline,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacOsLaunchAgent {
    pub label: String,
    pub program_arguments: Vec<String>,
    pub run_at_load: bool,
}

impl MacOsLaunchAgent {
    pub fn from_config(config: &AutostartConfig) -> Result<Self, AutostartError> {
        config.validate()?;
        let label = if config.app_id.trim().is_empty() {
            config.app_name.clone()
        } else {
            config.app_id.clone()
        };
        let mut program_arguments = vec![config.exec_path.clone()];
        program_arguments.extend(config.effective_args());
        Ok(Self {
            label,
            program_arguments,
            run_at_load: config.enabled,
        })
    }
}

/// Resolves `<home>/Library/LaunchAgents/<label>.plist`.
pub fn macos_launch_agent_path(home_dir: &Path, label: &str) -> PathBuf {
    home_dir
        .join("Library")
        .join("LaunchAgents")
        .join(format!("{label}.plist"))
}

/// Renders a macOS LaunchAgent `.plist` XML document with XML escaping.
pub fn macos_launch_agent_plist(agent: &MacOsLaunchAgent) -> Result<String, AutostartError> {
    if agent.label.trim().is_empty() {
        return Err(AutostartError::EmptyName);
    }
    if agent.program_arguments.is_empty() || agent.program_arguments[0].trim().is_empty() {
        return Err(AutostartError::EmptyExec);
    }
    if has_ctrl(&agent.label) || agent.program_arguments.iter().any(|a| has_ctrl(a)) {
        return Err(AutostartError::ControlCharacter);
    }

    let mut args_xml = String::new();
    for arg in &agent.program_arguments {
        args_xml.push_str("\t\t<string>");
        args_xml.push_str(&escape_xml(arg));
        args_xml.push_str("</string>\n");
    }
    let run_tag = if agent.run_at_load {
        "<true/>"
    } else {
        "<false/>"
    };

    Ok(format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
         <plist version=\"1.0\">\n\
         <dict>\n\
         \t<key>Label</key>\n\
         \t<string>{}</string>\n\
         \t<key>ProgramArguments</key>\n\
         \t<array>\n\
         {args_xml}\t</array>\n\
         \t<key>RunAtLoad</key>\n\
         \t{run_tag}\n\
         \t<key>KeepAlive</key>\n\
         \t<false/>\n\
         </dict>\n\
         </plist>\n",
        escape_xml(&agent.label)
    ))
}

fn escape_xml(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            other => out.push(other),
        }
    }
    out
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsRunEntry {
    pub registry_key: &'static str,
    pub value_name: String,
    pub command_line: String,
}

/// Formats a Windows command line with proper quoting for `HKCU\...\Run`.
pub fn windows_run_command_line(
    exec_path: &str,
    extra_args: &[String],
    start_minimized: bool,
) -> Result<String, AutostartError> {
    if exec_path.trim().is_empty() {
        return Err(AutostartError::EmptyExec);
    }
    if has_ctrl(exec_path) || extra_args.iter().any(|a| has_ctrl(a)) {
        return Err(AutostartError::ControlCharacter);
    }

    let mut parts = vec![quote_windows_arg(exec_path)];
    for arg in extra_args {
        if arg != CLI_FLAG_START_MINIMIZED {
            parts.push(quote_windows_arg(arg));
        }
    }
    if start_minimized {
        parts.push(CLI_FLAG_START_MINIMIZED.to_string());
    }
    Ok(parts.join(" "))
}

pub fn windows_run_registry_entry(
    config: &AutostartConfig,
) -> Result<WindowsRunEntry, AutostartError> {
    config.validate()?;
    let command_line = windows_run_command_line(
        &config.exec_path,
        &config.extra_args,
        config.start_minimized,
    )?;
    Ok(WindowsRunEntry {
        registry_key: WINDOWS_RUN_KEY,
        value_name: config.app_name.clone(),
        command_line,
    })
}

fn quote_windows_arg(arg: &str) -> String {
    if !arg.is_empty() && !arg.chars().any(|c| matches!(c, ' ' | '\t' | '"')) {
        return arg.to_string();
    }
    let mut out = String::with_capacity(arg.len() + 2);
    out.push('"');
    let mut backslashes = 0usize;
    for ch in arg.chars() {
        match ch {
            '\\' => backslashes += 1,
            '"' => {
                out.push_str(&"\\".repeat(backslashes * 2 + 1));
                out.push('"');
                backslashes = 0;
            }
            other => {
                out.push_str(&"\\".repeat(backslashes));
                backslashes = 0;
                out.push(other);
            }
        }
    }
    out.push_str(&"\\".repeat(backslashes * 2));
    out.push('"');
    out
}

#[derive(Debug, Clone)]
pub struct SystemAutostartController {
    target_os: TargetOs,
    root_dir: PathBuf,
    config: AutostartConfig,
}

impl SystemAutostartController {
    pub fn with_root(target_os: TargetOs, root_dir: PathBuf, config: AutostartConfig) -> Self {
        Self {
            target_os,
            root_dir,
            config,
        }
    }

    pub fn is_enabled(&self) -> bool {
        match self.target_os {
            TargetOs::Linux => self.linux_desktop_path().is_file(),
            TargetOs::MacOs => {
                macos_launch_agent_path(&self.root_dir, &self.config.app_id).is_file()
            }
            TargetOs::Windows => self.windows_state_path().is_file(),
        }
    }

    pub fn set_enabled(&mut self, enabled: bool) -> Result<bool, AutostartError> {
        self.config.enabled = enabled;
        if enabled {
            self.install_for_os()?;
            Ok(true)
        } else {
            self.remove_for_os()?;
            Ok(false)
        }
    }

    pub fn toggle(&mut self) -> Result<bool, AutostartError> {
        let next = !self.is_enabled();
        self.set_enabled(next)
    }

    pub fn set_start_minimized(&mut self, minimized: bool) -> Result<bool, AutostartError> {
        self.config.start_minimized = minimized;
        if self.is_enabled() {
            self.install_for_os()?;
        }
        Ok(minimized)
    }

    pub fn start_minimized(&self) -> bool {
        self.config.start_minimized
    }

    fn linux_desktop_path(&self) -> PathBuf {
        let file = if self.config.app_id.trim().is_empty() {
            "discord-lab.desktop".to_string()
        } else {
            format!("{}.desktop", self.config.app_id)
        };
        self.root_dir.join("autostart").join(file)
    }

    fn windows_state_path(&self) -> PathBuf {
        self.root_dir
            .join("windows_run")
            .join(format!("{}.reg", self.config.app_name))
    }

    fn install_for_os(&self) -> Result<(), AutostartError> {
        match self.target_os {
            TargetOs::Linux => {
                let args = self.config.effective_args();
                let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
                let entry = AutostartEntry {
                    name: &self.config.app_name,
                    exec: &self.config.exec_path,
                    args: &arg_refs,
                    hidden: !self.config.enabled,
                };
                let text = desktop_entry(&entry)?;
                write_file_secure(&self.linux_desktop_path(), text.as_bytes())
            }
            TargetOs::MacOs => {
                let agent = MacOsLaunchAgent::from_config(&self.config)?;
                let plist = macos_launch_agent_plist(&agent)?;
                let path = macos_launch_agent_path(&self.root_dir, &agent.label);
                write_file_secure(&path, plist.as_bytes())
            }
            TargetOs::Windows => {
                let entry = windows_run_registry_entry(&self.config)?;
                let payload = format!(
                    "[{}]\n\"{}\"={}\n",
                    entry.registry_key, entry.value_name, entry.command_line
                );
                write_file_secure(&self.windows_state_path(), payload.as_bytes())
            }
        }
    }

    fn remove_for_os(&self) -> Result<(), AutostartError> {
        let path = match self.target_os {
            TargetOs::Linux => self.linux_desktop_path(),
            TargetOs::MacOs => macos_launch_agent_path(&self.root_dir, &self.config.app_id),
            TargetOs::Windows => self.windows_state_path(),
        };
        if path.exists() {
            fs::remove_file(&path).map_err(|_| AutostartError::IoError("remove_file"))?;
        }
        Ok(())
    }
}

fn write_file_secure(path: &Path, bytes: &[u8]) -> Result<(), AutostartError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|_| AutostartError::IoError("create_dir_all"))?;
    }
    fs::write(path, bytes).map_err(|_| AutostartError::IoError("write"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o644))
            .map_err(|_| AutostartError::IoError("set_permissions"))?;
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchArgs {
    pub start_minimized: bool,
    pub start_inactive: bool,
}

impl LaunchArgs {
    pub fn parse<I, S>(args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut start_minimized = false;
        let mut start_inactive = false;
        for arg in args {
            match arg.as_ref() {
                CLI_FLAG_START_MINIMIZED | CLI_FLAG_START_MINIMIZED_SHORT => {
                    start_minimized = true;
                }
                CLI_FLAG_START_INACTIVE => {
                    start_inactive = true;
                }
                _ => {}
            }
        }
        Self {
            start_minimized,
            start_inactive,
        }
    }
}

/// Flicker-free startup window state: when minimized on startup, the window is never created/mapped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartupWindowState {
    pub open_window_initially: bool,
    pub window_visible: bool,
    pub window_focused: bool,
    pub skip_taskbar: bool,
    pub tray_active: bool,
}

impl StartupWindowState {
    pub fn resolve(cli: &LaunchArgs, setting_start_minimized: bool) -> Self {
        let minimized = cli.start_minimized || setting_start_minimized;
        Self {
            open_window_initially: !minimized,
            window_visible: !minimized,
            window_focused: !minimized && !cli.start_inactive,
            skip_taskbar: minimized,
            tray_active: true,
        }
    }

    pub fn activate_window(&mut self) {
        self.window_visible = true;
        self.window_focused = true;
        self.skip_taskbar = false;
    }
}
