//! Arbitrary-precision unsigned integer arithmetic (`BigUint`) over 32-bit limbs (`u32`).
//!
//! Provides constant-dependency big integer primitives required by RSA-OAEP-2048 (`SHA-256`)
//! key generation, encryption (`e = 65537`), decryption, and modular inversion in `remote_auth`.

#![forbid(unsafe_code)]

use std::cmp::Ordering;

/// Standard RSA public exponent (`2^16 + 1 = 65537`).
pub const RSA_PUBLIC_EXPONENT: u32 = 65_537;

/// Arbitrary-precision unsigned integer represented in little-endian base-$2^{32}$ limbs.
///
/// Canonical invariant: the most significant limb in `limbs` is never zero
/// (zero is represented by an empty `limbs` vector).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BigUint {
    limbs: Vec<u32>,
}

impl BigUint {
    /// Creates a [`BigUint`] from a single 32-bit unsigned integer.
    ///
    /// # Arguments
    /// * `val` - 32-bit unsigned integer value.
    ///
    /// # Returns
    /// Canonical [`BigUint`] representing `val`.
    #[inline]
    #[must_use]
    pub fn from_u32(val: u32) -> Self {
        if val == 0 {
            Self { limbs: Vec::new() }
        } else {
            Self { limbs: vec![val] }
        }
    }

    /// Decodes a big-endian byte slice into a canonical [`BigUint`].
    ///
    /// Leading zero bytes are stripped automatically to maintain the canonical limb invariant.
    ///
    /// # Arguments
    /// * `bytes` - Big-endian byte representation of the unsigned integer.
    ///
    /// # Returns
    /// Decoded [`BigUint`].
    #[inline]
    #[must_use]
    pub fn from_be_bytes(bytes: &[u8]) -> Self {
        let mut limbs = Vec::with_capacity(bytes.len().div_ceil(4));
        for chunk in bytes.rchunks(4) {
            let mut word = 0u32;
            for &b in chunk {
                word = (word << 8) | u32::from(b);
            }
            limbs.push(word);
        }
        let mut res = Self { limbs };
        res.trim();
        res
    }

    /// Encodes the integer as a big-endian byte vector left-padded with zeros to `len` bytes.
    ///
    /// If `len` is smaller than the byte length of the integer, the output is truncated to the
    /// least significant `len` bytes.
    ///
    /// # Arguments
    /// * `len` - Target length of the returned big-endian byte vector.
    ///
    /// # Returns
    /// Big-endian byte vector of exact length `len`.
    #[inline]
    #[must_use]
    pub fn to_be_bytes_padded(&self, len: usize) -> Vec<u8> {
        let mut out = vec![0u8; len];
        for (i, &word) in self.limbs.iter().enumerate() {
            let bytes = word.to_le_bytes();
            for (j, &b) in bytes.iter().enumerate() {
                let pos = i * 4 + j;
                if pos < len {
                    out[len - 1 - pos] = b;
                }
            }
        }
        out
    }

    /// Removes trailing zero limbs to preserve the canonical representation.
    #[inline]
    fn trim(&mut self) {
        while self.limbs.last() == Some(&0) {
            self.limbs.pop();
        }
    }

