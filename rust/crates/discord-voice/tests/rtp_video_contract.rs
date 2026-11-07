//! Контрактные тесты TASK-615: RTP H.264/AV1 Video Packetizer (`discord_voice::rtp::video`)
//! и сквозное двухслойное шифрование видеокадров DAVE MLS v1 (`discord_voice::dave`).

use discord_voice::dave::{
    DAVE_MARKER_BYTES, DAVE_PROTOCOL_VERSION_V1, DaveMlsEngine, StreamKey, has_dave_magic_marker,
};
use discord_voice::media::rtp::{
    ANNEX_B_START_CODE, VideoCodec, VideoFrameAssembler, av1_is_keyframe, h264_has_parameter_sets,
    h264_is_keyframe, parse_rtp, restore_rtx_bytes,
};
use discord_voice::rtp::video::{
    AV1_AGGR_N_NEW_CODED_VIDEO_SEQ, AV1_AGGR_Y_FRAGMENTED, AV1_AGGR_Z_CONTINUATION, AV1_OBU_FRAME,
    AV1_OBU_METADATA, AV1_OBU_SEQUENCE_HEADER, GW_OP_STREAM_CREATE, GW_OP_STREAM_DELETE,
    GW_OP_STREAM_PING, GW_OP_STREAM_SET_PAUSED, GW_OP_STREAM_WATCH, H264_FU_HEADER_END,
    H264_FU_HEADER_START, H264_NAL_FU_A, H264_NAL_IDR, H264_NAL_PPS, H264_NAL_SPS, H264_NAL_STAP_A,
    H264PacketizationKind, MAX_VIDEO_RTP_PAYLOAD_LEN, RTP_AV1_PAYLOAD_TYPE,
    RTP_AV1_RTX_PAYLOAD_TYPE, RTP_H264_PAYLOAD_TYPE, RTP_H264_RTX_PAYLOAD_TYPE, RtpVideoPacketizer,
    VIDEO_CLOCK_RATE, VIDEO_TICKS_30FPS, VIDEO_TICKS_60FPS, VideoRtpError,
    build_stream_create_call, build_stream_create_guild, build_stream_delete, build_stream_ping,
    build_stream_set_paused, build_stream_watch, encode_h264_fu_a, encode_h264_stap_a,
    packetize_av1_obus_with_mtu, packetize_h264_annex_b_with_mtu, parse_av1_obu_stream,
    parse_h264_annex_b,
};
use discord_voice::srtp::{EncryptionMode, SrtpError};

const SAMPLE_SRTP_SECRET_KEY: [u8; 32] = [
    0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28, 0x29, 0x2a, 0x2b, 0x2c, 0x2d, 0x2e, 0x2f, 0x30,
    0x31, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3a, 0x3b, 0x3c, 0x3d, 0x3e, 0x3f, 0x40,
];

fn make_annex_b(nalus: &[&[u8]]) -> Vec<u8> {
    let mut out = Vec::new();
    for nalu in nalus {
        out.extend_from_slice(&ANNEX_B_START_CODE);
        out.extend_from_slice(nalu);
    }
    out
}

fn setup_dave_mls_pair(mode: EncryptionMode) -> (DaveMlsEngine, DaveMlsEngine) {
    let (mut sender, kp_sender) = DaveMlsEngine::init_on_op4(
        111_111,
        9209,
        DAVE_PROTOCOL_VERSION_V1,
        &SAMPLE_SRTP_SECRET_KEY,
        mode,
    )
    .expect("sender op4");
    let (mut receiver, kp_receiver) = DaveMlsEngine::init_on_op4(
        222_222,
        9209,
        DAVE_PROTOCOL_VERSION_V1,
        &SAMPLE_SRTP_SECRET_KEY,
        mode,
    )
    .expect("receiver op4");
    assert_eq!(kp_sender[0], 26);
    assert_eq!(kp_receiver[0], 26);

    sender
        .on_clients_connect_op11(&[111_111, 222_222])
        .expect("sender op11");
    receiver
        .on_clients_connect_op11(&[111_111, 222_222])
        .expect("receiver op11");

    let ext_sender_pkg = [0x00, 0x01, 25, 0xAA, 0xBB, 0xCC, 0xDD];
    sender
        .on_external_sender_op25(&ext_sender_pkg)
        .expect("sender op25");
    receiver
        .on_external_sender_op25(&ext_sender_pkg)
        .expect("receiver op25");

    let proposals = [0x00, 0x02, 27, 0x00, 0x10, 0x20, 0x30, 0x40];
    let commit_welcome = sender
        .on_proposals_op27(&proposals)
        .expect("sender op27")
        .expect("sender op28");
    assert_eq!(commit_welcome[0], 28);
    let commit_payload = &commit_welcome[1..];

    let transition_id: u16 = 9;
    let mut op29 = vec![0x00, 0x03, 29];
    op29.extend_from_slice(&transition_id.to_be_bytes());
    op29.extend_from_slice(commit_payload);

    let mut op30 = vec![0x00, 0x03, 30];
    op30.extend_from_slice(&transition_id.to_be_bytes());
    op30.extend_from_slice(commit_payload);

    sender
        .on_commit_or_welcome_op29_op30(&op29)
        .expect("sender op29");
    receiver
        .on_commit_or_welcome_op29_op30(&op30)
        .expect("receiver op30");

    sender
        .on_execute_transition_op22(transition_id)
        .expect("sender op22");
    receiver
        .on_execute_transition_op22(transition_id)
        .expect("receiver op22");

    assert!(sender.is_ready());
    assert!(receiver.is_ready());
    (sender, receiver)
}

