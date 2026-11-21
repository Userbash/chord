//! Wire framing helpers for RFC 9420 `VLBytes`, DAVE `LEB128`, H.264 Annex-B unencrypted ranges,
//! and binary MLS message envelopes (pure serialization — zero cryptographic algorithms).

use crate::protocol::{DAVE_MAGIC_MARKER, DaveError};

/// Unencrypted byte range within a media frame authenticated as AEAD associated data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnencryptedRange {
    pub offset: usize,
    pub size: usize,
}

/// Encodes a slice with RFC 9420 §2.1.2 variable-length integer prefix (`VLBytes`).
pub fn encode_vlbytes(data: &[u8], out: &mut Vec<u8>) -> Result<(), DaveError> {
    let len = data.len();
    if len < 64 {
        out.push(len as u8);
    } else if len < 16_384 {
        let hdr = 0x4000u16 | (len as u16);
        out.extend_from_slice(&hdr.to_be_bytes());
    } else if len <= 0x3FFF_FFFF {
        let hdr = 0x8000_0000u32 | (len as u32);
        out.extend_from_slice(&hdr.to_be_bytes());
    } else {
        return Err(DaveError::MalformedWire("VLBytes exceeds 30-bit length"));
    }
    out.extend_from_slice(data);
    Ok(())
}

/// Decodes one RFC 9420 §2.1.2 `VLBytes` slice and returns `(slice, remaining)`.
pub fn decode_vlbytes(input: &[u8]) -> Result<(&[u8], &[u8]), DaveError> {
    let (&first, _) = input
        .split_first()
        .ok_or(DaveError::MalformedWire("truncated VLBytes prefix"))?;
    let (hdr_len, len) = match first >> 6 {
        0 => (1usize, usize::from(first & 0x3F)),
        1 => {
            if input.len() < 2 {
                return Err(DaveError::MalformedWire("truncated 2-byte VLBytes"));
            }
            let n = (usize::from(first & 0x3F) << 8) | usize::from(input[1]);
            if n < 64 {
                return Err(DaveError::MalformedWire("non-minimal 2-byte VLBytes"));
            }
            (2, n)
        }
        2 => {
            if input.len() < 4 {
                return Err(DaveError::MalformedWire("truncated 4-byte VLBytes"));
            }
            let n = (usize::from(first & 0x3F) << 24)
                | (usize::from(input[1]) << 16)
                | (usize::from(input[2]) << 8)
                | usize::from(input[3]);
            if n < 16_384 {
                return Err(DaveError::MalformedWire("non-minimal 4-byte VLBytes"));
            }
            (4, n)
        }
        _ => return Err(DaveError::MalformedWire("invalid 0b11 VLBytes prefix")),
    };
    if input.len() < hdr_len + len {
        return Err(DaveError::MalformedWire("truncated VLBytes body"));
    }
    Ok((&input[hdr_len..hdr_len + len], &input[hdr_len + len..]))
}

pub fn read_u16_be(input: &[u8]) -> Result<(u16, &[u8]), DaveError> {
    if input.len() < 2 {
        return Err(DaveError::MalformedWire("truncated u16"));
    }
    Ok((u16::from_be_bytes([input[0], input[1]]), &input[2..]))
}

pub fn read_u64_be(input: &[u8]) -> Result<(u64, &[u8]), DaveError> {
    if input.len() < 8 {
        return Err(DaveError::MalformedWire("truncated u64"));
    }
    let mut arr = [0u8; 8];
    arr.copy_from_slice(&input[..8]);
    Ok((u64::from_be_bytes(arr), &input[8..]))
}

#[must_use]
pub const fn leb128_len(mut value: u64) -> usize {
    let mut count = 1usize;
    while value >= 0x80 {
        value >>= 7;
        count += 1;
    }
    count
}

pub fn write_leb128_u32(mut value: u32, out: &mut Vec<u8>) {
    while value >= 0x80 {
        out.push(0x80 | ((value as u8) & 0x7F));
        value >>= 7;
    }
    out.push(value as u8);
}

pub fn read_leb128_u32(input: &[u8]) -> Result<(u32, usize), DaveError> {
    let mut acc = 0u64;
    let mut shift = 0u32;
    for (idx, &byte) in input.iter().take(5).enumerate() {
        acc |= u64::from(byte & 0x7F) << shift;
        if (byte & 0x80) == 0 {
            if idx > 0 && byte == 0 {
                return Err(DaveError::MalformedWire("non-minimal leb128"));
            }
            let v =
                u32::try_from(acc).map_err(|_| DaveError::MalformedWire("leb128 u32 overflow"))?;
            return Ok((v, idx + 1));
        }
        shift += 7;
    }
    Err(DaveError::MalformedWire("unterminated leb128"))
}

/// Identifies Annex-B start code + 1-byte NAL header ranges kept clear as AAD for H.264 video.
#[must_use]
pub fn extract_h264_unencrypted_ranges(frame: &[u8]) -> Vec<UnencryptedRange> {
    let mut ranges = Vec::new();
    let mut i = 0usize;
    while i + 4 <= frame.len() {
        let sc_len = if frame[i..].starts_with(&[0, 0, 1]) {
            3
        } else if frame[i..].starts_with(&[0, 0, 0, 1]) {
            4
        } else {
            0
        };
        if sc_len > 0 && i + sc_len < frame.len() {
            ranges.push(UnencryptedRange {
                offset: i,
                size: sc_len + 1,
            });
            i += sc_len + 1;
        } else {
            i += 1;
        }
    }
    ranges
}

