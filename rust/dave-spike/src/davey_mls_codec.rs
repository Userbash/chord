//! MLS 1.0 roster, `KeyPackage`, `Proposal`, `Commit`, and `Welcome` wire codecs for `DaveySession`.

use std::collections::BTreeSet;

use zeroize::Zeroize;

use crate::davey_crypto::{
    EpochSecrets, P256_PUB_LEN, SigningIdentity, hpke_seal_p256, seal_welcome_group_info,
    sha256_digest, verify_p256_ecdsa,
};
use crate::protocol::{DAVE_MLS_CIPHERSUITE_ID, DAVE_PROTOCOL_VERSION_V1, DaveError};
use crate::wire::{decode_vlbytes, encode_vlbytes, read_u16_be, read_u64_be};

#[derive(Clone)]
pub struct KemPubSlot {
    pub slot_id: u8,
    pub pub_sec1: [u8; P256_PUB_LEN],
}

#[derive(Clone)]
pub struct MemberNode {
    pub user_id: u64,
    pub sig_pub: [u8; P256_PUB_LEN],
    pub kem_slots: Vec<KemPubSlot>,
}

#[derive(Clone)]
pub enum QueuedProposalKind {
    Add(MemberNode),
    Remove(u64),
}

#[derive(Clone)]
pub struct QueuedProposal {
    pub prop_ref: [u8; 16],
    pub kind: QueuedProposalKind,
}

pub struct RecipientTarget {
    pub user_id: u64,
    pub slot_id: u8,
    pub pub_sec1: [u8; P256_PUB_LEN],
    pub is_welcome: bool,
}

pub type OwnHpkeEntry = (u8, [u8; P256_PUB_LEN], Vec<u8>);

pub fn parse_key_package(raw: &[u8]) -> Result<MemberNode, DaveError> {
    let (body, r1) = decode_vlbytes(raw)?;
    let (sig, r2) = decode_vlbytes(r1)?;
    if !r2.is_empty() {
        return Err(DaveError::MalformedWire("trailing bytes in KeyPackage"));
    }
    let (ver, b1) = read_u16_be(body)?;
    let (suite, b2) = read_u16_be(b1)?;
    let (cred_type, b3) = read_u16_be(b2)?;
    if ver != DAVE_PROTOCOL_VERSION_V1 || suite != DAVE_MLS_CIPHERSUITE_ID || cred_type != 1 {
        return Err(DaveError::MalformedWire(
            "unsupported KeyPackage parameters",
        ));
    }
    let (uid_bytes, b4) = decode_vlbytes(b3)?;
    let (user_id, uid_rem) = read_u64_be(uid_bytes)?;
    let (sig_slice, b5) = decode_vlbytes(b4)?;
    if !uid_rem.is_empty() || user_id == 0 || sig_slice.len() != P256_PUB_LEN {
        return Err(DaveError::MalformedWire("invalid KeyPackage identity"));
    }
    let (kem_slots, b_cur) = decode_kem_slot_list(b5, true)?;
    let (_, b_nb) = read_u64_be(b_cur)?;
    let (_, b_end) = read_u64_be(b_nb)?;
    if !b_end.is_empty() {
        return Err(DaveError::MalformedWire("trailing KeyPackage body"));
    }
    let mut sig_pub = [0u8; P256_PUB_LEN];
    sig_pub.copy_from_slice(sig_slice);
    let mut tbs = b"MLS 1.0 KP".to_vec();
    tbs.extend_from_slice(body);
    verify_p256_ecdsa(&sig_pub, &tbs, sig)?;
    Ok(MemberNode {
        user_id,
        sig_pub,
        kem_slots,
    })
}

fn decode_kem_slot_list(
    input: &[u8],
    require_non_empty: bool,
) -> Result<(Vec<KemPubSlot>, &[u8]), DaveError> {
    let (&slot_count, mut cur) = input
        .split_first()
        .ok_or(DaveError::MalformedWire("missing slot count"))?;
    if (require_non_empty && slot_count == 0) || slot_count > 16 {
        return Err(DaveError::MalformedWire("invalid KEM slot count"));
    }
    let mut slots = Vec::with_capacity(usize::from(slot_count));
    for _ in 0..slot_count {
        let (&slot_id, rest) = cur
            .split_first()
            .ok_or(DaveError::MalformedWire("truncated slot"))?;
        let (kpub, rest2) = decode_vlbytes(rest)?;
        let pub_sec1: [u8; P256_PUB_LEN] = kpub
            .try_into()
            .map_err(|_| DaveError::MalformedWire("bad kem pub"))?;
        slots.push(KemPubSlot { slot_id, pub_sec1 });
        cur = rest2;
    }
    Ok((slots, cur))
}