#[test]
fn h264_single_nalu_and_exact_1200_vs_1201_fu_a_boundary() {
    assert_eq!(VIDEO_CLOCK_RATE, 90_000);
    assert_eq!(VIDEO_TICKS_30FPS, 3_000);
    assert_eq!(VIDEO_TICKS_60FPS, 1_500);
    assert_eq!(MAX_VIDEO_RTP_PAYLOAD_LEN, 1200);
    assert_eq!(RTP_H264_PAYLOAD_TYPE, 105);
    assert_eq!(RTP_H264_RTX_PAYLOAD_TYPE, 106);

    // 1. Точная граница 1200 байт: укладывается в один Single NALU пакет без фрагментации FU-A.
    let mut nalu_1200 = vec![0x41]; // F=0, NRI=2, Type=1 (Non-IDR slice)
    nalu_1200.extend((0..1199).map(|i| (i & 0xFF) as u8));
    assert_eq!(nalu_1200.len(), 1200);

    let annex_b_1200 = make_annex_b(&[&nalu_1200]);
    let chunks_1200 = packetize_h264_annex_b_with_mtu(&annex_b_1200, MAX_VIDEO_RTP_PAYLOAD_LEN)
        .expect("1200-byte NALU must packetize");
    assert_eq!(chunks_1200.len(), 1);
    assert_eq!(chunks_1200[0].0, nalu_1200);
    assert!(chunks_1200[0].1, "single packet must carry marker=true");
    assert_eq!(chunks_1200[0].2, H264PacketizationKind::SingleNalu);

    // 2. Точная граница 1201 байт: обязана фрагментироваться по RFC 6184 FU-A (type=28) на 2 пакета.
    let mut nalu_1201 = vec![0x65]; // F=0, NRI=3 (0x60), Type=5 (IDR slice)
    nalu_1201.extend((0..1200).map(|i| ((i * 7 + 3) & 0xFF) as u8));
    assert_eq!(nalu_1201.len(), 1201);

    let annex_b_1201 = make_annex_b(&[&nalu_1201]);
    let chunks_1201 = packetize_h264_annex_b_with_mtu(&annex_b_1201, MAX_VIDEO_RTP_PAYLOAD_LEN)
        .expect("1201-byte NALU must fragment via FU-A");
    assert_eq!(chunks_1201.len(), 2);

    // Первый фрагмент: 2B FU-A заголовок + 1198B данных = 1200B, S=1, E=0, marker=false
    let (frag0, marker0, kind0) = &chunks_1201[0];
    assert_eq!(frag0.len(), 1200);
    assert!(!marker0);
    assert_eq!(
        *kind0,
        H264PacketizationKind::FuA {
            start: true,
            end: false
        }
    );
    assert_eq!(frag0[0], 0x60 | H264_NAL_FU_A); // 0x7C
    assert_eq!(frag0[1], H264_FU_HEADER_START | H264_NAL_IDR); // 0x85
    assert_eq!(&frag0[2..], &nalu_1201[1..1199]);

    // Второй фрагмент: 2B FU-A заголовок + оставшиеся 2B данных = 4B, S=0, E=1, marker=true
    let (frag1, marker1, kind1) = &chunks_1201[1];
    assert_eq!(frag1.len(), 4);
    assert!(marker1);
    assert_eq!(
        *kind1,
        H264PacketizationKind::FuA {
            start: false,
            end: true
        }
    );
    assert_eq!(frag1[0], 0x60 | H264_NAL_FU_A); // 0x7C
    assert_eq!(frag1[1], H264_FU_HEADER_END | H264_NAL_IDR); // 0x45
    assert_eq!(&frag1[2..], &nalu_1201[1199..1201]);
}

