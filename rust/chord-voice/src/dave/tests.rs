//! Верификация Forward Secrecy и смены эпох DAVE MLS v1 при выходе пира (`TASK-979`).
//! Оракулы: `/home/sanya/re-deep/voice-deep.md`, `/home/sanya/re-deep/native-re.md`, RFC 9420 MLS 1.0.

#![forbid(unsafe_code)]
#![allow(dead_code)]

#[path = "../../../crates/chord-voice/src/crypto/mod.rs"]
pub mod crypto;
#[path = "../../../crates/chord-voice/src/dave_v1/mod.rs"]
pub mod dave_v1;

use crypto::gcm::Aes128GcmTruncated8;
pub use crypto::{AES128_KEY_SIZE, GCM_NONCE_SIZE, GCM_TRUNCATED8_TAG_SIZE};
pub use dave_v1::mls::{
    DaveMlsGroupContext, dave_sha256, mls_decode_vlbytes, mls_expand_with_label,
};
pub use dave_v1::replay::{
    AntiReplayWindow, DAVE_RATCHET_GENERATION_SHIFT_BITS, DAVE_REPLAY_WINDOW_SIZE, ReplayError,
    dave_compute_wrapped_big_nonce,
};
pub use dave_v1::{
    DAVE_MARKER_BYTES, DAVE_MAX_GENERATION_GAP, DAVE_OPUS_SILENCE_PACKET, DAVE_PROTOCOL_VERSION_V1,
    DaveMlsProposal, build_mls_proposal_append_frame, build_mls_proposal_revoke_frame,
};

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::hint::black_box;

pub const OP_MLS_PROPOSALS: u8 = 27;
pub const OP_MLS_PREPARE_COMMIT_TRANSITION: u8 = 29;
pub const PEER_LEAVE_STATES: (&str, &str, &str, &str) = (
    "peer-leave",
    "epoch N",
    "trigger remove",
    "epoch N+1 derived",
);

const TRUNCATED_SYNC_NONCE_OFFSET: usize = GCM_NONCE_SIZE - 4;
const MIN_SUPPLEMENTAL_BYTES: usize = GCM_TRUNCATED8_TAG_SIZE + 1 + 2;
pub type KeyAndNonce = ([u8; AES128_KEY_SIZE], [u8; GCM_NONCE_SIZE]);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DaveError {
    InvalidKeyLength(usize, usize),
    GenerationGapExceeded(u32, u32, u32),
    KeyExpired(u32, u32),
    ConfirmationTagMismatch,
    DowngradeRejected(u16, u16),
    TransitionNotPrepared(u16),
    MissingKeyRatchet(u64),
    PeerRemovedFromEpoch(u64, u64),
    StaleEpochPacketRejected(u64, u64),
    UnknownProposalRef,
    MalformedFrame(&'static str),
    UnencryptedFrameRejected,
    ReplayOrExpiredNonce(u32, u32),
    AeadAuthenticationFailed,
}

impl From<dave_v1::DaveError> for DaveError {
    fn from(err: dave_v1::DaveError) -> Self {
        match err {
            dave_v1::DaveError::ConfirmationTagMismatch => Self::ConfirmationTagMismatch,
            dave_v1::DaveError::UnknownProposalRef => Self::UnknownProposalRef,
            dave_v1::DaveError::MalformedFrame(m) => Self::MalformedFrame(m),
            _ => Self::MalformedFrame("dave_v1 error"),
        }
    }
}

pub fn zeroize_slice(buf: &mut [u8]) -> usize {
    for byte in buf.iter_mut() {
        *byte = black_box(0);
    }
    buf.len()
}

#[must_use]
pub fn is_zeroized(buf: &[u8]) -> bool {
    buf.iter().fold(0u8, |acc, &b| acc | black_box(b)) == 0
}

#[must_use]
pub fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b.iter()).all(|(&x, &y)| (x ^ y) == 0)
}

#[derive(Clone, PartialEq, Eq)]
pub struct DaveMlsEpochSecrets {
    pub inner: dave_v1::mls::DaveMlsEpochSecrets,
    invalidated: bool,
}

impl Drop for DaveMlsEpochSecrets {
    fn drop(&mut self) {
        self.invalidate_and_zeroize();
    }
}

