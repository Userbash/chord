//! SRTP/SRTCP-кадрирование Discord (`aead_aes256_gcm_rtpsize`) — карточка P4-03.
//!
//! Раскладка кадра подтверждена capture медиа-пакетов движка
//! (`strace-media-s3.jsonl`, разбор `waves/RW-25/research/media-packet-layout.mjs`):
//!
//! ```text
//! [clear][ciphertext][tag 16][counter 4 LE]
//! ```
//!
//! Исходящий счётчик = `0x80000000 | n`, где `n` — сквозной счётчик, **общий**
//! для RTP и SRTCP одного сокета (в capture 8 RTP-пакетов дали `n = 0..7`, затем
//! SR-пакеты — `n = 8, 9, 10`).
//!
//! Честная граница: ключевого материала в capture нет, поэтому совместимость с wire
//! Discord **не заявляется**. Формат самосогласован, раскладка байт — по capture,
//! криптопримитивы — по KAT-векторам NIST/RFC (см. `tests/crypto_vectors.rs`).
//!
//! Входящие счётчики не монотонны (в capture `0x00..0x0a`, затем `0x76`, `0x66`,
//! `0x0d`), поэтому на расшифровке трейлер трактуется как непрозрачные 4 байта:
//! монотонность не проверяется, а его биты не интерпретируются — из него только
//! строится nonce (уникальность кадра), как и при шифровании.

use crate::crypto::aes::Aes256Key;
use crate::crypto::chacha::XChaCha20Poly1305;
use crate::crypto::gcm::Gcm;
use crate::crypto::kdf::{hkdf_sha1_16, hkdf_sha1_32};
use crate::crypto::{CryptoError, GCM_NONCE_SIZE, XCHACHA_NONCE_SIZE};
use std::collections::{HashSet, VecDeque};
use std::fmt;

/// Длина трейлера-счётчика (4 Б, little endian).
const TRAILER_LEN: usize = 4;
/// Длина тега AEAD (16 Б для всех режимов карточки).
const AEAD_TAG_LEN: usize = 16;
/// Минимальная длина RTP-заголовка (12 Б).
const RTP_HEADER_MIN: usize = 12;
/// Длина clear-области SRTCP: заголовок RTCP (4 Б) + SSRC (4 Б).
const SRTCP_CLEAR_LEN: usize = 8;
/// Максимальное значение сквозного счётчика `n`: старший бит трейлера — маркер.
const MAX_COUNTER: u64 = 0x7FFF_FFFF;
/// Размер окна приёма: сколько последних трейлеров (nonce) помним.
const RECEIVE_WINDOW: usize = 1024;
/// Метка HKDF для ключа AEAD сессии.
const LABEL_KEY: &[u8] = b"discord-lab/srtp/aead-key";
/// Метка HKDF для seed нонса.
const LABEL_NONCE: &[u8] = b"discord-lab/srtp/nonce-seed";
/// Режимы движка, объявленные, но не покрытые карточкой.
const UNSUPPORTED_ENGINE_MODES: [&str; 4] = [
    "xsalsa20_poly1305_lite_rtpsize",
    "xsalsa20_poly1305_lite",
    "xsalsa20_poly1305_suffix",
    "xsalsa20_poly1305",
];

/// Режим шифрования из лога движка (`docs/re/voice-deep.md` §3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncryptionMode {
    /// `aead_aes256_gcm_rtpsize` — AEAD защищает только полезную нагрузку,
    /// заголовок RTP/RTCP уходит в AAD.
    Aes256GcmRtpSize,
    /// `aead_aes256_gcm` — шифруется весь кадр, AAD пустой.
    Aes256Gcm,
    /// `aead_xchacha20_poly1305_rtpsize` — как `Aes256GcmRtpSize`, но XChaCha20.
    XChaCha20Poly1305RtpSize,
}

impl EncryptionMode {
    /// Разбор имени режима движка; неизвестное имя ⇒ `None`.
    pub fn from_engine_name(name: &str) -> Option<Self> {
        match name {
            "aead_aes256_gcm_rtpsize" => Some(Self::Aes256GcmRtpSize),
            "aead_aes256_gcm" => Some(Self::Aes256Gcm),
            "aead_xchacha20_poly1305_rtpsize" => Some(Self::XChaCha20Poly1305RtpSize),
            _ => None,
        }
    }

