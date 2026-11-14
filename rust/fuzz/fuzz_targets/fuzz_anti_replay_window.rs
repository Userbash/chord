#![no_main]
//! Фаззинг `AntiReplayWindow` и арифметики поколений DAVE v1 на переполнениях `u64`/`u32`
//! со сверкой каждого шага против эталонной математической модели на `HashSet<u64>`.

use arbitrary::{Arbitrary, Result as ArbResult, Unstructured};
use chord_voice::dave_v1::{
    AntiReplayWindow, DAVE_REPLAY_WINDOW_SIZE, ReplayError, dave_compute_wrapped_big_nonce,
    dave_compute_wrapped_generation,
};
use libfuzzer_sys::fuzz_target;
use std::collections::HashSet;

#[derive(Debug)]
enum NonceAction {
    /// Произвольный 64-битный nonce.
    Raw(u64),
    /// Значение вблизи верхней границы `u64::MAX - delta` (тестирование переполнений).
    NearMax(u16),
    /// Значение вблизи нуля `0..=65535`.
    NearZero(u16),
    /// Относительное смещение от текущего максимума `highest +/- offset`.
    Relative(i16),
    /// Проверка развёртки поколения `(oldest_u32, generation_u8, truncated_nonce)`.
    WrappedGen {
        oldest: u32,
        gen_u8: u8,
        truncated_nonce: u32,
    },
}

impl<'a> Arbitrary<'a> for NonceAction {
    fn arbitrary(u: &mut Unstructured<'a>) -> ArbResult<Self> {
        match u.int_in_range(0u8..=4)? {
            0 => Ok(Self::Raw(u.arbitrary()?)),
            1 => Ok(Self::NearMax(u.arbitrary()?)),
            2 => Ok(Self::NearZero(u.arbitrary()?)),
            3 => Ok(Self::Relative(u.arbitrary()?)),
            _ => Ok(Self::WrappedGen {
                oldest: u.arbitrary()?,
                gen_u8: u.arbitrary()?,
                truncated_nonce: u.arbitrary()?,
            }),
        }
    }
}

fuzz_target!(|data: &[u8]| {
    let mut u = Unstructured::new(data);
    let mut window = AntiReplayWindow::new();
    let mut seen = HashSet::new();
    let mut ref_highest: Option<u64> = None;

    for _ in 0..128 {
        let Ok(action) = NonceAction::arbitrary(&mut u) else {
            break;
        };
        let nonce = match action {
            NonceAction::Raw(v) => v,
            NonceAction::NearMax(d) => u64::MAX - u64::from(d),
            NonceAction::NearZero(v) => u64::from(v),
            NonceAction::Relative(delta) => {
                let base = ref_highest.unwrap_or(1_024);
                if delta >= 0 {
                    base.saturating_add(delta as u64)
                } else {
                    base.saturating_sub(delta.unsigned_abs() as u64)
                }
            }
            NonceAction::WrappedGen {
                oldest,
                gen_u8,
                truncated_nonce,
            } => {
                let wrapped_gen = dave_compute_wrapped_generation(oldest, u32::from(gen_u8));
                assert!(wrapped_gen >= oldest);
                dave_compute_wrapped_big_nonce(wrapped_gen, truncated_nonce)
            }
        };

        let expected = match ref_highest {
            None => Ok(()),
            Some(h) if nonce > h => Ok(()),
            Some(h) if h - nonce >= DAVE_REPLAY_WINDOW_SIZE => Err(ReplayError::ExpiredNonce {
                nonce,
                oldest_allowed: h.saturating_sub(DAVE_REPLAY_WINDOW_SIZE - 1),
            }),
            Some(_) if seen.contains(&nonce) => Err(ReplayError::DuplicateNonce(nonce)),
            Some(_) => Ok(()),
        };

        assert_eq!(window.can_accept(nonce), expected.is_ok());
        let actual = window.accept(nonce);
        assert_eq!(actual, expected);

        if expected.is_ok() {
            seen.insert(nonce);
            let next_h = Some(ref_highest.map_or(nonce, |h| h.max(nonce)));
            ref_highest = next_h;
            if let Some(h) = ref_highest {
                let min_kept = h.saturating_sub(DAVE_REPLAY_WINDOW_SIZE - 1);
                seen.retain(|&n| n >= min_kept);
            }
        }
        assert_eq!(window.highest_nonce(), ref_highest);
    }
});
