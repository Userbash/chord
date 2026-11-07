//! Точка входа `discord-rs` — единая сборка нативных крейтов волн RW-01..RW-40.
//!
//! Многопроцессная модель (main/renderer → единый процесс с worker-пулами) — P0-03;
//! конфигурация и пути — P0-04 (`docs/rust-client/PLAN.md`).

fn main() {
    let etf_tags = discord_codec::ETF_TAG_CODES.len();
    let locales = discord_i18n::TOTAL_LOCALES;
    let store_schema = discord_store::SCHEMA_VERSION;
    let voice_pt = discord_voice::RTP_OPUS_PAYLOAD_TYPE;
    let media_pt = discord_media::RTP_VIDEO_PAYLOAD_TYPE;
    let ui_contracts = discord_ui::COMPONENT_CONTRACTS;
    let update_ep = discord_updater::DEFAULT_UPDATE_ENDPOINT;

    println!(
        "discord-rs: каркас P0 — heartbeat {} мс, REST {}",
        discord_core::HEARTBEAT_INTERVAL_MS,
        discord_core::API_V9_BASE
    );
    println!(
        "Единый граф крейтов: ETF={etf_tags} тегов, i18n={locales} локалей, SQLite v{store_schema}, Voice PT={voice_pt}, Media PT={media_pt}, UI={ui_contracts} контрактов, Updater={update_ep}"
    );
    println!("Реализация — волнами P1–P8 по docs/rust-client/PLAN.md (red-first).");
}
