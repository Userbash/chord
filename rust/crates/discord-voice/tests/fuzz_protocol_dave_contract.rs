//! Контрактные тесты сетевых парсеров Discord Voice и криптографии DAVE MLS v1 (`TASK-903`).
//!
//! Оракулы:
//! - `/home/sanya/re-deep/voice-deep.md` §2.1–§2.3 (74B IP Discovery, 8B Keepalive `0x1337cafe`/`0x1337f00d`,
//!   RTP `0xBEDE`, DAVE v1 `0xFAFA`, тишина Opus `[0xF8, 0xFF, 0xFE]`);
//! - RFC 5285 / RFC 8285 §4.2–§4.3 (One-Byte `0xBEDE` и Two-Byte `0x1000..=0x100F` расширения RTP);
//! - NIST FIPS-197 App. C.1 (AES-128) и NIST SP 800-38D Test Case 4 (AES-128-GCM + 8B truncated tag);
//! - RFC 9420 §8–§9 (MLS 1.0 Key Schedule, `GroupContext`, `VLBytes`, Commit/Proposal верификация).

use discord_voice::crypto::CryptoError;
use discord_voice::crypto::aes::{Aes128, Aes128Key};
use discord_voice::crypto::gcm::Aes128GcmTruncated8;
use discord_voice::dave_v1::{
    AntiReplayWindow, DAVE_MARKER_BYTES, DAVE_OPUS_SILENCE_PACKET, DaveError, DaveFrameDecryptor,
    DaveFrameEncryptor, DaveHashRatchet, DaveMlsGroupContext, DaveMlsKeySchedule, DaveMlsProposal,
    DaveVoiceSession, ReplayError, build_mls_commit_frame, build_mls_proposal_append_frame,
    build_mls_proposal_revoke_frame, dave_compute_wrapped_big_nonce,
    dave_compute_wrapped_generation, mls_decode_vlbytes, mls_encode_vlbytes,
};
use discord_voice::media::rtp::{
    EXTENSION_PROFILE, RtpError, RtpExtensionElement, RtpExtensionError, RtpExtensionFormat,
    RtpHeader, TWO_BYTE_EXTENSION_PROFILE, encode_rtp_one_byte_extensions,
    encode_rtp_two_byte_extensions, parse_rtp, parse_rtp_extension_elements,
};
use discord_voice::udp::{
    DISCOVERY_LEN, DiscoveryPacket, KEEPALIVE_LEN, KeepalivePacket, UdpError, VoiceUdpPacket,
    keepalive_request, keepalive_response,
};

fn hex(s: &str) -> Vec<u8> {
    assert!(s.len().is_multiple_of(2));
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("валидный hex"))
        .collect()
}

const FIPS197_AES128_KEY: &str = "000102030405060708090a0b0c0d0e0f";
const FIPS197_AES128_PT: &str = "00112233445566778899aabbccddeeff";
const FIPS197_AES128_CT: &str = "69c4e0d86a7b0430d8cdb78070b4c55a";

const NIST_GCM_TC4_KEY: &str = "feffe9928665731c6d6a8f9467308308";
const NIST_GCM_TC4_IV: &str = "cafebabefacedbaddecaf888";
const NIST_GCM_TC4_AAD: &str = "feedfacedeadbeeffeedfacedeadbeefabaddad2";
const NIST_GCM_TC4_PT: &str = "d9313225f88406e5a55909c5aff5269a86a7a9531534f7da2e4c303d8a318a721\
                               c3c0c95956809532fcf0e2449a6b525b16aedf5aa0de657ba637b39";
const NIST_GCM_TC4_CT: &str = "42831ec2217774244b7221b784d0d49ce3aa212f2c02a4e035c17e2329aca12e2\
                               1d514b25466931c7d8f6a5aac84aa051ba30b396a0aac973d58e091";
const NIST_GCM_TC4_TAG8: &str = "5bc94fbc3221a5db";

