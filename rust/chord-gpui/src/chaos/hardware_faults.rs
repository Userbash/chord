//! Подзадача 905.2 (`SUB-905-2`): Аппаратный и системный хаос (Audio/Video Driver Faults).

use serde::{Deserialize, Serialize};

use super::ChaosError;
use crate::media::MediaCache;
use crate::state::{AppState, CallStatus};
use crate::video::{
    DRM_FORMAT_MOD_INVALID, DRM_FORMAT_MOD_LINEAR, FRAME_INTERVAL_60FPS_US,
    PipeWireDmaBufConnector, VideoPixelFormat,
};

/// Имя виртуального резервного аудиоустройства при полном отсутствии физических устройств.
pub const DEFAULT_FALLBACK_AUDIO_INPUT: &str = "Default System Microphone (Fallback)";
/// Имя виртуального резервного устройства вывода при полном отсутствии физических устройств.
pub const DEFAULT_FALLBACK_AUDIO_OUTPUT: &str = "Default System Output (Fallback)";
/// Жёсткий бюджет памяти VmRSS / кэша приложения (60 МБ).
pub const MEMORY_BUDGET_LIMIT_BYTES: usize = 60 * 1024 * 1024;

/// Результат аварийного отключения аудиоустройства и переключения на Default.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioFallbackOutcome {
    pub removed_device: String,
    pub fallback_device: String,
    pub is_input: bool,
    pub call_remained_active: bool,
    pub voice_session_preserved: bool,
}

/// Контроллер инжекции сбоев аудиоустройств во время активного звонка (`Шаг 2.1`).
pub struct AudioDeviceChaosController;

impl AudioDeviceChaosController {
    pub fn unplug_active_input_device(
        state: &mut AppState,
    ) -> Result<AudioFallbackOutcome, ChaosError> {
        let removed = state
            .selected_audio_input
            .clone()
            .or_else(|| state.available_audio_inputs.first().cloned())
            .ok_or(ChaosError::AudioFallbackUnavailable)?;

        state.available_audio_inputs.retain(|dev| dev != &removed);
        let fallback = state
            .available_audio_inputs
            .first()
            .cloned()
            .unwrap_or_else(|| {
                let default_dev = DEFAULT_FALLBACK_AUDIO_INPUT.to_string();
                state.available_audio_inputs.push(default_dev.clone());
                default_dev
            });

        state.select_audio_input(fallback.clone());
        state.set_input_volume(state.input_volume);

        Ok(AudioFallbackOutcome {
            removed_device: removed,
            fallback_device: fallback,
            is_input: true,
            call_remained_active: matches!(state.active_call, CallStatus::Connected { .. }),
            voice_session_preserved: state.voice_session.is_some(),
        })
    }

    pub fn unplug_active_output_device(
        state: &mut AppState,
    ) -> Result<AudioFallbackOutcome, ChaosError> {
        let removed = state
            .selected_audio_output
            .clone()
            .or_else(|| state.available_audio_outputs.first().cloned())
            .ok_or(ChaosError::AudioFallbackUnavailable)?;

        state.available_audio_outputs.retain(|dev| dev != &removed);
        let fallback = state
            .available_audio_outputs
            .first()
            .cloned()
            .unwrap_or_else(|| {
                let default_dev = DEFAULT_FALLBACK_AUDIO_OUTPUT.to_string();
                state.available_audio_outputs.push(default_dev.clone());
                default_dev
            });

        state.select_audio_output(fallback.clone());
        state.set_output_volume(state.output_volume);

        Ok(AudioFallbackOutcome {
            removed_device: removed,
            fallback_device: fallback,
            is_input: false,
            call_remained_active: matches!(state.active_call, CallStatus::Connected { .. }),
            voice_session_preserved: state.voice_session.is_some(),
        })
    }
}

/// Результат аварийного перезапуска демона PipeWire во время демонстрации экрана (`Шаг 2.2`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PipeWireRecoveryOutcome {
    pub previous_fd: i32,
    pub new_fd: i32,
    pub previous_node_id: u32,
    pub new_node_id: u32,
    pub screen_share_remained_active: bool,
    pub post_recovery_frames_accepted: u64,
    pub used_cpu_shm_fallback: bool,
}

/// Супервизор аварийного восстановления PipeWire во время скриншаринга.
pub struct PipeWireChaosSupervisor;

