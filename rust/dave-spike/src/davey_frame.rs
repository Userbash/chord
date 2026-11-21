//! Pure-Rust Serein `davey` frame encryptor, multi-epoch decryptor, and 1024-nonce `AntiReplayWindow`.

use std::collections::VecDeque;

use zeroize::Zeroize;

use crate::davey_crypto::{
    DaveyHashRatchet, aes128_gcm_truncated8_open, aes128_gcm_truncated8_seal,
};
use crate::protocol::{Codec, DAVE_MAGIC_MARKER, DAVE_OPUS_SILENCE_PACKET, DaveError, MediaType};
use crate::wire::{
    UnencryptedRange, ciphertext_contains_h26x_start_code, extract_h264_unencrypted_ranges,
    leb128_len, parse_dave_supplemental_frame, reconstruct_frame_from_ranges,
    split_frame_by_ranges, write_leb128_u32,
};

const RATCHET_GENERATION_SHIFT_BITS: u32 = 24;
const RETAINED_EPOCH_MANAGERS: usize = 2;
const RETAINED_GENERATIONS: u32 = 4;
const MAX_VALIDATION_ATTEMPTS: usize = 10;

#[must_use]
pub const fn compute_wrapped_generation(oldest: u32, gen_u8: u32) -> u32 {
    let rem = oldest & 0xFF;
    let delta = gen_u8.wrapping_sub(rem) & 0xFF;
    oldest.saturating_add(delta)
}

#[must_use]
pub const fn compute_wrapped_big_nonce(generation: u32, truncated_nonce: u32) -> u64 {
    let low24 = (truncated_nonce as u64) & 0x00FF_FFFF;
    ((generation as u64) << RATCHET_GENERATION_SHIFT_BITS) | low24
}

/// 1024-position constant-space (`128 B`) sliding anti-replay bitmap.
#[derive(Clone, Debug, Default)]
pub struct AntiReplayWindow {
    initialized: bool,
    highest: u64,
    bitmap: [u64; 16],
}

impl AntiReplayWindow {
    #[must_use]
    pub fn can_accept(&self, nonce: u64) -> bool {
        if !self.initialized || nonce > self.highest {
            return true;
        }
        let diff = self.highest - nonce;
        if diff >= 1024 {
            return false;
        }
        let word = (diff / 64) as usize;
        let mask = 1u64 << (diff % 64);
        if (self.bitmap[word] & mask) != 0 {
            return false;
        }
        true
    }

    pub fn record(&mut self, nonce: u64) {
        if !self.initialized {
            self.initialized = true;
            self.highest = nonce;
            self.bitmap = [0u64; 16];
            self.bitmap[0] = 1;
            return;
        }
        if nonce > self.highest {
            let shift = nonce - self.highest;
            self.shift_window(shift);
            self.highest = nonce;
            self.bitmap[0] |= 1;
        } else {
            let diff = self.highest - nonce;
            if diff < 1024 {
                self.bitmap[(diff / 64) as usize] |= 1u64 << (diff % 64);
            }
        }
    }

    fn shift_window(&mut self, shift: u64) {
        if shift >= 1024 {
            self.bitmap = [0u64; 16];
            return;
        }
        let word_shift = (shift / 64) as usize;
        let bit_shift = (shift % 64) as u32;
        let mut next = [0u64; 16];
        for (i, slot) in next.iter_mut().enumerate().skip(word_shift) {
            let mut v = self.bitmap[i - word_shift] << bit_shift;
            if bit_shift > 0 && i > word_shift {
                v |= self.bitmap[i - word_shift - 1] >> (64 - bit_shift);
            }
            *slot = v;
        }
        self.bitmap = next;
    }
}

/// Outbound media frame encryptor (`[payload][8B tag][leb128 nonce][ranges][size][0xFAFA]`).
pub struct DaveyEncryptor {
    ratchet: Option<DaveyHashRatchet>,
    current_gen: u32,
    current_key: [u8; 16],
    has_key: bool,
    truncated_nonce: u32,
    passthrough: bool,
}

