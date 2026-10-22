//! Микробенчмарк ETF для CI-smoke (P8-03 / T-HARN-06).
//!
//! Не заменяет T-HARN-04: порог регрессии >10% и история — отдельная карточка P7-02.
//! Здесь фиксируется воспроизводимый smoke: encode+decode типичного gateway-терма.
//! Запуск: `cargo run --offline --release -p discord-codec --example etf_bench -- --iterations 2000`.

use std::hint::black_box;
use std::time::Instant;

use discord_codec::{EtfValue, decode, encode_checked};

fn sample() -> EtfValue {
    EtfValue::Map(vec![
        (EtfValue::Atom("op".into()), EtfValue::Int(dispatch_op())),
        (
            EtfValue::Atom("d".into()),
            EtfValue::Map(vec![
                (
                    EtfValue::Atom("heartbeat_interval".into()),
                    EtfValue::Int(13_750),
                ),
                (
                    EtfValue::Atom("modes".into()),
                    EtfValue::List(vec![
                        EtfValue::Str("aead_aes256_gcm_rtpsize".into()),
                        EtfValue::Str("aead_xchacha20_poly1305_rtpsize".into()),
                    ]),
                ),
            ]),
        ),
    ])
}

fn dispatch_op() -> i64 {
    2
}

fn main() {
    let mut iterations = 2_000usize;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--iterations" => {
                iterations = args
                    .next()
                    .and_then(|value| value.parse().ok())
                    .filter(|value| *value > 0)
                    .expect("--iterations требует положительное число");
            }
            "-h" | "--help" => {
                println!("etf_bench [--iterations N]");
                return;
            }
            other => panic!("неизвестный аргумент: {other}"),
        }
    }

    let value = sample();
    let golden = encode_checked(&value).expect("sample encode");
    let started = Instant::now();
    let mut total_bytes = 0usize;
    for _ in 0..iterations {
        let bytes = encode_checked(black_box(&value)).expect("encode");
        total_bytes = total_bytes.saturating_add(bytes.len());
        let (decoded, used) = decode(black_box(&bytes)).expect("decode");
        assert_eq!(used, bytes.len(), "decode consumed");
        black_box(decoded);
    }
    let elapsed = started.elapsed();
    let nanos = elapsed.as_nanos().max(1);
    let ops_per_sec = (iterations as u128).saturating_mul(1_000_000_000) / nanos;
    println!(
        "etf_bench iterations={iterations} golden_bytes={} total_bytes={total_bytes} elapsed_ms={} ops_per_sec={ops_per_sec}",
        golden.len(),
        elapsed.as_millis()
    );
}
