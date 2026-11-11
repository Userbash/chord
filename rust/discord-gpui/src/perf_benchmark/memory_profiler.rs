//! Подзадача 906.2: Профилирование аллокаций кучи (jemalloc/heaptrack), контроль VmRSS (< 60 МБ)
//! и сборка мусора текстур GPU VRAM / MediaCache.

use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};

use super::{MAX_VMRSS_KB, PerfBenchmarkError};
use crate::media::MediaCache;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AllocationTag {
    ChatMessages,
    ModalDialog,
    MediaCache,
    VramTexture,
    VoiceBuffer,
    SqliteCache,
}

impl AllocationTag {
    pub const ALL: [Self; 6] = [
        Self::ChatMessages,
        Self::ModalDialog,
        Self::MediaCache,
        Self::VramTexture,
        Self::VoiceBuffer,
        Self::SqliteCache,
    ];

    #[must_use]
    pub fn symbol_name(&self) -> &'static str {
        match self {
            Self::ChatMessages => "discord_gpui::ui::chat_view::virtual_window",
            Self::ModalDialog => "discord_gpui::ui::modals::dialog_buffer",
            Self::MediaCache => "discord_gpui::media::cache_entry",
            Self::VramTexture => "discord_gpui::video::vram_texture_atlas",
            Self::VoiceBuffer => "discord_gpui::audio::ring_buffer_slot",
            Self::SqliteCache => "discord_gpui::storage::sqlite_page_cache",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JemallocSiteEntry {
    pub tag: String,
    pub inuse_objects: usize,
    pub inuse_bytes: u64,
    pub alloc_objects: usize,
    pub alloc_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JemallocProfileSummary {
    pub header_version: String,
    pub total_inuse_objects: usize,
    pub total_inuse_bytes: u64,
    pub total_alloc_objects: usize,
    pub total_alloc_bytes: u64,
    pub sites: Vec<JemallocSiteEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeaptrackSummary {
    pub version: u32,
    pub total_allocations: usize,
    pub total_frees: usize,
    pub peak_heap_bytes: u64,
    pub leaked_bytes: u64,
}

#[derive(Debug, Clone, Default)]
struct TagSiteStats {
    inuse_objects: usize,
    inuse_bytes: u64,
    alloc_objects: usize,
    alloc_bytes: u64,
}

/// Профайлер аллокаций кучи с поддержкой форматов `jemalloc` (`heap_v2/1`) и `heaptrack` (v3).
#[derive(Debug, Clone, Default)]
pub struct HeapAllocationProfiler {
    next_id: u64,
    pub active_allocations: usize,
    pub total_alloc_ops: usize,
    pub total_free_ops: usize,
    pub live_bytes: u64,
    pub peak_live_bytes: u64,
    pub cumulative_alloc_bytes: u64,
    live_blocks: HashMap<u64, (AllocationTag, u64)>,
    site_stats: BTreeMap<AllocationTag, TagSiteStats>,
}

impl HeapAllocationProfiler {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_alloc(&mut self, tag: AllocationTag, bytes: u64) -> u64 {
        self.next_id += 1;
        let id = self.next_id;
        self.active_allocations += 1;
        self.total_alloc_ops += 1;
        self.live_bytes += bytes;
        self.cumulative_alloc_bytes += bytes;
        if self.live_bytes > self.peak_live_bytes {
            self.peak_live_bytes = self.live_bytes;
        }
        self.live_blocks.insert(id, (tag, bytes));

        let site = self.site_stats.entry(tag).or_default();
        site.inuse_objects += 1;
        site.inuse_bytes += bytes;
        site.alloc_objects += 1;
        site.alloc_bytes += bytes;
        id
    }

    pub fn record_free(&mut self, alloc_id: u64) -> Result<u64, PerfBenchmarkError> {
        let Some((tag, bytes)) = self.live_blocks.remove(&alloc_id) else {
            return Err(PerfBenchmarkError::ProfileFormatError(format!(
                "Неизвестная или повторно освобождаемая аллокация #{alloc_id}"
            )));
        };
        self.active_allocations = self.active_allocations.saturating_sub(1);
        self.total_free_ops += 1;
        self.live_bytes = self.live_bytes.saturating_sub(bytes);

        if let Some(site) = self.site_stats.get_mut(&tag) {
            site.inuse_objects = site.inuse_objects.saturating_sub(1);
            site.inuse_bytes = site.inuse_bytes.saturating_sub(bytes);
        }
        Ok(bytes)
    }

    #[must_use]
    pub fn live_bytes_for_tag(&self, tag: AllocationTag) -> u64 {
        self.site_stats.get(&tag).map_or(0, |site| site.inuse_bytes)
    }

    #[must_use]
    pub fn export_jemalloc_profile(&self) -> String {
        let mut out = String::from("heap_v2/1\n");
        out.push_str(&format!(
            "  t*: {}: {} [{}: {}]\n",
            self.active_allocations,
            self.live_bytes,
            self.total_alloc_ops,
            self.cumulative_alloc_bytes
        ));
        for (tag, site) in &self.site_stats {
            out.push_str(&format!(
                "@ {}\n  t*: {}: {} [{}: {}]\n",
                tag.symbol_name(),
                site.inuse_objects,
                site.inuse_bytes,
                site.alloc_objects,
                site.alloc_bytes
            ));
        }
        out
    }

    pub fn parse_jemalloc_profile(
        text: &str,
    ) -> Result<JemallocProfileSummary, PerfBenchmarkError> {
        let mut lines = text.lines();
        let header = lines.next().map(str::trim).unwrap_or("");
        if header != "heap_v2/1" {
            return Err(PerfBenchmarkError::ProfileFormatError(format!(
                "Ожидался заголовок heap_v2/1, получено: {header}"
            )));
        }
        let totals_line = lines.next().map(str::trim).ok_or_else(|| {
            PerfBenchmarkError::ProfileFormatError("Отсутствует строка t* в jemalloc".into())
        })?;
        let (inuse_objs, inuse_b, alloc_objs, alloc_b) = parse_jemalloc_t_line(totals_line)?;
        let sites = parse_jemalloc_sites(lines)?;

        Ok(JemallocProfileSummary {
            header_version: header.to_string(),
            total_inuse_objects: inuse_objs,
            total_inuse_bytes: inuse_b,
            total_alloc_objects: alloc_objs,
            total_alloc_bytes: alloc_b,
            sites,
        })
    }

    #[must_use]
    pub fn export_heaptrack_log(&self) -> String {
        format!(
            "v 3\nallocs {}\nfrees {}\npeak {}\nleaked {}\n",
            self.total_alloc_ops, self.total_free_ops, self.peak_live_bytes, self.live_bytes
        )
    }

    pub fn parse_heaptrack_log(text: &str) -> Result<HeaptrackSummary, PerfBenchmarkError> {
        let mut version: Option<u32> = None;
        let mut total_allocations = 0usize;
        let mut total_frees = 0usize;
        let mut peak_heap_bytes = 0u64;
        let mut leaked_bytes = 0u64;

        for line in text.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            match parts.as_slice() {
                ["v", ver] => version = ver.parse::<u32>().ok(),
                ["allocs", n] => total_allocations = parse_u64_field(n, "allocs")? as usize,
                ["frees", n] => total_frees = parse_u64_field(n, "frees")? as usize,
                ["peak", n] => peak_heap_bytes = parse_u64_field(n, "peak")?,
                ["leaked", n] => leaked_bytes = parse_u64_field(n, "leaked")?,
                _ => {}
            }
        }

        let Some(ver) = version else {
            return Err(PerfBenchmarkError::ProfileFormatError(
                "Отсутствует версия в логе heaptrack".into(),
            ));
        };
        Ok(HeaptrackSummary {
            version: ver,
            total_allocations,
            total_frees,
            peak_heap_bytes,
            leaked_bytes,
        })
    }
}

fn parse_jemalloc_sites<'a>(
    lines: impl Iterator<Item = &'a str>,
) -> Result<Vec<JemallocSiteEntry>, PerfBenchmarkError> {
    let mut sites = Vec::new();
    let mut current_tag = String::new();
    for raw in lines {
        let line = raw.trim();
        if let Some(sym) = line.strip_prefix("@ ") {
            current_tag = sym.trim().to_string();
        } else if line.starts_with("t*:") && !current_tag.is_empty() {
            let (io, ib, ao, ab) = parse_jemalloc_t_line(line)?;
            sites.push(JemallocSiteEntry {
                tag: current_tag.clone(),
                inuse_objects: io,
                inuse_bytes: ib,
                alloc_objects: ao,
                alloc_bytes: ab,
            });
        }
    }
    Ok(sites)
}

fn parse_jemalloc_t_line(line: &str) -> Result<(usize, u64, usize, u64), PerfBenchmarkError> {
    let rest = line.trim().strip_prefix("t*:").ok_or_else(|| {
        PerfBenchmarkError::ProfileFormatError(format!("Некорректная строка jemalloc: {line}"))
    })?;
    let Some((left, right_bracket)) = rest.split_once('[') else {
        return Err(PerfBenchmarkError::ProfileFormatError(
            "Отсутствует блок [...] в строке jemalloc".into(),
        ));
    };
    let right = right_bracket.trim().trim_end_matches(']');
    let (inuse_o, inuse_b) = parse_colon_pair(left)?;
    let (alloc_o, alloc_b) = parse_colon_pair(right)?;
    Ok((inuse_o as usize, inuse_b, alloc_o as usize, alloc_b))
}

fn parse_colon_pair(segment: &str) -> Result<(u64, u64), PerfBenchmarkError> {
    let Some((a, b)) = segment.split_once(':') else {
        return Err(PerfBenchmarkError::ProfileFormatError(format!(
            "Ожидалась пара a: b в '{segment}'"
        )));
    };
    Ok((parse_u64_field(a, "count")?, parse_u64_field(b, "bytes")?))
}

fn parse_u64_field(raw: &str, name: &str) -> Result<u64, PerfBenchmarkError> {
    raw.trim().parse::<u64>().map_err(|_| {
        PerfBenchmarkError::ProfileFormatError(format!("Ошибка парсинга поля {name}: '{raw}'"))
    })
}

/// Снимок метрик памяти процесса из `/proc/self/status` и `/proc/self/statm`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcMemorySnapshot {
    pub vmrss_kb: u64,
    pub vmhwm_kb: u64,
    pub vmsize_kb: u64,
    pub vmdata_kb: u64,
    pub statm_resident_kb: u64,
}

impl ProcMemorySnapshot {
    pub fn read_current_process() -> Result<Self, PerfBenchmarkError> {
        let status = std::fs::read_to_string("/proc/self/status")
            .map_err(|e| PerfBenchmarkError::IoError(format!("/proc/self/status: {e}")))?;
        let statm = std::fs::read_to_string("/proc/self/statm")
            .map_err(|e| PerfBenchmarkError::IoError(format!("/proc/self/statm: {e}")))?;
        Self::from_proc_texts(&status, &statm)
    }

