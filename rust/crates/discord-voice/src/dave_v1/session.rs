//! Кадровый шифр Opus (`0xFAFA`) и многопользовательская голосовая сессия `DaveVoiceSession`
//! с верификацией `Proposal` (op:27) и `Commit` (op:29).

use super::mls::{
    DaveHashRatchet, DaveMlsGroupContext, DaveMlsKeySchedule, Sha256State, mls_decode_vlbytes,
    mls_encode_vlbytes,
};
use super::replay::{
    AntiReplayWindow, DAVE_RATCHET_GENERATION_SHIFT_BITS, dave_compute_wrapped_big_nonce,
    dave_compute_wrapped_generation,
};
use super::{
    DAVE_MARKER_BYTES, DAVE_MAX_GENERATION_GAP, DAVE_OPUS_SILENCE_PACKET, DAVE_PROTOCOL_VERSION_V1,
    DaveError, zeroize_slice,
};
use crate::crypto::gcm::Aes128GcmTruncated8;
use crate::crypto::{GCM_NONCE_SIZE, GCM_TRUNCATED8_TAG_SIZE};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

const TRUNCATED_SYNC_NONCE_BYTES: usize = 4;
const TRUNCATED_SYNC_NONCE_OFFSET: usize = GCM_NONCE_SIZE - TRUNCATED_SYNC_NONCE_BYTES;
const MIN_SUPPLEMENTAL_BYTES: usize = GCM_TRUNCATED8_TAG_SIZE + 1 + 2;
const RETAINED_GENERATIONS: u32 = 4;
const MAX_RETAINED_EPOCH_MANAGERS: usize = 2;
const MAX_PENDING_PROPOSALS: usize = 128;

const MLS_OP_PROPOSALS: u8 = 27;
const MLS_OP_PREPARE_COMMIT: u8 = 29;
const PROPOSAL_OP_APPEND: u8 = 0;
const PROPOSAL_OP_REVOKE: u8 = 1;
const PROPOSAL_TYPE_ADD: u8 = 1;
const PROPOSAL_TYPE_REMOVE: u8 = 2;

fn leb128_write_u32(mut value: u32, out: &mut [u8; 5]) -> usize {
    let mut i = 0;
    while value >= 0x80 {
        out[i] = 0x80 | ((value as u8) & 0x7F);
        i += 1;
        value >>= 7;
    }
    out[i] = value as u8;
    i + 1
}

fn leb128_read_u32(slice: &[u8]) -> Option<(u32, usize)> {
    let mut value = 0u64;
    let mut shift = 0u32;
    for (idx, &byte) in slice.iter().enumerate() {
        if idx >= 5 {
            return None;
        }
        value |= u64::from(byte & 0x7F) << shift;
        if byte & 0x80 == 0 {
            let v32 = u32::try_from(value).ok()?;
            if idx > 0 && byte == 0 {
                return None;
            }
            return Some((v32, idx + 1));
        }
        shift += 7;
    }
    None
}

/// Шифратор исходящих аудио-фреймов Opus (`[ciphertext][8B tag][leb128(nonce)][size][0xFAFA]`).
#[derive(Clone)]
pub struct DaveFrameEncryptor {
    ratchet: Option<DaveHashRatchet>,
    cipher: Option<Aes128GcmTruncated8>,
    current_generation: u32,
    truncated_nonce: u32,
}

impl DaveFrameEncryptor {
    #[must_use]
    pub fn new(ratchet: DaveHashRatchet) -> Self {
        Self {
            ratchet: Some(ratchet),
            cipher: None,
            current_generation: 0,
            truncated_nonce: 0,
        }
    }

    pub fn set_key_ratchet(&mut self, ratchet: DaveHashRatchet) {
        self.ratchet = Some(ratchet);
        self.cipher = None;
        self.current_generation = 0;
        self.truncated_nonce = 0;
    }

