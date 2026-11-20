//! Pure-Rust Serein `davey` + `OpenMLS` session state machine (`501.2`) implementing [`DaveBackend`].

use std::collections::{BTreeMap, BTreeSet};

use zeroize::Zeroize;

use crate::bridge::{CommitWelcomeOutput, DaveBackend, DaveBackendKind};
use crate::davey_crypto::{
    DaveyHashRatchet, EpochSecrets, KEM_PREKEY_SLOTS, KemPrivateSlot, P256_PUB_LEN,
    SigningIdentity, build_group_context, compute_pairwise_fingerprint, derive_epoch_from_commit,
    derive_epoch_from_joiner, export_user_media_secret, hpke_open_p256, open_welcome_group_info,
    random_bytes_32, sha256_digest, verify_confirmation_tag, verify_p256_ecdsa,
};
use crate::davey_frame::{DaveyDecryptor, DaveyEncryptor};
use crate::davey_mls_codec::{
    KemPubSlot, MemberNode, QueuedProposal, compute_next_roster, compute_transcript_hash,
    decode_roster, encode_roster, find_own_hpke_entry, parse_external_proposal,
    pop_recipient_slots, seal_recipient_secrets, sign_and_seal_welcome_wire, sign_commit_wire,
};
use crate::protocol::{
    Codec, DAVE_MLS_CIPHERSUITE_ID, DAVE_PROTOCOL_VERSION_V1, DaveError, MediaType,
    ProposalsOperation, generate_displayable_code,
};
use crate::wire::{decode_vlbytes, encode_vlbytes, read_u16_be, read_u64_be};

struct ParsedPeerCommit<'a> {
    new_epoch: u64,
    new_trans: [u8; 32],
    roster_b: &'a [u8],
    ct_count: u16,
    ct_slice: &'a [u8],
}

struct StagedOwnCommit {
    raw_commit: Vec<u8>,
    new_epoch: u64,
    new_roster: Vec<MemberNode>,
    secrets: EpochSecrets,
    new_transcript: [u8; 32],
}

struct PreparedEpochMaterials {
    new_epoch: u64,
    next_roster: Vec<MemberNode>,
    added_uids: BTreeSet<u64>,
    roster_bytes: Vec<u8>,
    tree_hash: [u8; 32],
    new_transcript: [u8; 32],
    ext_raw: Vec<u8>,
    secrets: EpochSecrets,
    commit_cts: Vec<u8>,
    commit_cnt: u16,
    welcome_cts: Vec<u8>,
    welcome_cnt: u16,
}

/// Pure-Rust DAVE v1 session (`501.2`) matching Serein's `davey::DaveSession` lifecycle.
pub struct DaveySession {
    protocol_version: u16,
    user_id: u64,
    channel_id: u64,
    identity: SigningIdentity,
    own_kem_slots: BTreeMap<u8, KemPrivateSlot>,
    external_sender: Option<([u8; P256_PUB_LEN], Vec<u8>)>,
    established: bool,
    epoch: u64,
    roster: Vec<MemberNode>,
    init_secret: [u8; 32],
    exporter_secret: [u8; 32],
    epoch_auth: [u8; 32],
    transcript_hash: [u8; 32],
    pending_proposals: Vec<QueuedProposal>,
    staged_commit: Option<StagedOwnCommit>,
    encryptor: DaveyEncryptor,
    decryptors: BTreeMap<u64, DaveyDecryptor>,
}

impl Drop for DaveySession {
    fn drop(&mut self) {
        self.init_secret.zeroize();
        self.exporter_secret.zeroize();
        self.epoch_auth.zeroize();
    }
}