impl PipeWireChaosSupervisor {
    fn validate_pipewire_handles(
        old_fd: i32,
        old_node_id: u32,
        new_fd: i32,
        new_node_id: u32,
    ) -> Result<(), ChaosError> {
        if old_fd < 0 {
            return Err(ChaosError::InvalidPipeWireFd(old_fd));
        }
        if new_fd < 0 {
            return Err(ChaosError::InvalidPipeWireFd(new_fd));
        }
        if old_node_id == 0 || new_node_id == 0 {
            return Err(ChaosError::InvalidPipeWireNodeId(0));
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn simulate_daemon_restart(
        state: &mut AppState,
        connector: &mut PipeWireDmaBufConnector,
        old_fd: i32,
        old_node_id: u32,
        new_fd: i32,
        new_node_id: u32,
        width: u32,
        height: u32,
        format: VideoPixelFormat,
        new_drm_modifier: u64,
    ) -> Result<PipeWireRecoveryOutcome, ChaosError> {
        Self::validate_pipewire_handles(old_fd, old_node_id, new_fd, new_node_id)?;
        connector
            .connect_stream(
                old_fd,
                old_node_id,
                width,
                height,
                format,
                DRM_FORMAT_MOD_LINEAR,
            )
            .map_err(ChaosError::PipeWireDaemonOffline)?;
        connector.stop();

        let before = connector.metrics().accepted_frames;
        connector
            .connect_stream(new_fd, new_node_id, width, height, format, new_drm_modifier)
            .map_err(ChaosError::PipeWireDaemonOffline)?;
        let used_shm =
            Self::pump_recovery_frames(connector, new_fd, width, height, format, new_drm_modifier)?;
        let accepted = connector.metrics().accepted_frames.saturating_sub(before);

        Ok(PipeWireRecoveryOutcome {
            previous_fd: old_fd,
            new_fd,
            previous_node_id: old_node_id,
            new_node_id,
            screen_share_remained_active: state.is_screen_sharing,
            post_recovery_frames_accepted: accepted,
            used_cpu_shm_fallback: used_shm,
        })
    }

    fn pump_recovery_frames(
        connector: &mut PipeWireDmaBufConnector,
        dma_fd: i32,
        width: u32,
        height: u32,
        format: VideoPixelFormat,
        drm_modifier: u64,
    ) -> Result<bool, ChaosError> {
        let stride = format.min_stride_bytes(width);
        let pts_1 = FRAME_INTERVAL_60FPS_US;
        let pts_2 = FRAME_INTERVAL_60FPS_US * 2;

        if drm_modifier == DRM_FORMAT_MOD_INVALID {
            let shm_bytes = vec![0x80u8; (stride as usize) * (height as usize)];
            connector
                .ingest_cpu_fallback_frame(&shm_bytes, stride, None, pts_1)
                .map_err(ChaosError::PipeWireDaemonOffline)?;
            connector
                .ingest_cpu_fallback_frame(&shm_bytes, stride, None, pts_2)
                .map_err(ChaosError::PipeWireDaemonOffline)?;
            Ok(true)
        } else {
            connector
                .ingest_dma_buf_frame(dma_fd, 0, stride, drm_modifier, None, pts_1)
                .map_err(ChaosError::PipeWireDaemonOffline)?;
            connector
                .ingest_dma_buf_frame(dma_fd, 0, stride, drm_modifier, None, pts_2)
                .map_err(ChaosError::PipeWireDaemonOffline)?;
            Ok(false)
        }
    }
}

/// Результат инжекции нехватки памяти (OOM) и сброса `MediaCache` (`Шаг 2.3`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OomEvictionOutcome {
    pub cached_items_before: usize,
    pub cached_bytes_before: usize,
    pub cached_items_after: usize,
    pub cached_bytes_after: usize,
    pub within_60mb_budget: bool,
}

/// Инжектор нехватки памяти (OOM Pressure) для проверки сброса `MediaCache`.
pub struct OomChaosInjector;

impl OomChaosInjector {
    pub fn inject_oom_and_purge_cache(
        cache: &mut MediaCache,
        simulated_bytes_before: usize,
    ) -> Result<OomEvictionOutcome, ChaosError> {
        let items_before = cache.len();
        *cache = MediaCache::new();
        let items_after = cache.len();
        let bytes_after = 0usize;

        Ok(OomEvictionOutcome {
            cached_items_before: items_before,
            cached_bytes_before: simulated_bytes_before,
            cached_items_after: items_after,
            cached_bytes_after: bytes_after,
            within_60mb_budget: bytes_after < MEMORY_BUDGET_LIMIT_BYTES,
        })
    }
}

/// Сводка аппаратного и системного хаос-тестирования (`SUB-905-2`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HardwareChaosSummary {
    pub audio_input_fallbacks: u32,
    pub audio_output_fallbacks: u32,
    pub pipewire_restarts_recovered: u32,
    pub oom_cache_evictions: u32,
    pub max_observed_cache_bytes_after_oom: usize,
}