#[test]
fn h264_stap_a_and_multi_fragment_fu_a_roundtrip_with_video_frame_assembler() {
    // Собираем ключевой кадр H.264: SPS (NAL 7) + PPS (NAL 8) + большой IDR-срез (NAL 5, 3100 байт).
    let sps = vec![
        0x60 | H264_NAL_SPS,
        0x64,
        0x00,
        0x28,
        0xAC,
        0xD9,
        0x40,
        0x78,
        0x02,
        0x27,
        0xE5,
        0x84,
    ];
    let pps = vec![0x60 | H264_NAL_PPS, 0xEB, 0xE3, 0xCB, 0x22, 0xC0];
    let mut idr = vec![0x60 | H264_NAL_IDR];
    idr.extend((0..3099).map(|i| ((i * 13 + 5) & 0xFF) as u8));

    let annex_b_frame = make_annex_b(&[&sps, &pps, &idr]);
    assert!(h264_has_parameter_sets(&annex_b_frame));
    assert!(h264_is_keyframe(&annex_b_frame));

    let mut packetizer =
        RtpVideoPacketizer::new_h264(0x9988_7766).with_initial_state(65534, 90_000);
    assert_eq!(packetizer.codec(), VideoCodec::H264);
    let bede_ext = [0x10, 0xFF, 0x00, 0x00];
    let packets = packetizer
        .packetize_h264_frame(&annex_b_frame, VIDEO_TICKS_30FPS, Some(&bede_ext))
        .expect("packetize H.264 keyframe");

    // 1 пакет STAP-A (SPS + PPS) + 3 пакета FU-A (1198 + 1198 + 703 = 3099 байт тела IDR) = 4 RTP-пакета.
    assert_eq!(packets.len(), 4);

    // Пакет 0: STAP-A (type=24), seq=65534, marker=false
    assert_eq!(packets[0].header.sequence, 65534);
    assert_eq!(packets[0].header.timestamp, 90_000);
    assert_eq!(packets[0].header.ssrc, 0x9988_7766);
    assert_eq!(packets[0].header.payload_type, RTP_H264_PAYLOAD_TYPE);
    assert!(!packets[0].header.marker);
    assert_eq!(packets[0].payload[0] & 0x1F, H264_NAL_STAP_A);
    let expected_stap = encode_h264_stap_a(&[&sps, &pps], MAX_VIDEO_RTP_PAYLOAD_LEN).unwrap();
    assert_eq!(packets[0].payload, expected_stap);

    // Пакет 1: FU-A Start (seq=65535, marker=false)
    assert_eq!(packets[1].header.sequence, 65535);
    assert!(!packets[1].header.marker);
    assert_eq!(packets[1].payload[0], 0x60 | H264_NAL_FU_A);
    assert_eq!(packets[1].payload[1], H264_FU_HEADER_START | H264_NAL_IDR);

    // Пакет 2: FU-A Middle (seq=0 после переполнения u16::MAX, marker=false)
    assert_eq!(packets[2].header.sequence, 0);
    assert!(!packets[2].header.marker);
    assert_eq!(packets[2].payload[0], 0x60 | H264_NAL_FU_A);
    assert_eq!(packets[2].payload[1], H264_NAL_IDR);

    // Пакет 3: FU-A End (seq=1, marker=true)
    assert_eq!(packets[3].header.sequence, 1);
    assert!(packets[3].header.marker);
    assert_eq!(packets[3].payload[0], 0x60 | H264_NAL_FU_A);
    assert_eq!(packets[3].payload[1], H264_FU_HEADER_END | H264_NAL_IDR);

    // Следующий sequence = 2, следующий timestamp = 93_000
    assert_eq!(packetizer.sequence(), 2);
    assert_eq!(packetizer.timestamp(), 93_000);

    // Полный roundtrip через VideoFrameAssembler::new_h264()
    let mut assembler = VideoFrameAssembler::new_h264();
    for (idx, pkt) in packets.iter().enumerate() {
        let parsed = parse_rtp(&pkt.raw).expect("valid RTP wire packet");
        assert_eq!(parsed.extension.as_ref().unwrap().profile, 0xBEDE);
        assert_eq!(parsed.extension.as_ref().unwrap().data, &bede_ext);
        let res = assembler.push_packet(&parsed);
        if idx + 1 < packets.len() {
            assert!(
                res.is_none(),
                "intermediate packet {idx} must not emit frame"
            );
        } else {
            let assembled = res.expect("final packet with marker=true must emit frame");
            assert_eq!(assembled, annex_b_frame);
            assert!(h264_has_parameter_sets(&assembled));
            assert!(h264_is_keyframe(&assembled));
        }
    }
    assert!(!assembler.needs_pli());
}