    /// Имя режима как в логе движка.
    pub fn as_engine_name(self) -> &'static str {
        match self {
            Self::Aes256GcmRtpSize => "aead_aes256_gcm_rtpsize",
            Self::Aes256Gcm => "aead_aes256_gcm",
            Self::XChaCha20Poly1305RtpSize => "aead_xchacha20_poly1305_rtpsize",
        }
    }

    /// `true` для режимов `*_rtpsize`: AEAD защищает только полезную нагрузку,
    /// заголовок — это AAD.
    pub fn covers_header(self) -> bool {
        matches!(
            self,
            Self::Aes256GcmRtpSize | Self::XChaCha20Poly1305RtpSize
        )
    }

    /// Длина тега AEAD (все режимы — 16).
    pub fn tag_size(self) -> usize {
        AEAD_TAG_LEN
    }
}

/// Вид кадра.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameKind {
    /// RTP-кадр.
    Rtp,
    /// SRTCP-кадр.
    Srtcp,
}

/// Ошибка SRTP-карточки.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SrtpError {
    /// Пакет короче минимально допустимого.
    PacketTooShort {
        /// Фактическая длина пакета в байтах.
        len: usize,
        /// Минимально допустимая длина в байтах.
        min: usize,
    },
    /// Некорректный RTP/RTCP-заголовок.
    MalformedHeader {
        /// Причина отказа (статическая строка).
        reason: &'static str,
    },
    /// Режим не поддерживается (например, `xsalsa20_poly1305` — вне карточки).
    UnsupportedMode {
        /// Имя режима движка.
        name: &'static str,
    },
    /// Несовпадение AEAD-тега.
    AuthenticationFailed,
    /// Ошибка примитива.
    Crypto(CryptoError),
    /// Переполнение 32-битного счётчика.
    CounterOverflow,
    /// Попытка перемотать счётчик назад: повтор nonce под тем же ключом недопустим.
    CounterRewind {
        /// Текущее значение счётчика `n`.
        current: u64,
        /// Запрошенное значение счётчика `n`.
        requested: u64,
    },
    /// Повтор трейлера (nonce) на приёме: кадр с уже использованным nonce отклоняется.
    NonceReuse {
        /// Значение трейлера кадра (4 байта, little endian).
        trailer: u32,
    },
}

impl fmt::Display for SrtpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PacketTooShort { len, min } => {
                write!(f, "пакет слишком короткий: {len} Б, нужно минимум {min} Б")
            }
            Self::MalformedHeader { reason } => write!(f, "некорректный заголовок: {reason}"),
            Self::UnsupportedMode { name } => {
                write!(f, "режим шифрования не поддерживается: {name}")
            }
            Self::AuthenticationFailed => write!(f, "проверка аутентификации не пройдена"),
            Self::Crypto(err) => write!(f, "ошибка крипто-примитива: {err}"),
            Self::CounterOverflow => write!(f, "переполнение счётчика кадров"),
            Self::CounterRewind { current, requested } => write!(
                f,
                "перемотка счётчика назад запрещена: текущий {current}, запрошен {requested}"
            ),
            Self::NonceReuse { trailer } => {
                write!(f, "повторное использование nonce (трейлер 0x{trailer:08x})")
            }
        }
    }
}

impl std::error::Error for SrtpError {}

impl From<CryptoError> for SrtpError {
    fn from(err: CryptoError) -> Self {
        match err {
            CryptoError::AuthenticationFailed => Self::AuthenticationFailed,
            other => Self::Crypto(other),
        }
    }
}

/// Контекст SRTP: ключ, соль, режим и сквозной счётчик кадров.
pub struct SrtpContext {
    mode: EncryptionMode,
    counter: u64,
    gcm: Gcm,
    xchacha: XChaCha20Poly1305,
    nonce_seed: [u8; 16],
    /// Окно приёма: трейлеры (nonce) уже принятых кадров.
    receive_window: VecDeque<u32>,
    /// Множество трейлеров из окна приёма — для быстрой проверки повтора.
    receive_seen: HashSet<u32>,
}

impl fmt::Debug for SrtpContext {
    /// Ключевой материал не показывается: в отладке видны только режим и счётчик.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SrtpContext")
            .field("mode", &self.mode)
            .field("counter", &self.counter)
            .finish_non_exhaustive()
    }
}

