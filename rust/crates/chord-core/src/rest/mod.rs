//! REST v9 (RW-08 / P1-07): каталог маршрутов, auth-заголовки, коды ответов, ретраи/таймауты.
//!
//! Границы RW-08: база — `crate::API_V9_BASE`; транспорт вошёл отдельной волной RW-10
//! (вариант A: ручной HTTP/1.1 на tokio + tokio-rustls, решение владельца 2026-10-20).
//!
//! Волны RW-10/RW-11: `http1`/`transport` — транспорт; `rate_limit` — P1-09;
//! `telemetry` — P1-11; `ack` — P1-12. Общие файлы правит только координатор.

pub mod ack;
pub mod base64;
pub mod client;
pub mod guilds;
pub mod headers;
pub mod http1;
pub mod identity;
pub mod rate_limit;
pub mod resources;
pub mod retry;
pub mod routes;
pub mod search;
pub mod super_properties;
pub mod telemetry;
pub mod transport;
pub mod user;
