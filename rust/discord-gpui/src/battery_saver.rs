//! Модуль снижения энергопотребления и Battery Saver Mode (`TASK-976`).
//! Контракт (`docs/re/backend-ipc.json`, `docs/re/main-arch.md` §10.1 `#4531`/`#7420`,
//! `waves/RW-31/RE-POWER.md`, `fixtures/dbus/power.json`, `waves/kanban-board.json` `TASK-976`):
//! 30 FPS (`33_333` мкс) на батарее / при неактивности, 0 FPS при `suspend`/`lock-screen`,
//! коалесценция таймеров строго `< 15 wakeups/s` (`MAX_IDLE_WAKEUPS_PER_SEC = 15.0`).

#![forbid(unsafe_code)]
#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

pub const AC_ACTIVE_FPS: u32 = 120;
pub const BATTERY_SAVER_FPS: u32 = 30;
pub const SUSPENDED_FPS: u32 = 0;
pub const FRAME_BUDGET_AC_US: u64 = 8_333;
pub const FRAME_BUDGET_SAVER_US: u64 = 33_333;

pub const MAX_IDLE_WAKEUPS_PER_SEC: f64 = 15.0;
pub const COALESCED_TIMER_QUANTUM_MS: u64 = 100;
pub const DEFAULT_IDLE_THRESHOLD_MS: u64 = 60_000;
pub const AUTO_SAVER_BATTERY_PERCENT: u8 = 20;
pub const CRITICAL_BATTERY_PERCENT: u8 = 15;
pub const MIN_DEEP_CSTATE_RESIDENCY_PCT: f64 = 85.0;
pub const IDLE_POLICY_IDLE_SECS: u64 = 600;
pub const IDLE_POLICY_DND_SECS: u64 = 1_800;

pub const IPC_POWER_MONITOR_GET_SYSTEM_IDLE_TIME: &str =
    "DISCORD_POWER_MONITOR_GET_SYSTEM_IDLE_TIME";
pub const IPC_POWER_MONITOR_SUSPEND: &str = "DISCORD_POWER_MONITOR_SUSPEND";
pub const IPC_POWER_MONITOR_RESUME: &str = "DISCORD_POWER_MONITOR_RESUME";
pub const IPC_POWER_MONITOR_LOCK_SCREEN: &str = "DISCORD_POWER_MONITOR_LOCK_SCREEN";
pub const IPC_POWER_MONITOR_UNLOCK_SCREEN: &str = "DISCORD_POWER_MONITOR_UNLOCK_SCREEN";
pub const IPC_POWER_SAVE_BLOCKER_BLOCK: &str = "DISCORD_POWER_SAVE_BLOCKER_BLOCK_DISPLAY_SLEEP";
pub const IPC_POWER_SAVE_BLOCKER_UNBLOCK: &str = "DISCORD_POWER_SAVE_BLOCKER_UNBLOCK_DISPLAY_SLEEP";
pub const IPC_POWER_SAVE_BLOCKER_CLEANUP: &str = "DISCORD_POWER_SAVE_BLOCKER_CLEANUP_DISPLAY_SLEEP";

pub const LOGIN1_DESTINATION: &str = "org.freedesktop.login1";
pub const LOGIN1_PATH: &str = "/org/freedesktop/login1";
pub const LOGIN1_INTERFACE: &str = "org.freedesktop.login1.Manager";
pub const PREPARE_FOR_SLEEP_SIGNAL: &str = "PrepareForSleep";
pub const SCREENSAVER_DESTINATION: &str = "org.freedesktop.ScreenSaver";
pub const SCREENSAVER_PATH: &str = "/org/freedesktop/ScreenSaver";
pub const SCREENSAVER_INTERFACE: &str = "org.freedesktop.ScreenSaver";
pub const GET_SESSION_IDLE_TIME_METHOD: &str = "GetSessionIdleTime";

pub const TOKEN_BG: &str = "#1e1f22";
pub const TOKEN_SURFACE: &str = "#2b2d31";
pub const TOKEN_TEXT_PRIMARY: &str = "#ffffff";
pub const TOKEN_ACCENT: &str = "#5865f2";
pub const TOKEN_BORDER: &str = "#3f4147";