#[test]
fn udp_keepalive_and_discovery_parse_roundtrip_and_boundaries() {
    for seq in [0u32, 1, 0x1234_5678, u32::MAX] {
        let req_wire = keepalive_request(seq);
        assert_eq!(req_wire.len(), KEEPALIVE_LEN);
        let parsed_req = KeepalivePacket::parse(&req_wire).expect("валидный keepalive request");
        assert_eq!(parsed_req, KeepalivePacket::Request { sequence: seq });
        assert_eq!(parsed_req.encode(), req_wire);
        assert_eq!(
            VoiceUdpPacket::parse(&req_wire).unwrap(),
            VoiceUdpPacket::Keepalive(KeepalivePacket::Request { sequence: seq })
        );

        let resp_wire = keepalive_response(seq);
        let parsed_resp = KeepalivePacket::parse(&resp_wire).expect("валидный keepalive response");
        assert_eq!(parsed_resp, KeepalivePacket::Response { sequence: seq });
        assert_eq!(parsed_resp.encode(), resp_wire);
    }

    // Ошибки длины и магии для Keepalive (пустой, усечённый, удлинённый, чужая магия)
    assert_eq!(KeepalivePacket::parse(&[]), Err(UdpError::InvalidLength));
    assert_eq!(
        KeepalivePacket::parse(&[0xfe; 7]),
        Err(UdpError::InvalidLength)
    );
    assert_eq!(
        KeepalivePacket::parse(&[0xfe; 9]),
        Err(UdpError::InvalidLength)
    );
    assert_eq!(
        KeepalivePacket::parse(&[0xde, 0xad, 0xbe, 0xef, 1, 0, 0, 0]),
        Err(UdpError::InvalidType)
    );

    // 74-байтный IP Discovery через общий диспетчер VoiceUdpPacket
    let disc = DiscoveryPacket::Response {
        ssrc: 9208,
        address: "203.0.113.42".to_string(),
        port: 50001,
    };
    let disc_wire = disc.encode().expect("кодирование 74B discovery");
    assert_eq!(disc_wire.len(), DISCOVERY_LEN);
    assert_eq!(
        VoiceUdpPacket::parse(&disc_wire).unwrap(),
        VoiceUdpPacket::Discovery(disc)
    );
    assert_eq!(
        VoiceUdpPacket::parse(&[0u8; 42]),
        Err(UdpError::InvalidLength)
    );
}