    pub fn encrypt_opus(&mut self, opus_frame: &[u8]) -> Result<Vec<u8>, DaveError> {
        if opus_frame == DAVE_OPUS_SILENCE_PACKET {
            return Ok(DAVE_OPUS_SILENCE_PACKET.to_vec());
        }
        let ratchet = self
            .ratchet
            .as_mut()
            .ok_or(DaveError::MissingKeyRatchet { user_id: 0 })?;

        self.truncated_nonce = self.truncated_nonce.wrapping_add(1);
        let next_gen = dave_compute_wrapped_generation(
            self.current_generation,
            self.truncated_nonce >> DAVE_RATCHET_GENERATION_SHIFT_BITS,
        );
        if next_gen != self.current_generation || self.cipher.is_none() {
            let (mut key, _) = ratchet.get(next_gen)?;
            let aead =
                Aes128GcmTruncated8::new(&key).map_err(|_| DaveError::AeadAuthenticationFailed)?;
            zeroize_slice(&mut key);
            if next_gen > 0 {
                ratchet.delete_keys_up_to(next_gen);
            }
            self.current_generation = next_gen;
            self.cipher = Some(aead);
        }
        let cipher = self
            .cipher
            .as_ref()
            .ok_or(DaveError::MalformedFrame("missing outbound cipher"))?;

        let mut nonce_buf = [0u8; GCM_NONCE_SIZE];
        nonce_buf[TRUNCATED_SYNC_NONCE_OFFSET..]
            .copy_from_slice(&self.truncated_nonce.to_le_bytes());

        let mut ciphertext = opus_frame.to_vec();
        let tag8 = cipher
            .encrypt_in_place_detached(&nonce_buf, &[], &mut ciphertext)
            .map_err(|_| DaveError::MalformedFrame("payload exceeds GCM limit"))?;

        let mut nonce_leb = [0u8; 5];
        let nonce_leb_len = leb128_write_u32(self.truncated_nonce, &mut nonce_leb);
        let supplemental_total = MIN_SUPPLEMENTAL_BYTES + nonce_leb_len;

        let mut out = Vec::with_capacity(ciphertext.len() + supplemental_total);
        out.extend_from_slice(&ciphertext);
        out.extend_from_slice(&tag8);
        out.extend_from_slice(&nonce_leb[..nonce_leb_len]);
        out.push(supplemental_total as u8);
        out.extend_from_slice(&DAVE_MARKER_BYTES);
        Ok(out)
    }
}

#[derive(Clone)]
struct DaveCipherManager {
    ratchet: DaveHashRatchet,
    ciphers: BTreeMap<u32, Aes128GcmTruncated8>,
    oldest_generation: u32,
    newest_generation: u32,
    replay_window: AntiReplayWindow,
}

impl DaveCipherManager {
    fn new(ratchet: DaveHashRatchet) -> Self {
        Self {
            ratchet,
            ciphers: BTreeMap::new(),
            oldest_generation: 0,
            newest_generation: 0,
            replay_window: AntiReplayWindow::new(),
        }
    }

    fn compute_generation(&self, gen_u8: u32) -> u32 {
        dave_compute_wrapped_generation(self.oldest_generation, gen_u8)
    }

    fn can_process_nonce(&self, generation: u32, truncated_nonce: u32) -> bool {
        let big = dave_compute_wrapped_big_nonce(generation, truncated_nonce);
        self.replay_window.can_accept(big)
    }

    fn get_or_derive_cipher(&mut self, generation: u32) -> Result<&Aes128GcmTruncated8, DaveError> {
        if generation < self.oldest_generation {
            return Err(DaveError::KeyExpired {
                generation,
                oldest_available: self.oldest_generation,
            });
        }
        if generation
            > self
                .newest_generation
                .saturating_add(DAVE_MAX_GENERATION_GAP)
        {
            return Err(DaveError::GenerationGapExceeded {
                requested: generation,
                current: self.newest_generation,
                max_gap: DAVE_MAX_GENERATION_GAP,
            });
        }
        if !self.ciphers.contains_key(&generation) {
            let (mut key, _) = self.ratchet.get(generation)?;
            let cipher =
                Aes128GcmTruncated8::new(&key).map_err(|_| DaveError::AeadAuthenticationFailed)?;
            zeroize_slice(&mut key);
            self.ciphers.insert(generation, cipher);
        }
        self.ciphers.get(&generation).ok_or(DaveError::KeyExpired {
            generation,
            oldest_available: self.oldest_generation,
        })
    }

