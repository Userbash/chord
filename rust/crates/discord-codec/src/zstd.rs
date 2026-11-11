//! # zstd-стрим шлюза: инкрементальный декодер поверх потокового `zstd`
//!
//! Шлюз Discord сжимает поток в **один непрерывный zstd-кадр**: магия (`28 b5 2f fd`) есть
//! только в первом WS-сообщении, дальше приходят продолжения того же кадра, а `end()` не
//! вызывается вовсе (спека `docs/re/gateway-protocol.md` §3; вариант B, P0-03). На границе
//! каждого WS-сообщения сервер делает `ZSTD_e_flush`, то есть закрывает блок, но не кадр.
//!
//! [`ZstdStream`] накапливает куски провода и после каждого [`ZstdStream::push`] возвращает
//! **все распакованные байты этого куска сразу**; история окна (словарь ссылок на предыдущие
//! блоки) живёт внутри декодера и переживает границы WS-сообщений.
//!
//! Почему не `ruzstd`: его публичный streaming API (`FrameDecoder::decode_from_to` /
//! `read`) отдаёт распакованное только когда кадр завершён (`is_finished()`), а до этого
//! удерживает последние `window_size` байт — у шлюза это 64 КиБ, кадр не завершается никогда,
//! поэтому мелкие DISPATCH-пакеты застревали в окне (находка F-GW-01, task-137). Слить
//! удержанный хвост, сохранив историю, у `ruzstd` нельзя: `DecodeBuffer`/`block_decoder`
//! закрыты (`pub(crate)`), а `drain()` обнуляет окно. Здесь используется потоковый декодер
//! libzstd (`ZSTD_decompressStream`): он копирует готовый выход в буфер вызывающего и держит
//! историю отдельно — тот же контракт, что у эталонного `zlib.createZstdDecompress()` из
//! RE-дампа.
//!
//! Семантика (контракт RW-02):
//! - байты, распакованные в вызове, который завершился `Err`, **теряются**: после ошибки поток
//!   мёртв (терминальный), продолжать чтение нельзя — переподключение = новый [`ZstdStream`];
//! - прототип поддерживает ровно один непрерывный кадр; повторная магия в середине потока
//!   трактуется как обычные данные (поток шлюза её не присылает: 0 из 136 кадров после первого);
//! - после завершённого кадра любой лишний байт — [`ZstdError::TrailingData`]: второй кадр
//!   шлюз в одном соединении не присылает;
//! - никаких `unwrap`/паник на входных данных: любой мусор — либо ошибка, либо пропуск;
//!   `unsafe` запрещён линтом крейта.
//!
//! Поток байт в байт совпадает с эталоном: на lab-снимке `gw-proto/zstd-frames-95974_1389.hexl`
//! отдаются все 824 816 Б / 137 термов (до исправления — 759 280 Б / 44 терма).

use std::fmt;
use zstd::stream::raw::{DParameter, Decoder, InBuffer, Operation, OutBuffer};

/// Ёмкость переиспользуемого буфера выдачи (≥64 КиБ по ТЗ): за один проход декодера
/// возвращается не больше ёмкости, цикл доберёт остаток.
const TARGET_CAPACITY: usize = 64 * 1024;

/// Максимальный `WindowLog` кадра zstd (2^24 = 16 МиБ; боевой шлюз Discord использует 64 КиБ).
pub const MAX_WINDOW_LOG: u32 = 24;

/// Максимальный объём распакованных байт за один вызов [`ZstdStream::push`] (16 МиБ, защита от zip-bomb).
pub const MAX_DECOMPRESSED_CHUNK_BYTES: usize = 16 * 1024 * 1024;

/// Максимальный объём сжатых неразобранных байт в накопителе [`ZstdStream`] (8 МиБ).
pub const MAX_PENDING_COMPRESSED_BYTES: usize = 8 * 1024 * 1024;

/// Ошибки zstd-стрима.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ZstdError {
    /// Поток не начинается с магии zstd-кадра (`28 b5 2f fd`) — это не поток шлюза.
    MissingMagic,
    /// `zstd` отверг данные (сообщение — `Display` его ошибки); поток терминальный.
    Decode {
        /// Текст ошибки libzstd.
        msg: String,
    },
    /// После завершения кадра остались неразобранные байты.
    TrailingData {
        /// Сколько байт осталось в накопителе.
        bytes: usize,
    },
    /// Поток уже завершился ошибкой; продолжать нельзя — нужен новый [`ZstdStream`].
    Poisoned,
}