#[test]
fn rtp_header_extensions_one_byte_bede_and_two_byte_rfc8285() {
    // 1. One-Byte Header (0xBEDE): элементы id=1 (1 Б), padding 0x00, id=3 (3 Б), терминатор 0xF0
    let one_byte_raw =
        encode_rtp_one_byte_extensions(&[(1, &[0xAB][..]), (3, &[0x11, 0x22, 0x33][..])])
            .expect("кодирование RFC 5285 0xBEDE");
    assert_eq!(one_byte_raw.len() % 4, 0);

    let header = RtpHeader {
        marker: true,
        payload_type: 120,
        sequence: 65535,
        timestamp: 960,
        ssrc: 0xDEAD_BEEF,
    };
    let mut rtp_wire = header.encode(Some(&one_byte_raw)).unwrap();
    rtp_wire.extend_from_slice(&[0xF8, 0xFF, 0xFE]);

    let parsed = parse_rtp(&rtp_wire).expect("разбор RTP с 0xBEDE");
    let ext = parsed.extension.as_ref().expect("расширение присутствует");
    assert_eq!(ext.profile, EXTENSION_PROFILE);
    assert!(ext.standard);
    let (fmt, elems) = parse_rtp_extension_elements(ext).expect("разбор элементов 0xBEDE");
    assert_eq!(fmt, RtpExtensionFormat::OneByte);
    assert_eq!(
        elems,
        vec![
            RtpExtensionElement {
                id: 1,
                data: &[0xAB]
            },
            RtpExtensionElement {
                id: 3,
                data: &[0x11, 0x22, 0x33]
            },
        ]
    );

    // Терминатор id=15 (0xF0) прекращает разбор оставшихся байт без ошибки (RFC 5285 §4.2)
    let mut with_terminator = header
        .encode(Some(&[0x10, 0x55, 0xF0, 0x99, 0x21, 0xAA, 0xBB, 0x00]))
        .unwrap();
    with_terminator.extend_from_slice(b"opus");
    let parsed_term = parse_rtp(&with_terminator).unwrap();
    let (_, term_elems) =
        parse_rtp_extension_elements(parsed_term.extension.as_ref().unwrap()).unwrap();
    assert_eq!(term_elems.len(), 1);
    assert_eq!(term_elems[0].id, 1);
    assert_eq!(term_elems[0].data, &[0x55]);

    // Некорректный id=0 с ненулевой длиной (0x01) и усечённый элемент в 0xBEDE отклоняются
    let bad_zero_ext = header.encode(Some(&[0x01, 0x00, 0x00, 0x00])).unwrap();
    let p_bad_zero = parse_rtp(&bad_zero_ext).unwrap();
    assert_eq!(
        parse_rtp_extension_elements(p_bad_zero.extension.as_ref().unwrap()),
        Err(RtpExtensionError::ReservedElementId(0))
    );

    let trunc_one_byte = header.encode(Some(&[0x25, 0xAA, 0xBB, 0xCC])).unwrap(); // id=2, len=6 > 3 доступных
    let p_trunc = parse_rtp(&trunc_one_byte).unwrap();
    assert!(matches!(
        parse_rtp_extension_elements(p_trunc.extension.as_ref().unwrap()),
        Err(RtpExtensionError::TruncatedElement { id: 2, .. })
    ));

    // 2. Two-Byte Header (0x1000..=0x100F, RFC 8285 §4.3): поддержка id 1..=255 и длины 0..=255
    let two_byte_body = encode_rtp_two_byte_extensions(&[
        (1, &[][..]),
        (15, &[0xCA, 0xFE][..]),
        (255, &[1, 2, 3, 4, 5][..]),
    ])
    .expect("кодирование Two-Byte расширения");
    assert_eq!(two_byte_body.len() % 4, 0);

    let mut two_byte_rtp = Vec::new();
    two_byte_rtp.push(0x90); // V=2, X=1
    two_byte_rtp.push(120);
    two_byte_rtp.extend_from_slice(&1u16.to_be_bytes());
    two_byte_rtp.extend_from_slice(&960u32.to_be_bytes());
    two_byte_rtp.extend_from_slice(&42u32.to_be_bytes());
    let profile_with_appbits = TWO_BYTE_EXTENSION_PROFILE | 0x0005;
    two_byte_rtp.extend_from_slice(&profile_with_appbits.to_be_bytes());
    two_byte_rtp.extend_from_slice(&((two_byte_body.len() / 4) as u16).to_be_bytes());
    two_byte_rtp.extend_from_slice(&two_byte_body);
    two_byte_rtp.extend_from_slice(b"payload");

    let parsed_two = parse_rtp(&two_byte_rtp).expect("разбор RTP с Two-Byte профилем 0x1005");
    assert_eq!(parsed_two.payload, b"payload");
    let ext_two = parsed_two.extension.as_ref().unwrap();
    assert!(ext_two.standard);
    let (fmt_two, elems_two) = parse_rtp_extension_elements(ext_two).unwrap();
    assert_eq!(fmt_two, RtpExtensionFormat::TwoByte { appbits: 5 });
    assert_eq!(elems_two.len(), 3);
    assert_eq!(elems_two[0], RtpExtensionElement { id: 1, data: &[] });
    assert_eq!(
        elems_two[1],
        RtpExtensionElement {
            id: 15,
            data: &[0xCA, 0xFE]
        }
    );
    assert_eq!(
        elems_two[2],
        RtpExtensionElement {
            id: 255,
            data: &[1, 2, 3, 4, 5]
        }
    );

    // Усечённый пакет с профилем 0x1000 обязан возвращать RtpError::TruncatedExtension
    let mut truncated_two_rtp = two_byte_rtp[..16].to_vec();
    truncated_two_rtp[14..16].copy_from_slice(&10u16.to_be_bytes()); // 40 байт заявлено, 0 в наличии
    assert_eq!(
        parse_rtp(&truncated_two_rtp),
        Err(RtpError::TruncatedExtension)
    );
}

