//! # Soundshare & PipeWire Watchdog Subsystem (Go Live Stream Audio)
//!
//! Модуль управления захватом системного звука и звука окон приложений/игр для трансляций:
//! - Формирование Voice Gateway OP 5 SPEAKING кадров с флагом `speaking: 2` (Soundshare).
//! - Авто-определение оптимального аудиомонитора PipeWire / PulseAudio (EasyEffects, sink-inputs активных окон).
//! - Непрерывный PipeWire-вотчдог, восстанавливающий громкость, un-mute и линковку источника
//!   при подключении или отключении участников голосового канала.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// Информация об аудиовыходе захвата стрима (`source-output`) в PipeWire/PulseAudio.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceOutputInfo {
    pub id: String,
    pub muted: bool,
    pub volume_percent: u32,
    pub source_id: String,
    pub is_target_stream: bool,
}

/// Необходимые действия по исправлению звукового тракта трансляции.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SoundshareRemediation {
    pub unmute_needed: bool,
    pub volume_reset_needed: bool,
    pub move_source_needed: bool,
}

/// Дескриптор управления работающим вотчдогом звука стрима.
#[derive(Debug, Clone)]
pub struct SoundshareWatchdogHandle {
    stop_signal: Arc<AtomicBool>,
}

impl SoundshareWatchdogHandle {
    /// Сигнализирует вотчдогу о необходимости завершить работу.
    pub fn stop(&self) {
        self.stop_signal.store(true, Ordering::SeqCst);
    }
}

/// Формирует JSON-строку Voice Gateway `OP 5 SPEAKING` с флагом `speaking: 2` (Soundshare).
///
/// В Discord Voice Protocol:
/// - `1 << 0` (1) = голос (микрофон);
/// - `1 << 1` (2) = звук трансляции / демонстрации экрана (Soundshare);
/// - `1 << 2` (4) = приоритетный звук.
#[must_use]
pub fn build_soundshare_speaking_op5(ssrc: u32) -> String {
    format!("{{\"op\":5,\"d\":{{\"speaking\":2,\"delay\":0,\"ssrc\":{ssrc}}}}}")
}

/// Разбирает текстовый вывод команды `pactl list source-outputs`.
///
/// Извлекает идентификатор, признак мьюта, процент громкости и сопоставляет
/// с целевым процессом стрима `child_pid`.
#[must_use]
pub fn parse_source_outputs(text: &str, child_pid: u32) -> Vec<SourceOutputInfo> {
    let mut results = Vec::new();
    let mut cur_id: Option<String> = None;
    let mut cur_source: Option<String> = None;
    let mut cur_muted = false;
    let mut cur_vol_pct: u32 = 100;
    let mut is_our_stream = false;

    let pid_marker = format!("application.process.id = \"{child_pid}\"");

    let flush_entry = |res: &mut Vec<SourceOutputInfo>,
                       id_opt: &mut Option<String>,
                       src_opt: &mut Option<String>,
                       muted: bool,
                       vol: u32,
                       is_target: bool| {
        if let (Some(id), true) = (id_opt.take(), is_target) {
            res.push(SourceOutputInfo {
                id,
                muted,
                volume_percent: vol,
                source_id: src_opt.take().unwrap_or_default(),
                is_target_stream: true,
            });
        }
    };

    for line in text.lines() {
        let trim = line.trim();
        if let Some(rest) = trim.strip_prefix("Source Output #") {
            flush_entry(
                &mut results,
                &mut cur_id,
                &mut cur_source,
                cur_muted,
                cur_vol_pct,
                is_our_stream,
            );
            cur_id = Some(rest.trim().to_string());
            cur_source = None;
            cur_muted = false;
            cur_vol_pct = 100;
            is_our_stream = false;
        } else if let Some(rest) = trim.strip_prefix("Source: ") {
            cur_source = Some(rest.trim().to_string());
        } else if let Some(rest) = trim.strip_prefix("Mute: ") {
            cur_muted = rest.trim().eq_ignore_ascii_case("yes");
        } else if trim.starts_with("Volume: ") {
            if let Some(pct_idx) = trim.find('%') {
                let prefix = &trim[..pct_idx];
                if let Some(space_idx) = prefix.rfind([' ', '/']) {
                    if let Ok(v) = prefix[space_idx + 1..].trim().parse::<u32>() {
                        cur_vol_pct = v;
                    }
                }
            }
        } else if trim.contains("lab.discord.stream.audio")
            || trim.contains(&pid_marker)
            || trim.contains("DiscordStreamAudio")
        {
            is_our_stream = true;
        }
    }

    flush_entry(
        &mut results,
        &mut cur_id,
        &mut cur_source,
        cur_muted,
        cur_vol_pct,
        is_our_stream,
    );
    results
}