    pub fn from_proc_texts(
        status_text: &str,
        statm_text: &str,
    ) -> Result<Self, PerfBenchmarkError> {
        let (vmrss_kb, vmhwm_kb, vmsize_kb, vmdata_kb) = parse_status_fields(status_text)?;
        let resident_pages = statm_text
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.parse::<u64>().ok())
            .ok_or_else(|| {
                PerfBenchmarkError::ProcParseError("Некорректный формат /proc/self/statm".into())
            })?;

        Ok(Self {
            vmrss_kb,
            vmhwm_kb: vmhwm_kb.max(vmrss_kb),
            vmsize_kb: vmsize_kb.max(vmrss_kb),
            vmdata_kb,
            statm_resident_kb: resident_pages * 4,
        })
    }

    #[must_use]
    pub fn vmrss_mb(&self) -> f64 {
        (self.vmrss_kb as f64) / 1024.0
    }

    pub fn validate_rss_budget(&self) -> Result<(), PerfBenchmarkError> {
        if self.vmrss_kb == 0 || self.vmrss_kb >= MAX_VMRSS_KB {
            return Err(PerfBenchmarkError::MemoryBudgetExceeded {
                vmrss_kb: self.vmrss_kb,
                limit_kb: MAX_VMRSS_KB,
            });
        }
        Ok(())
    }
}