#[test]
fn aes128_and_aes128_gcm_truncated8_match_nist_kat_and_reject_truncation() {
    // 1. FIPS-197 App. C.1: AES-128 block encrypt/decrypt
    let key128 = Aes128Key::from_bytes(&hex(FIPS197_AES128_KEY)).unwrap();
    let aes = Aes128::new(&key128);
    let pt_block: [u8; 16] = hex(FIPS197_AES128_PT).try_into().unwrap();
    let ct_block = aes.encrypt_block(&pt_block);
    assert_eq!(ct_block.as_slice(), hex(FIPS197_AES128_CT).as_slice());
    assert_eq!(aes.decrypt_block(&ct_block), pt_block);

    // 2. NIST SP 800-38D Test Case 4: AES-128-GCM с 8-байтным усечённым тегом DAVE v1
    let aead = Aes128GcmTruncated8::new(&hex(NIST_GCM_TC4_KEY)).unwrap();
    let nonce: [u8; 12] = hex(NIST_GCM_TC4_IV).try_into().unwrap();
    let aad = hex(NIST_GCM_TC4_AAD);
    let pt = hex(NIST_GCM_TC4_PT);
    let expected_ct = hex(NIST_GCM_TC4_CT);
    let expected_tag8: [u8; 8] = hex(NIST_GCM_TC4_TAG8).try_into().unwrap();

    let mut in_place = pt.clone();
    let tag8 = aead
        .encrypt_in_place_detached(&nonce, &aad, &mut in_place)
        .unwrap();
    assert_eq!(in_place, expected_ct);
    assert_eq!(tag8, expected_tag8);

    // Проверка подделки тега НЕ должна портить входной буфер шифротекста
    let mut bad_tag = tag8;
    bad_tag[7] ^= 0x01;
    assert_eq!(
        aead.decrypt_in_place_detached(&nonce, &aad, &mut in_place, &bad_tag),
        Err(CryptoError::AuthenticationFailed)
    );
    assert_eq!(
        in_place, expected_ct,
        "при отказе аутентификации буфер не должен изменяться"
    );

    aead.decrypt_in_place_detached(&nonce, &aad, &mut in_place, &tag8)
        .unwrap();
    assert_eq!(in_place, pt);

    // Слитное API encrypt/decrypt на пустых, невыровненных и усечённых буферах (0..=7 байт)
    let empty_sealed = aead.encrypt(&nonce, &[], &[]).unwrap();
    assert_eq!(empty_sealed.len(), 8);
    assert_eq!(
        aead.decrypt(&nonce, &[], &empty_sealed).unwrap(),
        Vec::<u8>::new()
    );

    for short_len in 0..8 {
        assert_eq!(
            aead.decrypt(&nonce, &[], &vec![0xAA; short_len]),
            Err(CryptoError::InvalidTagLength {
                expected: 8,
                actual: short_len
            })
        );
    }

    // Ошибки длины ключа (0, 15, 17, 32 байта)
    for bad_key_len in [0usize, 15, 17, 32] {
        assert_eq!(
            Aes128GcmTruncated8::new(&vec![0x11; bad_key_len]).unwrap_err(),
            CryptoError::InvalidKeyLength {
                expected: 16,
                actual: bad_key_len
            }
        );
    }
}