/// Вычисляет, требуется ли восстановление параметров для `source-output`.
#[must_use]
pub fn evaluate_source_output_remediation(
    info: &SourceOutputInfo,
    target_monitor: &str,
) -> SoundshareRemediation {
    SoundshareRemediation {
        unmute_needed: info.muted,
        volume_reset_needed: info.volume_percent < 80,
        move_source_needed: !target_monitor.is_empty()
            && !info.source_id.is_empty()
            && !target_monitor.starts_with(&info.source_id),
    }
}

/// Разрешает человекочитаемое имя аудио-синка по его числовому идентификатору в PulseAudio/PipeWire.
pub fn resolve_sink_name_by_id(sink_id: &str) -> Option<String> {
    if let Ok(out) = std::process::Command::new("pactl")
        .args(["list", "short", "sinks"])
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
    {
        if out.status.success() {
            let text = String::from_utf8_lossy(&out.stdout);
            for line in text.lines() {
                let cols: Vec<&str> = line.split_whitespace().collect();
                if cols.first().copied() == Some(sink_id) {
                    if let Some(&name) = cols.get(1) {
                        return Some(name.to_string());
                    }
                }
            }
        }
    }
    None
}

/// Определяет наилучший источник монитора звука для демонстрации экрана.
///
/// Иерархия приоритетов:
/// 1. `easyeffects_sink.monitor` (если используется EasyEffects для объединения звука окон);
/// 2. Монитор активного приложения/игры через `sink-inputs` (исключая системные и собственные звуки Chord);
/// 3. Пользовательское устройство вывода из настроек приложения;
/// 4. Системный sink по умолчанию (`pactl get-default-sink`).
pub fn resolve_optimal_stream_soundshare_source() -> String {
    // 1. Приоритет: easyeffects_sink
    if let Ok(out) = std::process::Command::new("pactl")
        .args(["list", "short", "sinks"])
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
    {
        if out.status.success() {
            let text = String::from_utf8_lossy(&out.stdout);
            for line in text.lines() {
                let cols: Vec<&str> = line.split_whitespace().collect();
                if let Some(&name) = cols.get(1) {
                    if name == "easyeffects_sink" {
                        return "easyeffects_sink.monitor".to_string();
                    }
                }
            }
        }
    }

    // 2. Приоритет: проверяем pactl list sink-inputs, куда играют звук активные приложения (игры, плееры, браузер)
    if let Ok(out) = std::process::Command::new("pactl")
        .args(["list", "sink-inputs"])
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
    {
        if out.status.success() {
            let text = String::from_utf8_lossy(&out.stdout);
            let mut active_sink_num: Option<String> = None;
            let mut is_excluded_app = false;
            for line in text.lines() {
                let trim = line.trim();
                if let Some(rest) = trim.strip_prefix("Sink Input #") {
                    let _ = rest;
                    active_sink_num = None;
                    is_excluded_app = false;
                } else if let Some(rest) = trim.strip_prefix("Sink: ") {
                    let sink_id = rest.trim();
                    if !sink_id.is_empty() {
                        active_sink_num = Some(sink_id.to_string());
                    }
                } else if trim.contains("application.name = \"Discord")
                    || trim.contains("application.name = \"Chord")
                    || trim.contains("media.name = \"Discord-SFX\"")
                {
                    is_excluded_app = true;
                } else if trim.starts_with("Corked: no") && !is_excluded_app {
                    if let Some(ref snum) = active_sink_num {
                        if let Some(sname) = resolve_sink_name_by_id(snum) {
                            if !sname.contains("auto_null") {
                                return format!("{sname}.monitor");
                            }
                        }
                    }
                }
            }
        }
    }

    // 3. Приоритет: монитор выбранного пользователем устройства вывода
    let (_, out_d) = crate::ws::get_selected_voice_devices();
    if let Some(dev) = out_d.filter(|d| !d.is_empty() && d != "default") {
        return format!("{dev}.monitor");
    }

    // 4. Приоритет: системный default sink
    if let Ok(out) = std::process::Command::new("pactl")
        .arg("get-default-sink")
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
    {
        if out.status.success() {
            let sink = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !sink.is_empty() && !sink.contains("auto_null") {
                return format!("{sink}.monitor");
            }
        }
    }

    "default".to_string()
}