impl SrtpContext {
    /// Создаёт контекст: `master_key` — 32 Б, `master_salt` — 14 Б (RFC 3711 master salt).
    ///
    /// Сессионный ключ и seed нонса выводятся из мастер-ключа и мастер-соли
    /// через HKDF-SHA1 (label-KDF, см. `crypto::kdf`).
    pub fn new(master_key: &[u8; 32], master_salt: &[u8; 14], mode: EncryptionMode) -> Self {
        let session_key = hkdf_sha1_32(master_key, master_salt, LABEL_KEY);
        let nonce_seed = hkdf_sha1_16(master_key, master_salt, LABEL_NONCE);
        Self {
            mode,
            counter: 0,
            gcm: Gcm::new(&Aes256Key::from_array(session_key)),
            xchacha: XChaCha20Poly1305::new(&session_key),
            nonce_seed,
            receive_window: VecDeque::new(),
            receive_seen: HashSet::new(),
        }
    }

    /// Создаёт контекст по имени режима движка; режимы вне карточки ⇒ `UnsupportedMode`.
    pub fn from_engine_name(
        master_key: &[u8; 32],
        master_salt: &[u8; 14],
        mode_name: &str,
    ) -> Result<Self, SrtpError> {
        let mode =
            EncryptionMode::from_engine_name(mode_name).ok_or(SrtpError::UnsupportedMode {
                name: unsupported_mode_name(mode_name),
            })?;
        Ok(Self::new(master_key, master_salt, mode))
    }

    /// Текущий счётчик `n` (следующий кадр получит `n`, затем `n + 1`).
    pub fn counter(&self) -> u64 {
        self.counter
    }

    /// Принудительно задаёт сквозной счётчик `n` (синхронизация, проверка границ).
    ///
    /// Движение назад запрещено: nonce выводится из счётчика, и повтор nonce под
    /// тем же ключом раскрывает XOR открытых текстов и позволяет подделать тег
    /// (см. `SrtpError::CounterRewind`).
    ///
    /// Значение больше `0x7FFF_FFFF` не представимо в трейлере ⇒ `CounterOverflow`.
    pub fn set_counter(&mut self, counter: u64) -> Result<(), SrtpError> {
        if counter < self.counter {
            return Err(SrtpError::CounterRewind {
                current: self.counter,
                requested: counter,
            });
        }
        if counter > MAX_COUNTER {
            return Err(SrtpError::CounterOverflow);
        }
        self.counter = counter;
        Ok(())
    }

    /// Шифрует RTP-кадр: результат = `[clear][ciphertext][tag16][counter4 LE]`.
    ///
    /// В режимах `*_rtpsize` clear-область — заголовок RTP (в т.ч. CSRC и
    /// расширение), полезная нагрузка шифруется, заголовок идёт в AAD.
    /// В режиме `aead_aes256_gcm` шифруется весь кадр, AAD пустой, clear-области
    /// в выводе нет.
    ///
    /// Счётчик растёт только при успехе: при ошибке он не меняется.
    pub fn encrypt_rtp(&mut self, packet: &[u8]) -> Result<Vec<u8>, SrtpError> {
        let trailer = self.next_trailer()?;
        if self.mode.covers_header() {
            let clear_len = rtp_header_len(packet)?;
            let (ciphertext, tag) =
                self.seal(&packet[clear_len..], &packet[..clear_len], trailer)?;
            let mut out = Vec::with_capacity(packet.len() + AEAD_TAG_LEN + TRAILER_LEN);
            out.extend_from_slice(&packet[..clear_len]);
            out.extend_from_slice(&ciphertext);
            out.extend_from_slice(&tag);
            out.extend_from_slice(&trailer);
            self.counter += 1;
            Ok(out)
        } else {
            let (ciphertext, tag) = self.seal(packet, &[], trailer)?;
            let mut out = Vec::with_capacity(packet.len() + AEAD_TAG_LEN + TRAILER_LEN);
            out.extend_from_slice(&ciphertext);
            out.extend_from_slice(&tag);
            out.extend_from_slice(&trailer);
            self.counter += 1;
            Ok(out)
        }
    }