impl fmt::Display for ZstdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingMagic => {
                write!(f, "поток не начинается с магии zstd-кадра (28 b5 2f fd)")
            }
            Self::Decode { msg } => write!(f, "ошибка декодирования zstd: {msg}"),
            Self::TrailingData { bytes } => write!(
                f,
                "после завершения zstd-кадра осталось {bytes} Б неразобранных данных"
            ),
            Self::Poisoned => write!(f, "zstd-поток уже завершился ошибкой"),
        }
    }
}

impl std::error::Error for ZstdError {}

/// Инкрементальный декодер непрерывного zstd-потока шлюза.
///
/// Внутри — один потоковый `zstd`-декодер (`ZSTD_DStream`) на весь поток: `end()` не
/// вызывается, поэтому окно и состояние блока переживают границы WS-сообщений.
pub struct ZstdStream {
    /// Потоковый декодер libzstd: держит окно/историю, выход отдаёт сразу.
    decoder: Decoder<'static>,
    /// Сжатые байты, которые декодер ещё не принял (в норме пусто: libzstd сам буферизует
    /// частичные заголовки и блоки между вызовами).
    pending: Vec<u8>,
    /// Проверена ли магия потока (первая четвёрка байт; только кадр 0).
    magic_checked: bool,
    /// Декодер уже принимал данные кадра (у свежего потока кадра ещё нет).
    initialized: bool,
    /// Кадр завершён (`ZSTD_decompressStream` вернул 0 — фрейм дочитан до `last_block`).
    finished: bool,
    /// Поток терминально завершён ошибкой; дальнейшие `push` запрещены.
    dead: bool,
    /// Переиспользуемый буфер выдачи.
    target: Vec<u8>,
    /// Сколько сжатых байт потока уже принято декодером.
    consumed: u64,
}

impl ZstdStream {
    /// Пустой поток; магия проверяется на первых поданных байтах.
    pub fn new() -> Self {
        let mut decoder =
            Decoder::new().expect("ZSTD_createDStream не может отказать на пустом стриме");
        decoder
            .set_parameter(DParameter::WindowLogMax(MAX_WINDOW_LOG))
            .expect("WindowLogMax(24) допустим в libzstd");
        Self {
            decoder,
            pending: Vec::new(),
            magic_checked: false,
            initialized: false,
            finished: false,
            dead: false,
            target: vec![0u8; TARGET_CAPACITY],
            consumed: 0,
        }
    }

    /// Добавляет кусок провода (одно WS-сообщение) и возвращает **все новые** распакованные байты.
    ///
    /// Алгоритм:
    /// 1. если поток уже ошибался — [`ZstdError::Poisoned`] (декодер не трогается);
    /// 2. кусок добавляется в накопитель `pending`;
    /// 3. пока не проверена магия: при `pending.len() >= 4` сверка с [`crate::ZSTD_FRAME_MAGIC`]
    ///    (нет — [`ZstdError::MissingMagic`]); при `pending.len() < 4` — `Ok` с пустым выходом;
    /// 4. цикл: декодер получает остаток накопителя и переиспользуемый буфер выдачи; выход
    ///    копируется в результат **сразу**, история окна остаётся внутри декодера;
    /// 5. цикл крутится, пока есть прогресс (приняты сжатые байты или выданы распакованные):
    ///    за проход отдаётся не более ёмкости буфера выдачи, а payload может быть больше;
    /// 6. `hint == 0` — кадр завершён; если после этого в накопителе остались байты, это
    ///    [`ZstdError::TrailingData`]. Ошибка libzstd → [`ZstdError::Decode`]. Любая из этих
    ///    ошибок переводит поток в `dead`.
    ///
    /// Паник нет: накопитель растёт ровно на поданные байты и снимается по счётчику декодера.
    pub fn push(&mut self, frame: &[u8]) -> Result<Vec<u8>, ZstdError> {
        if self.dead {
            return Err(ZstdError::Poisoned);
        }
        if self.pending.len().saturating_add(frame.len()) > MAX_PENDING_COMPRESSED_BYTES {
            self.dead = true;
            self.pending.clear();
            return Err(ZstdError::Decode {
                msg: format!(
                    "превышен лимит сжатого буфера zstd ({MAX_PENDING_COMPRESSED_BYTES} Б)"
                ),
            });
        }
        self.pending.extend_from_slice(frame);

        if !self.magic_checked {
            if self.pending.len() < crate::ZSTD_FRAME_MAGIC.len() {
                // Магия ещё не собрана — ничего не декодируем и не решаем.
                return Ok(Vec::new());
            }
            if self.pending[..crate::ZSTD_FRAME_MAGIC.len()] != crate::ZSTD_FRAME_MAGIC {
                self.dead = true;
                return Err(ZstdError::MissingMagic);
            }
            self.magic_checked = true;
        }

        let mut out = Vec::new();
        let mut offset = 0usize;
        while !self.finished {
            let mut input = InBuffer::around(&self.pending[offset..]);
            let mut output = OutBuffer::around(&mut self.target);
            let hint = match self.decoder.run(&mut input, &mut output) {
                Ok(hint) => hint,
                Err(error) => {
                    self.dead = true;
                    return Err(ZstdError::Decode {
                        msg: error.to_string(),
                    });
                }
            };
            let taken = input.pos();
            let produced = output.pos();
            offset += taken;
            self.consumed += taken as u64;
            if taken > 0 || produced > 0 {
                self.initialized = true;
            }
            if produced > 0 {
                if out.len().saturating_add(produced) > MAX_DECOMPRESSED_CHUNK_BYTES {
                    self.dead = true;
                    self.pending.clear();
                    return Err(ZstdError::Decode {
                        msg: format!(
                            "превышен лимит распаковки одного сообщения zstd ({MAX_DECOMPRESSED_CHUNK_BYTES} Б)"
                        ),
                    });
                }
                out.extend_from_slice(&output.as_slice()[..produced]);
            }
            if hint == 0 {
                // Кадр дочитан до `last_block`: весь выход уже в `out`.
                self.finished = true;
            } else if taken == 0 && produced == 0 {
                // Прогресса нет: вход исчерпан, внутренний буфер пуст — ждём следующий push.
                break;
            }
        }
        self.pending.drain(..offset);

        if self.finished && !self.pending.is_empty() {
            self.dead = true;
            return Err(ZstdError::TrailingData {
                bytes: self.pending.len(),
            });
        }
        Ok(out)
    }

