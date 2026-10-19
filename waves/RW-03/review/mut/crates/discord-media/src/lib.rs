//! # discord-media
//!
//! Медиа-конвейер: аудио (Opus) и видео (AV1) кодеки, захват экрана/камеры, стриминг.
//!
//! Спека: `docs/re/voice-deep.md` §4 (движок кодеков из логов `[Connection]`).
//! План: P3-06…P3-09 (медиа), P8 (скриншот-стрим); тесты: T-MEDIA-*.
//!
//! Статус: каркас P0.

/// Аудио-кодек медиа-канала.
pub const AUDIO_CODEC: &str = "opus";

/// Видео-кодек медиа-канала (Session Description).
pub const VIDEO_CODEC: &str = "AV1";

/// Доступные движку кодеки из лога клиента (`Available engine codecs`).
pub const ENGINE_CODECS: [&str; 4] = ["AV1", "H264", "H265", "VP8"];

#[cfg(test)]
mod tests {
    #[test]
    fn engine_codecs_match_client_log() {
        assert_eq!(super::ENGINE_CODECS.len(), 4);
        assert!(super::ENGINE_CODECS.contains(&super::VIDEO_CODEC));
    }
}
