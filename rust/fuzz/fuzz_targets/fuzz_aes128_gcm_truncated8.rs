#![no_main]
//! Фаззинг `Aes128GcmTruncated8` (DAVE v1 media AEAD) и верификации повреждённых
//! сообщений MLS `Proposal` (op:27) и `Commit` / `Welcome` (op:29/30) без утечек памяти.

use discord_voice::DaveMlsEngine;
use discord_voice::crypto::CryptoError;
use discord_voice::crypto::gcm::Aes128GcmTruncated8;
use discord_voice::dave_v1::{
    DaveFrameDecryptor, DaveFrameEncryptor, DaveHashRatchet, DaveMlsGroupContext,
    DaveMlsKeySchedule, DaveVoiceSession,
};
use discord_voice::srtp::EncryptionMode;
use libfuzzer_sys::fuzz_target;
use std::cell::RefCell;

struct StatefulDaveHarness {
    enc: DaveFrameEncryptor,
    pair_dec: DaveFrameDecryptor,
    wild_dec: DaveFrameDecryptor,
    session: DaveVoiceSession,
    engine: DaveMlsEngine,
    steps: u32,
}

impl StatefulDaveHarness {
    fn new() -> Self {
        let ctx = DaveMlsGroupContext {
            group_id: b"fuzz-voice-chan".to_vec(),
            epoch: 1,
            tree_hash: [0x11; 32],
            confirmed_transcript_hash: [0x22; 32],
            extensions: Vec::new(),
        };
        let ks = DaveMlsKeySchedule::from_initial_epoch([0x33; 32], &[0x44; 32], &ctx, None)
            .expect("шаблон расписания ключей DAVE");
        let session = DaveVoiceSession::new(1001, ks, &[1001, 2002], 1)
            .expect("шаблон сессии DaveVoiceSession");
        let (mut engine, _) =
            DaveMlsEngine::init_on_op4(1001, 777, 1, &[0x55; 32], EncryptionMode::Aes256GcmRtpSize)
                .expect("шаблон DaveMlsEngine");
        let _ = engine.on_clients_connect_op11(&[1001, 2002]);
        let _ = engine.on_external_sender_op25(&[0, 1, 25, 0xAA, 0xBB, 0xCC, 0xDD]);

        Self {
            enc: DaveFrameEncryptor::new(DaveHashRatchet::new(vec![0xBB; 16]).unwrap()),
            pair_dec: DaveFrameDecryptor::new(DaveHashRatchet::new(vec![0xBB; 16]).unwrap()),
            wild_dec: DaveFrameDecryptor::new(DaveHashRatchet::new(vec![0xAA; 16]).unwrap()),
            session,
            engine,
            steps: 0,
        }
    }
}

std::thread_local! {
    static HARNESS: RefCell<StatefulDaveHarness> = RefCell::new(StatefulDaveHarness::new());
}

fuzz_target!(|data: &[u8]| {
    if data.is_empty() || data.len() > 512 {
        return;
    }

    let lane = data[0] & 0x03;
    let payload = &data[1..];

    // Всегда проверяем граничные длины ключей AES-128
    let key_slice_len = usize::from(data[0] % 34).min(payload.len());
    let key_res = Aes128GcmTruncated8::new(&payload[..key_slice_len]);
    if key_slice_len != 16 {
        assert!(key_res.is_err());
    }

    match lane {
        // Ветка 0: произвольная/усечённая расшифровка + round-trip и контроль искажения 8-байтного тега
        0 => {
            if payload.len() >= 28 {
                let aead = Aes128GcmTruncated8::new(&payload[..16]).unwrap();
                let mut nonce = [0u8; 12];
                nonce.copy_from_slice(&payload[16..28]);
                let rest = &payload[28..];
                let aad_len = rest
                    .first()
                    .map_or(0, |&b| usize::from(b % 17).min(rest.len()));
                let (aad, body) = rest.split_at(aad_len);

                let _ = aead.decrypt(&nonce, aad, body);

                let rt_body = &body[..body.len().min(64)];
                let sealed = aead.encrypt(&nonce, aad, rt_body).unwrap();
                let opened = aead.decrypt(&nonce, aad, &sealed).unwrap();
                assert_eq!(opened, rt_body);

                let mut tampered = sealed;
                let last = tampered.len() - 1;
                tampered[last] ^= 0x01;
                assert_eq!(
                    aead.decrypt(&nonce, aad, &tampered),
                    Err(CryptoError::AuthenticationFailed)
                );
            }
        }
        // Ветка 1: in-place detached AEAD — гарантия отсутствия порчи буфера при невалидном теге
        1 => {
            if payload.len() >= 36 {
                let aead = Aes128GcmTruncated8::new(&payload[..16]).unwrap();
                let mut nonce = [0u8; 12];
                nonce.copy_from_slice(&payload[16..28]);
                let body = &payload[28..payload.len().min(124)];
                let split = body.len() - 8;
                let mut in_place = body[..split].to_vec();
                let orig = in_place.clone();
                let mut tag = [0u8; 8];
                tag.copy_from_slice(&body[split..]);
                if aead
                    .decrypt_in_place_detached(&nonce, &[], &mut in_place, &tag)
                    .is_err()
                {
                    assert_eq!(in_place, orig);
                }
            }
        }
        // Ветка 2: кадровый шифратор/дешифратор DAVE Opus (0xFAFA, LEB128 nonce, рэтчет)
        2 => {
            HARNESS.with(|cell| {
                let mut h = cell.borrow_mut();
                if h.steps >= 4096 {
                    *h = StatefulDaveHarness::new();
                }
                h.steps += 1;
                let _ = h.wild_dec.decrypt_opus(payload);
                if payload.len() <= 96 {
                    let wire = h.enc.encrypt_opus(payload).unwrap();
                    assert_eq!(h.pair_dec.decrypt_opus(&wire).unwrap(), payload);
                }
            });
        }
        // Ветка 3: повреждённые MLS Proposals (op:27) и Commits/Welcome (op:29/30)
        _ => {
            HARNESS.with(|cell| {
                let mut h = cell.borrow_mut();
                if h.steps >= 4096 {
                    *h = StatefulDaveHarness::new();
                }
                h.steps += 1;
                let epoch_before = h.session.current_epoch();
                let _ = h.session.process_mls_proposal(payload);
                if h.session.process_mls_commit(payload).is_err() {
                    assert_eq!(h.session.current_epoch(), epoch_before);
                }
                let _ = h.session.decrypt_opus(2002, payload);
                let _ = h.engine.on_proposals_op27(payload);
                let _ = h.engine.on_commit_or_welcome_op29_op30(payload);
            });
        }
    }
});