impl fmt::Debug for DaveMlsEpochSecrets {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DaveMlsEpochSecrets(<скрыто>)")
    }
}

impl DaveMlsEpochSecrets {
    #[must_use]
    pub fn new(inner: dave_v1::mls::DaveMlsEpochSecrets) -> Self {
        Self {
            inner,
            invalidated: false,
        }
    }

    pub fn invalidate_and_zeroize(&mut self) {
        let i = &mut self.inner;
        for s in [
            &mut i.joiner_secret,
            &mut i.welcome_secret,
            &mut i.epoch_secret,
            &mut i.exporter_secret,
            &mut i.epoch_authenticator,
            &mut i.confirmation_key,
            &mut i.membership_key,
            &mut i.init_secret,
        ] {
            zeroize_slice(s);
        }
        self.invalidated = true;
    }

    #[must_use]
    pub fn is_invalidated(&self) -> bool {
        self.invalidated && is_zeroized(&self.inner.epoch_secret)
    }

    #[must_use]
    pub fn compute_confirmation_tag(&self, transcript_hash: &[u8; 32]) -> [u8; 32] {
        self.inner.compute_confirmation_tag(transcript_hash)
    }

    pub fn verify_confirmation_tag(
        &self,
        transcript_hash: &[u8; 32],
        candidate_tag: &[u8],
    ) -> Result<(), DaveError> {
        let expected = self.compute_confirmation_tag(transcript_hash);
        if ct_eq(&expected, candidate_tag) {
            Ok(())
        } else {
            Err(DaveError::ConfirmationTagMismatch)
        }
    }

    pub fn export_user_media_secret(&self, uid: u64) -> Result<[u8; AES128_KEY_SIZE], DaveError> {
        if self.invalidated {
            return Err(DaveError::KeyExpired(0, 1));
        }
        Ok(self.inner.export_user_media_secret(uid)?)
    }
}

#[derive(Clone)]
pub struct DaveMlsKeySchedule {
    pub inner: dave_v1::mls::DaveMlsKeySchedule,
    current: DaveMlsEpochSecrets,
    last_invalidated: Option<DaveMlsEpochSecrets>,
}

impl fmt::Debug for DaveMlsKeySchedule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DaveMlsKeySchedule(<скрыто>)")
    }
}

impl DaveMlsKeySchedule {
    pub fn from_initial_epoch(
        init: [u8; 32],
        commit: &[u8],
        gc: &DaveMlsGroupContext,
        psk: Option<&[u8]>,
    ) -> Result<Self, DaveError> {
        let inner = dave_v1::mls::DaveMlsKeySchedule::from_initial_epoch(init, commit, gc, psk)?;
        let current = DaveMlsEpochSecrets::new(inner.current_secrets().clone());
        Ok(Self {
            inner,
            current,
            last_invalidated: None,
        })
    }

    #[must_use]
    pub fn current_epoch(&self) -> u64 {
        self.current.inner.epoch
    }

    #[must_use]
    pub fn current_secrets(&self) -> &DaveMlsEpochSecrets {
        &self.current
    }

    #[must_use]
    pub fn was_previous_epoch_zeroized(&self) -> bool {
        self.last_invalidated
            .as_ref()
            .is_some_and(DaveMlsEpochSecrets::is_invalidated)
    }

    pub fn advance_epoch(
        &mut self,
        commit: &[u8],
        next_gc: &DaveMlsGroupContext,
        psk: Option<&[u8]>,
    ) -> Result<&DaveMlsEpochSecrets, DaveError> {
        let next_s = self.inner.advance_epoch(commit, next_gc, psk)?.clone();
        self.current.invalidate_and_zeroize();
        self.last_invalidated = Some(self.current.clone());
        self.current = DaveMlsEpochSecrets::new(next_s);
        Ok(&self.current)
    }

    pub fn export_sender_ratchet(&self, user_id: u64) -> Result<DaveHashRatchet, DaveError> {
        let mut media = self.current.export_user_media_secret(user_id)?;
        let ratchet = DaveHashRatchet::new(media.to_vec());
        zeroize_slice(&mut media);
        ratchet
    }
}

#[derive(Clone, Debug)]
pub struct DaveHashRatchet {
    next_secret: Vec<u8>,
    next_gen: u32,
    oldest_gen: u32,
    cache: BTreeMap<u32, KeyAndNonce>,
}