    /// Расшифровывает RTP-кадр; при несовпадении тега — `AuthenticationFailed`.
    ///
    /// Возвращает исходный кадр (clear-область + открытая полезная нагрузка).
    /// Счётчик при расшифровке не меняется.
    pub fn decrypt_rtp(&mut self, packet: &[u8]) -> Result<Vec<u8>, SrtpError> {
        if self.mode.covers_header() {
            let clear_len = rtp_header_len(packet)?;
            let tail_len = AEAD_TAG_LEN + TRAILER_LEN;
            let min = clear_len + tail_len;
            if packet.len() < min {
                return Err(SrtpError::PacketTooShort {
                    len: packet.len(),
                    min,
                });
            }
            let tail_start = packet.len() - tail_len;
            let tag = &packet[tail_start..tail_start + AEAD_TAG_LEN];
            let trailer = read_trailer(&packet[tail_start + AEAD_TAG_LEN..]);
            let trailer_value = u32::from_le_bytes(trailer);
            self.check_receive_trailer(trailer_value)?;
            let ciphertext = &packet[clear_len..tail_start];
            let plaintext = self.open(ciphertext, tag, &packet[..clear_len], trailer)?;
            self.remember_receive_trailer(trailer_value);
            let mut out = Vec::with_capacity(clear_len + plaintext.len());
            out.extend_from_slice(&packet[..clear_len]);
            out.extend_from_slice(&plaintext);
            Ok(out)
        } else {
            let tail_len = AEAD_TAG_LEN + TRAILER_LEN;
            if packet.len() < tail_len {
                return Err(SrtpError::PacketTooShort {
                    len: packet.len(),
                    min: tail_len,
                });
            }
            let tail_start = packet.len() - tail_len;
            let tag = &packet[tail_start..tail_start + AEAD_TAG_LEN];
            let trailer = read_trailer(&packet[tail_start + AEAD_TAG_LEN..]);
            let trailer_value = u32::from_le_bytes(trailer);
            self.check_receive_trailer(trailer_value)?;
            let plaintext = self.open(&packet[..tail_start], tag, &[], trailer)?;
            self.remember_receive_trailer(trailer_value);
            Ok(plaintext)
        }
    }

    /// Шифрует SRTCP-кадр (clear = 8 Б: заголовок RTCP + SSRC).
    ///
    /// Результат = `[clear][ciphertext][tag16][counter4 LE]`; счётчик общий с RTP.
    pub fn encrypt_srtcp(&mut self, packet: &[u8]) -> Result<Vec<u8>, SrtpError> {
        let trailer = self.next_trailer()?;
        if self.mode.covers_header() {
            if packet.len() < SRTCP_CLEAR_LEN {
                return Err(SrtpError::PacketTooShort {
                    len: packet.len(),
                    min: SRTCP_CLEAR_LEN,
                });
            }
            if packet[0] >> 6 != 2 {
                return Err(SrtpError::MalformedHeader {
                    reason: "версия RTCP ≠ 2",
                });
            }
            let (ciphertext, tag) = self.seal(
                &packet[SRTCP_CLEAR_LEN..],
                &packet[..SRTCP_CLEAR_LEN],
                trailer,
            )?;
            let mut out = Vec::with_capacity(packet.len() + AEAD_TAG_LEN + TRAILER_LEN);
            out.extend_from_slice(&packet[..SRTCP_CLEAR_LEN]);
            out.extend_from_slice(&ciphertext);
            out.extend_from_slice(&tag);
            out.extend_from_slice(&trailer);
            self.counter += 1;
            Ok(out)
        } else {
            let (ciphertext, tag) = self.seal(packet, &[], trailer)?;
            let mut out = Vec::with_capacity(packet.len() + AEAD_TAG_LEN + TRAILER_LEN);
            out.extend_from_slice(&ciphertext);
            out.extend_from_slice(&tag);
            out.extend_from_slice(&trailer);
            self.counter += 1;
            Ok(out)
        }
    }

