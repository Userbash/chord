//! # discord-core
//!
//! Транспорт и логика Discord: WebSocket-шлюз, REST v9, сессии, heartbeat, реконнект.
//!
//! Спека: `docs/re/gateway-protocol.md` §4–6, `docs/re/gateway-schema.md`.
//! План: P1 (транспорт), P2 (REST/данные); тесты: T-GW-*, T-REST-*.
//!
//! Статус: каркас P0 — константы, подтверждённые захватом.

/// База REST API v9 (47 подтверждённых маршрутов, 133 запроса в захвате D-02).
pub const API_V9_BASE: &str = "https://discord.com/api/v9";

/// Интервал heartbeat из op10 HELLO — **41 250 мс** (подтверждён в `gateway`).
pub const HEARTBEAT_INTERVAL_MS: u64 = 41_250;

/// Число ключей в READY (top-level) — 40 (`gateway/by-type/002-READY.json`).
pub const READY_TOP_LEVEL_KEYS: usize = 40;

/// Идентификаторы op-кодов шлюза, наблюдавшиеся в волне R7 (op0…op11).
pub const OBSERVED_OPCODES: [u8; 9] = [0, 1, 2, 4, 6, 7, 9, 10, 11];

/// Всего op0-диспатчей в захвате (union 7 сокетов) — 1 340.
pub const DISPATCH_COUNT_R7: u32 = 1_340;

/// Типов диспатчей (op0) в захвате — 21 (`gateway-protocol.md` §4).
pub const DISPATCH_TYPE_COUNT_R7: usize = 21;

#[cfg(test)]
mod tests {
    #[test]
    fn gateway_constants_match_capture() {
        assert_eq!(super::HEARTBEAT_INTERVAL_MS, 41_250);
        assert_eq!(super::READY_TOP_LEVEL_KEYS, 40);
        assert_eq!(super::DISPATCH_TYPE_COUNT_R7, 21);
    }
}