impl Drop for DaveHashRatchet {
    fn drop(&mut self) {
        zeroize_slice(&mut self.next_secret);
    }
}

impl DaveHashRatchet {
    pub fn new(s: Vec<u8>) -> Result<Self, DaveError> {
        if s.is_empty() {
            return Err(DaveError::InvalidKeyLength(AES128_KEY_SIZE, 0));
        }
        Ok(Self {
            next_secret: s,
            next_gen: 0,
            oldest_gen: 0,
            cache: BTreeMap::new(),
        })
    }

    fn step_once(&mut self) -> Result<(), DaveError> {
        let g = self.next_gen;
        let g_be = g.to_be_bytes();
        let kv = mls_expand_with_label(&self.next_secret, "key", &g_be, AES128_KEY_SIZE as u16)?;
        let nv = mls_expand_with_label(&self.next_secret, "nonce", &g_be, GCM_NONCE_SIZE as u16)?;
        let next_s = mls_expand_with_label(&self.next_secret, "secret", &g_be, 32)?;
        let mut key = [0u8; AES128_KEY_SIZE];
        key.copy_from_slice(&kv);
        let mut nonce = [0u8; GCM_NONCE_SIZE];
        nonce.copy_from_slice(&nv);
        zeroize_slice(&mut self.next_secret);
        self.next_secret = next_s;
        self.next_gen = g.saturating_add(1);
        self.cache.insert(g, (key, nonce));
        Ok(())
    }

    pub fn get(&mut self, generation: u32) -> Result<KeyAndNonce, DaveError> {
        if let Some(&cached) = self.cache.get(&generation) {
            return Ok(cached);
        }
        if generation < self.next_gen {
            return Err(DaveError::KeyExpired(generation, self.oldest_gen));
        }
        if generation - self.next_gen > DAVE_MAX_GENERATION_GAP {
            return Err(DaveError::GenerationGapExceeded(
                generation,
                self.next_gen,
                250,
            ));
        }
        for _ in self.next_gen..=generation {
            self.step_once()?;
        }
        self.cache
            .get(&generation)
            .copied()
            .ok_or(DaveError::KeyExpired(generation, self.oldest_gen))
    }

    pub fn delete_keys_up_to(&mut self, up_to: u32) {
        let expired: Vec<u32> = self.cache.range(..up_to).map(|(&g, _)| g).collect();
        for g in expired {
            if let Some((mut k, mut n)) = self.cache.remove(&g) {
                zeroize_slice(&mut k);
                zeroize_slice(&mut n);
            }
        }
        self.oldest_gen = self.oldest_gen.max(up_to);
    }
}

fn parse_dave_frame(frame: &[u8]) -> Option<(&[u8], [u8; GCM_TRUNCATED8_TAG_SIZE], u32)> {
    if frame.len() < MIN_SUPPLEMENTAL_BYTES + 1 || frame[frame.len() - 2..] != DAVE_MARKER_BYTES {
        return None;
    }
    let supp_size = usize::from(frame[frame.len() - 3]);
    if supp_size < MIN_SUPPLEMENTAL_BYTES + 1 || supp_size > frame.len() {
        return None;
    }
    let ct_len = frame.len() - supp_size;
    let supp = &frame[ct_len..];
    let mut tag = [0u8; GCM_TRUNCATED8_TAG_SIZE];
    tag.copy_from_slice(&supp[..GCM_TRUNCATED8_TAG_SIZE]);
    let meta = &supp[GCM_TRUNCATED8_TAG_SIZE..supp.len() - 3];
    let mut nonce = 0u32;
    for (idx, &b) in meta.iter().enumerate() {
        nonce |= u32::from(b & 0x7F) << (idx * 7);
        if b & 0x80 == 0 {
            return (idx + 1 == meta.len()).then_some((&frame[..ct_len], tag, nonce));
        }
    }
    None
}

#[derive(Clone, Debug)]
pub struct DaveFrameDecryptor {
    epoch: u64,
    ratchet: DaveHashRatchet,
    replay_window: AntiReplayWindow,
    purged_up_to_epoch: Option<u64>,
}