    /// Расшифровывает SRTCP-кадр; возвращает clear-область + открытую нагрузку.
    pub fn decrypt_srtcp(&mut self, packet: &[u8]) -> Result<Vec<u8>, SrtpError> {
        if self.mode.covers_header() {
            if packet.len() < SRTCP_CLEAR_LEN {
                return Err(SrtpError::PacketTooShort {
                    len: packet.len(),
                    min: SRTCP_CLEAR_LEN,
                });
            }
            if packet[0] >> 6 != 2 {
                return Err(SrtpError::MalformedHeader {
                    reason: "версия RTCP ≠ 2",
                });
            }
            let tail_len = AEAD_TAG_LEN + TRAILER_LEN;
            let min = SRTCP_CLEAR_LEN + tail_len;
            if packet.len() < min {
                return Err(SrtpError::PacketTooShort {
                    len: packet.len(),
                    min,
                });
            }
            let tail_start = packet.len() - tail_len;
            let tag = &packet[tail_start..tail_start + AEAD_TAG_LEN];
            let trailer = read_trailer(&packet[tail_start + AEAD_TAG_LEN..]);
            let trailer_value = u32::from_le_bytes(trailer);
            self.check_receive_trailer(trailer_value)?;
            let ciphertext = &packet[SRTCP_CLEAR_LEN..tail_start];
            let plaintext = self.open(ciphertext, tag, &packet[..SRTCP_CLEAR_LEN], trailer)?;
            self.remember_receive_trailer(trailer_value);
            let mut out = Vec::with_capacity(SRTCP_CLEAR_LEN + plaintext.len());
            out.extend_from_slice(&packet[..SRTCP_CLEAR_LEN]);
            out.extend_from_slice(&plaintext);
            Ok(out)
        } else {
            let tail_len = AEAD_TAG_LEN + TRAILER_LEN;
            if packet.len() < tail_len {
                return Err(SrtpError::PacketTooShort {
                    len: packet.len(),
                    min: tail_len,
                });
            }
            let tail_start = packet.len() - tail_len;
            let tag = &packet[tail_start..tail_start + AEAD_TAG_LEN];
            let trailer = read_trailer(&packet[tail_start + AEAD_TAG_LEN..]);
            let trailer_value = u32::from_le_bytes(trailer);
            self.check_receive_trailer(trailer_value)?;
            let plaintext = self.open(&packet[..tail_start], tag, &[], trailer)?;
            self.remember_receive_trailer(trailer_value);
            Ok(plaintext)
        }
    }

    /// Трейлер исходящего кадра: `0x80000000 | n` в little endian.
    fn next_trailer(&self) -> Result<[u8; TRAILER_LEN], SrtpError> {
        if self.counter > MAX_COUNTER {
            return Err(SrtpError::CounterOverflow);
        }
        let value = 0x8000_0000u32 | (self.counter as u32);
        Ok(value.to_le_bytes())
    }

    /// Шифрование полезной нагрузки: возвращает `(ciphertext, tag)`.
    fn seal(
        &self,
        plaintext: &[u8],
        aad: &[u8],
        trailer: [u8; TRAILER_LEN],
    ) -> Result<(Vec<u8>, [u8; AEAD_TAG_LEN]), SrtpError> {
        match self.mode {
            EncryptionMode::Aes256GcmRtpSize | EncryptionMode::Aes256Gcm => {
                let nonce = self.gcm_nonce(trailer);
                let out = self.gcm.encrypt(&nonce, aad, plaintext)?;
                Ok(split_ciphertext_tag(out))
            }
            EncryptionMode::XChaCha20Poly1305RtpSize => {
                let nonce = self.xchacha_nonce(trailer);
                let out = self.xchacha.encrypt(&nonce, aad, plaintext)?;
                Ok(split_ciphertext_tag(out))
            }
        }
    }

    /// Расшифрование полезной нагрузки.
    fn open(
        &self,
        ciphertext: &[u8],
        tag: &[u8],
        aad: &[u8],
        trailer: [u8; TRAILER_LEN],
    ) -> Result<Vec<u8>, SrtpError> {
        let mut buf = Vec::with_capacity(ciphertext.len() + tag.len());
        buf.extend_from_slice(ciphertext);
        buf.extend_from_slice(tag);
        match self.mode {
            EncryptionMode::Aes256GcmRtpSize | EncryptionMode::Aes256Gcm => {
                let nonce = self.gcm_nonce(trailer);
                Ok(self.gcm.decrypt(&nonce, aad, &buf)?)
            }
            EncryptionMode::XChaCha20Poly1305RtpSize => {
                let nonce = self.xchacha_nonce(trailer);
                Ok(self.xchacha.decrypt(&nonce, aad, &buf)?)
            }
        }
    }

    /// Проверяет, что трейлер (nonce) ещё не использовался на приёме.
    ///
    /// Повтор nonce под тем же ключом — катастрофа GCM: раскрывается XOR открытых
    /// текстов и подделывается тег. Поэтому кадр с уже использованным трейлером
    /// отклоняется до попытки расшифровки.
    fn check_receive_trailer(&self, trailer: u32) -> Result<(), SrtpError> {
        if self.receive_seen.contains(&trailer) {
            return Err(SrtpError::NonceReuse { trailer });
        }
        Ok(())
    }