impl DaveySession {
    pub fn new(protocol_version: u16, user_id: u64, channel_id: u64) -> Result<Self, DaveError> {
        if protocol_version != DAVE_PROTOCOL_VERSION_V1 {
            return Err(DaveError::UnsupportedProtocolVersion(protocol_version));
        }
        if user_id == 0 {
            return Err(DaveError::InvalidArgument("user_id must be non-zero"));
        }
        let identity = SigningIdentity::generate()?;
        let own_kem_slots = Self::generate_kem_slots()?;
        Ok(Self {
            protocol_version,
            user_id,
            channel_id,
            identity,
            own_kem_slots,
            external_sender: None,
            established: false,
            epoch: 0,
            roster: Vec::new(),
            init_secret: [0u8; 32],
            exporter_secret: [0u8; 32],
            epoch_auth: [0u8; 32],
            transcript_hash: [0u8; 32],
            pending_proposals: Vec::new(),
            staged_commit: None,
            encryptor: DaveyEncryptor::new(),
            decryptors: BTreeMap::new(),
        })
    }

    pub fn pairwise_fingerprint(&self, version: u16, peer_uid: u64) -> Result<Vec<u8>, DaveError> {
        if !self.established {
            return Err(DaveError::InvalidState("group not established"));
        }
        let peer = self
            .roster
            .iter()
            .find(|m| m.user_id == peer_uid)
            .ok_or(DaveError::UnknownSender(peer_uid))?;
        compute_pairwise_fingerprint(
            version,
            self.user_id,
            self.identity.public_key_sec1(),
            peer_uid,
            &peer.sig_pub,
        )
    }

    fn generate_kem_slots() -> Result<BTreeMap<u8, KemPrivateSlot>, DaveError> {
        let mut map = BTreeMap::new();
        for sid in 0..KEM_PREKEY_SLOTS {
            map.insert(sid, KemPrivateSlot::generate()?);
        }
        Ok(map)
    }

    fn self_member_node(&self) -> MemberNode {
        let kem_slots = self
            .own_kem_slots
            .iter()
            .map(|(&slot_id, s)| KemPubSlot {
                slot_id,
                pub_sec1: s.pub_sec1,
            })
            .collect();
        MemberNode {
            user_id: self.user_id,
            sig_pub: *self.identity.public_key_sec1(),
            kem_slots,
        }
    }

    fn install_epoch_state(
        &mut self,
        epoch: u64,
        roster: Vec<MemberNode>,
        sec: EpochSecrets,
        trans: [u8; 32],
    ) -> Result<(), DaveError> {
        self.init_secret.zeroize();
        self.exporter_secret.zeroize();
        self.epoch_auth.zeroize();
        self.epoch = epoch;
        self.roster = roster;
        self.init_secret = sec.init_secret;
        self.exporter_secret = sec.exporter_secret;
        self.epoch_auth = sec.epoch_authenticator;
        self.transcript_hash = trans;
        self.pending_proposals.clear();
        self.staged_commit = None;
        self.established = true;

        let own_base = export_user_media_secret(&self.exporter_secret, self.user_id)?;
        self.encryptor
            .set_key_ratchet(DaveyHashRatchet::new(own_base));

        let active_ids: BTreeSet<u64> = self.roster.iter().map(|m| m.user_id).collect();
        for m in &self.roster {
            if m.user_id != self.user_id {
                let base = export_user_media_secret(&self.exporter_secret, m.user_id)?;
                self.decryptors
                    .entry(m.user_id)
                    .or_default()
                    .transition_to_key_ratchet(DaveyHashRatchet::new(base));
            }
        }
        self.decryptors.retain(|uid, _| active_ids.contains(uid));
        Ok(())
    }

    fn append_proposals(&mut self, inner: &[u8], expected: &[u64]) -> Result<(), DaveError> {
        let (ext_pub, _) = self
            .external_sender
            .as_ref()
            .ok_or(DaveError::InvalidState("no external sender"))?;
        let mut rem = inner;
        while !rem.is_empty() {
            let (prop_wire, next_rem) = decode_vlbytes(rem)?;
            rem = next_rem;
            let parsed =
                parse_external_proposal(prop_wire, ext_pub, self.channel_id, self.epoch, expected)?;
            self.pending_proposals.push(parsed);
        }
        Ok(())
    }