    /// Returns `true` if the integer is zero (`0`).
    #[inline]
    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.limbs.is_empty()
    }

    /// Returns `true` if the least significant bit is `1` (the integer is odd).
    #[inline]
    #[must_use]
    pub fn is_odd(&self) -> bool {
        self.limbs.first().is_some_and(|w| (w & 1) == 1)
    }

    /// Returns the minimum number of bits required to represent this integer (`0` for zero).
    #[inline]
    #[must_use]
    pub fn bit_len(&self) -> usize {
        match self.limbs.last() {
            None => 0,
            Some(&top) => (self.limbs.len() - 1) * 32 + (32 - top.leading_zeros() as usize),
        }
    }

    /// Tests whether the bit at `bit_idx` (0-indexed from least significant bit) is set.
    #[inline]
    #[must_use]
    pub fn get_bit(&self, bit_idx: usize) -> bool {
        let word_idx = bit_idx / 32;
        let bit_offset = bit_idx % 32;
        self.limbs
            .get(word_idx)
            .is_some_and(|&w| ((w >> bit_offset) & 1) == 1)
    }

    /// Compares `self` with `other` and returns their [`Ordering`].
    #[inline]
    #[must_use]
    pub fn cmp(&self, other: &Self) -> Ordering {
        if self.limbs.len() != other.limbs.len() {
            return self.limbs.len().cmp(&other.limbs.len());
        }
        for (&a, &b) in self.limbs.iter().rev().zip(other.limbs.iter().rev()) {
            if a != b {
                return a.cmp(&b);
            }
        }
        Ordering::Equal
    }

    /// Adds a 32-bit unsigned scalar `rhs` to `self`.
    #[inline]
    #[must_use]
    pub fn add_u32(&self, rhs: u32) -> Self {
        if self.is_zero() {
            return Self::from_u32(rhs);
        }
        let mut limbs = self.limbs.clone();
        let mut carry = u64::from(rhs);
        for w in &mut limbs {
            if carry == 0 {
                break;
            }
            let sum = u64::from(*w) + carry;
            *w = sum as u32;
            carry = sum >> 32;
        }
        if carry != 0 {
            limbs.push(carry as u32);
        }
        let mut res = Self { limbs };
        res.trim();
        res
    }

    /// Subtracts a 32-bit unsigned scalar `rhs` from `self` (`self >= rhs`).
    #[inline]
    #[must_use]
    pub fn sub_u32(&self, rhs: u32) -> Self {
        let mut limbs = self.limbs.clone();
        let mut borrow = i64::from(rhs);
        for w in &mut limbs {
            if borrow == 0 {
                break;
            }
            let diff = i64::from(*w) - borrow;
            if diff < 0 {
                *w = (diff + (1i64 << 32)) as u32;
                borrow = 1;
            } else {
                *w = diff as u32;
                borrow = 0;
            }
        }
        let mut res = Self { limbs };
        res.trim();
        res
    }

    /// Multiplies `self` by `rhs` using schoolbook $O(n \cdot m)$ limb multiplication.
    #[inline]
    #[must_use]
    pub fn mul(&self, rhs: &Self) -> Self {
        if self.is_zero() || rhs.is_zero() {
            return Self::default();
        }
        let mut out = vec![0u32; self.limbs.len() + rhs.limbs.len()];
        for (i, &a) in self.limbs.iter().enumerate() {
            let mut carry = 0u64;
            for (j, &b) in rhs.limbs.iter().enumerate() {
                let prod = u64::from(a) * u64::from(b) + u64::from(out[i + j]) + carry;
                out[i + j] = prod as u32;
                carry = prod >> 32;
            }
            out[i + rhs.limbs.len()] = carry as u32;
        }
        let mut res = Self { limbs: out };
        res.trim();
        res
    }

    /// Multiplies `self` by a 32-bit unsigned scalar `rhs`.
    #[inline]
    #[must_use]
    pub fn mul_u32(&self, rhs: u32) -> Self {
        if self.is_zero() || rhs == 0 {
            return Self::default();
        }
        let mut limbs = Vec::with_capacity(self.limbs.len() + 1);
        let mut carry = 0u64;
        for &w in &self.limbs {
            let prod = u64::from(w) * u64::from(rhs) + carry;
            limbs.push(prod as u32);
            carry = prod >> 32;
        }
        if carry != 0 {
            limbs.push(carry as u32);
        }
        Self { limbs }
    }

    /// Divides `self` by a non-zero 32-bit unsigned `divisor`, returning `(quotient, remainder)`.
    #[inline]
    #[must_use]
    pub fn div_rem_u32(&self, divisor: u32) -> (Self, u32) {
        let mut quot = vec![0u32; self.limbs.len()];
        let mut rem = 0u64;
        let div = u64::from(divisor);
        for (i, &w) in self.limbs.iter().enumerate().rev() {
            let cur = (rem << 32) | u64::from(w);
            quot[i] = (cur / div) as u32;
            rem = cur % div;
        }
        let mut res = Self { limbs: quot };
        res.trim();
        (res, rem as u32)
    }

    /// Computes `(quotient, remainder)` of `self / divisor` using Knuth's Algorithm D (base $2^{32}$).
    ///
    /// # Panics
    /// Panics if `divisor` is zero.
    #[inline]
    #[must_use]
    pub fn div_mod(&self, divisor: &Self) -> (Self, Self) {
        assert!(!divisor.is_zero(), "BigUint division by zero");
        if self.cmp(divisor) == Ordering::Less {
            return (Self::default(), self.clone());
        }
        let n = divisor.limbs.len();
        if n == 1 {
            let (q, r) = self.div_rem_u32(divisor.limbs[0]);
            return (q, Self::from_u32(r));
        }

        let m = self.limbs.len();
        let shift = divisor.limbs[n - 1].leading_zeros();
        let v = divisor.shl_bits(shift);
        let mut u = self.shl_bits(shift);
        u.limbs.resize(m + 1, 0);
        let vn1 = u64::from(v.limbs[n - 1]);
        let vn2 = u64::from(v.limbs[n - 2]);
        let mut quot = vec![0u32; m - n + 1];

        for j in (0..=(m - n)).rev() {
            let uj_n = u64::from(u.limbs[j + n]);
            let uj_n1 = u64::from(u.limbs[j + n - 1]);
            let uj_n2 = u64::from(u.limbs[j + n - 2]);
            let dividend = (uj_n << 32) | uj_n1;
            let mut qhat = dividend / vn1;
            let mut rhat = dividend % vn1;
            while qhat >= (1u64 << 32) || (qhat * vn2 > ((rhat << 32) | uj_n2)) {
                qhat -= 1;
                rhat += vn1;
                if rhat >= (1u64 << 32) {
                    break;
                }
            }

            if qhat == 0 {
                continue;
            }

            let mut k = 0i128;
            for i in 0..n {
                let p = i128::from(qhat) * i128::from(v.limbs[i]);
                let t = i128::from(u.limbs[j + i]) - k - (p & 0xffff_ffff);
                u.limbs[j + i] = t as u32;
                k = (p >> 32) - (t >> 32);
            }
            let t = i128::from(u.limbs[j + n]) - k;
            u.limbs[j + n] = t as u32;
            if t < 0 {
                qhat -= 1;
                let mut carry = 0u64;
                for i in 0..n {
                    let sum = u64::from(u.limbs[j + i]) + u64::from(v.limbs[i]) + carry;
                    u.limbs[j + i] = sum as u32;
                    carry = sum >> 32;
                }
                u.limbs[j + n] = u.limbs[j + n].wrapping_add(carry as u32);
            }
            quot[j] = qhat as u32;
        }

        let mut q = Self { limbs: quot };
        q.trim();
        u.limbs.truncate(n);
        u.trim();
        let r = u.shr_bits(shift);
        (q, r)
    }

    /// Computes the modular reduction `self % modulus` using Knuth's Algorithm D (base $2^{32}$).
    #[inline]
    #[must_use]
    pub fn rem(&self, modulus: &Self) -> Self {
        self.div_mod(modulus).1
    }

    /// Shifts `self` left by `shift` bits (`0 <= shift < 32`).
    #[inline]
    #[must_use]
    pub fn shl_bits(&self, shift: u32) -> Self {
        if shift == 0 || self.is_zero() {
            return self.clone();
        }
        let mut limbs = Vec::with_capacity(self.limbs.len() + 1);
        let mut carry = 0u32;
        for &w in &self.limbs {
            limbs.push((w << shift) | carry);
            carry = w >> (32 - shift);
        }
        if carry != 0 {
            limbs.push(carry);
        }
        Self { limbs }
    }

    /// Shifts `self` right by `shift` bits (`0 <= shift < 32`).
    #[inline]
    #[must_use]
    pub fn shr_bits(&self, shift: u32) -> Self {
        if shift == 0 || self.is_zero() {
            return self.clone();
        }
        let mut limbs = vec![0u32; self.limbs.len()];
        let mut carry = 0u32;
        for (i, &w) in self.limbs.iter().enumerate().rev() {
            limbs[i] = (w >> shift) | carry;
            carry = w << (32 - shift);
        }
        let mut res = Self { limbs };
        res.trim();
        res
    }

    /// Computes modular exponentiation `self^exp mod modulus` using a 4-bit fixed window.
    #[inline]
    #[must_use]
    pub fn mod_pow(&self, exp: &Self, modulus: &Self) -> Self {
        if exp.is_zero() {
            return Self::from_u32(1);
        }
        let base = self.rem(modulus);
        let mut table = Vec::with_capacity(16);
        table.push(Self::from_u32(1));
        table.push(base.clone());
        for i in 2..16 {
            let next = table[i - 1].mul(&base).rem(modulus);
            table.push(next);
        }

        let total_bits = exp.bit_len();
        let top_nibble = total_bits.div_ceil(4);
        let mut acc = Self::from_u32(1);

        for nibble_idx in (0..top_nibble).rev() {
            for _ in 0..4 {
                acc = acc.mul(&acc).rem(modulus);
            }
            let base_bit = nibble_idx * 4;
            let mut win = 0usize;
            for b in 0..4 {
                if exp.get_bit(base_bit + b) {
                    win |= 1 << b;
                }
            }
            if win != 0 {
                acc = acc.mul(&table[win]).rem(modulus);
            }
        }
        acc
    }

    /// Computes `self^65537 mod modulus` (`e = 2^16 + 1`) via 16 modular squarings and 1 multiplication.
    #[inline]
    #[must_use]
    pub fn mod_pow_65537(&self, modulus: &Self) -> Self {
        let base = self.rem(modulus);
        let mut acc = base.clone();
        for _ in 0..16 {
            acc = acc.mul(&acc).rem(modulus);
        }
        acc.mul(&base).rem(modulus)
    }
}

