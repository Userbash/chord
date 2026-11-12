//! Инкрементальный декодер zstd-потока шлюза Discord (волна RW-02, P0-03 / T-ENC-04).
//!
//! **Провенанс оракулов** (независимые артефакты, не подгонка под реализацию):
//! - синтетический кадр `tests/data/synth-zstd-frame.hexl` (67 Б) собран Node-скриптом
//!   `/home/user/clean-room-oracle/gw-make-synth-zstd.mjs` (`zstdCompressSync`, payload без данных
//!   Discord); md5 файла `0a7742ae83a58088f3f80a16be77ad83`; распаковка — терм 34 Б × 4096 =
//!   139 264 Б, md5 payload `4dbd601d877520056a7bcc3ca534ad4a`;
//! - lab-поток `{RE_LAB_ROOT:-/home/user/clean-room-oracle}/gw-proto/zstd-frames-95974_1389.hexl` —
//!   снимок из `capture-run2.jsonl` (`gw-extract-frames.mjs`), 137 кадров, 104 066 Б сжатых;
//!   распаковка 824 816 Б, из них ruzstd отдаёт 759 280 Б (удержание окна 65 536 = 824 816 −
//!   759 280), совпало с Node `createZstdDecompress`; границы термов — из
//!   `gw-proto/msgs-run2-95974_1389.jsonl`: 44 полных терма, смещение 758 576, остаток 704 Б;
//! - `gw-proto/zstd-frames-95974_8158.hexl` — 1 кадр (74 Б) середины потока, без магии
//!   (референс Node: «Unknown frame descriptor»).
//!
//! Тесты используют только публичный API крейта; лабораторные файлы опциональны: нет файла —
//! skip с причиной, а не падение.

use discord_codec::etf;
use discord_codec::{ZstdError, ZstdStream};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

/// Магия zstd-кадра — независимая копия оракула провода (`28 b5 2f fd`).
const MAGIC: [u8; 4] = [0x28, 0xb5, 0x2f, 0xfd];

/// Терм payload синтетики (34 Б): `op=10`, `s`/`t` — атомы «nil», `d = {op: "PING"}`.
const PAYLOAD_TERM_HEX: &str =
    "8374000000036400026f70610a640001736400036e696c6400017464000450494e47";

/// HELLO-терм (138 Б) из оракула захвата — первые байты lab-потока 1389.
const HELLO_HEX: &str = "8374000000046400016474000000026400065f74726163656c000000016d000000325b22676174657761792d7072642d61726d2d75732d65617374312d642d30346d35222c7b226d6963726f73223a302e307d5d6a6400126865617274626561745f696e74657276616c620000a1226400026f70610a640001736400036e696c640001746400036e696c";

/// Строка hex → байты (пробелы игнорируются).
fn unhex(s: &str) -> Vec<u8> {
    let compact: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    assert_eq!(compact.len() % 2, 0, "hex нечётной длины: {s}");
    (0..compact.len() / 2)
        .map(|i| u8::from_str_radix(&compact[2 * i..2 * i + 2], 16).expect("hex"))
        .collect()
}

/// Байты → строка hex (для сверки с оракулом).
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Lab-корень: `RE_LAB_ROOT` или `/home/user/clean-room-oracle`.
fn lab_root() -> PathBuf {
    PathBuf::from(
        std::env::var("RE_LAB_ROOT").unwrap_or_else(|_| "/home/user/clean-room-oracle".to_string()),
    )
}

/// Кадры lab-файла (строка hex на кадр): нет файла → `None` (skip-aware).
///
/// Прочие ошибки чтения не глотаются — это не пропуск, а сломанная среда.
fn load_lab_frames(path: &Path) -> Option<Vec<Vec<u8>>> {
    match fs::read_to_string(path) {
        Ok(text) => Some(
            text.lines()
                .filter(|line| !line.trim().is_empty())
                .map(unhex)
                .collect(),
        ),
        Err(error) if error.kind() == ErrorKind::NotFound => None,
        Err(error) => panic!("lab-файл {}: {error}", path.display()),
    }
}