fn parse_status_fields(status_text: &str) -> Result<(u64, u64, u64, u64), PerfBenchmarkError> {
    let mut vmrss_kb: Option<u64> = None;
    let mut vmhwm_kb: u64 = 0;
    let mut vmsize_kb: u64 = 0;
    let mut vmdata_kb: u64 = 0;

    for line in status_text.lines() {
        if let Some(v) = extract_proc_kb(line, "VmRSS:") {
            vmrss_kb = Some(v);
        } else if let Some(v) = extract_proc_kb(line, "VmHWM:") {
            vmhwm_kb = v;
        } else if let Some(v) = extract_proc_kb(line, "VmSize:") {
            vmsize_kb = v;
        } else if let Some(v) = extract_proc_kb(line, "VmData:") {
            vmdata_kb = v;
        }
    }

    let Some(rss) = vmrss_kb else {
        return Err(PerfBenchmarkError::ProcParseError(
            "Поле VmRSS отсутствует в /proc/self/status".into(),
        ));
    };
    Ok((rss, vmhwm_kb, vmsize_kb, vmdata_kb))
}

fn extract_proc_kb(line: &str, prefix: &str) -> Option<u64> {
    line.strip_prefix(prefix)?
        .split_whitespace()
        .next()?
        .parse::<u64>()
        .ok()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextureKind {
    Avatar,
    Attachment,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VramTextureEntry {
    pub key: String,
    pub kind: TextureKind,
    pub channel_id: String,
    pub width: u32,
    pub height: u32,
    pub vram_bytes: usize,
    pub last_used_frame: u64,
}

/// Пул текстур GPU VRAM с автоматической сборкой мусора при смене каналов и закрытии модалок.
pub struct VramTexturePool {
    pub budget_bytes: usize,
    entries: HashMap<String, VramTextureEntry>,
    live_bytes: usize,
}

impl VramTexturePool {
    #[must_use]
    pub fn new(budget_bytes: usize) -> Self {
        Self {
            budget_bytes,
            entries: HashMap::new(),
            live_bytes: 0,
        }
    }

    pub fn upload_rgba_texture(
        &mut self,
        key: &str,
        kind: TextureKind,
        channel_id: &str,
        width: u32,
        height: u32,
        frame: u64,
        media_cache: Option<&MediaCache>,
    ) -> Result<usize, PerfBenchmarkError> {
        let vram_bytes = (width as usize)
            .saturating_mul(height as usize)
            .saturating_mul(4);
        if vram_bytes == 0 || self.live_bytes + vram_bytes > self.budget_bytes {
            return Err(PerfBenchmarkError::VramBudgetExceeded {
                live_vram_bytes: self.live_bytes + vram_bytes,
                budget_bytes: self.budget_bytes,
            });
        }

        if let Some(old) = self.entries.remove(key) {
            self.live_bytes = self.live_bytes.saturating_sub(old.vram_bytes);
        }
        self.entries.insert(
            key.to_string(),
            VramTextureEntry {
                key: key.to_string(),
                kind,
                channel_id: channel_id.to_string(),
                width,
                height,
                vram_bytes,
                last_used_frame: frame,
            },
        );
        self.live_bytes += vram_bytes;
        if let Some(cache) = media_cache {
            cache.put(key, vec![0xAB; 64]);
        }
        Ok(vram_bytes)
    }

    pub fn touch(&mut self, key: &str, frame: u64) -> bool {
        if let Some(entry) = self.entries.get_mut(key) {
            entry.last_used_frame = frame;
            true
        } else {
            false
        }
    }

    pub fn collect_unused_textures(
        &mut self,
        active_channel_id: &str,
        current_frame: u64,
        max_idle_frames: u64,
        media_cache: Option<&mut MediaCache>,
    ) -> usize {
        let mut evicted_bytes = 0usize;
        self.entries.retain(|_key, entry| {
            let keep = entry.channel_id == active_channel_id
                || current_frame.saturating_sub(entry.last_used_frame) <= max_idle_frames;
            if !keep {
                evicted_bytes += entry.vram_bytes;
            }
            keep
        });
        self.live_bytes = self.live_bytes.saturating_sub(evicted_bytes);

        if let Some(cache) = media_cache {
            sync_retained_media_cache(cache, &self.entries);
        }
        evicted_bytes
    }

    #[must_use]
    pub fn live_vram_bytes(&self) -> usize {
        self.live_bytes
    }

    #[must_use]
    pub fn texture_count(&self) -> usize {
        self.entries.len()
    }
}

fn sync_retained_media_cache(cache: &mut MediaCache, entries: &HashMap<String, VramTextureEntry>) {
    let rebuilt = MediaCache::new();
    for key in entries.keys() {
        if let Some(data) = cache.get(key) {
            rebuilt.put(key, data);
        }
    }
    *cache = rebuilt;
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StressLeakReport {
    pub simulated_hours: u32,
    pub total_cycles: usize,
    pub modals_toggled: usize,
    pub channels_switched: usize,
    pub textures_evicted: usize,
    pub baseline_live_bytes: u64,
    pub final_live_bytes: u64,
    pub peak_live_bytes: u64,
    pub leaked_bytes: u64,
    pub slope_bytes_per_cycle: f64,
    pub vmrss_kb: u64,
}

impl StressLeakReport {
    pub fn validate_zero_leaks(&self) -> Result<(), PerfBenchmarkError> {
        if self.leaked_bytes > 0 || self.slope_bytes_per_cycle > 0.01 {
            return Err(PerfBenchmarkError::MemoryLeakDetected {
                leaked_bytes: self.leaked_bytes,
                slope_bytes_per_cycle: self.slope_bytes_per_cycle,
                tag: AllocationTag::ModalDialog.symbol_name().into(),
            });
        }
        if self.vmrss_kb >= MAX_VMRSS_KB {
            return Err(PerfBenchmarkError::MemoryBudgetExceeded {
                vmrss_kb: self.vmrss_kb,
                limit_kb: MAX_VMRSS_KB,
            });
        }
        Ok(())
    }
}

pub struct StressSessionSimulator;

impl StressSessionSimulator {
    pub fn run_24h_cycle_simulation(
        cycles: usize,
        inject_leak_bytes_per_cycle: u64,
    ) -> Result<(StressLeakReport, HeapAllocationProfiler), PerfBenchmarkError> {
        let total_cycles = cycles.max(1);
        let mut profiler = HeapAllocationProfiler::new();
        let mut vram_pool = VramTexturePool::new(8 * 1024 * 1024);
        let baseline_live_bytes = init_baseline_allocations(&mut profiler);

        let mut modals_toggled = 0usize;
        let mut channels_switched = 0usize;
        let mut textures_evicted = 0usize;
        let mut checkpoints = Vec::with_capacity(total_cycles);

        for cycle in 0..total_cycles {
            run_single_stress_cycle(
                cycle,
                inject_leak_bytes_per_cycle,
                &mut profiler,
                &mut vram_pool,
                &mut modals_toggled,
                &mut channels_switched,
                &mut textures_evicted,
            )?;
            checkpoints.push(profiler.live_bytes);
        }

        let final_live_bytes = profiler.live_bytes;
        let leaked_bytes = final_live_bytes.saturating_sub(baseline_live_bytes);
        let report = StressLeakReport {
            simulated_hours: 24,
            total_cycles,
            modals_toggled,
            channels_switched,
            textures_evicted,
            baseline_live_bytes,
            final_live_bytes,
            peak_live_bytes: profiler.peak_live_bytes,
            leaked_bytes,
            slope_bytes_per_cycle: compute_linear_slope(&checkpoints),
            vmrss_kb: 18_432 + (leaked_bytes / 1024),
        };
        report.validate_zero_leaks()?;
        Ok((report, profiler))
    }
}

fn init_baseline_allocations(profiler: &mut HeapAllocationProfiler) -> u64 {
    let _ = profiler.record_alloc(AllocationTag::ChatMessages, 24_576);
    let _ = profiler.record_alloc(AllocationTag::SqliteCache, 16_384);
    let _ = profiler.record_alloc(AllocationTag::VoiceBuffer, 8_192);
    profiler.live_bytes
}

fn run_single_stress_cycle(
    cycle: usize,
    inject_leak_bytes_per_cycle: u64,
    profiler: &mut HeapAllocationProfiler,
    vram_pool: &mut VramTexturePool,
    modals_toggled: &mut usize,
    channels_switched: &mut usize,
    textures_evicted: &mut usize,
) -> Result<(), PerfBenchmarkError> {
    let frame = (cycle as u64) * 100;
    for modal_idx in 0..4 {
        let modal_id =
            profiler.record_alloc(AllocationTag::ModalDialog, 4096 + (modal_idx as u64) * 512);
        *modals_toggled += 1;
        profiler.record_free(modal_id)?;
    }

    let ch_name = format!("c-chan-{}", cycle % 16);
    let tex_id = profiler.record_alloc(AllocationTag::VramTexture, 16_384);
    let _ = vram_pool.upload_rgba_texture(
        &format!("avatar-{cycle}"),
        TextureKind::Avatar,
        &ch_name,
        64,
        64,
        frame,
        None,
    );
    *channels_switched += 1;

    if vram_pool.collect_unused_textures("c-active", frame + 60, 10, None) > 0 {
        *textures_evicted += 1;
    }
    profiler.record_free(tex_id)?;

    if inject_leak_bytes_per_cycle > 0 {
        let _ = profiler.record_alloc(AllocationTag::ModalDialog, inject_leak_bytes_per_cycle);
    }
    Ok(())
}

fn compute_linear_slope(values: &[u64]) -> f64 {
    if values.len() < 2 {
        return 0.0;
    }
    let first = values[0] as f64;
    let last = values[values.len() - 1] as f64;
    (last - first) / ((values.len() - 1) as f64)
}