/// Computes the modular multiplicative inverse `65537^-1 mod phi` using the identity
/// `65537 * d = k * phi + 1`, where `k = (65537 - (phi mod 65537)^-1) mod 65537`.
///
/// # Arguments
/// * `phi` - Euler's totient $\varphi(n) = (p - 1)(q - 1)$ or Carmichael factor.
///
/// # Returns
/// `Some(d)` if $\gcd(65537, \varphi) = 1$, or `None` if not coprime.
#[inline]
#[must_use]
pub fn invert_65537_mod(phi: &BigUint) -> Option<BigUint> {
    const E: i64 = 65_537;
    let (_, rem_u32) = phi.div_rem_u32(RSA_PUBLIC_EXPONENT);
    if rem_u32 == 0 {
        return None;
    }

    let (mut t, mut new_t) = (0i64, 1i64);
    let (mut r, mut new_r) = (E, i64::from(rem_u32));
    while new_r != 0 {
        let q = r / new_r;
        (t, new_t) = (new_t, t - q * new_t);
        (r, new_r) = (new_r, r - q * new_r);
    }
    if r != 1 {
        return None;
    }
    if t < 0 {
        t += E;
    }
    let k = ((E - t) % E) as u32;
    let num = phi.mul_u32(k).add_u32(1);
    let (d, zero_rem) = num.div_rem_u32(RSA_PUBLIC_EXPONENT);
    debug_assert_eq!(zero_rem, 0);
    Some(d)
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[test]
    fn from_be_bytes() {
        let zero = BigUint::from_be_bytes(&[0, 0, 0, 0, 0]);
        assert!(zero.is_zero());
        assert_eq!(zero.bit_len(), 0);

        let raw = [0x00, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05];
        let val = BigUint::from_be_bytes(&raw);
        assert!(!val.is_zero());
        assert!(val.is_odd());
        assert_eq!(val.bit_len(), 33);
        assert_eq!(val.to_be_bytes_padded(5), vec![0x01, 0x02, 0x03, 0x04, 0x05]);
    }

    #[test]
    fn to_be_bytes_padded() {
        let val = BigUint::from_be_bytes(&[0xde, 0xad, 0xbe, 0xef]);
        assert_eq!(
            val.to_be_bytes_padded(8),
            vec![0x00, 0x00, 0x00, 0x00, 0xde, 0xad, 0xbe, 0xef]
        );
        assert_eq!(val.to_be_bytes_padded(4), vec![0xde, 0xad, 0xbe, 0xef]);
        assert_eq!(BigUint::default().to_be_bytes_padded(4), vec![0, 0, 0, 0]);
    }

    #[test]
    fn mul() {
        let a = BigUint::from_be_bytes(&[0xff, 0xff, 0xff, 0xff, 0x12, 0x34, 0x56, 0x78]);
        let b = BigUint::from_be_bytes(&[0x01, 0x00, 0x00, 0x00, 0x01]);
        let prod = a.mul(&b);
        // a * (2^32 + 1) = (a << 32) + a
        let expected = BigUint::from_be_bytes(&[
            0x01, 0x00, 0x00, 0x00, 0x00, 0x12, 0x34, 0x56, 0x77, 0x12, 0x34, 0x56, 0x78,
        ]);
        assert_eq!(prod, expected);
        assert!(a.mul(&BigUint::default()).is_zero());
    }

    #[test]
    fn div_mod() {
        let a = BigUint::from_be_bytes(&[
            0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66,
            0x77, 0x88,
        ]);
        let b = BigUint::from_be_bytes(&[0x00, 0xf1, 0xe2, 0xd3, 0xc4, 0xb5, 0xa6, 0x97, 0x88]);
        let (q, r) = a.div_mod(&b);
        assert_eq!(r.cmp(&b), Ordering::Less);
        assert_eq!(r, a.rem(&b));
        // Verify q * b + r == a
        let qb = q.mul(&b);
        let qb_bytes = qb.to_be_bytes_padded(16);
        let r_bytes = r.to_be_bytes_padded(16);
        let mut reconstructed = [0u8; 16];
        let mut carry = 0u16;
        for i in (0..16).rev() {
            let sum = u16::from(qb_bytes[i]) + u16::from(r_bytes[i]) + carry;
            reconstructed[i] = sum as u8;
            carry = sum >> 8;
        }
        assert_eq!(carry, 0);
        assert_eq!(BigUint::from_be_bytes(&reconstructed), a);
    }

    #[test]
    fn mod_pow_65537() {
        let base = BigUint::from_be_bytes(&[0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf1]);
        let modulus = BigUint::from_be_bytes(&[
            0xf1, 0x83, 0xbe, 0xe4, 0x20, 0x7e, 0x92, 0xc9, 0x32, 0xcc, 0xc9, 0x1f, 0x6c, 0xdb,
            0xcc, 0x31,
        ]);
        let exp = BigUint::from_u32(RSA_PUBLIC_EXPONENT);
        let fast = base.mod_pow_65537(&modulus);
        let windowed = base.mod_pow(&exp, &modulus);
        assert_eq!(fast, windowed);
        assert_eq!(fast.cmp(&modulus), Ordering::Less);
    }
}