    /// Сколько сжатых байт потока уже принято декодером.
    pub fn compressed_consumed(&self) -> u64 {
        self.consumed
    }

    /// Сколько сжатых байт осталось в накопителе (не принято декодером).
    ///
    /// В норме `0` после каждого `push`: libzstd сам держит частичные заголовки и блоки,
    /// поэтому «вечного хвоста» на выходе нет — все распакованные байты отдаются сразу.
    pub fn pending_bytes(&self) -> usize {
        self.pending.len()
    }

    /// Терминально ли поток завершён ошибкой; после `true` `push` возвращает [`ZstdError::Poisoned`].
    pub fn is_dead(&self) -> bool {
        self.dead
    }

    /// Завершён ли текущий кадр.
    ///
    /// У свежего потока — `false` (кадра ещё нет), после полного кадра — `true`, у непрерывного
    /// потока шлюза (`end()` не вызывался) — `false`.
    pub fn is_finished(&self) -> bool {
        self.initialized && self.finished
    }
}

impl Default for ZstdStream {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    //! Контракт потока шлюза с `ZSTD_e_flush` (F-GW-01, task-137).
    //!
    //! **Провенанс фикстуры** (независимый оракул, не подгонка под реализацию): поток собран
    //! энкодером `zstd` 0.14.3 (libzstd): уровень 3, `WindowLog(16)` (окно 64 КиБ, как у шлюза),
    //! `ChecksumFlag(false)`, кадр **не завершён** (`end()` не вызывался), `flush()`
    //! (= `ZSTD_e_flush`) на границе каждого сообщения. Магия `28 b5 2f fd` — только в первом
    //! чанке. Шесть сообщений — термоподобные байты по 39…64 Б; сообщения 3…6 повторяют
    //! структуру предыдущих, поэтому компрессор ссылается на историю окна (13…14 Б на чанк).
    //!
    //! Оракул поведения — `zlib.createZstdDecompress()` из RE-дампа (`gateway-protocol.md` §3):
    //! декодер без `end()` отдаёт накопленный вывод полностью, в том числе хвост меньше окна.

    use super::{ZstdError, ZstdStream};

    /// Чанки провода: ровно то, что вернул `flush()` после каждого сообщения.
    const FLUSH_WIRE: [&str; 6] = [
        "28b52ffd00303801008374000000046400016474000000026400047479706564000b4d4553534147455f435245415445",
        "000200640001736400013164000264640002696467000000016400086368616e6e656c5f69646700000001640007636f6e74656e7464000c7072697665742d68692d31",
        "54000010323202008423831b80",
        "5c00001864333302000447307005",
        "5c00001864343402000447307005",
        "5c00001864353502000447307005",
    ];