    fn revoke_proposals(&mut self, inner: &[u8]) -> Result<(), DaveError> {
        let mut rem = inner;
        while !rem.is_empty() {
            let (ref_slice, next_rem) = decode_vlbytes(rem)?;
            rem = next_rem;
            if ref_slice.len() != 16 {
                return Err(DaveError::MalformedWire("invalid proposal_ref length"));
            }
            let before = self.pending_proposals.len();
            self.pending_proposals
                .retain(|p| p.prop_ref.as_slice() != ref_slice);
            if self.pending_proposals.len() == before {
                return Err(DaveError::MalformedWire("unknown proposal_ref in revoke"));
            }
        }
        Ok(())
    }

    fn prepare_commit_materials(&self) -> Result<PreparedEpochMaterials, DaveError> {
        let (_, ext_raw) = self
            .external_sender
            .as_ref()
            .ok_or(DaveError::InvalidState("no external sender"))?
            .clone();
        let (mut next_roster, added_uids) =
            compute_next_roster(&self.roster, &self.pending_proposals)?;
        let recipients = pop_recipient_slots(&mut next_roster, self.user_id, &added_uids)?;
        let roster_bytes = encode_roster(&next_roster)?;
        let tree_hash = sha256_digest(&roster_bytes);
        let new_transcript =
            compute_transcript_hash(&self.transcript_hash, &self.pending_proposals, &tree_hash);
        let new_epoch = self.epoch.saturating_add(1);
        let gc = build_group_context(
            self.channel_id,
            new_epoch,
            &tree_hash,
            &new_transcript,
            &ext_raw,
        )?;
        let mut commit_secret = random_bytes_32()?;
        let secrets =
            derive_epoch_from_commit(&self.init_secret, &commit_secret, &gc, &new_transcript)?;
        let (commit_cts, commit_cnt, welcome_cts, welcome_cnt) = seal_recipient_secrets(
            self.channel_id,
            new_epoch,
            &recipients,
            &commit_secret,
            &secrets,
        )?;
        commit_secret.zeroize();
        Ok(PreparedEpochMaterials {
            new_epoch,
            next_roster,
            added_uids,
            roster_bytes,
            tree_hash,
            new_transcript,
            ext_raw,
            secrets,
            commit_cts,
            commit_cnt,
            welcome_cts,
            welcome_cnt,
        })
    }

    fn build_staged_commit_and_welcome(&mut self) -> Result<CommitWelcomeOutput, DaveError> {
        let m = self.prepare_commit_materials()?;
        let commit = sign_commit_wire(
            &self.identity,
            self.channel_id,
            m.new_epoch,
            self.user_id,
            &m.new_transcript,
            &m.roster_bytes,
            m.commit_cnt,
            &m.commit_cts,
            &m.secrets.confirmation_tag,
        )?;
        let welcome = (!m.added_uids.is_empty())
            .then(|| {
                sign_and_seal_welcome_wire(
                    &self.identity,
                    self.channel_id,
                    m.new_epoch,
                    self.user_id,
                    &m.tree_hash,
                    &m.new_transcript,
                    &m.ext_raw,
                    &m.roster_bytes,
                    m.welcome_cnt,
                    &m.welcome_cts,
                    &m.secrets,
                )
            })
            .transpose()?;
        self.staged_commit = Some(StagedOwnCommit {
            raw_commit: commit.clone(),
            new_epoch: m.new_epoch,
            new_roster: m.next_roster,
            secrets: m.secrets,
            new_transcript: m.new_transcript,
        });
        Ok(CommitWelcomeOutput { commit, welcome })
    }

