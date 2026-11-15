// Intentionally unsupported AVX2, isolated diagnostic only. Never application code.
fn main() { unsafe { core::arch::asm!("vpxor ymm0, ymm0, ymm0", out("ymm0") _, options(nomem, nostack)); } }
