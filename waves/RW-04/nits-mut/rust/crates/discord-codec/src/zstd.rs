//! # zstd-стрим шлюза: инкрементальный декодер поверх `ruzstd`
//!
//! Шлюз Discord сжимает поток в **один непрерывный zstd-кадр**: магия (`28 b5 2f fd`) есть
//! только в первом WS-сообщении, дальше приходят продолжения того же кадра, а `end()` не
//! вызывается вовсе (спека `docs/re/gateway-protocol.md` §3; вариант B, P0-03).
//!
//! [`ZstdStream`] накапливает куски провода и после каждого [`ZstdStream::push`] возвращает
//! **только новые распакованные байты**; контекст (окно, состояние блока, счётчик) между
//! кадрами сохраняется — декодер создаётся один раз и не завершается.
//!
//! Семантика (контракт RW-02):
//! - байты, распакованные в вызове, который завершился `Err`, **теряются**: после ошибки поток
//!   мёртв (терминальный), продолжать чтение нельзя — переподключение = новый [`ZstdStream`];
//! - прототип поддерживает ровно один непрерывный кадр; повторная магия в середине потока
//!   трактуется как обычные данные (поток шлюза её не присылает: 0 из 136 кадров после первого);
//! - **удержание окна**: пока кадр не завершён, ruzstd отдаёт не более `распаковано − окно`;
//!   у потока шлюза окно 64 КиБ (window descriptor `0x30` → 2¹⁶). Для сокета `95974.1389`:
//!   824 816 − 65 536 = 759 280 Б на выходе, хвост удерживается до закрытия соединения — это
//!   ограничение ruzstd, а не потеря: байты выдадутся по мере поступления следующих кадров
//!   (кандидат на пересмотр в P1-02);
//! - никаких `unwrap`/паник на входных данных: любой мусор — либо ошибка, либо пропуск;
//!   `unsafe` запрещён линтом крейта.

use ruzstd::decoding::FrameDecoder;
use std::fmt;

/// Максимальный размер заголовка zstd-кадра: магия 4 + дескриптор 1 + окно 1 + dict-id до 4 +
/// размер содержимого до 8 = 18 Б. Пока столько байт не набрано, `ruzstd` не инициализируется:
/// `decode_from_to` читает заголовок из среза целиком (частичный заголовок — ошибка).
const FRAME_HEADER_MAX: usize = 18;

/// Ёмкость переиспользуемого буфера выдачи (≥64 КиБ по ТЗ; больше payload синтетики 139 264 Б
/// держать не нужно — цикл доберёт остаток следующими вызовами).
const TARGET_CAPACITY: usize = 64 * 1024;

/// Ошибки zstd-стрима.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ZstdError {
    /// Поток не начинается с магии zstd-кадра (`28 b5 2f fd`) — это не поток шлюза.
    MissingMagic,
    /// `ruzstd` отверг данные (сообщение — `Display` его ошибки); поток терминальный.
    Decode {
        /// Текст ошибки `ruzstd`.
        msg: String,
    },
    /// После завершения кадра остались неразобранные байты.
    TrailingData {
        /// Сколько байт осталось в накопителе.
        bytes: usize,
    },
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
        }
    }
}

impl std::error::Error for ZstdError {}

/// Инкрементальный декодер непрерывного zstd-потока шлюза.
///
/// Внутри — один [`FrameDecoder`] на весь поток: `end()` (сброс/завершение) не вызывается,
/// поэтому окно и состояние блока переживают границы WS-сообщений.
pub struct ZstdStream {
    /// Декодер непрерывного кадра `ruzstd`.
    decoder: FrameDecoder,
    /// Ещё не отданные сжатые байты (хвост, который декодер не смог разобрать целиком).
    pending: Vec<u8>,
    /// Проверена ли магия потока (первая четвёрка байт; только кадр 0).
    magic_checked: bool,
    /// Переиспользуемый буфер выдачи.
    target: Vec<u8>,
}

impl ZstdStream {
    /// Пустой поток; магия проверяется на первых поданных байтах.
    pub fn new() -> Self {
        Self {
            decoder: FrameDecoder::new(),
            pending: Vec::new(),
            magic_checked: false,
            target: vec![0u8; TARGET_CAPACITY],
        }
    }

