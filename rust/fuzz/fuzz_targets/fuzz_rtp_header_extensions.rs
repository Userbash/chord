#![no_main]
//! Фаззинг RTP-заголовков, расширений RFC 5285 / RFC 8285 One-Byte (`0xBEDE`) и
//! Two-Byte (`0x1000..=0x100F`), распаковки RTX (RFC 4588) и сборщика `VideoFrameAssembler`.

use discord_voice::media::rtp::{
    RtpExtensionFormat, VideoFrameAssembler, encode_rtp_one_byte_extensions,
    encode_rtp_two_byte_extensions, parse_rtp, parse_rtp_extension_elements, restore_rtx_bytes,
};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if data.len() > 4096 {
        return;
    }

    // 1. Разбор RTP-пакета и расширений заголовка One-Byte / Two-Byte
    if let Ok(pkt) = parse_rtp(data) {
        if let Some(ext) = &pkt.extension
            && let Ok((format, elements)) = parse_rtp_extension_elements(ext)
        {
            let pairs: Vec<(u8, &[u8])> = elements.iter().map(|e| (e.id, e.data)).collect();
            match format {
                RtpExtensionFormat::OneByte => {
                    let encoded = encode_rtp_one_byte_extensions(&pairs)
                        .expect("разобранные элементы 0xBEDE обязаны кодироваться обратно");
                    assert_eq!(encoded.len() % 4, 0);
                }
                RtpExtensionFormat::TwoByte { .. } => {
                    let encoded = encode_rtp_two_byte_extensions(&pairs)
                        .expect("разобранные элементы Two-Byte обязаны кодироваться обратно");
                    assert_eq!(encoded.len() % 4, 0);
                }
            }
        }

        // Распаковка RTX-ретрансляции поверх валидного RTP-кадра
        let _ = restore_rtx_bytes(
            data,
            pkt.header.ssrc,
            pkt.header.payload_type,
            pkt.header.ssrc.wrapping_sub(1),
            105,
        );
    }

    // 2. Многопакетная сборка фрагментированных видеокадров H.264 (FU-A/STAP-A) и AV1 (OBU)
    let mut h264 = VideoFrameAssembler::new_h264();
    let mut av1 = VideoFrameAssembler::new_av1();
    for chunk in data.chunks(96).take(16) {
        if let Ok(pkt) = parse_rtp(chunk) {
            let _ = h264.push_packet(&pkt);
            let _ = av1.push_packet(&pkt);
        }
    }
});