    fn parse_verified_commit<'a>(
        &self,
        commit: &'a [u8],
    ) -> Result<ParsedPeerCommit<'a>, DaveError> {
        let (body, r1) = decode_vlbytes(commit).map_err(|_| DaveError::CommitRejected)?;
        let (sig, r2) = decode_vlbytes(r1).map_err(|_| DaveError::CommitRejected)?;
        let (gid, b1) = read_u64_be(body).map_err(|_| DaveError::CommitRejected)?;
        let (new_epoch, b2) = read_u64_be(b1).map_err(|_| DaveError::CommitRejected)?;
        let (sender_uid, b3) = read_u64_be(b2).map_err(|_| DaveError::CommitRejected)?;
        let (trans_s, b4) = decode_vlbytes(b3).map_err(|_| DaveError::CommitRejected)?;
        let (roster_b, b5) = decode_vlbytes(b4).map_err(|_| DaveError::CommitRejected)?;
        let (ct_count, ct_slice) = read_u16_be(b5).map_err(|_| DaveError::CommitRejected)?;
        if !r2.is_empty() || gid != self.channel_id || new_epoch != self.epoch + 1 {
            return Err(DaveError::CommitRejected);
        }
        let new_trans: [u8; 32] = trans_s.try_into().map_err(|_| DaveError::CommitRejected)?;
        let sender = self
            .roster
            .iter()
            .find(|m| m.user_id == sender_uid)
            .ok_or(DaveError::CommitRejected)?;
        verify_p256_ecdsa(
            &sender.sig_pub,
            &[b"MLS 1.0 CMT".as_slice(), body].concat(),
            sig,
        )
        .map_err(|_| DaveError::CommitRejected)?;
        Ok(ParsedPeerCommit {
            new_epoch,
            new_trans,
            roster_b,
            ct_count,
            ct_slice,
        })
    }

    fn verify_and_apply_peer_commit(
        &mut self,
        commit: &[u8],
        ext_raw: &[u8],
    ) -> Result<(), DaveError> {
        let p = self.parse_verified_commit(commit)?;
        let (mine, b7) = find_own_hpke_entry(p.ct_slice, p.ct_count, self.user_id)
            .map_err(|_| DaveError::CommitRejected)?;
        let (recv_tag, b8) = decode_vlbytes(b7).map_err(|_| DaveError::CommitRejected)?;
        let Some((sid, eph_pub, ct)) = mine else {
            return Err(DaveError::CommitRejected);
        };
        if !b8.is_empty() {
            return Err(DaveError::CommitRejected);
        }
        let cs = self.decrypt_commit_secret(sid, &eph_pub, &ct, self.channel_id, p.new_epoch)?;
        let new_roster = decode_roster(p.roster_b).map_err(|_| DaveError::CommitRejected)?;
        let gc = build_group_context(
            self.channel_id,
            p.new_epoch,
            &sha256_digest(p.roster_b),
            &p.new_trans,
            ext_raw,
        )?;
        let secrets = derive_epoch_from_commit(&self.init_secret, &cs, &gc, &p.new_trans)?;
        if !verify_confirmation_tag(&secrets.confirmation_tag, recv_tag) {
            return Err(DaveError::CommitRejected);
        }
        self.install_epoch_state(p.new_epoch, new_roster, secrets, p.new_trans)
    }

    fn decrypt_commit_secret(
        &mut self,
        sid: u8,
        eph_pub: &[u8; P256_PUB_LEN],
        ct: &[u8],
        gid: u64,
        new_epoch: u64,
    ) -> Result<[u8; 32], DaveError> {
        let slot = self
            .own_kem_slots
            .remove(&sid)
            .ok_or(DaveError::CommitRejected)?;
        let aad = [
            gid.to_be_bytes(),
            new_epoch.to_be_bytes(),
            self.user_id.to_be_bytes(),
        ]
        .concat();
        let mut cs_vec =
            hpke_open_p256(slot, eph_pub, &aad, ct).map_err(|_| DaveError::CommitRejected)?;
        let cs: [u8; 32] = cs_vec
            .as_slice()
            .try_into()
            .map_err(|_| DaveError::CommitRejected)?;
        cs_vec.zeroize();
        Ok(cs)
    }

    fn complete_welcome_join(
        &mut self,
        w_epoch: u64,
        joiner: [u8; 32],
        enc_gi: &[u8],
        ext_raw: &[u8],
        expected_users: &[u64],
    ) -> Result<(), DaveError> {
        let dummy_sec = derive_epoch_from_joiner(joiner, &[], &[0u8; 32])
            .map_err(|_| DaveError::WelcomeRejected)?;
        let gi_full =
            open_welcome_group_info(&dummy_sec.welcome_key, &dummy_sec.welcome_nonce, enc_gi)
                .map_err(|_| DaveError::WelcomeRejected)?;
        let (gid, g1) = read_u64_be(&gi_full).map_err(|_| DaveError::WelcomeRejected)?;
        let (ep, g2) = read_u64_be(g1).map_err(|_| DaveError::WelcomeRejected)?;
        let (sender_uid, g3) = read_u64_be(g2).map_err(|_| DaveError::WelcomeRejected)?;
        let (th_s, g4) = decode_vlbytes(g3).map_err(|_| DaveError::WelcomeRejected)?;
        let (tr_s, g5) = decode_vlbytes(g4).map_err(|_| DaveError::WelcomeRejected)?;
        let (joined_ext, g6) = decode_vlbytes(g5).map_err(|_| DaveError::WelcomeRejected)?;
        let (roster_b, g7) = decode_vlbytes(g6).map_err(|_| DaveError::WelcomeRejected)?;
        let (conf_tag, g8) = decode_vlbytes(g7).map_err(|_| DaveError::WelcomeRejected)?;
        let (w_sig, g9) = decode_vlbytes(g8).map_err(|_| DaveError::WelcomeRejected)?;
        if gid != self.channel_id || ep != w_epoch || joined_ext != ext_raw || !g9.is_empty() {
            return Err(DaveError::WelcomeRejected);
        }
        let roster = self.validate_welcome_roster(
            roster_b,
            sender_uid,
            &gi_full[..gi_full.len() - g8.len()],
            w_sig,
            expected_users,
        )?;
        let tree_hash: [u8; 32] = th_s.try_into().map_err(|_| DaveError::WelcomeRejected)?;
        let trans_hash: [u8; 32] = tr_s.try_into().map_err(|_| DaveError::WelcomeRejected)?;
        if sha256_digest(roster_b) != tree_hash {
            return Err(DaveError::WelcomeRejected);
        }
        let gc = build_group_context(gid, ep, &tree_hash, &trans_hash, ext_raw)?;
        let secrets = derive_epoch_from_joiner(joiner, &gc, &trans_hash)?;
        if !verify_confirmation_tag(&secrets.confirmation_tag, conf_tag) {
            return Err(DaveError::WelcomeRejected);
        }
        self.install_epoch_state(ep, roster, secrets, trans_hash)
    }

    fn validate_welcome_roster(
        &self,
        roster_b: &[u8],
        sender_uid: u64,
        signed_prefix: &[u8],
        w_sig: &[u8],
        expected_users: &[u64],
    ) -> Result<Vec<MemberNode>, DaveError> {
        let roster = decode_roster(roster_b).map_err(|_| DaveError::WelcomeRejected)?;
        let valid_members = roster.iter().any(|m| m.user_id == self.user_id)
            && (expected_users.is_empty()
                || roster.iter().all(|m| expected_users.contains(&m.user_id)));
        if !valid_members {
            return Err(DaveError::WelcomeRejected);
        }
        let sender = roster
            .iter()
            .find(|m| m.user_id == sender_uid)
            .ok_or(DaveError::WelcomeRejected)?;
        verify_p256_ecdsa(
            &sender.sig_pub,
            &[b"MLS 1.0 WLC".as_slice(), signed_prefix].concat(),
            w_sig,
        )
        .map_err(|_| DaveError::WelcomeRejected)?;
        Ok(roster)
    }
}