    fn record_success(&mut self, generation: u32, truncated_nonce: u32) {
        let big = dave_compute_wrapped_big_nonce(generation, truncated_nonce);
        let _ = self.replay_window.accept(big);
        if generation > self.newest_generation {
            self.newest_generation = generation;
            if generation > RETAINED_GENERATIONS {
                let prune_before = generation - RETAINED_GENERATIONS;
                self.oldest_generation = self.oldest_generation.max(prune_before);
                self.ratchet.delete_keys_up_to(self.oldest_generation);
                let stale: Vec<u32> = self
                    .ciphers
                    .range(..self.oldest_generation)
                    .map(|(&g, _)| g)
                    .collect();
                for g in stale {
                    self.ciphers.remove(&g);
                }
            }
        }
    }
}

fn parse_dave_encrypted_frame(frame: &[u8]) -> Option<(&[u8], [u8; GCM_TRUNCATED8_TAG_SIZE], u32)> {
    if frame.len() < MIN_SUPPLEMENTAL_BYTES + 1 {
        return None;
    }
    if frame[frame.len() - 2..] != DAVE_MARKER_BYTES {
        return None;
    }
    let supplemental_size = usize::from(frame[frame.len() - 3]);
    if supplemental_size < MIN_SUPPLEMENTAL_BYTES + 1 || supplemental_size > frame.len() {
        return None;
    }
    let ct_len = frame.len() - supplemental_size;
    let supplemental = &frame[ct_len..];
    let mut tag = [0u8; GCM_TRUNCATED8_TAG_SIZE];
    tag.copy_from_slice(&supplemental[..GCM_TRUNCATED8_TAG_SIZE]);
    let meta = &supplemental[GCM_TRUNCATED8_TAG_SIZE..supplemental.len() - 3];
    let (truncated_nonce, used) = leb128_read_u32(meta)?;
    if used != meta.len() {
        return None;
    }
    Some((&frame[..ct_len], tag, truncated_nonce))
}

/// Дешифратор входящих аудио-фреймов Opus с окном `AntiReplayWindow` и удержанием предыдущей эпохи.
#[derive(Clone)]
pub struct DaveFrameDecryptor {
    managers: VecDeque<DaveCipherManager>,
    passthrough_enabled: bool,
}

impl DaveFrameDecryptor {
    #[must_use]
    pub fn new(ratchet: DaveHashRatchet) -> Self {
        let mut managers = VecDeque::with_capacity(MAX_RETAINED_EPOCH_MANAGERS);
        managers.push_back(DaveCipherManager::new(ratchet));
        Self {
            managers,
            passthrough_enabled: false,
        }
    }

    pub fn transition_to_key_ratchet(&mut self, ratchet: DaveHashRatchet) {
        if self.managers.len() >= MAX_RETAINED_EPOCH_MANAGERS {
            self.managers.pop_front();
        }
        self.managers.push_back(DaveCipherManager::new(ratchet));
    }

    pub fn decrypt_opus(&mut self, encrypted_frame: &[u8]) -> Result<Vec<u8>, DaveError> {
        if encrypted_frame == DAVE_OPUS_SILENCE_PACKET {
            return Ok(DAVE_OPUS_SILENCE_PACKET.to_vec());
        }
        let Some((ciphertext, tag, truncated_nonce)) = parse_dave_encrypted_frame(encrypted_frame)
        else {
            if self.passthrough_enabled {
                return Ok(encrypted_frame.to_vec());
            }
            return Err(DaveError::UnencryptedFrameRejected);
        };

        let mut nonce_buf = [0u8; GCM_NONCE_SIZE];
        nonce_buf[TRUNCATED_SYNC_NONCE_OFFSET..].copy_from_slice(&truncated_nonce.to_le_bytes());

        let gen_lsb = truncated_nonce >> DAVE_RATCHET_GENERATION_SHIFT_BITS;
        let mut saw_replay = None;
        let mut last_err = None;

        for manager in self.managers.iter_mut().rev() {
            let generation = manager.compute_generation(gen_lsb);
            if !manager.can_process_nonce(generation, truncated_nonce) {
                saw_replay = Some((generation, truncated_nonce));
                continue;
            }
            let cipher = match manager.get_or_derive_cipher(generation) {
                Ok(c) => c,
                Err(e) => {
                    last_err = Some(e);
                    continue;
                }
            };
            let mut candidate = ciphertext.to_vec();
            if cipher
                .decrypt_in_place_detached(&nonce_buf, &[], &mut candidate, &tag)
                .is_ok()
            {
                manager.record_success(generation, truncated_nonce);
                return Ok(candidate);
            }
        }
        if let Some((generation, truncated_nonce)) = saw_replay {
            return Err(DaveError::ReplayOrExpiredNonce {
                generation,
                truncated_nonce,
            });
        }
        if let Some(err) = last_err {
            return Err(err);
        }
        Err(DaveError::AeadAuthenticationFailed)
    }
}

