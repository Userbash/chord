// Временный пробник RW-02: сравнение двух условий выхода цикла при буфере выдачи 64 КиБ.
// (1) literal — «pending пуст → стоп» (как в ТЗ, буквально); (2) progress — стоп при нулевом прогрессе.
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

fn feed(
    dec: &mut FrameDecoder,
    pending: &mut Vec<u8>,
    data: &[u8],
    target: &mut [u8],
    literal: bool,
) -> Result<usize, String> {
    pending.extend_from_slice(data);
    let mut out_total = 0usize;
    loop {
        if literal && pending.is_empty() {
            break;
        }
        if dec.bytes_read_from_source() == 0 && pending.len() < 18 {
            break;
        }
        let before = dec.bytes_read_from_source();
        match dec.decode_from_to(&pending[..], target) {
            Ok((_r, w)) => {
                let adv = (dec.bytes_read_from_source() - before) as usize;
                if adv > 0 {
                    pending.drain(..adv);
                }
                out_total += w;
                if adv == 0 && w == 0 {
                    break;
                }
            }
            Err(e) => return Err(format!("{e:?}")),
        }
    }
    Ok(out_total)
}

fn run(path: &str, literal: bool) -> (u64, usize, bool) {
    let frames = load_frames(path);
    let mut dec = FrameDecoder::new();
    let mut pending: Vec<u8> = Vec::new();
    let mut target = vec![0u8; 64 * 1024];
    let mut out_total = 0usize;
    for f in &frames {
        match feed(&mut dec, &mut pending, f, &mut target, literal) {
            Ok(n) => out_total += n,
            Err(e) => {
                eprintln!("   ОШИБКА: {e}");
                break;
            }
        }
    }
    (dec.bytes_read_from_source(), out_total, dec.is_finished())
}

fn main() {
    for path in [
        "/home/user/clean-room-oracle/gw-proto/synth-zstd-frame.hexl",
        "/home/user/clean-room-oracle/gw-proto/zstd-frames-95974_1389.hexl",
    ] {
        println!("== {path}");
        for (name, literal) in [("literal(pending пуст → стоп)", true), ("progress(нулевой прогресс → стоп)", false)] {
            let (consumed, out, finished) = run(path, literal);
            println!(
                "   {name}: сжатых {consumed}, распаковано {out}, finished {finished}"
            );
        }
    }
}
