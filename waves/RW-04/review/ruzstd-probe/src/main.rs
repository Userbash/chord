//! Независимый зонд RW-04: проверка семантики удержания окна ruzstd 0.9.1.
//!
//! Режимы:
//! - `whole <вход.zst> <выход.bin>` — скормить весь сжатый поток одним куском;
//! - `frames <вход.hexl> <выход.bin>` — скормить кадр за кадром (как пайп RW-04).
//!
//! В обоих режимах используется тот же примитив `FrameDecoder`, но цикл собственный.

use ruzstd::decoding::FrameDecoder;
use std::fs;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mode = args.get(1).map(String::as_str).unwrap_or("whole");
    let input = &args[2];
    let output = &args[3];

    let mut decoder = FrameDecoder::new();
    let mut buf = vec![0u8; 64 * 1024];
    let mut out: Vec<u8> = Vec::new();
    let mut pending: Vec<u8> = Vec::new();

    let mut feed = |decoder: &mut FrameDecoder, pending: &mut Vec<u8>, chunk: &[u8], out: &mut Vec<u8>| {
        pending.extend_from_slice(chunk);
        loop {
            let before = decoder.bytes_read_from_source();
            let written = match decoder.decode_from_to(pending, &mut buf) {
                Ok((_reported, written)) => written,
                Err(error) => {
                    eprintln!("ruzstd error: {error}");
                    break;
                }
            };
            let delta = decoder.bytes_read_from_source().saturating_sub(before) as usize;
            if delta > 0 {
                pending.drain(..delta);
            }
            if written > 0 {
                out.extend_from_slice(&buf[..written]);
            }
            if delta == 0 && written == 0 {
                break;
            }
        }
    };

    if mode == "frames" {
        let text = fs::read_to_string(input).expect("hexl читается");
        for line in text.lines().filter(|l| !l.trim().is_empty()) {
            let compact: String = line.chars().filter(|c| !c.is_whitespace()).collect();
            let bytes: Vec<u8> = (0..compact.len() / 2)
                .map(|i| u8::from_str_radix(&compact[2 * i..2 * i + 2], 16).expect("hex"))
                .collect();
            feed(&mut decoder, &mut pending, &bytes, &mut out);
        }
    } else {
        let data = fs::read(input).expect("входные данные читаются");
        feed(&mut decoder, &mut pending, &data, &mut out);
    }

    println!(
        "режим {mode}: распаковано {} Б; decoder прочитал {} из {} сжатых; хвост pending {} Б; is_finished={}",
        out.len(),
        decoder.bytes_read_from_source(),
        fs::metadata(input).map(|m| m.len()).unwrap_or(0),
        pending.len(),
        decoder.is_finished()
    );
    fs::write(output, &out).expect("выход записывается");
}