/// Предложение изменения состава группы MLS (`op:27 MlsProposals`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DaveMlsProposal {
    Add { user_id: u64, key_package: Vec<u8> },
    Remove { user_id: u64 },
}

fn encode_proposal_body(proposal: &DaveMlsProposal) -> Result<Vec<u8>, DaveError> {
    let mut body = Vec::new();
    match proposal {
        DaveMlsProposal::Add {
            user_id,
            key_package,
        } => {
            if *user_id == 0 || key_package.is_empty() {
                return Err(DaveError::MalformedFrame("invalid Add proposal"));
            }
            body.push(PROPOSAL_TYPE_ADD);
            body.extend_from_slice(&user_id.to_be_bytes());
            mls_encode_vlbytes(key_package, &mut body)?;
        }
        DaveMlsProposal::Remove { user_id } => {
            if *user_id == 0 {
                return Err(DaveError::MalformedFrame("invalid Remove proposal"));
            }
            body.push(PROPOSAL_TYPE_REMOVE);
            body.extend_from_slice(&user_id.to_be_bytes());
        }
    }
    Ok(body)
}

fn decode_proposal_body(bytes: &[u8]) -> Result<DaveMlsProposal, DaveError> {
    if bytes.len() < 9 {
        return Err(DaveError::MalformedFrame("MLS Proposal body too short"));
    }
    let prop_type = bytes[0];
    let mut uid_bytes = [0u8; 8];
    uid_bytes.copy_from_slice(&bytes[1..9]);
    let user_id = u64::from_be_bytes(uid_bytes);
    if user_id == 0 {
        return Err(DaveError::MalformedFrame(
            "MLS Proposal user_id cannot be 0",
        ));
    }
    match prop_type {
        PROPOSAL_TYPE_ADD => {
            let (kp, used) = mls_decode_vlbytes(&bytes[9..])?;
            if kp.is_empty() || 9 + used != bytes.len() {
                return Err(DaveError::MalformedFrame("malformed Add KeyPackage"));
            }
            Ok(DaveMlsProposal::Add {
                user_id,
                key_package: kp.to_vec(),
            })
        }
        PROPOSAL_TYPE_REMOVE => {
            if bytes.len() != 9 {
                return Err(DaveError::MalformedFrame(
                    "trailing bytes in Remove proposal",
                ));
            }
            Ok(DaveMlsProposal::Remove { user_id })
        }
        _ => Err(DaveError::MalformedFrame("unknown MLS ProposalType")),
    }
}

fn compute_proposal_ref(proposal_body: &[u8]) -> [u8; 16] {
    let mut h = Sha256State::new();
    h.update(b"MLS 1.0 ProposalRef");
    h.update(proposal_body);
    let digest = h.finalize();
    let mut out = [0u8; 16];
    out.copy_from_slice(&digest[..16]);
    out
}

/// Собирает бинарный кадр `op:27` (`APPEND`) и возвращает `(wire_bytes, proposal_ref)`.
pub fn build_mls_proposal_append_frame(
    seq_ack: u16,
    proposal: &DaveMlsProposal,
) -> Result<(Vec<u8>, [u8; 16]), DaveError> {
    let body = encode_proposal_body(proposal)?;
    let prop_ref = compute_proposal_ref(&body);
    let mut list_buf = Vec::new();
    mls_encode_vlbytes(&body, &mut list_buf)?;

    let mut frame = Vec::with_capacity(4 + list_buf.len() + 4);
    frame.extend_from_slice(&seq_ack.to_be_bytes());
    frame.push(MLS_OP_PROPOSALS);
    frame.push(PROPOSAL_OP_APPEND);
    mls_encode_vlbytes(&list_buf, &mut frame)?;
    Ok((frame, prop_ref))
}

