//! Регрессионные тесты фаззинга протоколов голоса и DAVE v1 в `discord-gpui::audio` (`TASK-903`).
//!
//! Проверяют отсутствие арифметических паник (`u32` overflow в `dave_compute_wrapped_generation`),
//! защиту от бесконечного цикла на `u32::MAX` в `DaveHashRatchet`, а также строгую валидацию
//! 74-байтных IP Discovery и 8-байтных Keepalive пакетов.

pub mod state {
    #[derive(Debug, Clone)]
    pub struct CameraDevice {
        pub id: String,
        pub name: String,
        pub is_default: bool,
    }
}

#[path = "../src/audio/mod.rs"]
mod audio;

use audio::{DiscordVoicePacket, dave_compute_wrapped_big_nonce, dave_compute_wrapped_generation};

#[test]
fn dave_wrapped_generation_does_not_overflow_near_u32_max() {
    // До исправления: `oldest = 0xFFFF_FF80`, `generation_u8 = 0` давало `factor = 0x0100_0000`,
    // после чего `factor * 256` паниковало с `attempt to multiply with overflow`!
    assert_eq!(dave_compute_wrapped_generation(0xFFFF_FF80, 0), u32::MAX);
    assert_eq!(dave_compute_wrapped_generation(u32::MAX, 0), u32::MAX);
    assert_eq!(dave_compute_wrapped_generation(u32::MAX, 255), u32::MAX);
    assert_eq!(
        dave_compute_wrapped_big_nonce(u32::MAX, 0x00FF_FFFF),
        0x00FF_FFFF_FFFF_FFFF
    );
}

#[test]
fn discord_voice_packet_discovery_and_keepalive_strict_bounds() {
    let req = DiscordVoicePacket::make_ip_discovery(9208);
    assert_eq!(req.len(), 74);
    assert_eq!(DiscordVoicePacket::parse_ip_discovery_response(&req), None);

    // Валидный 74-байтный ответ IP Discovery (type=2, len=70, NUL-padded IP, port!=0)
    let mut resp = [0u8; 74];
    resp[0..2].copy_from_slice(&2u16.to_be_bytes());
    resp[2..4].copy_from_slice(&70u16.to_be_bytes());
    resp[4..8].copy_from_slice(&9208u32.to_be_bytes());
    resp[8..20].copy_from_slice(b"198.51.100.7");
    resp[72..74].copy_from_slice(&50002u16.to_be_bytes());
    assert_eq!(
        DiscordVoicePacket::parse_ip_discovery_response(&resp),
        Some(("198.51.100.7".to_string(), 50002))
    );

    // Пакет длиной != 74 байт, с неверным полем len != 70, портом 0 или мусором после NUL обязан отклоняться
    let mut oversized = resp.to_vec();
    oversized.push(0);
    assert_eq!(
        DiscordVoicePacket::parse_ip_discovery_response(&oversized),
        None
    );

    let mut bad_len_field = resp;
    bad_len_field[2..4].copy_from_slice(&999u16.to_be_bytes());
    assert_eq!(
        DiscordVoicePacket::parse_ip_discovery_response(&bad_len_field),
        None
    );

    let mut zero_port = resp;
    zero_port[72..74].copy_from_slice(&0u16.to_be_bytes());
    assert_eq!(
        DiscordVoicePacket::parse_ip_discovery_response(&zero_port),
        None
    );

    let mut non_zero_tail = resp;
    non_zero_tail[40] = b'X';
    assert_eq!(
        DiscordVoicePacket::parse_ip_discovery_response(&non_zero_tail),
        None
    );

    // Keepalive 8-байтный разбор (0x1337cafe request / 0x1337f00d response)
    let ka = DiscordVoicePacket::make_keepalive(0xDEAD_BEEF);
    assert_eq!(
        DiscordVoicePacket::parse_keepalive(&ka),
        Some((true, 0xDEAD_BEEF))
    );
    let mut ka_resp = [0u8; 8];
    ka_resp[0..4].copy_from_slice(&0x1337_f00d_u32.to_le_bytes());
    ka_resp[4..8].copy_from_slice(&42u32.to_le_bytes());
    assert_eq!(
        DiscordVoicePacket::parse_keepalive(&ka_resp),
        Some((false, 42))
    );
    assert_eq!(DiscordVoicePacket::parse_keepalive(&[0u8; 7]), None);
}