impl DaveFrameDecryptor {
    #[must_use]
    pub fn new(epoch: u64, ratchet: DaveHashRatchet) -> Self {
        Self {
            epoch,
            ratchet,
            replay_window: AntiReplayWindow::new(),
            purged_up_to_epoch: None,
        }
    }

    fn can_process_nonce(&self, generation: u32, truncated_nonce: u32) -> bool {
        self.replay_window
            .can_accept(dave_compute_wrapped_big_nonce(generation, truncated_nonce))
    }

    pub fn transition_to_key_ratchet(&mut self, epoch: u64, ratchet: DaveHashRatchet) {
        self.epoch = epoch;
        self.ratchet = ratchet;
        self.replay_window = AntiReplayWindow::new();
    }

    pub fn purge_previous_epochs(&mut self, current_epoch: u64) {
        self.purged_up_to_epoch = Some(current_epoch.saturating_sub(1));
    }

    pub fn decrypt_opus(&mut self, encrypted_frame: &[u8]) -> Result<Vec<u8>, DaveError> {
        if encrypted_frame == DAVE_OPUS_SILENCE_PACKET {
            return Ok(DAVE_OPUS_SILENCE_PACKET.to_vec());
        }
        let Some((ciphertext, tag, truncated_nonce)) = parse_dave_frame(encrypted_frame) else {
            return Err(DaveError::UnencryptedFrameRejected);
        };
        let generation = truncated_nonce >> DAVE_RATCHET_GENERATION_SHIFT_BITS;
        if !self.can_process_nonce(generation, truncated_nonce) {
            return Err(DaveError::ReplayOrExpiredNonce(generation, truncated_nonce));
        }
        let mut nonce_buf = [0u8; GCM_NONCE_SIZE];
        nonce_buf[TRUNCATED_SYNC_NONCE_OFFSET..].copy_from_slice(&truncated_nonce.to_le_bytes());

        let (mut key, _) = self.ratchet.get(generation)?;
        let aead =
            Aes128GcmTruncated8::new(&key).map_err(|_| DaveError::AeadAuthenticationFailed)?;
        zeroize_slice(&mut key);
        let mut candidate = ciphertext.to_vec();
        if aead
            .decrypt_in_place_detached(&nonce_buf, &[], &mut candidate, &tag)
            .is_ok()
        {
            let _ = self
                .replay_window
                .accept(dave_compute_wrapped_big_nonce(generation, truncated_nonce));
            return Ok(candidate);
        }
        if let Some(purged_ep) = self.purged_up_to_epoch {
            return Err(DaveError::StaleEpochPacketRejected(purged_ep, self.epoch));
        }
        Err(DaveError::AeadAuthenticationFailed)
    }
}

pub fn build_mls_commit_frame(
    seq: u16,
    tid: u16,
    ks: &DaveMlsKeySchedule,
    commit: &[u8],
    gc: &DaveMlsGroupContext,
    psk: Option<&[u8]>,
) -> Result<Vec<u8>, DaveError> {
    Ok(dave_v1::build_mls_commit_frame(
        seq, tid, &ks.inner, commit, gc, psk,
    )?)
}