pub fn parse_external_proposal(
    prop_wire: &[u8],
    ext_pub: &[u8; P256_PUB_LEN],
    expected_gid: u64,
    expected_epoch: u64,
    expected_users: &[u64],
) -> Result<QueuedProposal, DaveError> {
    let (body, r1) = decode_vlbytes(prop_wire)?;
    let (sig, r2) = decode_vlbytes(r1)?;
    if !r2.is_empty() {
        return Err(DaveError::MalformedWire("trailing proposal bytes"));
    }
    let mut tbs = b"MLS 1.0 PRP".to_vec();
    tbs.extend_from_slice(body);
    verify_p256_ecdsa(ext_pub, &tbs, sig)?;

    let (gid, b1) = read_u64_be(body)?;
    let (ep, b2) = read_u64_be(b1)?;
    let (&ptype, b3) = b2
        .split_first()
        .ok_or(DaveError::MalformedWire("missing proposal type"))?;
    if gid != expected_gid || ep != expected_epoch {
        return Err(DaveError::MalformedWire("proposal group/epoch mismatch"));
    }
    let digest = sha256_digest(prop_wire);
    let mut prop_ref = [0u8; 16];
    prop_ref.copy_from_slice(&digest[..16]);
    let kind = parse_proposal_kind(ptype, b3, expected_users)?;
    Ok(QueuedProposal { prop_ref, kind })
}

fn parse_proposal_kind(
    ptype: u8,
    body_rem: &[u8],
    expected_users: &[u64],
) -> Result<QueuedProposalKind, DaveError> {
    match ptype {
        1 => {
            let (kp_raw, rem) = decode_vlbytes(body_rem)?;
            if !rem.is_empty() {
                return Err(DaveError::MalformedWire("trailing Add bytes"));
            }
            let node = parse_key_package(kp_raw)?;
            if !expected_users.is_empty() && !expected_users.contains(&node.user_id) {
                return Err(DaveError::UnrecognizedParticipant(node.user_id));
            }
            Ok(QueuedProposalKind::Add(node))
        }
        2 => {
            let (uid, rem) = read_u64_be(body_rem)?;
            if !rem.is_empty() {
                return Err(DaveError::MalformedWire("trailing Remove bytes"));
            }
            Ok(QueuedProposalKind::Remove(uid))
        }
        _ => Err(DaveError::MalformedWire("unknown proposal type")),
    }
}

pub fn compute_next_roster(
    current: &[MemberNode],
    proposals: &[QueuedProposal],
) -> Result<(Vec<MemberNode>, BTreeSet<u64>), DaveError> {
    let mut next = current.to_vec();
    let mut added = BTreeSet::new();
    for p in proposals {
        match &p.kind {
            QueuedProposalKind::Add(node) => {
                if next.iter().any(|m| m.user_id == node.user_id) {
                    return Err(DaveError::DuplicateParticipant(node.user_id));
                }
                added.insert(node.user_id);
                next.push(node.clone());
            }
            QueuedProposalKind::Remove(uid) => {
                next.retain(|m| m.user_id != *uid);
            }
        }
    }
    if next.is_empty() {
        return Err(DaveError::MalformedWire("empty group roster"));
    }
    next.sort_by_key(|m| m.user_id);
    Ok((next, added))
}

pub fn pop_recipient_slots(
    roster: &mut [MemberNode],
    committer_uid: u64,
    added_uids: &BTreeSet<u64>,
) -> Result<Vec<RecipientTarget>, DaveError> {
    let mut out = Vec::new();
    for m in roster.iter_mut() {
        if m.user_id == committer_uid {
            continue;
        }
        if m.kem_slots.is_empty() {
            return Err(DaveError::InvalidState("member KEM slots exhausted"));
        }
        let slot = m.kem_slots.remove(0);
        out.push(RecipientTarget {
            user_id: m.user_id,
            slot_id: slot.slot_id,
            pub_sec1: slot.pub_sec1,
            is_welcome: added_uids.contains(&m.user_id),
        });
    }
    Ok(out)
}

