//! REST v9 (RW-08 / P1-07): каталог маршрутов, auth-заголовки, коды ответов, ретраи/таймауты.
//!
//! Границы волны: база — `crate::API_V9_BASE`; транспорт (TLS/HTTP-клиент) не входит —
//! вопрос владельцу открыт; тесты волны работают на локальном стабе (`127.0.0.1`).
//!
//! Каркас волны RW-08: файлы-модули — воркерам W8a/W8b/W8c; этот файл заморожен.

pub mod base64;
pub mod headers;
pub mod identity;
pub mod retry;
pub mod routes;
pub mod super_properties;
