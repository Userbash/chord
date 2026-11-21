#![no_main]
//! Фаззинг UDP-пакетов голосового транспорта Discord:
//! 74-байтный IP Discovery (`type 1/2`), 8-байтный Keepalive (`0x1337cafe` / `0x1337f00d`)
//! и составные пакеты обратной связи RTCP (`SR`, `RR`, `SDES`, `BYE`, `NACK`, `PLI`, `FIR`, `REMB`).

use chord_voice::decode_discovery_response;
use chord_voice::rtcp::parse_compound;
use chord_voice::udp::{DiscoveryPacket, KeepalivePacket, VoiceUdpPacket};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if data.len() > 4096 {
        return;
    }

    // 1. 74-байтный пакет IP Discovery + проверка инварианта round-trip
    if let Ok(disc) = DiscoveryPacket::parse(data) {
        let encoded = disc
            .encode()
            .expect("успешно разобранный DiscoveryPacket обязан кодироваться обратно");
        let reparsed = DiscoveryPacket::parse(&encoded)
            .expect("закодированный DiscoveryPacket обязан разбираться без ошибок");
        assert_eq!(disc, reparsed);
    }

    // 2. 8-байтный пакет Keepalive (0x1337cafe / 0x1337f00d) + round-trip
    if let Ok(ka) = KeepalivePacket::parse(data) {
        let wire = ka.encode();
        let reparsed = KeepalivePacket::parse(&wire)
            .expect("закодированный KeepalivePacket обязан разбираться обратно");
        assert_eq!(ka, reparsed);
    }

    // 3. Единый диспетчер служебных UDP-датаграмм + строгий декодер ответа сессии
    let _ = VoiceUdpPacket::parse(data);
    let _ = decode_discovery_response(data);

    // 4. Составной парсер RTCP / SRTCP
    let _ = parse_compound(data);
});
