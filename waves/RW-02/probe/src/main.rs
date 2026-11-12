// Пробник алгоритма «push-стрим на FrameDecoder» (+ скан порчи кадра) — подготовка ТЗ RW-02.
// Запуск: cargo run --offline -q -- <hexl-1> [<hexl-2>]
//         cargo run --offline -q -- --scan <hexl>
use ruzstd::decoding::FrameDecoder;
use std::fs;

fn unhex(s: &str) -> Vec<u8> {
    let s: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    (0..s.len() / 2)
        .map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap())
        .collect()
}

fn load_frames(path: &str) -> Vec<Vec<u8>> {
    fs::read_to_string(path)
        .unwrap()
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(unhex)
        .collect()
}

/// Ядро: накопление + цикл decode_from_to; возвращает суммарно выданное.
fn feed(
    dec: &mut FrameDecoder,
    pending: &mut Vec<u8>,
    data: &[u8],
    target: &mut [u8],
    first: &mut Vec<u8>,
) -> Result<usize, String> {
    pending.extend_from_slice(data);
    let mut out_total = 0usize;
    let mut guard = 0;
    loop {
        guard += 1;
        if guard > 100_000 {
            return Err("предохранитель цикла".into());
        }
        if pending.is_empty() {
            break;
        }
        let before = dec.bytes_read_from_source();
        match dec.decode_from_to(&pending[..], target) {
            Ok((_r, w)) => {
                let adv = (dec.bytes_read_from_source() - before) as usize;
                if adv > 0 {
                    pending.drain(..adv);
                }
                if w > 0 {
                    if first.len() < 300 {
                        let n = (300 - first.len()).min(w);
                        first.extend_from_slice(&target[..n]);
                    }
                    out_total += w;
                }
                if adv == 0 && w == 0 {
                    break;
                }
            }
            Err(e) => return Err(format!("{e:?}")),
        }
    }
    Ok(out_total)
}

fn run(path: &str) {
    let frames = load_frames(path);
    let magic = [0x28u8, 0xb5, 0x2f, 0xfd];
    println!("== {path}: кадров {}", frames.len());
    println!(
        "   магия кадр0: {} | кадры 1+: {}",
        frames[0].starts_with(&magic),
        frames[1..].iter().filter(|f| f.starts_with(&magic)).count()
    );

    let mut dec = FrameDecoder::new();
    let mut pending: Vec<u8> = Vec::new();
    let mut target = vec![0u8; 256 * 1024];
    let mut first = Vec::<u8>::new();
    let mut out_total = 0usize;
    for (i, f) in frames.iter().enumerate() {
        match feed(&mut dec, &mut pending, f, &mut target, &mut first) {
            Ok(n) => out_total += n,
            Err(e) => {
                println!("   ОШИБКА на кадре {i}: {e}");
                break;
            }
        }
    }
    let hx: String = first.iter().map(|b| format!("{b:02x}")).collect();
    println!("   потреблено сжатых: {}", dec.bytes_read_from_source());
    println!("   выдано распакованных: {out_total}");
    println!("   can_collect: {}", dec.can_collect());
    println!("   первые байты: {hx}");
}

fn scan(path: &str) {
    let frames = load_frames(path);
    let orig = &frames[0];
    println!("скан порчи: {path} ({} Б)", orig.len());
    for i in 0..orig.len() {
        let mut f = orig.clone();
        f[i] ^= 0xff;
        let mut dec = FrameDecoder::new();
        let mut pending: Vec<u8> = Vec::new();
        let mut target = vec![0u8; 256 * 1024];
        let mut first = Vec::<u8>::new();
        match feed(&mut dec, &mut pending, &f, &mut target, &mut first) {
            Ok(n) => println!("  idx {i:2}: Ok(выдано {n})"),
            Err(e) => println!("  idx {i:2}: Err({e})"),
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("--scan") {
        scan(args.get(2).expect("использование: --scan <hexl>"));
        return;
    }
    let a = args
        .get(1)
        .cloned()
        .unwrap_or_else(|| "/home/user/clean-room-oracle/gw-proto/zstd-frames-95974_1389.hexl".into());
    run(&a);
    if let Some(b) = args.get(2) {
        run(b);
    }
}
