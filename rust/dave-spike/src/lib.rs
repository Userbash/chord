#![forbid(unsafe_code)]
//! Comparative spike prototype for Discord DAVE E2EE (`LIB-SIMPLIFY-05`):
//! `libdave` (official Discord C++ ABI via Rust FFI) vs `davey` + `OpenMLS`/RustCrypto from Serein.

pub mod audit;
pub mod bridge;
pub mod davey_crypto;
pub mod davey_frame;
pub mod davey_mls_codec;
pub mod davey_session;
pub mod protocol;
pub mod wire;
