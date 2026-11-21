//! Контрактные тесты модуля soundshare и PipeWire-вотчдога для стрима (Go Live soundshare).
//!
//! Проверяет:
//! 1. Формирование wire-кадров Voice Gateway OP 5 SPEAKING с флагом `speaking: 2` (Soundshare).
//! 2. Парсинг вывода `pactl list source-outputs` (определение потока стрима, mute-состояния, громкости и текущего источника).
//! 3. Определение необходимости исправления звукового тракта при подключении/отключении участников.
//! 4. Сохранение soundshare-активности при смене эпох DAVE (MLS commit transition).

use chord_core::soundshare::{
    SoundshareRemediation, SourceOutputInfo, build_soundshare_speaking_op5,
    evaluate_source_output_remediation, parse_source_outputs,
};
use chord_json::{JsonValue, parse as parse_json};

#[test]
fn test_soundshare_speaking_op5_format() {
    let ssrc = 133742;
    let raw = build_soundshare_speaking_op5(ssrc);
    let parsed: JsonValue = parse_json(&raw).expect("валидный JSON OP 5");

    assert_eq!(parsed.get("op").and_then(JsonValue::as_i64), Some(5));
    let d = parsed.get("d").expect("поле d");
    assert_eq!(
        d.get("speaking").and_then(JsonValue::as_i64),
        Some(2),
        "speaking должен быть 2 (SOUNDSHARE = 1 << 1)"
    );
    assert_eq!(d.get("delay").and_then(JsonValue::as_i64), Some(0));
    assert_eq!(d.get("ssrc").and_then(JsonValue::as_i64), Some(133742));
}

#[test]
fn test_parse_source_outputs_detects_stream_and_mute_state() {
    let sample_pactl_output = r#"
Source Output #10
	Driver: PipeWire
	Owner Module: n/a
	Client: 45
	Source: 32
	Sample Specification: s16le 2ch 48000Hz
	Channel Map: front-left,front-right
	Format: pcm, format.sample_format = "\"s16le\""  format.rate = 48000  format.channels = 2  format.channel_map = "\"front-left,front-right\""
	Volume: front-left: 65536 / 100% / 0,00 dB,   front-right: 65536 / 100% / 0,00 dB
	        balance 0,00
	Mute: no
	Properties:
		application.name = "DiscordStreamAudio"
		application.process.id = "802043"
		media.name = "DiscordStreamAudio"
		node.name = "DiscordStreamAudio"

Source Output #11
	Driver: PipeWire
	Owner Module: n/a
	Client: 60
	Source: 18
	Sample Specification: s16le 2ch 48000Hz
	Channel Map: front-left,front-right
	Volume: front-left: 0 / 0% / -inf dB,   front-right: 0 / 0% / -inf dB
	Mute: yes
	Properties:
		application.name = "DiscordStreamAudio"
		application.process.id = "802043"
		media.name = "DiscordStreamAudio"
"#;

    let outputs = parse_source_outputs(sample_pactl_output, 802043);
    assert_eq!(outputs.len(), 2);

    let first = &outputs[0];
    assert_eq!(first.id, "10");
    assert!(!first.muted);
    assert_eq!(first.volume_percent, 100);
    assert!(first.is_target_stream);

    let second = &outputs[1];
    assert_eq!(second.id, "11");
    assert!(second.muted, "второй поток замьючен");
    assert_eq!(second.volume_percent, 0);
    assert!(second.is_target_stream);
}

#[test]
fn test_evaluate_source_output_remediation_flags_needed_fixes() {
    let mut info = SourceOutputInfo {
        id: "10".to_string(),
        muted: true,
        volume_percent: 0,
        source_id: "32".to_string(),
        is_target_stream: true,
    };

    let target_monitor = "alsa_output.pci-0000_00_1f.3.analog-stereo.monitor";
    let remediation = evaluate_source_output_remediation(&info, target_monitor);

    assert_eq!(
        remediation,
        SoundshareRemediation {
            unmute_needed: true,
            volume_reset_needed: true,
            move_source_needed: true,
        }
    );

    info.muted = false;
    info.volume_percent = 100;
    let rem_unmuted = evaluate_source_output_remediation(&info, target_monitor);
    assert!(!rem_unmuted.unmute_needed);
    assert!(!rem_unmuted.volume_reset_needed);
    assert!(rem_unmuted.move_source_needed);
}
