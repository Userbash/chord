//! Точка входа `discord-rs` — каркас фазы P0.
//!
//! Многопроцессная модель (main/renderer → единый процесс с worker-пулами) — P0-03;
//! конфигурация и пути — P0-04 (`docs/rust-client/PLAN.md`).

fn main() {
    // Проверяем связность воркспейса: константы ядра выведены в стартовый баннер.
    println!(
        "discord-rs: каркас P0 — heartbeat {} мс, REST {}",
        discord_core::HEARTBEAT_INTERVAL_MS,
        discord_core::API_V9_BASE
    );
    println!("Реализация — волнами P1–P8 по docs/rust-client/PLAN.md (red-first).");
}
