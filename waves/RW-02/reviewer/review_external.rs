//! Внешние тесты ревьюера RW-02 — ТОЛЬКО копия (`/tmp/opencode/review-rw-02/ws`).
//! В репозиторий не входит. Проверки идут через публичный API крейта и независимые оракулы.
use discord_codec::{ZstdError, ZstdStream};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

const MAGIC: [u8; 4] = [0x28, 0xb5, 0x2f, 0xfd];
const TERM_HEX: &str = "8374000000036400026f70610a640001736400036e696c6400017464000450494e47";

fn unhex(s: &str) -> Vec<u8> {
    let c: String = s.chars().filter(|ch| !ch.is_whitespace()).collect();
    (0..c.len() / 2)
        .map(|i| u8::from_str_radix(&c[2 * i..2 * i + 2], 16).expect("hex"))
        .collect()
}

fn check(counter: &mut usize, cond: bool, what: &str) {
    *counter += 1;
    assert!(cond, "{what}");
}

fn synth_frame() -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/synth-zstd-frame.hexl");
    unhex(&fs::read_to_string(&path).expect("hexl"))
}

fn synth_payload() -> Vec<u8> {
    unhex(TERM_HEX).repeat(4096)
}

fn lab_root() -> PathBuf {
    PathBuf::from(std::env::var("RE_LAB_ROOT").unwrap_or_else(|_| "/home/sanya/re-deep".into()))
}

fn load_lab(path: &Path) -> Option<Vec<Vec<u8>>> {
    match fs::read_to_string(path) {
        Ok(t) => Some(
            t.lines()
                .filter(|l| !l.trim().is_empty())
                .map(unhex)
                .collect(),
        ),
        Err(e) if e.kind() == ErrorKind::NotFound => None,
        Err(e) => panic!("lab {}: {e}", path.display()),
    }
}

#[test]
fn ext_synth_whole_and_5b_chunks() {
    let mut n = 0usize;
    let frame = synth_frame();
    let payload = synth_payload();
    let mut s = ZstdStream::new();
    let out = s.push(&frame).expect("целый кадр");
    check(&mut n, out == payload, "целый кадр == payload");
    check(&mut n, s.is_finished(), "is_finished");
    check(&mut n, s.compressed_consumed() == 67, "consumed == 67");
    check(&mut n, s.push(&[]).expect("пустой push").is_empty(), "пустой push");
    let mut s = ZstdStream::new();
    let mut out = Vec::new();
    let mut i = 0;
    while i < frame.len() {
        let end = (i + 5).min(frame.len());
        out.extend(s.push(&frame[i..end]).expect("кусок по 5"));
        i = end;
    }
    check(&mut n, i == 67, "покрыто 67 Б кусками по 5");
    check(&mut n, out == payload, "куски по 5 Б == payload");
    eprintln!("ext_synth_whole_and_5b_chunks: checks={n}");
}

#[test]
fn ext_lcg_garbage_300() {
    let mut n = 0usize;
    let (mut ok, mut miss, mut dec) = (0usize, 0usize, 0usize);
    let mut x: u32 = 0x9e37_79b9;
    for _ in 0..300 {
        x = x.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        let len = (x as usize) % 257;
        let mut buf = vec![0u8; len];
        let mut y = x;
        for b in buf.iter_mut() {
            y = y.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            *b = (y >> 24) as u8;
        }
        let (magic_prefix, buf_len, empty_ok) = (buf.starts_with(&MAGIC), buf.len(), buf.len() < 4);
        let mut s = ZstdStream::new();
        match s.push(&buf) {
            Ok(v) => {
                ok += 1;
                check(&mut n, v.is_empty() && (empty_ok || !magic_prefix), "Ok только для <4 Б или магии");
            }
            Err(ZstdError::MissingMagic) => {
                miss += 1;
                check(&mut n, buf_len >= 4 && !magic_prefix, "MissingMagic при >=4 Б без магии");
            }
            Err(ZstdError::Decode { .. }) => {
                dec += 1;
                check(&mut n, magic_prefix, "Decode только если мусор начинался с магии");
            }
            Err(other) => panic!("неожиданный TrailingData на свежем потоке: {other:?}"),
        }
    }
    check(&mut n, ok + miss + dec == 300, "всего 300 входов");
    eprintln!("ext_lcg_garbage_300: checks={n} ok={ok} missing_magic={miss} decode={dec}");
}