pub fn encode_roster(roster: &[MemberNode]) -> Result<Vec<u8>, DaveError> {
    let mut out = Vec::new();
    let count =
        u16::try_from(roster.len()).map_err(|_| DaveError::MalformedWire("roster overflow"))?;
    out.extend_from_slice(&count.to_be_bytes());
    for m in roster {
        out.extend_from_slice(&m.user_id.to_be_bytes());
        encode_vlbytes(&m.sig_pub, &mut out)?;
        out.push(m.kem_slots.len() as u8);
        for s in &m.kem_slots {
            out.push(s.slot_id);
            encode_vlbytes(&s.pub_sec1, &mut out)?;
        }
    }
    Ok(out)
}

pub fn decode_roster(input: &[u8]) -> Result<Vec<MemberNode>, DaveError> {
    let (count, mut rem) = read_u16_be(input)?;
    if count == 0 || count > 64 {
        return Err(DaveError::MalformedWire("invalid roster count"));
    }
    let mut out = Vec::with_capacity(usize::from(count));
    for _ in 0..count {
        let (user_id, r1) = read_u64_be(rem)?;
        let (spub, r2) = decode_vlbytes(r1)?;
        if user_id == 0 || spub.len() != P256_PUB_LEN {
            return Err(DaveError::MalformedWire("invalid roster member"));
        }
        let (kem_slots, cur) = decode_kem_slot_list(r2, false)?;
        let mut sig_pub = [0u8; P256_PUB_LEN];
        sig_pub.copy_from_slice(spub);
        out.push(MemberNode {
            user_id,
            sig_pub,
            kem_slots,
        });
        rem = cur;
    }
    if !rem.is_empty() {
        return Err(DaveError::MalformedWire("trailing bytes in roster"));
    }
    Ok(out)
}

#[must_use]
pub fn compute_transcript_hash(
    prev: &[u8; 32],
    props: &[QueuedProposal],
    tree_hash: &[u8; 32],
) -> [u8; 32] {
    let mut buf = Vec::with_capacity(64 + props.len() * 16);
    buf.extend_from_slice(prev);
    for p in props {
        buf.extend_from_slice(&p.prop_ref);
    }
    buf.extend_from_slice(tree_hash);
    sha256_digest(&buf)
}

pub fn seal_recipient_secrets(
    group_id: u64,
    new_epoch: u64,
    recipients: &[RecipientTarget],
    commit_secret: &[u8; 32],
    secrets: &EpochSecrets,
) -> Result<(Vec<u8>, u16, Vec<u8>, u16), DaveError> {
    let (mut c_cts, mut w_cts) = (Vec::new(), Vec::new());
    let (mut c_cnt, mut w_cnt) = (0u16, 0u16);
    for r in recipients {
        let aad = [
            group_id.to_be_bytes(),
            new_epoch.to_be_bytes(),
            r.user_id.to_be_bytes(),
        ]
        .concat();
        if !r.is_welcome {
            let (eph, ct) = hpke_seal_p256(&r.pub_sec1, &aad, commit_secret)?;
            append_hpke_entry(&mut c_cts, r.user_id, r.slot_id, &eph, &ct)?;
            c_cnt += 1;
        } else {
            let mut pt = [0u8; 64];
            pt[..32].copy_from_slice(&secrets.joiner_secret);
            pt[32..].copy_from_slice(&secrets.init_secret);
            let res = hpke_seal_p256(&r.pub_sec1, &aad, &pt);
            pt.zeroize();
            let (eph, ct) = res?;
            append_hpke_entry(&mut w_cts, r.user_id, r.slot_id, &eph, &ct)?;
            w_cnt += 1;
        }
    }
    Ok((c_cts, c_cnt, w_cts, w_cnt))
}