/// Принудительно выставляет un-mute, полную громкость и целевой источник для `source-output`.
pub fn enforce_pactl_source_output(id: &str, target_monitor: &str) {
    let _ = std::process::Command::new("pactl")
        .args(["set-source-output-mute", id, "0"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    let _ = std::process::Command::new("pactl")
        .args(["set-source-output-volume", id, "65536"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    if !target_monitor.is_empty() && target_monitor != "default" {
        let _ = std::process::Command::new("pactl")
            .args(["move-source-output", id, target_monitor])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
    }
}

/// Проверяет, существует ли процесс с заданным PID в операционной системе.
#[must_use]
pub fn is_process_alive(pid: u32) -> bool {
    if pid <= 1 {
        return false;
    }
    #[cfg(target_os = "linux")]
    {
        std::path::Path::new(&format!("/proc/{pid}")).exists()
    }
    #[cfg(not(target_os = "linux"))]
    {
        std::process::Command::new("kill")
            .args(["-0", &pid.to_string()])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
}

/// Запускает фоновый вотчдог защиты звукового потока стрима.
///
/// Работает непрерывно на протяжении всего времени жизни процесса стрима `child_pid`,
/// периодически проверяя состояние в PulseAudio/PipeWire и восстанавливая звук при любых
/// внешних сбросах (подключение/отключение участников, переключение устройств).
pub fn spawn_stream_soundshare_enforcer(
    child_pid: u32,
    target_monitor: String,
) -> SoundshareWatchdogHandle {
    let stop_signal = Arc::new(AtomicBool::new(false));
    let stop_signal_clone = Arc::clone(&stop_signal);

    std::thread::Builder::new()
        .name(format!("soundshare-enforcer-{child_pid}"))
        .spawn(move || {
            let mut attached = false;
            let mut last_full_check = Instant::now();

            while !stop_signal_clone.load(Ordering::Relaxed) {
                if !is_process_alive(child_pid) {
                    break;
                }

                let sleep_ms = if attached { 1500 } else { 350 };
                std::thread::sleep(Duration::from_millis(sleep_ms));

                if stop_signal_clone.load(Ordering::Relaxed) {
                    break;
                }

                if let Ok(out) = std::process::Command::new("pactl")
                    .args(["list", "source-outputs"])
                    .stdin(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .output()
                {
                    if out.status.success() {
                        let text = String::from_utf8_lossy(&out.stdout);
                        let outputs = parse_source_outputs(&text, child_pid);

                        if !outputs.is_empty() {
                            attached = true;
                            for out_info in outputs {
                                let rem =
                                    evaluate_source_output_remediation(&out_info, &target_monitor);
                                if rem.unmute_needed
                                    || rem.volume_reset_needed
                                    || rem.move_source_needed
                                    || last_full_check.elapsed() > Duration::from_secs(10)
                                {
                                    enforce_pactl_source_output(&out_info.id, &target_monitor);
                                }
                            }
                            if last_full_check.elapsed() > Duration::from_secs(10) {
                                last_full_check = Instant::now();
                            }
                        } else {
                            attached = false;
                        }
                    }
                }
            }
        })
        .ok();

    SoundshareWatchdogHandle { stop_signal }
}