impl DaveBackend for DaveySession {
    fn kind(&self) -> DaveBackendKind {
        DaveBackendKind::DaveyRustCrypto
    }

    fn protocol_version(&self) -> u16 {
        self.protocol_version
    }

    fn user_id(&self) -> u64 {
        self.user_id
    }

    fn channel_id(&self) -> u64 {
        self.channel_id
    }

    fn is_ready(&self) -> bool {
        self.established
    }

    fn epoch(&self) -> Option<u64> {
        self.external_sender.as_ref().map(|_| self.epoch)
    }

    fn member_ids(&self) -> Vec<u64> {
        self.roster.iter().map(|m| m.user_id).collect()
    }

    fn reset(&mut self) -> Result<(), DaveError> {
        self.init_secret.zeroize();
        self.exporter_secret.zeroize();
        self.epoch_auth.zeroize();
        self.established = false;
        self.epoch = 0;
        self.transcript_hash = [0u8; 32];
        self.pending_proposals.clear();
        self.staged_commit = None;
        self.encryptor = DaveyEncryptor::new();
        self.decryptors.clear();
        self.own_kem_slots = Self::generate_kem_slots()?;
        self.roster = if self.external_sender.is_some() {
            vec![self.self_member_node()]
        } else {
            Vec::new()
        };
        Ok(())
    }