/// Собирает бинарный кадр `op:27` (`REVOKE`) со списком 16-байтных `ProposalRef`.
pub fn build_mls_proposal_revoke_frame(
    seq_ack: u16,
    refs: &[[u8; 16]],
) -> Result<Vec<u8>, DaveError> {
    if refs.is_empty() {
        return Err(DaveError::MalformedFrame("empty ProposalRef revoke list"));
    }
    let mut raw_refs = Vec::with_capacity(refs.len() * 16);
    for r in refs {
        raw_refs.extend_from_slice(r);
    }
    let mut frame = Vec::with_capacity(8 + raw_refs.len());
    frame.extend_from_slice(&seq_ack.to_be_bytes());
    frame.push(MLS_OP_PROPOSALS);
    frame.push(PROPOSAL_OP_REVOKE);
    mls_encode_vlbytes(&raw_refs, &mut frame)?;
    Ok(frame)
}

/// Собирает бинарный кадр `op:29` (`MLS Commit`) с подлинным `confirmation_tag` следующей эпохи.
pub fn build_mls_commit_frame(
    seq_ack: u16,
    transition_id: u16,
    current_schedule: &DaveMlsKeySchedule,
    commit_secret: &[u8],
    next_group_context: &DaveMlsGroupContext,
    psk_secret: Option<&[u8]>,
) -> Result<Vec<u8>, DaveError> {
    let mut preview = current_schedule.clone();
    let next_secrets = preview.advance_epoch(commit_secret, next_group_context, psk_secret)?;
    let confirmation_tag =
        next_secrets.compute_confirmation_tag(&next_group_context.confirmed_transcript_hash);
    let gc_bytes = next_group_context.to_mls_bytes()?;

    let mut frame = Vec::with_capacity(16 + commit_secret.len() + gc_bytes.len() + 32);
    frame.extend_from_slice(&seq_ack.to_be_bytes());
    frame.push(MLS_OP_PREPARE_COMMIT);
    frame.extend_from_slice(&transition_id.to_be_bytes());
    mls_encode_vlbytes(commit_secret, &mut frame)?;
    mls_encode_vlbytes(&gc_bytes, &mut frame)?;
    mls_encode_vlbytes(psk_secret.unwrap_or(&[]), &mut frame)?;
    frame.extend_from_slice(&confirmation_tag);
    Ok(frame)
}

#[derive(Clone)]
struct PreparedTransition {
    next_schedule: DaveMlsKeySchedule,
    member_user_ids: BTreeSet<u64>,
}

/// Многопользовательская голосовая сессия Discord DAVE MLS v1 с верификацией `Proposal`/`Commit`.
#[derive(Clone)]
pub struct DaveVoiceSession {
    self_user_id: u64,
    protocol_version: u16,
    key_schedule: DaveMlsKeySchedule,
    members: BTreeSet<u64>,
    encryptor: DaveFrameEncryptor,
    decryptors: BTreeMap<u64, DaveFrameDecryptor>,
    pending_proposals: Vec<([u8; 16], DaveMlsProposal)>,
    pending_transitions: BTreeMap<u16, PreparedTransition>,
}