#[test]
fn h264_rtx_packet_generation_and_restoration() {
    let mut packetizer = RtpVideoPacketizer::new_h264(0x1111_2222)
        .with_rtx(0x1111_2223, RTP_H264_RTX_PAYLOAD_TYPE)
        .with_initial_state(400, 180_000);
    let p_slice = make_annex_b(&[&[0x41, 0xAA, 0xBB, 0xCC, 0xDD]]);
    let ext = [0x10, 0x80, 0x00, 0x00];
    let packets = packetizer
        .packetize_h264_frame(&p_slice, VIDEO_TICKS_60FPS, Some(&ext))
        .expect("packetize P-slice");
    assert_eq!(packets.len(), 1);
    assert_eq!(packetizer.timestamp(), 181_500);

    let rtx_pkt = packetizer
        .build_rtx_packet(&packets[0])
        .expect("build RTX packet");
    assert_eq!(rtx_pkt.header.ssrc, 0x1111_2223);
    assert_eq!(rtx_pkt.header.payload_type, RTP_H264_RTX_PAYLOAD_TYPE);
    assert_eq!(rtx_pkt.header.sequence, 0);
    assert_eq!(packetizer.rtx_sequence(), 1);
    assert_eq!(&rtx_pkt.payload[0..2], &400u16.to_be_bytes());
    assert_eq!(&rtx_pkt.payload[2..], &packets[0].payload[..]);

    let restored_raw = restore_rtx_bytes(
        &rtx_pkt.raw,
        0x1111_2223,
        RTP_H264_RTX_PAYLOAD_TYPE,
        0x1111_2222,
        RTP_H264_PAYLOAD_TYPE,
    )
    .expect("restore RTX bytes");
    assert_eq!(restored_raw, packets[0].raw);
}

#[test]
fn av1_single_multi_w2_w3_w0_and_fragmented_obu_roundtrip() {
    assert_eq!(RTP_AV1_PAYLOAD_TYPE, 101);
    assert_eq!(RTP_AV1_RTX_PAYLOAD_TYPE, 102);

    let mut packetizer = RtpVideoPacketizer::new_av1(0x5566_7788).with_initial_state(10, 3_000);
    let mut assembler = VideoFrameAssembler::new_av1();

    // 1. Агрегация 2 OBU (W=2, N=1 на ключевом Sequence Header):
    // OBU_SEQUENCE_HEADER (type=1 -> hdr=0x0A, has_size_field=1, size=4) + OBU_FRAME (type=6 -> hdr=0x32, size=6)
    let obu_seq = [
        (AV1_OBU_SEQUENCE_HEADER << 3) | 0x02,
        0x04,
        0x01,
        0x02,
        0x03,
        0x04,
    ];
    let obu_small_frame = [0x32, 0x06, 0x10, 0x20, 0x30, 0x40, 0x50, 0x60];
    let mut bitstream_kf = Vec::new();
    bitstream_kf.extend_from_slice(&obu_seq);
    bitstream_kf.extend_from_slice(&obu_small_frame);

    let parsed_obus = parse_av1_obu_stream(&bitstream_kf).expect("parse self-delimiting AV1 OBUs");
    assert_eq!(parsed_obus, vec![&obu_seq[..], &obu_small_frame[..]]);

    let kf_packets = packetizer
        .packetize_av1_bitstream(&bitstream_kf, VIDEO_TICKS_30FPS, None)
        .expect("packetize AV1 keyframe");
    assert_eq!(kf_packets.len(), 1);
    assert!(kf_packets[0].header.marker);
    assert_eq!(kf_packets[0].header.payload_type, RTP_AV1_PAYLOAD_TYPE);
    // W=2 (0x20) | N=1 (0x08) = 0x28
    assert_eq!(
        kf_packets[0].payload[0],
        (2 << 4) | AV1_AGGR_N_NEW_CODED_VIDEO_SEQ
    );

    let parsed_rtp = parse_rtp(&kf_packets[0].raw).unwrap();
    let assembled_kf = assembler
        .push_packet(&parsed_rtp)
        .expect("AV1 W=2 keyframe assembled");
    assert_eq!(assembled_kf, bitstream_kf);
    assert!(av1_is_keyframe(&assembled_kf));
    assert!(!assembler.needs_pli());

    // 2. Агрегация 4 маленьких OBU (W=0: каждый элемент снабжён LEB128 длиной):
    let obu_meta1 = [(AV1_OBU_METADATA << 3) | 0x02, 0x02, 0xAA, 0xBB];
    let obu_meta2 = [(AV1_OBU_METADATA << 3) | 0x02, 0x02, 0xCC, 0xDD];
    let obu_meta3 = [(AV1_OBU_METADATA << 3) | 0x02, 0x01, 0xEE];
    let obu_f = [(AV1_OBU_FRAME << 3) | 0x02, 0x03, 0x01, 0x02, 0x03];
    let four_obus: [&[u8]; 4] = [&obu_meta1, &obu_meta2, &obu_meta3, &obu_f];
    let w0_packets = packetizer
        .packetize_av1_obus(&four_obus, VIDEO_TICKS_30FPS, None)
        .expect("packetize 4 OBUs as W=0");
    assert_eq!(w0_packets.len(), 1);
    assert_eq!(w0_packets[0].payload[0] & 0x30, 0x00, "4 OBUs must use W=0");
    let assembled_w0 = assembler
        .push_packet(&parse_rtp(&w0_packets[0].raw).unwrap())
        .expect("W=0 assembled");
    let expected_w0: Vec<u8> = four_obus.iter().flat_map(|s| s.iter().copied()).collect();
    assert_eq!(assembled_w0, expected_w0);

    // 3. Большой OBU (> 1200 байт, например 2600 байт) -> фрагментация на 3 пакета (1199 + 1199 + 202 байт) с флагами Z/Y:
    let mut large_obu = vec![(AV1_OBU_FRAME << 3)]; // has_size_field=0
    large_obu.extend((0..2599).map(|i| ((i * 11 + 9) & 0xFF) as u8));
    assert_eq!(large_obu.len(), 2600);

    let frag_packets = packetizer
        .packetize_av1_obus(&[&large_obu], VIDEO_TICKS_30FPS, None)
        .expect("fragment large AV1 OBU");
    assert_eq!(frag_packets.len(), 3);

    // Пакет 0: Z=0, Y=1, W=1 -> 0x50, длина = 1200, marker=false
    assert_eq!(frag_packets[0].payload.len(), 1200);
    assert_eq!(frag_packets[0].payload[0], AV1_AGGR_Y_FRAGMENTED | (1 << 4));
    assert!(!frag_packets[0].header.marker);

    // Пакет 1: Z=1, Y=1, W=1 -> 0xD0, длина = 1200, marker=false
    assert_eq!(frag_packets[1].payload.len(), 1200);
    assert_eq!(
        frag_packets[1].payload[0],
        AV1_AGGR_Z_CONTINUATION | AV1_AGGR_Y_FRAGMENTED | (1 << 4)
    );
    assert!(!frag_packets[1].header.marker);

    // Пакет 2: Z=1, Y=0, W=1 -> 0x90, длина = 1 + 202 = 203, marker=true
    assert_eq!(frag_packets[2].payload.len(), 203);
    assert_eq!(
        frag_packets[2].payload[0],
        AV1_AGGR_Z_CONTINUATION | (1 << 4)
    );
    assert!(frag_packets[2].header.marker);

    assert!(
        assembler
            .push_packet(&parse_rtp(&frag_packets[0].raw).unwrap())
            .is_none()
    );
    assert!(
        assembler
            .push_packet(&parse_rtp(&frag_packets[1].raw).unwrap())
            .is_none()
    );
    let assembled_large = assembler
        .push_packet(&parse_rtp(&frag_packets[2].raw).unwrap())
        .expect("final AV1 fragment completes frame");
    assert_eq!(assembled_large, large_obu);
}