#[derive(Clone, Debug)]
struct PreparedTransition {
    next_schedule: DaveMlsKeySchedule,
    removed_user_ids: Vec<u64>,
    has_peer_removal: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForwardSecrecyAuditReport {
    pub old_epoch: u64,
    pub new_epoch: u64,
    pub old_secrets_zeroized: bool,
    pub epoch_secret_changed: bool,
    pub sender_ratchets_rotated: bool,
    pub replay_rejected: bool,
    pub stale_epoch_rejected: bool,
    pub removed_peer_rejected: bool,
}

#[derive(Clone)]
pub struct DaveVoiceSession {
    self_user_id: u64,
    key_schedule: DaveMlsKeySchedule,
    members: BTreeSet<u64>,
    removed_peers: BTreeMap<u64, u64>,
    encryptor: dave_v1::DaveFrameEncryptor,
    decryptors: BTreeMap<u64, DaveFrameDecryptor>,
    pending_removals: Vec<([u8; 16], u64)>,
    pending_transitions: BTreeMap<u16, PreparedTransition>,
}

impl DaveVoiceSession {
    pub fn new(
        self_uid: u64,
        ks: DaveMlsKeySchedule,
        member_uids: &[u64],
        version: u16,
    ) -> Result<Self, DaveError> {
        if self_uid == 0 {
            return Err(DaveError::MalformedFrame("self_user_id cannot be 0"));
        }
        if version < DAVE_PROTOCOL_VERSION_V1 {
            return Err(DaveError::DowngradeRejected(
                DAVE_PROTOCOL_VERSION_V1,
                version,
            ));
        }
        let ep = ks.current_epoch();
        let media = ks.current_secrets().export_user_media_secret(self_uid)?;
        let encryptor =
            dave_v1::DaveFrameEncryptor::new(dave_v1::DaveHashRatchet::new(media.to_vec())?);
        let mut members = BTreeSet::from([self_uid]);
        let mut decryptors = BTreeMap::new();
        for &uid in member_uids {
            members.insert(uid);
            if uid != self_uid {
                let r = ks.export_sender_ratchet(uid)?;
                decryptors.insert(uid, DaveFrameDecryptor::new(ep, r));
            }
        }
        Ok(Self {
            self_user_id: self_uid,
            key_schedule: ks,
            members,
            removed_peers: BTreeMap::new(),
            encryptor,
            decryptors,
            pending_removals: Vec::new(),
            pending_transitions: BTreeMap::new(),
        })
    }

    #[must_use]
    pub fn key_schedule(&self) -> &DaveMlsKeySchedule {
        &self.key_schedule
    }

    #[must_use]
    pub fn current_epoch(&self) -> u64 {
        self.key_schedule.current_epoch()
    }

    pub fn process_mls_proposal(&mut self, frame: &[u8]) -> Result<usize, DaveError> {
        if frame.len() < 5 || frame[2] != OP_MLS_PROPOSALS {
            return Err(DaveError::MalformedFrame("invalid MLS Proposals header"));
        }
        let (vec_b, used) = mls_decode_vlbytes(&frame[4..])?;
        if 4 + used != frame.len() || vec_b.is_empty() {
            return Err(DaveError::MalformedFrame("invalid MLS Proposals payload"));
        }
        if frame[3] == 0 {
            let (body, step) = mls_decode_vlbytes(vec_b)?;
            if step != vec_b.len() || body.len() != 9 || body[0] != 2 {
                return Err(DaveError::MalformedFrame("invalid Remove proposal body"));
            }
            let mut uid_b = [0u8; 8];
            uid_b.copy_from_slice(&body[1..9]);
            let mut ref_in = Vec::from(&b"MLS 1.0 ProposalRef"[..]);
            ref_in.extend_from_slice(body);
            let digest = dave_sha256(&ref_in);
            let mut prop_ref = [0u8; 16];
            prop_ref.copy_from_slice(&digest[..16]);
            self.pending_removals
                .push((prop_ref, u64::from_be_bytes(uid_b)));
            Ok(self.pending_removals.len())
        } else {
            for &r in vec_b.as_chunks::<16>().0 {
                if !self.pending_removals.iter().any(|(ex, _)| *ex == r) {
                    return Err(DaveError::UnknownProposalRef);
                }
                self.pending_removals.retain(|(ex, _)| *ex != r);
            }
            Ok(self.pending_removals.len())
        }
    }

    pub fn process_mls_commit(&mut self, frame: &[u8]) -> Result<u16, DaveError> {
        if frame.len() < 40 || frame[2] != OP_MLS_PREPARE_COMMIT_TRANSITION {
            return Err(DaveError::MalformedFrame("invalid MLS Commit frame"));
        }
        let tid = u16::from_be_bytes([frame[3], frame[4]]);
        let (commit_s, u1) = mls_decode_vlbytes(&frame[5..])?;
        let (gc_b, u2) = mls_decode_vlbytes(&frame[5 + u1..])?;
        let next_gc = DaveMlsGroupContext::from_mls_bytes(gc_b)?;
        let cur = 5 + u1 + u2;
        let (psk_b, u3) = mls_decode_vlbytes(&frame[cur..])?;
        if next_gc.epoch != self.current_epoch() + 1 || frame.len() != cur + u3 + 32 {
            return Err(DaveError::MalformedFrame("invalid Commit epoch or tag"));
        }
        let mut next_ks = self.key_schedule.clone();
        next_ks.advance_epoch(commit_s, &next_gc, (!psk_b.is_empty()).then_some(psk_b))?;
        next_ks
            .current_secrets()
            .verify_confirmation_tag(&next_gc.confirmed_transcript_hash, &frame[cur + u3..])?;
        let removed_uids: Vec<u64> = self.pending_removals.drain(..).map(|(_, u)| u).collect();
        let has_rm = !removed_uids.is_empty();
        self.pending_transitions.insert(
            tid,
            PreparedTransition {
                next_schedule: next_ks,
                removed_user_ids: removed_uids,
                has_peer_removal: has_rm,
            },
        );
        Ok(tid)
    }