impl DaveVoiceSession {
    pub fn new(
        self_user_id: u64,
        key_schedule: DaveMlsKeySchedule,
        member_user_ids: &[u64],
        protocol_version: u16,
    ) -> Result<Self, DaveError> {
        if self_user_id == 0 {
            return Err(DaveError::MalformedFrame("self_user_id cannot be 0"));
        }
        if protocol_version < DAVE_PROTOCOL_VERSION_V1 {
            return Err(DaveError::DowngradeRejected {
                current_version: DAVE_PROTOCOL_VERSION_V1,
                requested_version: protocol_version,
            });
        }
        let self_ratchet = key_schedule.export_sender_ratchet(self_user_id)?;
        let encryptor = DaveFrameEncryptor::new(self_ratchet);
        let mut members = BTreeSet::new();
        members.insert(self_user_id);
        let mut decryptors = BTreeMap::new();
        for &uid in member_user_ids {
            if uid == 0 {
                return Err(DaveError::MalformedFrame("member user_id cannot be 0"));
            }
            members.insert(uid);
            if uid != self_user_id {
                let peer_ratchet = key_schedule.export_sender_ratchet(uid)?;
                decryptors.insert(uid, DaveFrameDecryptor::new(peer_ratchet));
            }
        }
        Ok(Self {
            self_user_id,
            protocol_version,
            key_schedule,
            members,
            encryptor,
            decryptors,
            pending_proposals: Vec::new(),
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

    #[must_use]
    pub fn protocol_version(&self) -> u16 {
        self.protocol_version
    }

    /// Обрабатывает входящий бинарный кадр `op:27 MlsProposals` (`APPEND` / `REVOKE`).
    pub fn process_mls_proposal(&mut self, frame: &[u8]) -> Result<usize, DaveError> {
        if frame.len() < 5 {
            return Err(DaveError::MalformedFrame("MLS Proposals frame too short"));
        }
        if frame[2] != MLS_OP_PROPOSALS {
            return Err(DaveError::MalformedFrame(
                "unexpected opcode for MLS Proposals",
            ));
        }
        let op_type = frame[3];
        let (vector_bytes, used) = mls_decode_vlbytes(&frame[4..])?;
        if 4 + used != frame.len() || vector_bytes.is_empty() {
            return Err(DaveError::MalformedFrame(
                "invalid or trailing bytes in MLS Proposals",
            ));
        }

        match op_type {
            PROPOSAL_OP_APPEND => {
                let mut cursor = 0usize;
                let mut parsed_batch = Vec::new();
                while cursor < vector_bytes.len() {
                    let (prop_body, step) = mls_decode_vlbytes(&vector_bytes[cursor..])?;
                    cursor += step;
                    let proposal = decode_proposal_body(prop_body)?;
                    if let DaveMlsProposal::Remove { user_id } = &proposal
                        && *user_id == self.self_user_id
                    {
                        return Err(DaveError::MalformedFrame("cannot remove self in proposal"));
                    }
                    let prop_ref = compute_proposal_ref(prop_body);
                    parsed_batch.push((prop_ref, proposal));
                    if self.pending_proposals.len() + parsed_batch.len() > MAX_PENDING_PROPOSALS {
                        return Err(DaveError::MalformedFrame("too many pending MLS proposals"));
                    }
                }
                for (prop_ref, proposal) in parsed_batch {
                    if !self.pending_proposals.iter().any(|(r, _)| *r == prop_ref) {
                        self.pending_proposals.push((prop_ref, proposal));
                    }
                }
                Ok(self.pending_proposals.len())
            }
            PROPOSAL_OP_REVOKE => {
                if !vector_bytes.len().is_multiple_of(16) {
                    return Err(DaveError::MalformedFrame(
                        "ProposalRef revoke list must be a multiple of 16 bytes",
                    ));
                }
                let mut refs_to_remove = Vec::with_capacity(vector_bytes.len() / 16);
                for &r in vector_bytes.as_chunks::<16>().0 {
                    if !self
                        .pending_proposals
                        .iter()
                        .any(|(existing, _)| *existing == r)
                    {
                        return Err(DaveError::UnknownProposalRef);
                    }
                    refs_to_remove.push(r);
                }
                self.pending_proposals
                    .retain(|(existing, _)| !refs_to_remove.contains(existing));
                Ok(self.pending_proposals.len())
            }
            _ => Err(DaveError::MalformedFrame("unknown MLS Proposals op_type")),
        }
    }

    /// Проверяет и подготавливает переход по входящему кадру `op:29` (`MLS Commit`).
    /// При несовпадении `confirmation_tag` или порче структуры состояние сессии не меняется.
    pub fn process_mls_commit(&mut self, frame: &[u8]) -> Result<u16, DaveError> {
        if frame.len() < 5 + 3 + 32 {
            return Err(DaveError::MalformedFrame("MLS Commit frame too short"));
        }
        if frame[2] != MLS_OP_PREPARE_COMMIT {
            return Err(DaveError::MalformedFrame(
                "unexpected opcode for MLS Commit",
            ));
        }
        let transition_id = u16::from_be_bytes([frame[3], frame[4]]);
        let mut cursor = 5usize;

        let (commit_secret, used) = mls_decode_vlbytes(&frame[cursor..])?;
        if commit_secret.is_empty() {
            return Err(DaveError::MalformedFrame("empty MLS commit_secret"));
        }
        cursor += used;

        let (gc_bytes, used) = mls_decode_vlbytes(&frame[cursor..])?;
        cursor += used;
        let next_gc = DaveMlsGroupContext::from_mls_bytes(gc_bytes)?;

        let expected_epoch = self
            .current_epoch()
            .checked_add(1)
            .ok_or(DaveError::MalformedFrame("MLS epoch overflow"))?;
        if next_gc.epoch != expected_epoch
            || next_gc.group_id != self.key_schedule.group_context().group_id
        {
            return Err(DaveError::MalformedFrame(
                "MLS Commit group_id or epoch continuity mismatch",
            ));
        }

        let (psk_bytes, used) = mls_decode_vlbytes(&frame[cursor..])?;
        cursor += used;

        if frame.len() != cursor + 32 {
            return Err(DaveError::MalformedFrame(
                "MLS Commit confirmation_tag must be exactly 32 bytes",
            ));
        }
        let candidate_tag = &frame[cursor..cursor + 32];

        let psk_opt = if psk_bytes.is_empty() {
            None
        } else {
            Some(psk_bytes)
        };
        let mut next_schedule = self.key_schedule.clone();
        next_schedule.advance_epoch(commit_secret, &next_gc, psk_opt)?;
        next_schedule
            .current_secrets()
            .verify_confirmation_tag(&next_gc.confirmed_transcript_hash, candidate_tag)?;

        let mut next_members = self.members.clone();
        for (_, proposal) in &self.pending_proposals {
            match proposal {
                DaveMlsProposal::Add { user_id, .. } => {
                    next_members.insert(*user_id);
                }
                DaveMlsProposal::Remove { user_id } => {
                    next_members.remove(user_id);
                }
            }
        }
        self.pending_proposals.clear();
        self.pending_transitions.insert(
            transition_id,
            PreparedTransition {
                next_schedule,
                member_user_ids: next_members,
            },
        );
        Ok(transition_id)
    }

    /// Активирует подготовленный переход эпохи (`op:22 ExecuteTransition`).
    pub fn execute_transition(&mut self, transition_id: u16) -> Result<u64, DaveError> {
        let prepared = self
            .pending_transitions
            .remove(&transition_id)
            .ok_or(DaveError::TransitionNotPrepared { transition_id })?;

        self.key_schedule = prepared.next_schedule;
        self.members = prepared.member_user_ids;

        let new_self_ratchet = self.key_schedule.export_sender_ratchet(self.self_user_id)?;
        self.encryptor.set_key_ratchet(new_self_ratchet);

        self.decryptors.retain(|uid, _| self.members.contains(uid));
        for &uid in &self.members {
            if uid != self.self_user_id {
                let peer_ratchet = self.key_schedule.export_sender_ratchet(uid)?;
                match self.decryptors.get_mut(&uid) {
                    Some(dec) => dec.transition_to_key_ratchet(peer_ratchet),
                    None => {
                        self.decryptors
                            .insert(uid, DaveFrameDecryptor::new(peer_ratchet));
                    }
                }
            }
        }
        Ok(self.key_schedule.current_epoch())
    }

    pub fn encrypt_opus(&mut self, opus_frame: &[u8]) -> Result<Vec<u8>, DaveError> {
        self.encryptor.encrypt_opus(opus_frame)
    }

    pub fn decrypt_opus(
        &mut self,
        sender_user_id: u64,
        dave_frame: &[u8],
    ) -> Result<Vec<u8>, DaveError> {
        let decryptor =
            self.decryptors
                .get_mut(&sender_user_id)
                .ok_or(DaveError::MissingKeyRatchet {
                    user_id: sender_user_id,
                })?;
        decryptor.decrypt_opus(dave_frame)
    }
}