pub const MAX_MESSAGE_CHARS: usize = 2_000;
pub const MAX_ATTACHMENTS_COUNT: usize = 10;
pub const MAX_ATTACHMENT_BYTES: u64 = 25 * 1024 * 1024;
pub const MAX_SEARCH_OFFSET: u32 = 9_975;
pub const MAX_VOLUME_PERCENT: u16 = 200;
pub const MAX_BADGE_CAP: u32 = 99;

#[derive(Debug, Clone, PartialEq)]
pub enum BatterySaverError {
    InvalidBatteryCapacity(u8),
    InvalidTimerQuantumMs(u64),
    InvalidSampleDurationMs(u64),
    WakeupBudgetExceeded { actual_per_sec: f64, limit: f64 },
    DeepCStateResidencyTooLow { actual_pct: f64, min_pct: f64 },
    UnknownPowerSaveBlockerId(u32),
    InvalidSysfsAttribute(String),
    BoundaryViolation(&'static str),
    AccessDenied(&'static str),
}

impl fmt::Display for BatterySaverError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for BatterySaverError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerSource {
    AcMains,
    BatteryDischarging,
    UpsDischarging,
    UnknownFallbackAc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerToggleState {
    AcMode,
    CheckBattery,
    SaverActive,
}

impl PowerToggleState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AcMode => "ac-mode",
            Self::CheckBattery => "check battery",
            Self::SaverActive => "saver-active",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatterySaverPolicy {
    AutoOnBatteryOrIdle,
    AlwaysOn,
    Disabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresenceIdleState {
    Online,
    Idle,
    Dnd,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SysfsPowerSupplySnapshot {
    pub supply_type: String,
    pub online: bool,
    pub discharging: bool,
    pub capacity_pct: u8,
    pub power_now_mw: u32,
}

impl SysfsPowerSupplySnapshot {
    pub fn from_key_values(pairs: &[(&str, &str)]) -> Result<Self, BatterySaverError> {
        let mut supply_type = String::new();
        let (mut online, mut status, mut capacity_pct) = (true, "Full", 100u8);
        let mut power_now_uw: u64 = 0;
        for &(k, v) in pairs {
            let val = v.trim();
            match k.trim() {
                "type" => supply_type = val.to_string(),
                "online" => online = parse_zero_one(val)?,
                "status" => status = val,
                "capacity" => capacity_pct = parse_capacity(val)?,
                "power_now" => {
                    power_now_uw = val
                        .parse::<u64>()
                        .map_err(|_| BatterySaverError::InvalidSysfsAttribute(val.to_string()))?;
                }
                _ => {}
            }
        }
        if supply_type.is_empty() {
            return Err(BatterySaverError::InvalidSysfsAttribute(
                "missing type".into(),
            ));
        }
        let discharging = status.eq_ignore_ascii_case("Discharging") || !online;
        Ok(Self {
            supply_type,
            online,
            discharging,
            capacity_pct,
            power_now_mw: (power_now_uw / 1_000) as u32,
        })
    }

    #[must_use]
    pub fn to_power_source(&self) -> PowerSource {
        if self.supply_type.eq_ignore_ascii_case("Battery") && self.discharging {
            PowerSource::BatteryDischarging
        } else if self.supply_type.eq_ignore_ascii_case("UPS") && self.discharging {
            PowerSource::UpsDischarging
        } else if self.supply_type.eq_ignore_ascii_case("Mains")
            || self.supply_type.eq_ignore_ascii_case("Battery")
        {
            PowerSource::AcMains
        } else {
            PowerSource::UnknownFallbackAc
        }
    }
}

fn parse_zero_one(val: &str) -> Result<bool, BatterySaverError> {
    match val {
        "1" => Ok(true),
        "0" => Ok(false),
        _ => Err(BatterySaverError::InvalidSysfsAttribute(val.to_string())),
    }
}

fn parse_capacity(val: &str) -> Result<u8, BatterySaverError> {
    let parsed = val
        .parse::<u16>()
        .map_err(|_| BatterySaverError::InvalidSysfsAttribute(val.to_string()))?;
    if parsed > 100 {
        return Err(BatterySaverError::InvalidBatteryCapacity(parsed as u8));
    }
    Ok(parsed as u8)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameGovernorDecision {
    pub target_fps: u32,
    pub frame_budget_us: u64,
    pub saver_engaged: bool,
    pub pause_background_animations: bool,
    pub reduced_motion: bool,
    pub toggle_state: PowerToggleState,
}

impl FrameGovernorDecision {
    const fn suspended() -> Self {
        Self {
            target_fps: SUSPENDED_FPS,
            frame_budget_us: 0,
            saver_engaged: true,
            pause_background_animations: true,
            reduced_motion: true,
            toggle_state: PowerToggleState::SaverActive,
        }
    }
}

#[must_use]
pub fn evaluate_target_fps(
    policy: BatterySaverPolicy,
    source: PowerSource,
    capacity_pct: u8,
    idle_ms: u64,
    idle_threshold_ms: u64,
    suspended_or_locked: bool,
    hovered: bool,
) -> FrameGovernorDecision {
    if suspended_or_locked {
        return FrameGovernorDecision::suspended();
    }
    let on_bat = matches!(
        source,
        PowerSource::BatteryDischarging | PowerSource::UpsDischarging
    );
    let auto_saver =
        on_bat || capacity_pct <= AUTO_SAVER_BATTERY_PERCENT || idle_ms >= idle_threshold_ms;
    let saver_engaged = match policy {
        BatterySaverPolicy::AlwaysOn => true,
        BatterySaverPolicy::Disabled => false,
        BatterySaverPolicy::AutoOnBatteryOrIdle => auto_saver,
    };
    let toggle_state = if saver_engaged {
        PowerToggleState::SaverActive
    } else if hovered {
        PowerToggleState::CheckBattery
    } else {
        PowerToggleState::AcMode
    };
    FrameGovernorDecision {
        target_fps: if saver_engaged {
            BATTERY_SAVER_FPS
        } else {
            AC_ACTIVE_FPS
        },
        frame_budget_us: if saver_engaged {
            FRAME_BUDGET_SAVER_US
        } else {
            FRAME_BUDGET_AC_US
        },
        saver_engaged,
        pause_background_animations: saver_engaged,
        reduced_motion: saver_engaged,
        toggle_state,
    }
}

#[derive(Debug, Clone)]
pub struct BatterySaverController {
    pub policy: BatterySaverPolicy,
    pub power_source: PowerSource,
    pub battery_capacity_pct: u8,
    pub system_idle_ms: u64,
    pub idle_threshold_ms: u64,
    pub system_suspended: bool,
    pub screen_locked: bool,
    pub toggle_hovered: bool,
    next_blocker_id: u32,
    active_blocker_ids: BTreeSet<u32>,
}

impl Default for BatterySaverController {
    fn default() -> Self {
        Self::new()
    }
}

impl BatterySaverController {
    #[must_use]
    pub fn new() -> Self {
        Self {
            policy: BatterySaverPolicy::AutoOnBatteryOrIdle,
            power_source: PowerSource::AcMains,
            battery_capacity_pct: 100,
            system_idle_ms: 0,
            idle_threshold_ms: DEFAULT_IDLE_THRESHOLD_MS,
            system_suspended: false,
            screen_locked: false,
            toggle_hovered: false,
            next_blocker_id: 1,
            active_blocker_ids: BTreeSet::new(),
        }
    }

    pub fn apply_sysfs_snapshot(
        &mut self,
        snap: &SysfsPowerSupplySnapshot,
    ) -> Result<FrameGovernorDecision, BatterySaverError> {
        if snap.capacity_pct > 100 {
            return Err(BatterySaverError::InvalidBatteryCapacity(snap.capacity_pct));
        }
        self.power_source = snap.to_power_source();
        self.battery_capacity_pct = snap.capacity_pct;
        Ok(self.current_decision())
    }

    pub fn handle_ipc_power_event(&mut self, channel: &str) -> FrameGovernorDecision {
        match channel {
            IPC_POWER_MONITOR_SUSPEND => {
                self.system_suspended = true;
                self.active_blocker_ids.clear();
            }
            IPC_POWER_MONITOR_RESUME => {
                self.system_suspended = false;
                self.system_idle_ms = 0;
            }
            IPC_POWER_MONITOR_LOCK_SCREEN => self.screen_locked = true,
            IPC_POWER_MONITOR_UNLOCK_SCREEN => {
                self.screen_locked = false;
                self.system_idle_ms = 0;
            }
            _ => {}
        }
        self.current_decision()
    }

    pub fn handle_prepare_for_sleep(&mut self, going_to_sleep: bool) -> FrameGovernorDecision {
        let ch = if going_to_sleep {
            IPC_POWER_MONITOR_SUSPEND
        } else {
            IPC_POWER_MONITOR_RESUME
        };
        self.handle_ipc_power_event(ch)
    }

    pub fn block_display_sleep(&mut self) -> u32 {
        let id = self.next_blocker_id;
        self.next_blocker_id = self.next_blocker_id.saturating_add(1);
        self.active_blocker_ids.insert(id);
        id
    }

    pub fn unblock_display_sleep(&mut self, id: u32) -> Result<(), BatterySaverError> {
        if self.active_blocker_ids.remove(&id) {
            Ok(())
        } else {
            Err(BatterySaverError::UnknownPowerSaveBlockerId(id))
        }
    }

    pub fn cleanup_display_sleep(&mut self) -> usize {
        let count = self.active_blocker_ids.len();
        self.active_blocker_ids.clear();
        count
    }

    #[must_use]
    pub fn active_blocker_count(&self) -> usize {
        self.active_blocker_ids.len()
    }

    #[must_use]
    pub fn presence_state(&self) -> PresenceIdleState {
        let idle_secs = self.system_idle_ms / 1_000;
        if idle_secs >= IDLE_POLICY_DND_SECS {
            PresenceIdleState::Dnd
        } else if idle_secs >= IDLE_POLICY_IDLE_SECS {
            PresenceIdleState::Idle
        } else {
            PresenceIdleState::Online
        }
    }

    #[must_use]
    pub fn current_decision(&self) -> FrameGovernorDecision {
        let suspended_or_locked = if self.system_suspended || self.screen_locked {
            true
        } else {
            false
        };
        evaluate_target_fps(
            self.policy,
            self.power_source,
            self.battery_capacity_pct,
            self.system_idle_ms,
            self.idle_threshold_ms,
            suspended_or_locked,
            self.toggle_hovered,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CStateResidencyEstimate {
    pub c0_active_pct: f64,
    pub c1_c3_light_pct: f64,
    pub c6_c10_deep_pct: f64,
    pub estimated_power_mw: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WakeupBenchmarkReport {
    pub duration_ms: u64,
    pub raw_timer_events: usize,
    pub coalesced_wakeups: usize,
    pub skipped_clean_frames: usize,
    pub wakeups_per_sec: f64,
    pub target_fps: u32,
    pub cstate: CStateResidencyEstimate,
    pub sla_passed: bool,
}

#[derive(Debug, Clone)]
pub struct TimerCoalescer {
    quantum_ms: u64,
    buckets: BTreeMap<u64, Vec<&'static str>>,
    raw_registered: usize,
    skipped_clean_frames: usize,
}

impl TimerCoalescer {
    pub fn new(quantum_ms: u64) -> Result<Self, BatterySaverError> {
        if quantum_ms == 0 || quantum_ms > 5_000 {
            return Err(BatterySaverError::InvalidTimerQuantumMs(quantum_ms));
        }
        Ok(Self {
            quantum_ms,
            buckets: BTreeMap::new(),
            raw_registered: 0,
            skipped_clean_frames: 0,
        })
    }

    pub fn schedule_timer(&mut self, subsystem: &'static str, deadline_ms: u64) -> u64 {
        let quantum = self.quantum_ms;
        let bucket_ms = deadline_ms.div_ceil(quantum) * quantum;
        self.raw_registered += 1;
        self.buckets.entry(bucket_ms).or_default().push(subsystem);
        bucket_ms
    }

    pub fn record_frame_poll(&mut self, at_ms: u64, dirty_regions: usize) -> bool {
        if dirty_regions == 0 {
            self.skipped_clean_frames += 1;
            return false;
        }
        let _ = self.schedule_timer("gpui_dirty_render", at_ms);
        true
    }

    pub fn finalize_window(
        &self,
        duration_ms: u64,
        target_fps: u32,
    ) -> Result<WakeupBenchmarkReport, BatterySaverError> {
        if duration_ms == 0 {
            return Err(BatterySaverError::InvalidSampleDurationMs(0));
        }
        let coalesced_wakeups = self.buckets.len();
        let wakeups_per_sec = (coalesced_wakeups as f64) * 1_000.0 / (duration_ms as f64);
        let cstate = estimate_cstate_residency(wakeups_per_sec, target_fps);
        validate_wakeup_sla(wakeups_per_sec, cstate.c6_c10_deep_pct)?;
        Ok(WakeupBenchmarkReport {
            duration_ms,
            raw_timer_events: self.raw_registered,
            coalesced_wakeups,
            skipped_clean_frames: self.skipped_clean_frames,
            wakeups_per_sec,
            target_fps,
            cstate,
            sla_passed: true,
        })
    }
}

#[must_use]
pub fn estimate_cstate_residency(wakeups_per_sec: f64, target_fps: u32) -> CStateResidencyEstimate {
    let fps_penalty = if target_fps <= BATTERY_SAVER_FPS {
        1.5
    } else {
        8.0
    };
    let c0_active_pct = (wakeups_per_sec * 0.22 + fps_penalty).clamp(0.5, 65.0);
    let c1_c3_light_pct = (wakeups_per_sec * 0.38).clamp(0.5, 30.0);
    let c6_c10_deep_pct = (100.0 - c0_active_pct - c1_c3_light_pct).clamp(0.0, 99.0);
    let base_mw = if target_fps == 0 { 450 } else { 850 };
    CStateResidencyEstimate {
        c0_active_pct,
        c1_c3_light_pct,
        c6_c10_deep_pct,
        estimated_power_mw: base_mw + (c0_active_pct * 45.0) as u32,
    }
}

pub fn validate_wakeup_sla(
    wakeups_per_sec: f64,
    deep_cstate_pct: f64,
) -> Result<(), BatterySaverError> {
    if wakeups_per_sec >= MAX_IDLE_WAKEUPS_PER_SEC {
        return Err(BatterySaverError::WakeupBudgetExceeded {
            actual_per_sec: wakeups_per_sec,
            limit: MAX_IDLE_WAKEUPS_PER_SEC,
        });
    }
    if deep_cstate_pct < MIN_DEEP_CSTATE_RESIDENCY_PCT {
        return Err(BatterySaverError::DeepCStateResidencyTooLow {
            actual_pct: deep_cstate_pct,
            min_pct: MIN_DEEP_CSTATE_RESIDENCY_PCT,
        });
    }
    Ok(())
}

pub fn run_battery_saver_benchmark(
    duration_ms: u64,
) -> Result<WakeupBenchmarkReport, BatterySaverError> {
    let mut coalescer = TimerCoalescer::new(COALESCED_TIMER_QUANTUM_MS)?;
    let mut t = 10u64;
    while t <= duration_ms {
        coalescer.schedule_timer("gateway_heartbeat_check", t);
        coalescer.schedule_timer("typing_indicator_gc", t + 25);
        coalescer.schedule_timer("presence_idle_probe", t + 60);
        coalescer.schedule_timer("voice_telemetry_digest", t + 85);
        let _ = coalescer.record_frame_poll(t, 0);
        t += 100;
    }
    coalescer.finalize_window(duration_ms, BATTERY_SAVER_FPS)
}

#[must_use]
pub fn resolve_linux_idle_time_ms(
    wayland_session: bool,
    wayland_idle_ms: Result<u64, &'static str>,
    dbus_screensaver_idle_secs: Option<u64>,
    x11_fallback_idle_secs: u64,
) -> u64 {
    if wayland_session && let Ok(ms) = wayland_idle_ms {
        return ms;
    }
    dbus_screensaver_idle_secs
        .unwrap_or(x11_fallback_idle_secs)
        .saturating_mul(1_000)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkloadBoundaryCheck {
    pub snowflake_id: u64,
    pub message_len: usize,
    pub attachment_count: usize,
    pub attachment_bytes: u64,
    pub search_offset: u32,
    pub volume_pct: u16,
    pub unread_count: u32,
    pub can_view_channel: bool,
    pub age_gated_allowed: bool,
}

pub fn validate_workload_boundaries(
    input: &WorkloadBoundaryCheck,
) -> Result<String, BatterySaverError> {
    if !input.can_view_channel {
        return Err(BatterySaverError::AccessDenied("MISSING_VIEW_CHANNEL"));
    }
    if !input.age_gated_allowed {
        return Err(BatterySaverError::AccessDenied("AGE_GATED_SPACES"));
    }
    if input.message_len > MAX_MESSAGE_CHARS {
        return Err(BatterySaverError::BoundaryViolation("message_len > 2000"));
    }
    if input.attachment_count > MAX_ATTACHMENTS_COUNT
        || input.attachment_bytes > MAX_ATTACHMENT_BYTES
    {
        return Err(BatterySaverError::BoundaryViolation(
            "attachments > 10 or > 25MB",
        ));
    }
    if input.search_offset > MAX_SEARCH_OFFSET || input.volume_pct > MAX_VOLUME_PERCENT {
        return Err(BatterySaverError::BoundaryViolation(
            "offset or volume out of bounds",
        ));
    }
    Ok(if input.unread_count > MAX_BADGE_CAP {
        "99+".to_string()
    } else {
        input.unread_count.to_string()
    })
}

#[must_use]
pub fn compute_rest_backoff_ms(
    http_status: u16,
    retry_after_ms: Option<u64>,
    attempt: u32,
    saver_active: bool,
) -> u64 {
    let base = match http_status {
        200 | 204 | 401 | 403 | 404 | 413 => 0,
        202 => {
            if saver_active {
                1_000
            } else {
                500
            }
        }
        429 => retry_after_ms.unwrap_or(1_500).max(250),
        500..=599 => (500u64 << attempt.min(5)).min(30_000),
        _ => 1_000,
    };
    if saver_active && base > 0 {
        base.div_ceil(COALESCED_TIMER_QUANTUM_MS) * COALESCED_TIMER_QUANTUM_MS
    } else {
        base
    }
}

#[must_use]
pub fn redact_power_telemetry_secrets(raw: &str) -> String {
    let mut out = Vec::new();
    for tok in raw.split_whitespace() {
        let masked = if tok.starts_with("mfa.") && tok.len() > 8 {
            "mfa.***".to_string()
        } else if tok.starts_with("Bot_") || tok.starts_with("Bearer_") {
            "[REDACTED_TOKEN]".to_string()
        } else {
            tok.to_string()
        };
        out.push(masked);
    }
    out.join(" ")
}

#[must_use]
pub fn export_power_telemetry_json(
    decision: &FrameGovernorDecision,
    report: &WakeupBenchmarkReport,
    diagnostic_note: &str,
) -> String {
    let safe_note = redact_power_telemetry_secrets(diagnostic_note);
    let (lb, rb) = ('{', '}');
    format!(
        "{lb}\"task\":\"TASK-976\",\"target_fps\":{},\"frame_budget_us\":{},\"saver_engaged\":{},\"toggle_state\":\"{}\",\"wakeups_per_sec\":{:.2},\"c6_c10_deep_pct\":{:.2},\"sla_passed\":{},\"note\":\"{}\"{rb}",
        decision.target_fps,
        decision.frame_budget_us,
        decision.saver_engaged,
        decision.toggle_state.as_str(),
        report.wakeups_per_sec,
        report.cstate.c6_c10_deep_pct,
        report.sla_passed,
        safe_note.replace('"', "'"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sub_976_1_oracles_sysfs_and_toggle_states() {
        assert_eq!(TOKEN_BG, "#1e1f22");
        assert_eq!(TOKEN_SURFACE, "#2b2d31");
        assert_eq!(TOKEN_TEXT_PRIMARY, "#ffffff");
        assert_eq!(TOKEN_ACCENT, "#5865f2");
        assert_eq!(TOKEN_BORDER, "#3f4147");
        assert_eq!(PowerToggleState::AcMode.as_str(), "ac-mode");
        assert_eq!(PowerToggleState::CheckBattery.as_str(), "check battery");
        assert_eq!(PowerToggleState::SaverActive.as_str(), "saver-active");
        let bat = SysfsPowerSupplySnapshot::from_key_values(&[
            ("type", "Battery"),
            ("online", "0"),
            ("status", "Discharging"),
            ("capacity", "65"),
            ("power_now", "6500000"),
        ])
        .expect("valid sysfs battery");
        assert_eq!(bat.to_power_source(), PowerSource::BatteryDischarging);
        assert_eq!(bat.capacity_pct, 65);
        assert_eq!(bat.power_now_mw, 6_500);
        let bad_cap =
            SysfsPowerSupplySnapshot::from_key_values(&[("type", "Battery"), ("capacity", "105")]);
        assert_eq!(bad_cap, Err(BatterySaverError::InvalidBatteryCapacity(105)));
    }

    #[test]
    fn test_sub_976_2_frame_governor_30fps_battery_and_suspend() {
        let mut ctrl = BatterySaverController::new();
        assert_eq!(ctrl.current_decision().target_fps, 120);
        assert_eq!(ctrl.current_decision().frame_budget_us, 8_333);
        ctrl.toggle_hovered = true;
        assert_eq!(
            ctrl.current_decision().toggle_state,
            PowerToggleState::CheckBattery
        );
        ctrl.power_source = PowerSource::BatteryDischarging;
        let on_bat = ctrl.current_decision();
        assert_eq!(on_bat.target_fps, 30);
        assert_eq!(on_bat.frame_budget_us, 33_333);
        assert!(on_bat.saver_engaged && on_bat.reduced_motion);
        let b1 = ctrl.block_display_sleep();
        assert_eq!(ctrl.active_blocker_count(), 1);
        assert!(ctrl.unblock_display_sleep(999).is_err());
        ctrl.unblock_display_sleep(b1).expect("unblock");
        let _ = ctrl.block_display_sleep();
        let susp = ctrl.handle_ipc_power_event(IPC_POWER_MONITOR_SUSPEND);
        assert_eq!(susp.target_fps, 0);
        assert_eq!(ctrl.active_blocker_count(), 0);
        assert_eq!(ctrl.handle_prepare_for_sleep(false).target_fps, 30);
    }

    #[test]
    fn test_sub_976_3_timer_coalescing_under_15_wakeups_and_cstates() {
        let report = run_battery_saver_benchmark(1_000).expect("benchmark pass");
        assert_eq!(report.target_fps, 30);
        assert_eq!(report.raw_timer_events, 40);
        assert_eq!(report.coalesced_wakeups, 10);
        assert_eq!(report.skipped_clean_frames, 10);
        assert!(report.wakeups_per_sec < 15.0);
        assert!(report.cstate.c6_c10_deep_pct >= 85.0);
        assert!(validate_wakeup_sla(14.99, 90.0).is_ok());
        assert_eq!(
            validate_wakeup_sla(15.0, 90.0),
            Err(BatterySaverError::WakeupBudgetExceeded {
                actual_per_sec: 15.0,
                limit: 15.0,
            })
        );
    }

    #[test]
    fn test_sub_976_4_nine_dimensions_boundaries_and_redaction() {
        let mut ok_in = WorkloadBoundaryCheck {
            snowflake_id: 0,
            message_len: 2_000,
            attachment_count: 10,
            attachment_bytes: 25 * 1024 * 1024,
            search_offset: 9_975,
            volume_pct: 200,
            unread_count: 150,
            can_view_channel: true,
            age_gated_allowed: true,
        };
        assert_eq!(validate_workload_boundaries(&ok_in), Ok("99+".to_string()));
        ok_in.snowflake_id = u64::MAX;
        ok_in.unread_count = 7;
        assert_eq!(validate_workload_boundaries(&ok_in), Ok("7".to_string()));
        ok_in.message_len = 2_001;
        assert!(validate_workload_boundaries(&ok_in).is_err());
        ok_in.message_len = 100;
        ok_in.can_view_channel = false;
        assert_eq!(
            validate_workload_boundaries(&ok_in),
            Err(BatterySaverError::AccessDenied("MISSING_VIEW_CHANNEL"))
        );
        assert_eq!(compute_rest_backoff_ms(202, None, 1, true), 1_000);
        assert_eq!(compute_rest_backoff_ms(429, Some(1_230), 1, true), 1_300);
        assert_eq!(compute_rest_backoff_ms(503, None, 2, true), 2_000);
        assert_eq!(
            resolve_linux_idle_time_ms(true, Ok(4_200), Some(10), 20),
            4_200
        );
        assert_eq!(
            resolve_linux_idle_time_ms(true, Err("wayland_err"), None, 15),
            15_000
        );
        let ctrl = BatterySaverController::new();
        let rep = run_battery_saver_benchmark(1_000).expect("bench");
        let json = export_power_telemetry_json(
            &ctrl.current_decision(),
            &rep,
            "user mfa.secretToken123456 Bot_abc123",
        );
        assert!(json.contains("mfa.***") && !json.contains("mfa.secretToken123456"));
    }
}