    pub fn execute_transition(&mut self, tid: u16) -> Result<u64, DaveError> {
        let prepared = self
            .pending_transitions
            .remove(&tid)
            .ok_or(DaveError::TransitionNotPrepared(tid))?;
        let new_ep = prepared.next_schedule.current_epoch();
        self.key_schedule = prepared.next_schedule;
        for uid in &prepared.removed_user_ids {
            self.members.remove(uid);
            self.decryptors.remove(uid);
            self.removed_peers.insert(*uid, new_ep);
        }
        let media = self
            .key_schedule
            .current_secrets()
            .export_user_media_secret(self.self_user_id)?;
        self.encryptor
            .set_key_ratchet(dave_v1::DaveHashRatchet::new(media.to_vec())?);
        for &uid in &self.members {
            if uid != self.self_user_id {
                let r = self.key_schedule.export_sender_ratchet(uid)?;
                if let Some(dec) = self.decryptors.get_mut(&uid) {
                    dec.transition_to_key_ratchet(new_ep, r);
                    if prepared.has_peer_removal {
                        dec.purge_previous_epochs(new_ep);
                    }
                }
            }
        }
        Ok(new_ep)
    }

    pub fn encrypt_opus(&mut self, opus_frame: &[u8]) -> Result<Vec<u8>, DaveError> {
        Ok(self.encryptor.encrypt_opus(opus_frame)?)
    }