    /// Добавляет кусок провода (одно WS-сообщение) и возвращает **новые** распакованные байты.
    ///
    /// Алгоритм:
    /// 1. кусок добавляется в накопитель `pending`;
    /// 2. пока не проверена магия: при `pending.len() >= 4` сверка с [`crate::ZSTD_FRAME_MAGIC`]
    ///    (нет — [`ZstdError::MissingMagic`]); при `pending.len() < 4` — `Ok` с пустым выходом;
    /// 3. цикл декодирования: `ruzstd` получает весь накопитель и переиспользуемый буфер выдачи.
    ///    Продвижение считается **дельтой `bytes_read_from_source()`**, а не возвращённым кортежем
    ///    (у `ruzstd` есть ветка с некорректным возвратом при добирании контрольной суммы);
    ///    разобранные сжатые байты снимаются с накопителя;
    /// 4. цикл крутится, пока есть прогресс (прочитаны сжатые байты или выданы распакованные):
    ///    за один вызов ruzstd отдаёт не более ёмкости буфера выдачи, а payload больше неё
    ///    (синтетика: 139 264 Б при буфере 64 КиБ);
    /// 5. ошибка `ruzstd` → [`ZstdError::Decode`]; если кадр уже начат и завершён, а накопитель
    ///    не пуст → [`ZstdError::TrailingData`] (у неинициализированного декодера `is_finished`
    ///    тоже `true`, поэтому проверяется ещё и факт инициализации).
    ///
    /// Паник нет: накопитель растёт ровно на поданные байты и снимается по счётчику декодера.
    pub fn push(&mut self, frame: &[u8]) -> Result<Vec<u8>, ZstdError> {
        self.pending.extend_from_slice(frame);

        if !self.magic_checked {
            if self.pending.len() < crate::ZSTD_FRAME_MAGIC.len() {
                // Магия ещё не собрана — ничего не декодируем и не решаем.
                return Ok(Vec::new());
            }
            if self.pending[..crate::ZSTD_FRAME_MAGIC.len()] != crate::ZSTD_FRAME_MAGIC {
                return Err(ZstdError::MissingMagic);
            }
            self.magic_checked = true;
        }

        let mut out = Vec::new();
        loop {
            // Заголовок читается из среза целиком: пока его нет, попытка бессмысленна.
            if self.decoder.bytes_read_from_source() == 0 && self.pending.len() < FRAME_HEADER_MAX {
                break;
            }
            let before = self.decoder.bytes_read_from_source();
            let result = self.decoder.decode_from_to(&self.pending, &mut self.target);
            let written = match result {
                Ok((_reported, written)) => written,
                Err(error) => {
                    return Err(ZstdError::Decode {
                        msg: error.to_string(),
                    });
                }
            };
            // Счётчик декодера монотонен, но берём saturating_sub: паника хуже, чем ноль.
            // Источник у декодера один — `pending`, поэтому дельта не превышает его длину;
            // `min` — страховка от паники `drain` на будущих версиях ruzstd.
            let delta = self.decoder.bytes_read_from_source().saturating_sub(before);
            let advanced = usize::try_from(delta)
                .unwrap_or(usize::MAX)
                .min(self.pending.len());
            if advanced > 0 {
                self.pending.drain(..advanced);
            }
            if written > 0 {
                out.extend_from_slice(&self.target[..written]);
            }
            if advanced == 0 && written == 0 {
                // Прогресса нет: источник и буфер выдачи исчерпаны до следующего push.
                break;
            }
        }

        // NB: свежий `FrameDecoder` без состояния тоже отвечает `is_finished() == true`
        // (кадра ещё нет), поэтому хвост проверяем только после инициализации декодера.
        let initialized = self.decoder.bytes_read_from_source() > 0;
        if initialized && self.decoder.is_finished() && !self.pending.is_empty() {
            return Err(ZstdError::TrailingData {
                bytes: self.pending.len(),
            });
        }
        Ok(out)
    }

    /// Сколько сжатых байт потока уже разобрано — `FrameDecoder::bytes_read_from_source()`.
    pub fn compressed_consumed(&self) -> u64 {
        self.decoder.bytes_read_from_source()
    }

    /// Завершён ли текущий кадр — `FrameDecoder::is_finished()`.
    ///
    /// NB: у свежего потока `ruzstd` без состояния отвечает `true` (кадра ещё нет), значение
    /// осмысленно после первого [`ZstdStream::push`]; у непрерывного потока шлюза
    /// (`end()` не вызывался) остаётся `false`.
    pub fn is_finished(&self) -> bool {
        self.decoder.is_finished()
    }
}

impl Default for ZstdStream {
    fn default() -> Self {
        Self::new()
    }
}