#[test]
fn dave_mls_v1_two_layer_video_encryption_roundtrip_h264_and_av1() {
    for mode in [
        EncryptionMode::XChaCha20Poly1305RtpSize,
        EncryptionMode::Aes256GcmRtpSize,
    ] {
        let (mut sender_dave, mut receiver_dave) = setup_dave_mls_pair(mode);
        let mut h264_packetizer = RtpVideoPacketizer::new_h264(9209);
        let mut h264_assembler = VideoFrameAssembler::new_h264();

        // Формируем H.264 IDR ключевой кадр: SPS + PPS (STAP-A) + IDR 2500 байт (3 пакета FU-A)
        let sps = [0x67, 0x64, 0x00, 0x1F, 0xAC, 0xD9];
        let pps = [0x68, 0xEB, 0xE3, 0xCB];
        let mut idr = vec![0x65];
        idr.extend((0..2499).map(|i| ((i * 5 + 1) & 0xFF) as u8));
        let annex_b = make_annex_b(&[&sps, &pps, &idr]);
        let bede_ext = [0x10, 0x7F, 0x22, 0x33];

        let encrypted_packets = sender_dave
            .packetize_and_encrypt_h264(
                &mut h264_packetizer,
                &annex_b,
                VIDEO_TICKS_30FPS,
                Some(&bede_ext),
            )
            .expect("packetize and encrypt H.264 frame with DAVE MLS v1 + SRTP");
        assert_eq!(encrypted_packets.len(), 4);

        let mut recovered_frame = None;
        for (idx, wire_pkt) in encrypted_packets.iter().enumerate() {
            // Проверяем, что внешний RTP-заголовок и расширение 0xBEDE остались открытыми (AAD для _rtpsize)
            let outer_parsed = parse_rtp(wire_pkt).expect("outer SRTP keeps valid RTP header");
            assert_eq!(outer_parsed.header.ssrc, 9209);
            assert_eq!(outer_parsed.header.payload_type, RTP_H264_PAYLOAD_TYPE);
            assert_eq!(outer_parsed.extension.as_ref().unwrap().profile, 0xBEDE);

            // Расшифровываем двухслойный пакет и подаём в сборщик видеокадров
            let maybe_frame = receiver_dave
                .decrypt_and_assemble_video_packet(&mut h264_assembler, wire_pkt)
                .expect("decrypt DAVE MLS v1 + SRTP and push to assembler");
            if idx + 1 < encrypted_packets.len() {
                assert!(maybe_frame.is_none());
            } else {
                recovered_frame = maybe_frame;
            }
        }
        assert_eq!(recovered_frame.expect("complete H.264 frame"), annex_b);

        // Проверяем AV1 ключевой кадр через тот же двухслойный конвейер DAVE MLS v1
        let mut av1_packetizer = RtpVideoPacketizer::new_av1(9209);
        let mut av1_assembler = VideoFrameAssembler::new_av1();
        let obu_seq = [0x0A, 0x03, 0x01, 0x02, 0x03];
        let mut obu_frame = vec![0x30];
        obu_frame.extend((0..1500).map(|i| (i & 0xFF) as u8));
        let av1_obus: [&[u8]; 2] = [&obu_seq, &obu_frame];

        let encrypted_av1 = sender_dave
            .packetize_and_encrypt_av1(
                &mut av1_packetizer,
                &av1_obus,
                VIDEO_TICKS_60FPS,
                Some(&bede_ext),
            )
            .expect("packetize and encrypt AV1 OBUs with DAVE MLS v1 + SRTP");
        assert!(encrypted_av1.len() >= 2);

        // Проверим также низкоуровневый маркер 0xFAFA после снятия только внешнего SRTP-слоя
        let mut probe_srtp =
            discord_voice::srtp::SrtpContext::from_secret_key(&SAMPLE_SRTP_SECRET_KEY, mode);
        // Сдвигаем probe_srtp или просто используем decrypt_rtp_only (который не требует счётчика на приём)
        let inner_dave_rtp = probe_srtp
            .decrypt_rtp(&encrypted_av1[0])
            .expect("outer SRTP decrypts");
        let inner_parsed = parse_rtp(&inner_dave_rtp).unwrap();
        assert!(has_dave_magic_marker(inner_parsed.payload));
        assert_eq!(
            &inner_parsed.payload[inner_parsed.payload.len() - 2..],
            &DAVE_MARKER_BYTES
        );

        let mut recovered_av1 = None;
        for wire_pkt in &encrypted_av1 {
            if let Some(frame) = receiver_dave
                .decrypt_and_assemble_video_packet(&mut av1_assembler, wire_pkt)
                .expect("decrypt AV1 DAVE packet")
            {
                recovered_av1 = Some(frame);
            }
        }
        let mut expected_av1 = Vec::new();
        expected_av1.extend_from_slice(&obu_seq);
        expected_av1.extend_from_slice(&obu_frame);
        assert_eq!(recovered_av1.expect("complete AV1 frame"), expected_av1);

        // Защита от повтора (NonceReuse) и подделки (AuthenticationFailed)
        let replay_err = receiver_dave
            .decrypt_video_rtp_packet(&encrypted_av1[0])
            .expect_err("replayed DAVE video packet must be rejected");
        assert!(matches!(replay_err, SrtpError::NonceReuse { .. }));

        let fresh_encrypted = sender_dave
            .packetize_and_encrypt_av1(&mut av1_packetizer, &[&obu_seq], VIDEO_TICKS_60FPS, None)
            .expect("encrypt fresh AV1 packet");
        let mut tampered = fresh_encrypted[0].clone();
        let mid = tampered.len() / 2;
        tampered[mid] ^= 0x55;
        let tamper_err = receiver_dave
            .decrypt_video_rtp_packet(&tampered)
            .expect_err("tampered DAVE video packet must fail authentication");
        assert_eq!(tamper_err, SrtpError::AuthenticationFailed);
    }
}