    /// Распакованные сообщения — оракул выхода, по одному на `push`.
    const FLUSH_MESSAGES: [&str; 6] = [
        "8374000000046400016474000000026400047479706564000b4d4553534147455f435245415445",
        "640001736400013164000264640002696467000000016400086368616e6e656c5f69646700000001640007636f6e74656e7464000c7072697665742d68692d31",
        "640001736400013264000264640002696467000000016400086368616e6e656c5f69646700000001640007636f6e74656e7464000c7072697665742d68692d32",
        "640001736400013364000264640002696467000000016400086368616e6e656c5f69646700000001640007636f6e74656e7464000c7072697665742d68692d33",
        "640001736400013464000264640002696467000000016400086368616e6e656c5f69646700000001640007636f6e74656e7464000c7072697665742d68692d34",
        "640001736400013564000264640002696467000000016400086368616e6e656c5f69646700000001640007636f6e74656e7464000c7072697665742d68692d35",
    ];

    /// Строка hex → байты.
    fn unhex(s: &str) -> Vec<u8> {
        assert_eq!(s.len() % 2, 0, "hex нечётной длины: {s}");
        (0..s.len() / 2)
            .map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).expect("hex"))
            .collect()
    }

    /// Байты → строка hex.
    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    #[test]
    fn flush_releases_each_message_immediately() {
        // F-GW-01: после каждого push выход равен ровно распакованному сообщению — ноль байт
        // остаётся в буфере декодера (при ruzstd здесь 0 Б на все шесть push: хвост 244 Б < окна).
        let mut stream = ZstdStream::new();
        for (index, (wire, message)) in FLUSH_WIRE.iter().zip(FLUSH_MESSAGES.iter()).enumerate() {
            let out = stream
                .push(&unhex(wire))
                .unwrap_or_else(|error| panic!("push {index}: {error}"));
            assert_eq!(
                hex(&out),
                *message,
                "push {index}: выход обязан содержать сообщение целиком и сразу"
            );
            assert_eq!(stream.pending_bytes(), 0, "push {index}: хвоста нет");
        }
        assert!(
            !stream.is_finished(),
            "кадр шлюза не завершается: end() не вызывается"
        );
        assert!(!stream.is_dead(), "поток живой");
    }

    #[test]
    fn flush_keeps_window_history_across_chunks() {
        // Сообщения 3…6 сжаты в 13…14 Б: это ссылки на историю окна из предыдущих блоков.
        // Точный выход доказывает, что словарь/окно сохраняется, а не только что байты отдаются.
        for wire in &FLUSH_WIRE[2..] {
            assert!(
                wire.len() / 2 <= 16,
                "чанк {wire} — короткая ({}) ссылка на окно, а не новое содержимое",
                wire.len() / 2
            );
        }
        let mut stream = ZstdStream::new();
        let mut accumulated = String::new();
        for (index, wire) in FLUSH_WIRE.iter().enumerate() {
            let out = stream
                .push(&unhex(wire))
                .unwrap_or_else(|error| panic!("push {index}: {error}"));
            accumulated.push_str(&hex(&out));
            assert_eq!(
                accumulated,
                FLUSH_MESSAGES[..=index].concat(),
                "push {index}: накопленный выход = все сообщения до текущего"
            );
        }
    }

    #[test]
    fn flush_then_frame_end_releases_everything() {
        // Завершение кадра (пустой последний блок `01 00 00`) по-прежнему отдаёт весь остаток,
        // а мусор после завершённого кадра — терминальный `TrailingData` (контракт RW-02).
        let mut stream = ZstdStream::new();
        let mut accumulated = String::new();
        for (index, wire) in FLUSH_WIRE.iter().enumerate() {
            accumulated.push_str(&hex(&stream
                .push(&unhex(wire))
                .unwrap_or_else(|error| panic!("push {index}: {error}"))));
        }
        accumulated.push_str(&hex(&stream
            .push(&[0x01, 0x00, 0x00])
            .expect("пустой последний блок завершает кадр")));
        assert_eq!(
            accumulated,
            FLUSH_MESSAGES.concat(),
            "весь поток отдан без потерь"
        );
        assert!(stream.is_finished(), "кадр завершён последним блоком");
        assert!(
            matches!(stream.push(&[0x00]), Err(ZstdError::TrailingData { .. })),
            "мусор после завершённого кадра терминален"
        );
        assert!(stream.is_dead(), "ошибка терминальна");
        assert!(matches!(stream.push(&[]), Err(ZstdError::Poisoned)));
    }
}
