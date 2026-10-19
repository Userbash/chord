// Ревью RW-03: независимая распаковка первых 3 WS-сообщений сокета 95974.1389 напрямую
// через ruzstd (без discord-codec): проверка, что первые 138 Б = HELLO-терм из ТЗ.
use std::fs::File;
use std::io::Read;

fn main() {
    let file = File::open("/tmp/opencode/rw-03/review/frames3.bin").expect("frames3.bin");
    let mut dec = ruzstd::decoding::StreamingDecoder::new(file).expect("заголовок zstd-кадра");
    let mut buf = vec![0u8; 138];
    let mut filled = 0usize;
    while filled < buf.len() {
        match dec.read(&mut buf[filled..]) {
            Ok(0) => break,
            Ok(n) => filled += n,
            Err(e) => {
                eprintln!("чтение: {e} (получено {filled} Б)");
                break;
            }
        }
    }
    let hex: String = buf[..filled].iter().map(|b| format!("{b:02x}")).collect();
    println!("получено: {filled} Б");
    println!("{hex}");
    let expected = "8374000000046400016474000000026400065f74726163656c000000016d000000325b22676174657761792d7072642d61726d2d75732d65617374312d642d30346d35222c7b226d6963726f73223a302e307d5d6a6400126865617274626561745f696e74657276616c620000a1226400026f70610a640001736400036e696c640001746400036e696c";
    println!(
        "байт-в-байт равно константе HELLO: {}",
        filled == 138 && hex == expected
    );
}