/// Синтетический кадр из репозитория (одна строка hex, 67 Б).
fn synth_frame() -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/synth-zstd-frame.hexl");
    let text = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("нет тест-данных {}: {e}", path.display()));
    let lines: Vec<&str> = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();
    assert_eq!(
        lines.len(),
        1,
        "в synth-zstd-frame.hexl ожидалась одна строка"
    );
    let frame = unhex(lines[0]);
    assert_eq!(frame.len(), 67, "синтетический кадр обязан быть 67 Б");
    frame
}

/// Payload-оракул синтетики: терм × 4096 = 139 264 Б (md5 в шапке модуля).
fn synth_payload() -> Vec<u8> {
    let term = unhex(PAYLOAD_TERM_HEX);
    assert_eq!(term.len(), 34, "терм payload синтетики — 34 Б");
    term.repeat(4096)
}

/// Сверка выхода с payload-оракулом: без печати 139 КиБ — длина + первое расхождение.
fn assert_matches_payload(actual: &[u8], what: &str) {
    let expected = synth_payload();
    assert_eq!(actual.len(), expected.len(), "{what}: длина выхода");
    if let Some(at) = actual.iter().zip(&expected).position(|(a, b)| a != b) {
        panic!(
            "{what}: первое расхождение с оракулом на байте {at} (0x{:02x} != 0x{:02x})",
            actual[at], expected[at]
        );
    }
}

#[test]
fn synth_frame_decodes_whole() {
    // Весь кадр одним push → весь payload; поток завершён; повторный пустой push безвреден.
    let frame = synth_frame();
    let mut stream = ZstdStream::new();
    let out = stream
        .push(&frame)
        .expect("целый кадр обязан распаковаться");
    assert_eq!(out.len(), 139_264, "длина payload = 34 × 4096");
    assert_matches_payload(&out, "целый кадр");
    assert!(stream.is_finished(), "кадр завершён");
    assert_eq!(
        stream.compressed_consumed(),
        67,
        "потреблено ровно 67 сжатых байт"
    );
    let empty = stream.push(&[]).expect("пустой push после завершения — Ok");
    assert!(empty.is_empty(), "пустой push не выдаёт байт");
}

#[test]
fn synth_frame_split_in_three() {
    // Куски [0..7), [7..23), [23..]; магия только в начале кадра (факт вектора) — куски 2 и 3
    // её не содержат, проверка магии обязана пережить разбиение.
    let frame = synth_frame();
    let chunks = [&frame[..7], &frame[7..23], &frame[23..]];
    assert!(chunks[0].starts_with(&MAGIC), "кусок 1 — начало кадра");
    assert!(!chunks[1].starts_with(&MAGIC), "кусок 2 — без магии");
    assert!(!chunks[2].starts_with(&MAGIC), "кусок 3 — без магии");
    let mut stream = ZstdStream::new();
    let mut out = Vec::new();
    for (i, chunk) in chunks.iter().enumerate() {
        out.extend(
            stream
                .push(chunk)
                .unwrap_or_else(|e| panic!("кусок {i}: {e}")),
        );
    }
    assert_matches_payload(&out, "кадр из трёх кусков");
}

#[test]
fn synth_frame_byte_by_byte() {
    // 67 пушей по одному байту: заголовок кадра набирается постепенно.
    let frame = synth_frame();
    let mut stream = ZstdStream::new();
    let mut out = Vec::new();
    for (i, byte) in frame.iter().enumerate() {
        out.extend(
            stream
                .push(&[*byte])
                .unwrap_or_else(|e| panic!("байт {i}: {e}")),
        );
    }
    assert_matches_payload(&out, "кадр по одному байту");
}

#[test]
fn missing_magic_is_rejected() {
    // Свежий поток с чужими байтами — терминальный `MissingMagic`.
    let mut stream = ZstdStream::new();
    match stream.push(&[0, 1, 2, 3, 4]) {
        Err(ZstdError::MissingMagic) => {}
        other => panic!("ожидался MissingMagic, получено {other:?}"),
    }
}

#[test]
fn magic_split_across_pushes() {
    // Магия приходит по два байта: до набора четвёрки push обязан молчать, а не падать.
    let frame = synth_frame();
    let mut stream = ZstdStream::new();
    let mut out = Vec::new();
    out.extend(
        stream
            .push(&frame[..2])
            .expect("2 байта: магия ещё не собрана"),
    );
    out.extend(stream.push(&frame[2..4]).expect("4 байта: магия собрана"));
    out.extend(stream.push(&frame[4..]).expect("остаток кадра"));
    assert_matches_payload(&out, "магия по частям");
}