    fn set_external_sender(&mut self, external_sender: &[u8]) -> Result<(), DaveError> {
        if self.established {
            return Err(DaveError::InvalidState("already in established group"));
        }
        let (pub_slice, r1) = decode_vlbytes(external_sender)?;
        let (cred_type, r2) = read_u16_be(r1)?;
        let (_, r3) = decode_vlbytes(r2)?;
        if pub_slice.len() != P256_PUB_LEN
            || pub_slice[0] != 0x04
            || cred_type != 1
            || !r3.is_empty()
        {
            return Err(DaveError::MalformedWire("invalid external sender package"));
        }
        let mut ext_pub = [0u8; P256_PUB_LEN];
        ext_pub.copy_from_slice(pub_slice);
        self.external_sender = Some((ext_pub, external_sender.to_vec()));
        self.epoch = 0;
        self.roster = vec![self.self_member_node()];
        Ok(())
    }

    fn create_key_package(&mut self) -> Result<Vec<u8>, DaveError> {
        self.own_kem_slots = Self::generate_kem_slots()?;
        if !self.established && self.external_sender.is_some() {
            self.roster = vec![self.self_member_node()];
        }
        let mut body = Vec::new();
        body.extend_from_slice(&DAVE_PROTOCOL_VERSION_V1.to_be_bytes());
        body.extend_from_slice(&DAVE_MLS_CIPHERSUITE_ID.to_be_bytes());
        body.extend_from_slice(&1u16.to_be_bytes());
        encode_vlbytes(&self.user_id.to_be_bytes(), &mut body)?;
        encode_vlbytes(self.identity.public_key_sec1(), &mut body)?;
        body.push(self.own_kem_slots.len() as u8);
        for (&sid, slot) in &self.own_kem_slots {
            body.push(sid);
            encode_vlbytes(&slot.pub_sec1, &mut body)?;
        }
        body.extend_from_slice(&0u64.to_be_bytes());
        body.extend_from_slice(&u64::MAX.to_be_bytes());

        let sig = self
            .identity
            .sign(&[b"MLS 1.0 KP".as_slice(), &body].concat())?;
        let mut out = Vec::new();
        encode_vlbytes(&body, &mut out)?;
        encode_vlbytes(&sig, &mut out)?;
        Ok(out)
    }

    fn process_proposals(
        &mut self,
        operation: ProposalsOperation,
        proposals: &[u8],
        expected_users: &[u64],
    ) -> Result<Option<CommitWelcomeOutput>, DaveError> {
        let (&op_byte, rest) = proposals
            .split_first()
            .ok_or(DaveError::MalformedWire("empty proposals"))?;
        if ProposalsOperation::from_u8(op_byte)? != operation {
            return Err(DaveError::InvalidArgument("proposal operation mismatch"));
        }
        let (inner, rem) = decode_vlbytes(rest)?;
        if !rem.is_empty() {
            return Err(DaveError::MalformedWire("trailing bytes in proposals"));
        }
        match operation {
            ProposalsOperation::Append => self.append_proposals(inner, expected_users)?,
            ProposalsOperation::Revoke => self.revoke_proposals(inner)?,
        }
        if self.pending_proposals.is_empty() {
            self.staged_commit = None;
            return Ok(None);
        }
        Ok(Some(self.build_staged_commit_and_welcome()?))
    }