fn append_hpke_entry(
    out: &mut Vec<u8>,
    uid: u64,
    slot_id: u8,
    eph: &[u8; P256_PUB_LEN],
    ct: &[u8],
) -> Result<(), DaveError> {
    out.extend_from_slice(&uid.to_be_bytes());
    out.push(slot_id);
    encode_vlbytes(eph, out)?;
    encode_vlbytes(ct, out)?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn sign_commit_wire(
    identity: &SigningIdentity,
    group_id: u64,
    new_epoch: u64,
    sender_uid: u64,
    new_transcript: &[u8; 32],
    roster_bytes: &[u8],
    ct_count: u16,
    commit_cts: &[u8],
    conf_tag: &[u8; 32],
) -> Result<Vec<u8>, DaveError> {
    let mut body = Vec::new();
    body.extend_from_slice(&group_id.to_be_bytes());
    body.extend_from_slice(&new_epoch.to_be_bytes());
    body.extend_from_slice(&sender_uid.to_be_bytes());
    encode_vlbytes(new_transcript, &mut body)?;
    encode_vlbytes(roster_bytes, &mut body)?;
    body.extend_from_slice(&ct_count.to_be_bytes());
    body.extend_from_slice(commit_cts);
    encode_vlbytes(conf_tag, &mut body)?;

    let mut tbs = b"MLS 1.0 CMT".to_vec();
    tbs.extend_from_slice(&body);
    let sig = identity.sign(&tbs)?;
    let mut out = Vec::new();
    encode_vlbytes(&body, &mut out)?;
    encode_vlbytes(&sig, &mut out)?;
    Ok(out)
}

#[allow(clippy::too_many_arguments)]
pub fn sign_and_seal_welcome_wire(
    identity: &SigningIdentity,
    group_id: u64,
    new_epoch: u64,
    sender_uid: u64,
    tree_hash: &[u8; 32],
    new_transcript: &[u8; 32],
    ext_raw: &[u8],
    roster_bytes: &[u8],
    welcome_cnt: u16,
    welcome_cts: &[u8],
    secrets: &EpochSecrets,
) -> Result<Vec<u8>, DaveError> {
    let mut gi = Vec::new();
    gi.extend_from_slice(&group_id.to_be_bytes());
    gi.extend_from_slice(&new_epoch.to_be_bytes());
    gi.extend_from_slice(&sender_uid.to_be_bytes());
    encode_vlbytes(tree_hash, &mut gi)?;
    encode_vlbytes(new_transcript, &mut gi)?;
    encode_vlbytes(ext_raw, &mut gi)?;
    encode_vlbytes(roster_bytes, &mut gi)?;
    encode_vlbytes(&secrets.confirmation_tag, &mut gi)?;

    let mut tbs = b"MLS 1.0 WLC".to_vec();
    tbs.extend_from_slice(&gi);
    let sig = identity.sign(&tbs)?;
    encode_vlbytes(&sig, &mut gi)?;

    let enc_gi = seal_welcome_group_info(&secrets.welcome_key, &secrets.welcome_nonce, &gi)?;
    let mut out = Vec::new();
    out.extend_from_slice(&DAVE_PROTOCOL_VERSION_V1.to_be_bytes());
    out.extend_from_slice(&DAVE_MLS_CIPHERSUITE_ID.to_be_bytes());
    out.extend_from_slice(&new_epoch.to_be_bytes());
    out.extend_from_slice(&welcome_cnt.to_be_bytes());
    out.extend_from_slice(welcome_cts);
    encode_vlbytes(&enc_gi, &mut out)?;
    Ok(out)
}

pub fn find_own_hpke_entry(
    mut cur: &[u8],
    count: u16,
    own_uid: u64,
) -> Result<(Option<OwnHpkeEntry>, &[u8]), DaveError> {
    let mut found = None;
    for _ in 0..count {
        let (ruid, r1) = read_u64_be(cur)?;
        let (&sid, r2) = r1
            .split_first()
            .ok_or(DaveError::MalformedWire("missing slot id"))?;
        let (ep, r3) = decode_vlbytes(r2)?;
        let (ct, r4) = decode_vlbytes(r3)?;
        let ep_arr: [u8; P256_PUB_LEN] = ep
            .try_into()
            .map_err(|_| DaveError::MalformedWire("bad eph pub"))?;
        if ruid == own_uid {
            found = Some((sid, ep_arr, ct.to_vec()));
        }
        cur = r4;
    }
    Ok((found, cur))
}