#[test]
fn corrupted_frame_is_reported() {
    // Индекс 12 — первый байт тела блока; порча проверена сканом координатора
    // (`FailedToReadBlockBody(MalformedSectionHeader { .. })`), но тест держит контракт:
    // любой `Decode` с непустым сообщением.
    let mut frame = synth_frame();
    frame[12] ^= 0xff;
    let mut stream = ZstdStream::new();
    match stream.push(&frame) {
        Err(ZstdError::Decode { msg }) => {
            assert!(!msg.is_empty(), "сообщение ruzstd непусто");
        }
        other => panic!("ожидался Decode, получено {other:?}"),
    }
}

#[test]
fn trailing_bytes_after_finished_frame() {
    // После завершения кадра любой лишний байт — `TrailingData` (не молчаливое проглатывание).
    let frame = synth_frame();
    let mut stream = ZstdStream::new();
    let out = stream.push(&frame).expect("кадр целиком");
    assert_matches_payload(&out, "кадр целиком");
    match stream.push(&[0]) {
        Err(ZstdError::TrailingData { bytes }) => assert_eq!(bytes, 1, "хвостовой байт"),
        other => panic!("ожидался TrailingData, получено {other:?}"),
    }
}

#[test]
fn lab_stream_1389_matches_oracle() {
    let path = lab_root().join("gw-proto/zstd-frames-95974_1389.hexl");
    let Some(frames) = load_lab_frames(&path) else {
        eprintln!("skip: нет lab-файла {}", path.display());
        return;
    };
    assert_eq!(frames.len(), 137, "кадров в снимке потока");
    assert!(frames[0].starts_with(&MAGIC), "кадр 0 — с магии");
    assert_eq!(
        frames[1..].iter().filter(|f| f.starts_with(&MAGIC)).count(),
        0,
        "магия только в кадре 0"
    );
    let mut stream = ZstdStream::new();
    let mut out = Vec::new();
    for (i, frame) in frames.iter().enumerate() {
        out.extend(
            stream
                .push(frame)
                .unwrap_or_else(|e| panic!("кадр {i}: {e}")),
        );
    }
    assert_eq!(
        out.len(),
        759_280,
        "выдано распакованных байт: 824 816 − удержанное окно 65 536"
    );
    assert_eq!(
        stream.compressed_consumed(),
        104_066,
        "потреблено сжатых байт"
    );
    assert!(
        !stream.is_finished(),
        "непрерывный кадр не завершён: end() не вызывается"
    );
    assert_eq!(hex(&out[..138]), HELLO_HEX, "HELLO из оракула захвата");
    // Жадный разбор `etf::decode`: 44 полных терма, смещение 758 576, остаток 704 Б
    // (последний терм обрезан удержанием окна ruzstd).
    let mut offset = 0usize;
    let mut terms = 0usize;
    while let Ok((_value, consumed)) = etf::decode(&out[offset..]) {
        assert!(consumed > 0, "нулевой consumed на смещении {offset}");
        offset += consumed;
        terms += 1;
    }
    assert_eq!(terms, 44, "полных термов");
    assert_eq!(offset, 758_576, "смещение после последнего полного терма");
    assert_eq!(out.len() - offset, 704, "остаток байт");
    assert!(
        etf::decode(&out[offset..]).is_err(),
        "неполный терм обязан давать ошибку"
    );
}

#[test]
fn lab_midstream_socket_is_rejected() {
    let path = lab_root().join("gw-proto/zstd-frames-95974_8158.hexl");
    let Some(frames) = load_lab_frames(&path) else {
        eprintln!("skip: нет lab-файла {}", path.display());
        return;
    };
    assert_eq!(frames.len(), 1, "в снимке один кадр");
    assert_eq!(frames[0].len(), 74, "кадр середины потока — 74 Б");
    assert!(
        !frames[0].starts_with(&MAGIC),
        "факт вектора: кадр без магии"
    );
    let mut stream = ZstdStream::new();
    match stream.push(&frames[0]) {
        Err(ZstdError::MissingMagic) => {}
        other => panic!("ожидался MissingMagic, получено {other:?}"),
    }
}