#[test]
fn anti_replay_window_handles_u64_max_overflow_and_out_of_order() {
    let mut win = AntiReplayWindow::new();

    // Начало с 0, приём вне очереди, отклонение дубликатов
    assert_eq!(win.accept(0), Ok(()));
    assert_eq!(win.accept(0), Err(ReplayError::DuplicateNonce(0)));
    assert_eq!(win.accept(5), Ok(()));
    assert_eq!(win.accept(3), Ok(()));
    assert_eq!(win.accept(3), Err(ReplayError::DuplicateNonce(3)));

    // Огромный скачок на границу переполнения `u64::MAX - 2` -> `u64::MAX`
    assert_eq!(win.accept(u64::MAX - 2), Ok(()));
    assert_eq!(win.accept(u64::MAX), Ok(()));
    // Внутри скользящего окна [u64::MAX - 1023 ..= u64::MAX]
    assert_eq!(win.accept(u64::MAX - 1), Ok(()));
    assert_eq!(win.accept(u64::MAX - 1023), Ok(()));

    // Дубликаты на границе `u64::MAX` и `u64::MAX - 1023` строго отклоняются
    assert_eq!(
        win.accept(u64::MAX),
        Err(ReplayError::DuplicateNonce(u64::MAX))
    );
    assert_eq!(
        win.accept(u64::MAX - 1023),
        Err(ReplayError::DuplicateNonce(u64::MAX - 1023))
    );

    // Значения ниже окна (`u64::MAX - 1024` и `0`) строго отклоняются как устаревшие без underflow
    assert!(matches!(
        win.accept(u64::MAX - 1024),
        Err(ReplayError::ExpiredNonce { .. })
    ));
    assert!(matches!(
        win.accept(0),
        Err(ReplayError::ExpiredNonce { .. })
    ));

    // Проверка отсутствия арифметического переполнения в `dave_compute_wrapped_generation` и `DaveHashRatchet` около `u32::MAX`
    assert_eq!(dave_compute_wrapped_generation(0xFFFF_FF80, 0), u32::MAX);
    assert_eq!(dave_compute_wrapped_generation(u32::MAX, 0), u32::MAX);
    let big = dave_compute_wrapped_big_nonce(u32::MAX, 0x00FF_FFFF);
    assert_eq!(big, 0x00FF_FFFF_FFFF_FFFF);
}