#[test]
fn gateway_streaming_opcodes_18_to_22_and_stream_key_contract() {
    assert_eq!(GW_OP_STREAM_CREATE, 18);
    assert_eq!(GW_OP_STREAM_DELETE, 19);
    assert_eq!(GW_OP_STREAM_WATCH, 20);
    assert_eq!(GW_OP_STREAM_PING, 21);
    assert_eq!(GW_OP_STREAM_SET_PAUSED, 22);

    let key = StreamKey::Guild {
        guild_id: "1014537851418980372".to_owned(),
        channel_id: "1176785373989445703".to_owned(),
        owner_id: "384299970632876032".to_owned(),
    };
    let key_str = key.format();
    assert_eq!(
        key_str,
        "guild:1014537851418980372:1176785373989445703:384299970632876032"
    );

    let op18_guild = build_stream_create_guild(
        "1014537851418980372",
        "1176785373989445703",
        Some("rotterdam"),
    );
    assert!(op18_guild.contains(r#""op":18"#));
    assert!(op18_guild.contains(r#""type":"guild""#));
    assert!(op18_guild.contains(r#""preferred_region":"rotterdam""#));

    let op18_call = build_stream_create_call("1176785373989445703", None);
    assert!(op18_call.contains(r#""op":18"#));
    assert!(op18_call.contains(r#""type":"call""#));
    assert!(op18_call.contains(r#""preferred_region":null"#));

    let op19 = build_stream_delete(&key_str);
    assert!(op19.contains(r#""op":19"#));
    assert!(op19.contains(&key_str));

    let op20 = build_stream_watch(&key_str);
    assert!(op20.contains(r#""op":20"#));
    assert!(op20.contains(&key_str));

    let op21 = build_stream_ping(&key_str);
    assert!(op21.contains(r#""op":21"#));
    assert!(op21.contains(&key_str));

    let op22 = build_stream_set_paused(&key_str, true);
    assert!(op22.contains(r#""op":22"#));
    assert!(op22.contains(r#""paused":true"#));
}

#[test]
fn negative_and_boundary_cases_are_rejected_with_typed_errors() {
    // Пустой кадр H.264
    assert_eq!(parse_h264_annex_b(&[]), Err(VideoRtpError::EmptyFrame));
    assert_eq!(
        parse_h264_annex_b(&[0, 0, 0, 1]),
        Err(VideoRtpError::EmptyFrame)
    );

    // Запрещённый бит F=1 (0x80) в заголовке NALU
    assert_eq!(
        parse_h264_annex_b(&[0, 0, 0, 1, 0xE5, 0x11]),
        Err(VideoRtpError::ForbiddenZeroBitSet(0xE5))
    );

    // Недопустимый тип NALU (0 или >23 во входном Annex-B потоке)
    assert_eq!(
        parse_h264_annex_b(&[0, 0, 0, 1, 0x60, 0x11]),
        Err(VideoRtpError::InvalidNalType(0))
    );
    assert_eq!(
        parse_h264_annex_b(&[0, 0, 0, 1, 0x7C, 0x11]),
        Err(VideoRtpError::InvalidNalType(28))
    );

    // Переполнение STAP-A сверх MTU
    let big_a = vec![0x67; 700];
    let big_b = vec![0x68; 600];
    assert!(matches!(
        encode_h264_stap_a(&[&big_a, &big_b], 1200),
        Err(VideoRtpError::StapATooLarge { .. })
    ));

    // Слишком малый MTU для FU-A или пакетизатора
    assert_eq!(
        encode_h264_fu_a(&[0x65, 1, 2, 3], 2),
        Err(VideoRtpError::InvalidMtu(2))
    );

    // Пустой или битый AV1 OBU (forbidden bit = 1)
    assert_eq!(
        packetize_av1_obus_with_mtu(&[], 1200),
        Err(VideoRtpError::EmptyFrame)
    );
    assert!(matches!(
        packetize_av1_obus_with_mtu(&[&[0x8A, 0x00]], 1200),
        Err(VideoRtpError::InvalidAv1Obu(_))
    ));
    assert!(matches!(
        parse_av1_obu_stream(&[]),
        Err(VideoRtpError::EmptyFrame)
    ));
    assert!(matches!(
        parse_av1_obu_stream(&[0x04]), // has_extension=1, но 2-го байта заголовка нет
        Err(VideoRtpError::InvalidAv1Obu(_))
    ));
    assert!(matches!(
        parse_av1_obu_stream(&[0x0A, 0x10, 0x01]), // заявлена длина 16 байт, но в буфере 1 байт
        Err(VideoRtpError::InvalidAv1Obu(_))
    ));

    // Проверка границ has_dave_magic_marker
    assert!(!has_dave_magic_marker(&[0xFA, 0xFA]));
    let mut fake_dave = vec![0u8; 24];
    fake_dave[21] = 20; // valid supplemental_size >= 20 && <= 24
    fake_dave[22] = 0xFA;
    fake_dave[23] = 0xFA;
    assert!(has_dave_magic_marker(&fake_dave));
    fake_dave[21] = 19; // < 20 (DAVE_GCM_TAG_LEN + 4)
    assert!(!has_dave_magic_marker(&fake_dave));
    fake_dave[21] = 25; // > len (24)
    assert!(!has_dave_magic_marker(&fake_dave));
    fake_dave[21] = 20;
    fake_dave[23] = 0xFB; // битый маркер
    assert!(!has_dave_magic_marker(&fake_dave));

    // Отклонение шифрования DAVE MLS до завершения рукопожатия (Ready == false)
    let (mut unready_dave, _) = DaveMlsEngine::init_on_op4(
        111_111,
        9209,
        DAVE_PROTOCOL_VERSION_V1,
        &SAMPLE_SRTP_SECRET_KEY,
        EncryptionMode::XChaCha20Poly1305RtpSize,
    )
    .unwrap();
    assert!(!unready_dave.is_ready());
    let mut pktizer = RtpVideoPacketizer::new_h264(9209);
    let err = unready_dave
        .packetize_and_encrypt_h264(&mut pktizer, &[0, 0, 0, 1, 0x65, 0x01], 3000, None)
        .expect_err("unready DAVE engine must reject video encryption");
    assert!(matches!(
        err,
        VideoRtpError::DaveSrtp(SrtpError::MalformedHeader { .. })
    ));
    assert!(!err.to_string().is_empty());
}

#[test]
fn h264_three_byte_start_codes_stap_a_flush_and_av1_w3_multi_byte_leb128() {
    // 1. Трёхбайтные стартовые коды 00 00 01 и сброс STAP-A при переполнении MTU + 1199B SingleNalu
    let mut stream_3b = Vec::new();
    stream_3b.extend_from_slice(&[0, 0, 1, 0x67, 0x42, 0x00, 0x1E]);
    stream_3b.extend_from_slice(&[0, 0, 1, 0x68, 0xCE, 0x3C, 0x80]);
    let mut nalu_1199 = vec![0x65];
    nalu_1199.extend((0..1198).map(|i| (i & 0xFF) as u8));
    stream_3b.extend_from_slice(&[0, 0, 1]);
    stream_3b.extend_from_slice(&nalu_1199);

    let chunks = packetize_h264_annex_b_with_mtu(&stream_3b, 1200).expect("3-byte start codes");
    assert_eq!(chunks.len(), 2);
    assert_eq!(chunks[0].2, H264PacketizationKind::StapA);
    assert!(!chunks[0].1);
    assert_eq!(chunks[1].2, H264PacketizationKind::SingleNalu);
    assert!(chunks[1].1);
    assert_eq!(chunks[1].0.len(), 1199);

    // 2. AV1 W=3 с 2-байтной LEB128 длиной (>= 128 байт) и расширенным заголовком OBU (has_extension=1)
    let mut obu_ext_130 = vec![
        (AV1_OBU_SEQUENCE_HEADER << 3) | 0x04 | 0x02,
        0x21,
        0x80,
        0x01,
    ];
    obu_ext_130.extend((0..128).map(|i| (i & 0x7F) as u8));
    assert_eq!(obu_ext_130.len(), 132);
    let obu_2 = [(AV1_OBU_METADATA << 3) | 0x02, 0x02, 0x11, 0x22];
    let obu_3 = [(AV1_OBU_FRAME << 3) | 0x02, 0x02, 0x33, 0x44];

    let mut av1_stream = Vec::new();
    av1_stream.extend_from_slice(&obu_ext_130);
    av1_stream.extend_from_slice(&obu_2);
    av1_stream.extend_from_slice(&obu_3);

    let parsed = parse_av1_obu_stream(&av1_stream).expect("parse AV1 with extension and 2B LEB128");
    assert_eq!(parsed.len(), 3);

    let pktizer = RtpVideoPacketizer::new_av1(1234)
        .with_max_payload_len(200)
        .expect("valid custom MTU 200");
    assert_eq!(pktizer.ssrc(), 1234);
    assert_eq!(pktizer.rtx_ssrc(), 1235);
    assert_eq!(pktizer.payload_type(), RTP_AV1_PAYLOAD_TYPE);
    assert_eq!(pktizer.rtx_payload_type(), RTP_AV1_RTX_PAYLOAD_TYPE);
    assert_eq!(pktizer.max_payload_len(), 200);
    assert_eq!(
        RtpVideoPacketizer::new_av1(1234).with_max_payload_len(8),
        Err(VideoRtpError::InvalidMtu(8))
    );

    let w3_chunks = packetize_av1_obus_with_mtu(&parsed, 200).expect("W=3 packetization");
    assert_eq!(w3_chunks.len(), 1);
    assert_eq!(w3_chunks[0].0[0], (3 << 4) | AV1_AGGR_N_NEW_CODED_VIDEO_SEQ);
    assert!(w3_chunks[0].1);

    let mut assembler = VideoFrameAssembler::new_av1();
    let reassembled = assembler
        .push(1, 3000, w3_chunks[0].1, &w3_chunks[0].0)
        .expect("W=3 reassembled");
    assert_eq!(reassembled, av1_stream);
}
