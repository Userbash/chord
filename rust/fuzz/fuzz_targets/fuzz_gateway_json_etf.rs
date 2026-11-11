#![no_main]
//! Фаззинг входящего конвейера шлюза Discord: JSON (`discord-json`), ETF (`discord-codec::etf`),
//! потоковый декодер `ZstdStream` (включая защиту от RLE zip-bomb) и `GatewayPipe` / `Session`.

use discord_codec::etf;
use discord_codec::zstd::ZstdStream;
use discord_core::ready::Ready;
use discord_core::{GatewayPipe, Session, State, apply_frame, route_dispatch};
use libfuzzer_sys::fuzz_target;
use std::sync::OnceLock;

fn base_state() -> &'static State {
    static BASE: OnceLock<State> = OnceLock::new();
    BASE.get_or_init(|| {
        let user = etf::EtfValue::Map(vec![
            (
                etf::EtfValue::Str("id".into()),
                etf::EtfValue::Str("1".into()),
            ),
            (
                etf::EtfValue::Str("username".into()),
                etf::EtfValue::Str("fuzz".into()),
            ),
        ]);
        let d = etf::EtfValue::Map(vec![
            (etf::EtfValue::Str("v".into()), etf::EtfValue::Int(10)),
            (etf::EtfValue::Str("user".into()), user),
            (
                etf::EtfValue::Str("guilds".into()),
                etf::EtfValue::List(Vec::new()),
            ),
            (
                etf::EtfValue::Str("session_id".into()),
                etf::EtfValue::Str("fuzz-session".into()),
            ),
        ]);
        let ready = Ready::parse(&d).expect("шаблон READY для фаззинга шлюза");
        State::from_ready(&ready)
    })
}

fuzz_target!(|data: &[u8]| {
    if data.len() > 16 * 1024 {
        return;
    }

    // 1. Входящие текстовые JSON-кадры основного и голосового шлюзов
    if let Ok(text) = std::str::from_utf8(data) {
        if let Ok(parsed) = discord_json::parse(text) {
            let serialized = discord_json::to_json_string(&parsed);
            let reparsed = discord_json::parse(&serialized)
                .expect("повторный разбор сериализованного JsonValue обязан проходить");
            assert_eq!(parsed, reparsed);
        }
        let _ = discord_voice::parse_frame(text);
    }

    // 2. Прямой разбор ETF-термов (с байтом версии 0x83 и без него) + сессия и редьюсер
    if let Ok((value, consumed)) = etf::decode(data) {
        assert!(consumed <= data.len());
        if let Ok(reencoded) = etf::encode_checked(&value) {
            let _ = etf::decode(&reencoded);
        }
        let mut session = Session::new(0x5EED_CAFE);
        let _ = session.on_value(&value, 1_000);
        let _ = route_dispatch(&value);
        let _ = apply_frame(base_state().clone(), &value);
    }
    let _ = etf::decode_term(data);

    // 3. Потоковый zstd + фрагментированный конвейер GatewayPipe
    let mut zstd = ZstdStream::new();
    if let Ok(chunk) = zstd.push(data) {
        assert!(chunk.len() <= discord_codec::zstd::MAX_DECOMPRESSED_CHUNK_BYTES);
    }

    if !data.is_empty() {
        let mut pipe = GatewayPipe::new(Session::new(42));
        let split = usize::from(data[0]) % data.len();
        if pipe.push_binary(&data[..split], 100).is_ok() {
            let _ = pipe.push_binary(&data[split..], 200);
        }
        let _ = pipe.take_events();
    }
});