#[test]
fn dave_voice_session_rejects_corrupted_mls_proposals_and_commits_without_panic() {
    let init_secret = [0x21u8; 32];
    let commit0 = [0x42u8; 32];
    let ctx1 = DaveMlsGroupContext {
        group_id: b"voice-chan-777".to_vec(),
        epoch: 1,
        tree_hash: [0x11; 32],
        confirmed_transcript_hash: [0x22; 32],
        extensions: Vec::new(),
    };
    let ctx1_bytes = ctx1.to_mls_bytes().unwrap();
    assert_eq!(
        DaveMlsGroupContext::from_mls_bytes(&ctx1_bytes).unwrap(),
        ctx1
    );

    // VLBytes roundtrip и отказ на неминимальном / обрезанном заголовке
    let mut vl = Vec::new();
    mls_encode_vlbytes(&[0xAB; 70], &mut vl).unwrap();
    let (decoded_vl, used_vl) = mls_decode_vlbytes(&vl).unwrap();
    assert_eq!(used_vl, vl.len());
    assert_eq!(decoded_vl, &[0xAB; 70]);
    assert!(mls_decode_vlbytes(&[0x40, 0x05, 1, 2, 3, 4, 5]).is_err()); // 2-byte кодировка для len=5 (< 64) запрещена RFC 9420
    assert!(mls_decode_vlbytes(&[0xC0, 0, 0, 1]).is_err()); // префикс 0b11 запрещён RFC 9420

    let ks_alice =
        DaveMlsKeySchedule::from_initial_epoch(init_secret, &commit0, &ctx1, None).unwrap();
    let ks_bob =
        DaveMlsKeySchedule::from_initial_epoch(init_secret, &commit0, &ctx1, None).unwrap();
    let mut alice = DaveVoiceSession::new(1001, ks_alice, &[1001, 2002], 1).unwrap();
    let mut bob = DaveVoiceSession::new(2002, ks_bob, &[1001, 2002], 1).unwrap();

    // Повреждённые MLS Proposals (пустые, обрезанные, неизвестный тип, user_id=0, чужой revoke)
    for bad_proposal in [
        &[][..],
        &[0x00, 0x01, 27][..],
        &[0x00, 0x01, 27, 0x00][..],
        &[0x00, 0x01, 27, 0x00, 99, 1, 2, 3][..], // неизвестный ProposalType=99
        &[0x00, 0x01, 27, 0x01, 0xFF, 0xFF][..],  // обрезанный Revoke
        &[0x00, 0x01, 27, 0x09, 0x00][..],        // неизвестный op_type=9
    ] {
        assert!(
            bob.process_mls_proposal(bad_proposal).is_err(),
            "повреждённый Proposal обязан отклоняться: {bad_proposal:?}"
        );
    }

    // Валидный Proposal Add для нового участника 3003 + отзыв несуществующего ref + валидный Commit
    let (valid_add_frame, prop_ref) = build_mls_proposal_append_frame(
        1,
        &DaveMlsProposal::Add {
            user_id: 3003,
            key_package: vec![0x55; 32],
        },
    )
    .unwrap();
    assert_ne!(prop_ref, [0u8; 16]);
    assert_eq!(alice.process_mls_proposal(&valid_add_frame).unwrap(), 1);
    assert_eq!(bob.process_mls_proposal(&valid_add_frame).unwrap(), 1);

    let bad_revoke = build_mls_proposal_revoke_frame(2, &[[0xEE; 16]]).unwrap();
    assert!(bob.process_mls_proposal(&bad_revoke).is_err());

    // Валидный Commit на эпоху 2 с подлинным confirmation_tag
    let commit_secret_e2 = [0x99u8; 32];
    let ctx2 = DaveMlsGroupContext {
        group_id: b"voice-chan-777".to_vec(),
        epoch: 2,
        tree_hash: [0x33; 32],
        confirmed_transcript_hash: [0x44; 32],
        extensions: Vec::new(),
    };
    let valid_commit =
        build_mls_commit_frame(3, 15, alice.key_schedule(), &commit_secret_e2, &ctx2, None)
            .unwrap();

    // Любое искажение байта в MLS Commit (усечение, порча GroupContext, подделка confirmation_tag) отклоняется!
    for cut in [0usize, 3, 6, 15, valid_commit.len() - 1] {
        assert!(bob.process_mls_commit(&valid_commit[..cut]).is_err());
    }
    let mut forged_commit = valid_commit.clone();
    let last = forged_commit.len() - 1;
    forged_commit[last] ^= 0x80;
    assert_eq!(
        bob.process_mls_commit(&forged_commit),
        Err(DaveError::ConfirmationTagMismatch)
    );
    assert_eq!(
        bob.current_epoch(),
        1,
        "невалидный Commit не должен менять эпоху"
    );

    // Приём подлинного Commit и переход на эпоху 2
    assert_eq!(alice.process_mls_commit(&valid_commit).unwrap(), 15);
    assert_eq!(bob.process_mls_commit(&valid_commit).unwrap(), 15);
    assert_eq!(alice.execute_transition(15).unwrap(), 2);
    assert_eq!(bob.execute_transition(15).unwrap(), 2);

    // Обмен зашифрованными Opus-кадрами и прозрачной тишиной в эпохе 2
    let mut enc = DaveFrameEncryptor::new(DaveHashRatchet::new(vec![0x77; 16]).unwrap());
    let mut dec = DaveFrameDecryptor::new(DaveHashRatchet::new(vec![0x77; 16]).unwrap());
    assert_eq!(
        enc.encrypt_opus(&DAVE_OPUS_SILENCE_PACKET).unwrap(),
        DAVE_OPUS_SILENCE_PACKET
    );
    assert_eq!(
        dec.decrypt_opus(&DAVE_OPUS_SILENCE_PACKET).unwrap(),
        DAVE_OPUS_SILENCE_PACKET
    );

    let encrypted = alice.encrypt_opus(b"opus-voice-e2").unwrap();
    assert_eq!(&encrypted[encrypted.len() - 2..], &DAVE_MARKER_BYTES);
    assert_eq!(
        bob.decrypt_opus(1001, &encrypted).unwrap(),
        b"opus-voice-e2"
    );
}