/// Checks that encrypted H.264 NAL slices do not accidentally synthesize Annex-B `00 00 00..03` sequences.
#[must_use]
pub fn ciphertext_contains_h26x_start_code(
    ranges: &[UnencryptedRange],
    reconstructed_payload: &[u8],
) -> bool {
    if ranges.is_empty() {
        return false;
    }
    let mut pos = 0usize;
    for idx in 0..=ranges.len() {
        let end = ranges
            .get(idx)
            .map_or(reconstructed_payload.len(), |r| r.offset);
        if end > pos + 2 {
            let slice = &reconstructed_payload[pos..end];
            if slice
                .windows(3)
                .any(|w| w[0] == 0 && w[1] == 0 && w[2] <= 3)
            {
                return true;
            }
        }
        if let Some(r) = ranges.get(idx) {
            pos = r.offset + r.size;
        }
    }
    false
}

/// Splits a raw frame by `ranges` into `(aad_clear_bytes, encryptable_bytes)`.
#[must_use]
pub fn split_frame_by_ranges(frame: &[u8], ranges: &[UnencryptedRange]) -> (Vec<u8>, Vec<u8>) {
    let mut aad = Vec::new();
    let mut plain = Vec::with_capacity(frame.len());
    let mut cursor = 0usize;
    for r in ranges {
        if r.offset > cursor {
            plain.extend_from_slice(&frame[cursor..r.offset]);
        }
        aad.extend_from_slice(&frame[r.offset..r.offset + r.size]);
        cursor = r.offset + r.size;
    }
    if cursor < frame.len() {
        plain.extend_from_slice(&frame[cursor..]);
    }
    (aad, plain)
}

/// Reconstructs `[clear AAD ranges + transformed bytes]` back into original frame positions.
#[must_use]
pub fn reconstruct_frame_from_ranges(
    ranges: &[UnencryptedRange],
    aad_bytes: &[u8],
    transformed_bytes: &[u8],
) -> Vec<u8> {
    let mut out = Vec::with_capacity(aad_bytes.len() + transformed_bytes.len());
    let mut aad_idx = 0usize;
    let mut tr_idx = 0usize;
    for r in ranges {
        if r.offset > out.len() {
            let need = r.offset - out.len();
            out.extend_from_slice(&transformed_bytes[tr_idx..tr_idx + need]);
            tr_idx += need;
        }
        out.extend_from_slice(&aad_bytes[aad_idx..aad_idx + r.size]);
        aad_idx += r.size;
    }
    if tr_idx < transformed_bytes.len() {
        out.extend_from_slice(&transformed_bytes[tr_idx..]);
    }
    out
}

/// Parsed DAVE v1 encrypted frame components before AEAD verification.
pub struct ParsedDaveFrame {
    pub aad: Vec<u8>,
    pub ciphertext: Vec<u8>,
    pub tag8: [u8; 8],
    pub truncated_nonce: u32,
    pub ranges: Vec<UnencryptedRange>,
}

/// Parses a DAVE v1 supplemental footer (`[payload][tag8][leb128 nonce][ranges...][size][0xFAFA]`).
pub fn parse_dave_supplemental_frame(frame: &[u8]) -> Result<ParsedDaveFrame, DaveError> {
    if frame.len() < 12 || frame[frame.len() - 2..] != DAVE_MAGIC_MARKER {
        return Err(DaveError::MalformedWire("missing DAVE 0xFAFA footer"));
    }
    let supp_size = usize::from(frame[frame.len() - 3]);
    if supp_size < 12 || supp_size > frame.len() {
        return Err(DaveError::MalformedWire("invalid DAVE supplemental length"));
    }
    let payload_len = frame.len() - supp_size;
    let supp = &frame[payload_len..];
    let mut tag8 = [0u8; 8];
    tag8.copy_from_slice(&supp[..8]);

    let meta = &supp[8..supp.len() - 3];
    let (truncated_nonce, used) = read_leb128_u32(meta)?;
    let mut rem = &meta[used..];
    let mut ranges = Vec::new();
    let mut prev_end = 0usize;
    while !rem.is_empty() {
        let (off, u1) = read_leb128_u32(rem)?;
        rem = &rem[u1..];
        let (sz, u2) = read_leb128_u32(rem)?;
        rem = &rem[u2..];
        let offset = off as usize;
        let size = sz as usize;
        if offset < prev_end || offset.saturating_add(size) > payload_len {
            return Err(DaveError::MalformedWire("invalid unencrypted range bounds"));
        }
        prev_end = offset + size;
        ranges.push(UnencryptedRange { offset, size });
    }
    let (aad, ciphertext) = split_frame_by_ranges(&frame[..payload_len], &ranges);
    Ok(ParsedDaveFrame {
        aad,
        ciphertext,
        tag8,
        truncated_nonce,
        ranges,
    })
}