    /// Запоминает использованный на приёме трейлер (nonce).
    ///
    /// Окно ограничено (`RECEIVE_WINDOW` последних трейлеров): при переполнении
    /// вытесняется самый старый. Окно приёма не связано со счётчиком отправки.
    fn remember_receive_trailer(&mut self, trailer: u32) {
        if self.receive_seen.insert(trailer) {
            self.receive_window.push_back(trailer);
            if self.receive_window.len() > RECEIVE_WINDOW
                && let Some(oldest) = self.receive_window.pop_front()
            {
                self.receive_seen.remove(&oldest);
            }
        }
    }

    /// Nonce GCM: seed сессии (8 Б) + трейлер кадра (4 Б).
    fn gcm_nonce(&self, trailer: [u8; TRAILER_LEN]) -> [u8; GCM_NONCE_SIZE] {
        let mut nonce = [0u8; GCM_NONCE_SIZE];
        nonce[..8].copy_from_slice(&self.nonce_seed[..8]);
        nonce[8..].copy_from_slice(&trailer);
        nonce
    }

    /// Nonce XChaCha20-Poly1305: трейлер кадра (4 Б) + seed сессии (16 Б) + нули (4 Б).
    fn xchacha_nonce(&self, trailer: [u8; TRAILER_LEN]) -> [u8; XCHACHA_NONCE_SIZE] {
        let mut nonce = [0u8; XCHACHA_NONCE_SIZE];
        nonce[..4].copy_from_slice(&trailer);
        nonce[4..20].copy_from_slice(&self.nonce_seed);
        nonce
    }
}

/// Отделяет шифротекст от тега AEAD.
fn split_ciphertext_tag(out: Vec<u8>) -> (Vec<u8>, [u8; AEAD_TAG_LEN]) {
    let split = out.len() - AEAD_TAG_LEN;
    let mut tag = [0u8; AEAD_TAG_LEN];
    tag.copy_from_slice(&out[split..]);
    (out[..split].to_vec(), tag)
}

/// Читает трейлер-счётчик (4 Б, little endian) — как непрозрачные байты.
fn read_trailer(tail: &[u8]) -> [u8; TRAILER_LEN] {
    let mut trailer = [0u8; TRAILER_LEN];
    trailer.copy_from_slice(&tail[..TRAILER_LEN]);
    trailer
}

/// Имя неподдерживаемого режима (статическая строка для `UnsupportedMode`).
fn unsupported_mode_name(name: &str) -> &'static str {
    for known in UNSUPPORTED_ENGINE_MODES {
        if known == name {
            return known;
        }
    }
    "unknown-engine-mode"
}

/// Длина clear-области RTP-кадра: 12 Б + CSRC + заголовок расширения.
///
/// Ошибки: `PacketTooShort` (кадр короче заголовка или не влезает ext-заголовок),
/// `MalformedHeader` (версия ≠ 2, `CC > 15`, длина расширения не влезает в кадр).
fn rtp_header_len(packet: &[u8]) -> Result<usize, SrtpError> {
    if packet.len() < RTP_HEADER_MIN {
        return Err(SrtpError::PacketTooShort {
            len: packet.len(),
            min: RTP_HEADER_MIN,
        });
    }
    if packet[0] >> 6 != 2 {
        return Err(SrtpError::MalformedHeader {
            reason: "версия RTP ≠ 2",
        });
    }
    let cc = usize::from(packet[0] & 0x0f);
    if cc > 15 {
        return Err(SrtpError::MalformedHeader { reason: "CC > 15" });
    }
    let mut header_len = RTP_HEADER_MIN + 4 * cc;
    if packet[0] & 0x10 != 0 {
        if packet.len() < header_len + 4 {
            return Err(SrtpError::PacketTooShort {
                len: packet.len(),
                min: header_len + 4,
            });
        }
        let ext_len_words = usize::from(u16::from_be_bytes([
            packet[header_len + 2],
            packet[header_len + 3],
        ]));
        header_len += 4 + 4 * ext_len_words;
        if packet.len() < header_len {
            return Err(SrtpError::MalformedHeader {
                reason: "длина RTP-расширения не влезает в кадр",
            });
        }
    }
    Ok(header_len)
}