#[test]
fn ext_all_prefixes_of_frame() {
    let mut n = 0usize;
    let frame = synth_frame();
    let payload = synth_payload();
    for len in 0..=frame.len() {
        let mut s = ZstdStream::new();
        match s.push(&frame[..len]) {
            Ok(v) => check(
                &mut n,
                payload.starts_with(&v),
                &format!("префикс {len}: выданное — префикс payload"),
            ),
            Err(e) => panic!("префикс {len}: неожиданная ошибка {e:?}"),
        }
    }
    check(&mut n, frame.len() == 67, "кадр 67 Б");
    eprintln!("ext_all_prefixes_of_frame: checks={n}");
}

#[test]
fn ext_two_frames_concat() {
    let mut n = 0usize;
    let frame = synth_frame();
    let payload = synth_payload();
    let mut s = ZstdStream::new();
    check(&mut n, s.push(&frame).expect("первый") == payload, "первый push == payload");
    match s.push(&frame) {
        Err(ZstdError::TrailingData { bytes }) => check(&mut n, bytes == 67, "второй push → TrailingData{{67}}"),
        other => panic!("второй push: ожидался TrailingData{{67}}, получено {other:?}"),
    }
    let mut s = ZstdStream::new();
    let mut cat = frame.clone();
    cat.extend_from_slice(&frame);
    match s.push(&cat) {
        Err(ZstdError::TrailingData { bytes }) => check(&mut n, bytes == 67, "склейка двумя кадрами → TrailingData{{67}}"),
        other => panic!("склейка: ожидался TrailingData{{67}}, получено {other:?}"),
    }
    eprintln!("ext_two_frames_concat: checks={n}");
}

#[test]
fn ext_after_decode_error() {
    let mut n = 0usize;
    let mut frame = synth_frame();
    frame[12] ^= 0xff;
    let mut s = ZstdStream::new();
    match s.push(&frame) {
        Err(ZstdError::Decode { msg }) => check(&mut n, !msg.is_empty(), "Decode с сообщением"),
        other => panic!("ожидался Decode, получено {other:?}"),
    }
    // Наблюдение: что будет при push валидного кадра после терминальной ошибки.
    let valid = synth_frame();
    match s.push(&valid) {
        Ok(v) => eprintln!("ext_after_decode_error: push после Err → Ok(len={}) — НЕ терминально!", v.len()),
        Err(e) => eprintln!("ext_after_decode_error: push после Err → Err({e:?}) — терминально (как в доккомментарии)"),
    }
    eprintln!("ext_after_decode_error: checks={n}");
}

#[test]
fn ext_fresh_stream_quirks() {
    let mut n = 0usize;
    let mut s = ZstdStream::new();
    check(&mut n, s.push(&[]).expect("пустой push на свежем").is_empty(), "свежий + [] → Ok(пусто)");
    check(&mut n, s.compressed_consumed() == 0, "consumed свежего == 0");
    check(&mut n, s.is_finished(), "оговорка: is_finished() свежего == true (ruzstd)");
    eprintln!("ext_fresh_stream_quirks: checks={n}");
}

#[test]
fn ext_lab_1389_real() {
    let path = lab_root().join("gw-proto/zstd-frames-95974_1389.hexl");
    let Some(frames) = load_lab(&path) else {
        eprintln!("skip: нет lab-файла {}", path.display());
        return;
    };
    let mut n = 0usize;
    check(&mut n, frames.len() == 137, "137 кадров");
    let mut s = ZstdStream::new();
    let mut out = Vec::new();
    for (i, f) in frames.iter().enumerate() {
        out.extend(s.push(f).unwrap_or_else(|e| panic!("кадр {i}: {e}")));
    }
    check(&mut n, out.len() == 759_280, "out.len() == 759 280");
    check(&mut n, s.compressed_consumed() == 104_066, "consumed == 104 066");
    check(&mut n, !s.is_finished(), "!is_finished()");
    eprintln!("ext_lab_1389_real: checks={n}");
}

#[test]
fn ext_lab_8158_midstream() {
    let path = lab_root().join("gw-proto/zstd-frames-95974_8158.hexl");
    let Some(frames) = load_lab(&path) else {
        eprintln!("skip: нет lab-файла {}", path.display());
        return;
    };
    let mut n = 0usize;
    check(&mut n, frames.len() == 1 && frames[0].len() == 74, "1 кадр 74 Б");
    check(&mut n, !frames[0].starts_with(&MAGIC), "без магии");
    let mut s = ZstdStream::new();
    match s.push(&frames[0]) {
        Err(ZstdError::MissingMagic) => check(&mut n, true, "MissingMagic"),
        other => panic!("ожидался MissingMagic, получено {other:?}"),
    }
    eprintln!("ext_lab_8158_midstream: checks={n}");
}