    fn process_commit(&mut self, commit: &[u8]) -> Result<(), DaveError> {
        let (_, ext_raw) = self
            .external_sender
            .as_ref()
            .ok_or(DaveError::CommitRejected)?
            .clone();
        if let Some(staged) = self.staged_commit.take()
            && bool::from(subtle::ConstantTimeEq::ct_eq(
                staged.raw_commit.as_slice(),
                commit,
            ))
        {
            return self.install_epoch_state(
                staged.new_epoch,
                staged.new_roster,
                staged.secrets,
                staged.new_transcript,
            );
        }
        if !self.established {
            return Err(DaveError::CommitRejected);
        }
        self.verify_and_apply_peer_commit(commit, &ext_raw)
    }

    fn process_welcome(&mut self, welcome: &[u8], expected_users: &[u64]) -> Result<(), DaveError> {
        let (_, ext_raw) = self
            .external_sender
            .as_ref()
            .ok_or(DaveError::WelcomeRejected)?
            .clone();
        if self.established {
            return Err(DaveError::WelcomeRejected);
        }
        let (ver, r1) = read_u16_be(welcome).map_err(|_| DaveError::WelcomeRejected)?;
        let (suite, r2) = read_u16_be(r1).map_err(|_| DaveError::WelcomeRejected)?;
        let (w_epoch, r3) = read_u64_be(r2).map_err(|_| DaveError::WelcomeRejected)?;
        let (ct_count, r4) = read_u16_be(r3).map_err(|_| DaveError::WelcomeRejected)?;
        if ver != DAVE_PROTOCOL_VERSION_V1 || suite != DAVE_MLS_CIPHERSUITE_ID || ct_count == 0 {
            return Err(DaveError::WelcomeRejected);
        }
        let (mine, r5) = find_own_hpke_entry(r4, ct_count, self.user_id)
            .map_err(|_| DaveError::WelcomeRejected)?;
        let (enc_gi, r6) = decode_vlbytes(r5).map_err(|_| DaveError::WelcomeRejected)?;
        let Some((sid, eph_pub, ct)) = mine else {
            return Err(DaveError::WelcomeRejected);
        };
        if !r6.is_empty() {
            return Err(DaveError::WelcomeRejected);
        }
        let slot = self
            .own_kem_slots
            .remove(&sid)
            .ok_or(DaveError::WelcomeRejected)?;
        let aad = [
            self.channel_id.to_be_bytes(),
            w_epoch.to_be_bytes(),
            self.user_id.to_be_bytes(),
        ]
        .concat();
        let mut kp_pt =
            hpke_open_p256(slot, &eph_pub, &aad, &ct).map_err(|_| DaveError::WelcomeRejected)?;
        if kp_pt.len() != 64 {
            return Err(DaveError::WelcomeRejected);
        }
        let mut joiner = [0u8; 32];
        joiner.copy_from_slice(&kp_pt[..32]);
        kp_pt.zeroize();
        self.complete_welcome_join(w_epoch, joiner, enc_gi, &ext_raw, expected_users)
    }

    fn epoch_authenticator(&self) -> Option<Vec<u8>> {
        self.established.then(|| self.epoch_auth.to_vec())
    }

    fn voice_privacy_code(&self) -> Option<String> {
        let auth = self.epoch_authenticator()?;
        generate_displayable_code(&auth, 30, 5).ok()
    }

    fn set_passthrough_mode(&mut self, passthrough: bool) {
        self.encryptor.set_passthrough_mode(passthrough);
        for dec in self.decryptors.values_mut() {
            dec.set_passthrough_mode(passthrough);
        }
    }

    fn encrypt(
        &mut self,
        media_type: MediaType,
        codec: Codec,
        frame: &[u8],
    ) -> Result<Vec<u8>, DaveError> {
        if !self.established {
            return Err(DaveError::NotReady);
        }
        self.encryptor.encrypt(media_type, codec, frame)
    }

    fn decrypt(
        &mut self,
        sender_uid: u64,
        media_type: MediaType,
        frame: &[u8],
    ) -> Result<Vec<u8>, DaveError> {
        let dec = self
            .decryptors
            .get_mut(&sender_uid)
            .ok_or(DaveError::UnknownSender(sender_uid))?;
        dec.decrypt(media_type, frame)
    }
}