impl Default for DaveyEncryptor {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for DaveyEncryptor {
    fn drop(&mut self) {
        self.current_key.zeroize();
    }
}

impl DaveyEncryptor {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            ratchet: None,
            current_gen: 0,
            current_key: [0u8; 16],
            has_key: false,
            truncated_nonce: 0,
            passthrough: false,
        }
    }

    pub fn set_key_ratchet(&mut self, ratchet: DaveyHashRatchet) {
        self.current_key.zeroize();
        self.ratchet = Some(ratchet);
        self.current_gen = 0;
        self.has_key = false;
        self.truncated_nonce = 0;
    }

    pub fn set_passthrough_mode(&mut self, enabled: bool) {
        self.passthrough = enabled;
    }

    pub fn encrypt(
        &mut self,
        media_type: MediaType,
        codec: Codec,
        frame: &[u8],
    ) -> Result<Vec<u8>, DaveError> {
        if media_type == MediaType::Audio && frame == DAVE_OPUS_SILENCE_PACKET {
            return Ok(DAVE_OPUS_SILENCE_PACKET.to_vec());
        }
        if self.passthrough && self.ratchet.is_none() {
            return Ok(frame.to_vec());
        }
        let ranges = if codec == Codec::H264 {
            extract_h264_unencrypted_ranges(frame)
        } else {
            Vec::new()
        };
        let (aad, plain) = split_frame_by_ranges(frame, &ranges);
        for _ in 0..MAX_VALIDATION_ATTEMPTS {
            let nonce32 = self.advance_nonce_and_key()?;
            if let Some(packet) = self.try_seal_once(nonce32, &aad, &plain, &ranges)? {
                return Ok(packet);
            }
        }
        Err(DaveError::EncryptionFailed)
    }

    fn advance_nonce_and_key(&mut self) -> Result<u32, DaveError> {
        let ratchet = self.ratchet.as_mut().ok_or(DaveError::NotReady)?;
        self.truncated_nonce = self.truncated_nonce.wrapping_add(1);
        let gen_u8 = self.truncated_nonce >> RATCHET_GENERATION_SHIFT_BITS;
        let next_gen = compute_wrapped_generation(self.current_gen, gen_u8);
        if next_gen != self.current_gen || !self.has_key {
            let (key, _) = ratchet.get(next_gen)?;
            self.current_key.zeroize();
            self.current_key = key;
            self.current_gen = next_gen;
            self.has_key = true;
            if next_gen > 0 {
                ratchet.delete_keys_up_to(next_gen);
            }
        }
        Ok(self.truncated_nonce)
    }

    fn try_seal_once(
        &self,
        nonce32: u32,
        aad: &[u8],
        plain: &[u8],
        ranges: &[UnencryptedRange],
    ) -> Result<Option<Vec<u8>>, DaveError> {
        let mut nonce12 = [0u8; 12];
        nonce12[8..12].copy_from_slice(&nonce32.to_le_bytes());
        let (ct, tag8) = aes128_gcm_truncated8_seal(&self.current_key, &nonce12, aad, plain)?;
        let reconstructed = reconstruct_frame_from_ranges(ranges, aad, &ct);
        if ciphertext_contains_h26x_start_code(ranges, &reconstructed) {
            return Ok(None);
        }
        let mut ranges_wire = Vec::new();
        for r in ranges {
            write_leb128_u32(r.offset as u32, &mut ranges_wire);
            write_leb128_u32(r.size as u32, &mut ranges_wire);
        }
        let supp_len = 8 + leb128_len(u64::from(nonce32)) + ranges_wire.len() + 1 + 2;
        let supp_u8 = u8::try_from(supp_len).map_err(|_| DaveError::EncryptionFailed)?;

        let mut out = Vec::with_capacity(reconstructed.len() + supp_len);
        out.extend_from_slice(&reconstructed);
        out.extend_from_slice(&tag8);
        write_leb128_u32(nonce32, &mut out);
        out.extend_from_slice(&ranges_wire);
        out.push(supp_u8);
        out.extend_from_slice(&DAVE_MAGIC_MARKER);
        Ok(Some(out))
    }
}