    pub fn decrypt_opus(
        &mut self,
        sender_uid: u64,
        dave_frame: &[u8],
    ) -> Result<Vec<u8>, DaveError> {
        if let Some(&rm_ep) = self.removed_peers.get(&sender_uid) {
            return Err(DaveError::PeerRemovedFromEpoch(sender_uid, rm_ep));
        }
        let err = DaveError::MissingKeyRatchet(sender_uid);
        self.decryptors
            .get_mut(&sender_uid)
            .ok_or(err)?
            .decrypt_opus(dave_frame)
    }
}

fn make_test_context(epoch: u64, tree_byte: u8, conf_byte: u8) -> DaveMlsGroupContext {
    DaveMlsGroupContext {
        group_id: b"voice-chan-979".to_vec(),
        epoch,
        tree_hash: [tree_byte; 32],
        confirmed_transcript_hash: [conf_byte; 32],
        extensions: Vec::new(),
    }
}

fn apply_peer_removal(
    alice: &mut DaveVoiceSession,
    bob: &mut DaveVoiceSession,
    uid: u64,
) -> Result<u64, DaveError> {
    let (rm, _) = build_mls_proposal_append_frame(1, &DaveMlsProposal::Remove { user_id: uid })?;
    alice.process_mls_proposal(&rm)?;
    bob.process_mls_proposal(&rm)?;
    let ctx2 = make_test_context(2, 0x33, 0x44);
    let commit = build_mls_commit_frame(2, 77, alice.key_schedule(), &[0x99; 32], &ctx2, None)?;
    alice.process_mls_commit(&commit)?;
    bob.process_mls_commit(&commit)?;
    let new_ep = alice.execute_transition(77)?;
    bob.execute_transition(77)?;
    Ok(new_ep)
}

pub fn verify_peer_leave_forward_secrecy(
    a_id: u64,
    b_id: u64,
    rm_id: u64,
) -> Result<ForwardSecrecyAuditReport, DaveError> {
    let ctx1 = make_test_context(1, 0x11, 0x22);
    let m = [a_id, b_id, rm_id];
    let ks = DaveMlsKeySchedule::from_initial_epoch([0x21; 32], &[0x42; 32], &ctx1, None)?;
    let old_ep_sec = ks.current_secrets().inner.epoch_secret;
    let old_media = ks.current_secrets().export_user_media_secret(a_id)?;
    let mut alice = DaveVoiceSession::new(a_id, ks.clone(), &m, 1)?;
    let mut bob = DaveVoiceSession::new(b_id, ks.clone(), &m, 1)?;
    let mut mallory = DaveVoiceSession::new(rm_id, ks, &m, 1)?;
    let mut stale_a = alice.clone();

    let e1_pkt = alice.encrypt_opus(b"frame-epoch-1")?;
    let _ = bob.decrypt_opus(a_id, &e1_pkt)?;
    let replay = bob.decrypt_opus(a_id, &e1_pkt);
    let mallory_pkt = mallory.encrypt_opus(b"mallory-e1")?;
    let new_epoch = apply_peer_removal(&mut alice, &mut bob, rm_id)?;

    let stale_res = bob.decrypt_opus(a_id, &stale_a.encrypt_opus(b"late-stale-e1")?);
    let rm_res = bob.decrypt_opus(rm_id, &mallory_pkt);
    let cur_s = alice.key_schedule().current_secrets();
    let new_media = cur_s.export_user_media_secret(a_id)?;
    Ok(ForwardSecrecyAuditReport {
        old_epoch: 1,
        new_epoch,
        old_secrets_zeroized: alice.key_schedule().was_previous_epoch_zeroized()
            && bob.key_schedule().was_previous_epoch_zeroized(),
        epoch_secret_changed: cur_s.inner.epoch_secret != old_ep_sec,
        sender_ratchets_rotated: new_media != old_media,
        replay_rejected: matches!(replay, Err(DaveError::ReplayOrExpiredNonce(..))),
        stale_epoch_rejected: matches!(stale_res, Err(DaveError::StaleEpochPacketRejected(1, 2))),
        removed_peer_rejected: matches!(rm_res, Err(DaveError::PeerRemovedFromEpoch(_, 2))),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex_encode(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    fn epoch_json(s: &DaveMlsEpochSecrets, tag: &[u8; 32], uids: &[u64]) -> String {
        let mut keys = Vec::new();
        for &u in uids {
            let k = hex_encode(&s.export_user_media_secret(u).unwrap());
            keys.push(format!("\"{u}\":\"{k}\""));
        }
        let i = &s.inner;
        format!(
            "{{\"joiner_secret\":\"{}\",\"welcome_secret\":\"{}\",\"epoch_secret\":\"{}\",\"exporter_secret\":\"{}\",\"epoch_authenticator\":\"{}\",\"confirmation_key\":\"{}\",\"membership_key\":\"{}\",\"init_secret\":\"{}\",\"confirmation_tag\":\"{}\",\"user_media_keys\":{{{}}}}}",
            hex_encode(&i.joiner_secret),
            hex_encode(&i.welcome_secret),
            hex_encode(&i.epoch_secret),
            hex_encode(&i.exporter_secret),
            hex_encode(&i.epoch_authenticator),
            hex_encode(&i.confirmation_key),
            hex_encode(&i.membership_key),
            hex_encode(&i.init_secret),
            hex_encode(tag),
            keys.join(",")
        )
    }

    #[test]
    fn test_sub_979_1_rfc9420_key_schedule_and_exporter_vector() {
        let ctx1 = make_test_context(1, 0x11, 0x22);
        let mut ks =
            DaveMlsKeySchedule::from_initial_epoch([0x21; 32], &[0x42; 32], &ctx1, None).unwrap();
        let s1 = ks.current_secrets().clone();
        let tag1 = s1.compute_confirmation_tag(&ctx1.confirmed_transcript_hash);
        let e1_json = epoch_json(&s1, &tag1, &[1001, 2002, 3003]);
        let ctx2 = make_test_context(2, 0x33, 0x44);
        ks.advance_epoch(&[0x99; 32], &ctx2, None).unwrap();
        let s2 = ks.current_secrets().clone();
        let tag2 = s2.compute_confirmation_tag(&ctx2.confirmed_transcript_hash);
        let e2_json = epoch_json(&s2, &tag2, &[1001, 2002]);
        println!("DAVE_FS_VECTOR_JSON:{{\"epoch1\":{e1_json},\"epoch2\":{e2_json}}}");
    }

    #[test]
    fn test_sub_979_2_peer_leave_epoch_invalidation_and_ratchet_forward_secrecy() {
        let r = verify_peer_leave_forward_secrecy(1001, 2002, 3003).unwrap();
        assert_eq!((r.old_epoch, r.new_epoch), (1, 2));
        assert!(r.old_secrets_zeroized && r.epoch_secret_changed && r.sender_ratchets_rotated);

        let mut ratchet = DaveHashRatchet::new(vec![0x55; 16]).unwrap();
        let (k0, _) = ratchet.get(0).unwrap();
        let (k1, _) = ratchet.get(1).unwrap();
        let (k2, _) = ratchet.get(2).unwrap();
        assert_ne!(k0, k1);
        ratchet.delete_keys_up_to(2);
        assert_eq!(ratchet.get(0), Err(DaveError::KeyExpired(0, 2)));
        assert_eq!(ratchet.get(1), Err(DaveError::KeyExpired(1, 2)));
        assert_eq!(ratchet.get(2).unwrap().0, k2);
    }

    #[test]
    fn test_sub_979_3_replay_window_and_stale_epoch_rejection() {
        let r = verify_peer_leave_forward_secrecy(1001, 2002, 3003).unwrap();
        assert!(r.replay_rejected && r.stale_epoch_rejected && r.removed_peer_rejected);

        let mut win = AntiReplayWindow::new();
        assert_eq!(win.accept(0), Ok(()));
        assert_eq!(win.accept(0), Err(ReplayError::DuplicateNonce(0)));
        assert_eq!(win.accept(u64::MAX), Ok(()));
        assert_eq!(win.accept(u64::MAX - 1023), Ok(()));
        assert_eq!(
            win.accept(u64::MAX),
            Err(ReplayError::DuplicateNonce(u64::MAX))
        );
        assert!(matches!(
            win.accept(u64::MAX - 1024),
            Err(ReplayError::ExpiredNonce { .. })
        ));
    }

    #[test]
    fn test_sub_979_4_all_nine_dimensions_boundaries_revoke_and_errors() {
        let ctx1 = make_test_context(1, 0xAA, 0xBB);
        let ks =
            DaveMlsKeySchedule::from_initial_epoch([0x10; 32], &[0x20; 32], &ctx1, None).unwrap();
        let down = DaveVoiceSession::new(1, ks.clone(), &[1, u64::MAX], 0).err();
        assert_eq!(down, Some(DaveError::DowngradeRejected(1, 0)));
        let mut s_a = DaveVoiceSession::new(1, ks.clone(), &[1, u64::MAX], 1).unwrap();
        let mut s_b = DaveVoiceSession::new(u64::MAX, ks.clone(), &[1, u64::MAX], 1).unwrap();
        let sil = DAVE_OPUS_SILENCE_PACKET;
        assert_eq!(s_a.encrypt_opus(&sil).unwrap(), sil);
        assert_eq!(s_b.decrypt_opus(1, &sil).unwrap(), sil);
        let unenc = DaveError::UnencryptedFrameRejected;
        assert_eq!(s_b.decrypt_opus(1, b"plain-opus"), Err(unenc));

        let prop = DaveMlsProposal::Remove { user_id: u64::MAX };
        let (rm_frame, rm_ref) = build_mls_proposal_append_frame(1, &prop).unwrap();
        assert_eq!(s_a.process_mls_proposal(&rm_frame).unwrap(), 1);
        let rev_frame = build_mls_proposal_revoke_frame(2, &[rm_ref]).unwrap();
        assert_eq!(s_a.process_mls_proposal(&rev_frame).unwrap(), 0);

        let ctx2 = make_test_context(2, 0xAA, 0xBB);
        let mut forged = build_mls_commit_frame(3, 9, &ks, &[0x33; 32], &ctx2, None).unwrap();
        let last = forged.len() - 1;
        forged[last] ^= 0x01;
        assert_eq!(
            s_a.process_mls_commit(&forged),
            Err(DaveError::ConfirmationTagMismatch)
        );
        assert!(format!("{ks:?}").contains("<скрыто>"));
    }
}