struct EpochCipherManager {
    ratchet: DaveyHashRatchet,
    oldest_gen: u32,
    newest_gen: u32,
    replay: AntiReplayWindow,
}

impl EpochCipherManager {
    fn new(ratchet: DaveyHashRatchet) -> Self {
        Self {
            ratchet,
            oldest_gen: 0,
            newest_gen: 0,
            replay: AntiReplayWindow::default(),
        }
    }
}

/// Inbound per-participant media decryptor retaining up to 2 adjacent MLS epochs for seamless transitions.
pub struct DaveyDecryptor {
    managers: VecDeque<EpochCipherManager>,
    passthrough: bool,
}

impl Default for DaveyDecryptor {
    fn default() -> Self {
        Self::new()
    }
}

impl DaveyDecryptor {
    #[must_use]
    pub fn new() -> Self {
        Self {
            managers: VecDeque::with_capacity(RETAINED_EPOCH_MANAGERS),
            passthrough: false,
        }
    }

    pub fn transition_to_key_ratchet(&mut self, ratchet: DaveyHashRatchet) {
        if self.managers.len() >= RETAINED_EPOCH_MANAGERS {
            self.managers.pop_front();
        }
        self.managers.push_back(EpochCipherManager::new(ratchet));
    }

    pub fn set_passthrough_mode(&mut self, enabled: bool) {
        self.passthrough = enabled;
    }

    pub fn decrypt(&mut self, media_type: MediaType, frame: &[u8]) -> Result<Vec<u8>, DaveError> {
        if media_type == MediaType::Audio && frame == DAVE_OPUS_SILENCE_PACKET {
            return Ok(DAVE_OPUS_SILENCE_PACKET.to_vec());
        }
        let parsed = match parse_dave_supplemental_frame(frame) {
            Ok(p) => p,
            Err(e) => {
                return if self.passthrough {
                    Ok(frame.to_vec())
                } else {
                    Err(e)
                };
            }
        };
        let mut saw_replay = false;
        for mgr in self.managers.iter_mut().rev() {
            match Self::try_decrypt_with_manager(mgr, &parsed) {
                Ok(plain) => return Ok(plain),
                Err(DaveError::ReplayDetected) => saw_replay = true,
                Err(_) => continue,
            }
        }
        Err(if saw_replay {
            DaveError::ReplayDetected
        } else {
            DaveError::DecryptionFailed
        })
    }

    fn try_decrypt_with_manager(
        mgr: &mut EpochCipherManager,
        parsed: &crate::wire::ParsedDaveFrame,
    ) -> Result<Vec<u8>, DaveError> {
        let mut nonce12 = [0u8; 12];
        nonce12[8..12].copy_from_slice(&parsed.truncated_nonce.to_le_bytes());
        let gen_lsb = parsed.truncated_nonce >> RATCHET_GENERATION_SHIFT_BITS;
        let generation = compute_wrapped_generation(mgr.oldest_gen, gen_lsb);
        let big_nonce = compute_wrapped_big_nonce(generation, parsed.truncated_nonce);
        if !mgr.replay.can_accept(big_nonce) {
            return Err(DaveError::ReplayDetected);
        }
        let (mut key16, _) = mgr.ratchet.get(generation)?;
        let opened = aes128_gcm_truncated8_open(
            &key16,
            &nonce12,
            &parsed.aad,
            &parsed.ciphertext,
            &parsed.tag8,
        );
        key16.zeroize();
        let plain = opened?;
        mgr.replay.record(big_nonce);
        if generation > mgr.newest_gen {
            mgr.newest_gen = generation;
            if generation > RETAINED_GENERATIONS {
                let keep = generation - RETAINED_GENERATIONS;
                mgr.ratchet.delete_keys_up_to(keep);
                mgr.oldest_gen = mgr.oldest_gen.max(keep);
            }
        }
        Ok(reconstruct_frame_from_ranges(
            &parsed.ranges,
            &parsed.aad,
            &plain,
        ))
    }
}
