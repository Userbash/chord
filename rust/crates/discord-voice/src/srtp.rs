//! SRTP/SRTCP-кадрирование Discord (`aead_aes256_gcm_rtpsize`, `aead_xchacha20_poly1305_rtpsize`) — карточки P4-03 / BE-RE-05 (F-VC-01).
//!
//! Раскладка кадра подтверждена capture медиа-пакетов движка
//! (`strace-media-s3.jsonl`, разбор `waves/RW-25/research/media-packet-layout.mjs`)
//! и дизассемблированием `discord::encryption::PacketCrypter` в `discord_voice.node`
//! (`voice-deep.json`, `voice-deep.md`):
//!
//! ```text
//! [clear RTP/SRTCP header (AAD)][ciphertext][tag 16][nonce_counter 4 LE]
//! ```
//!
//! В режимах Discord `*_rtpsize`:
//! - 32-байтный `secret_key` из OP 4 `SESSION_DESCRIPTION` используется **напрямую**
//!   как ключ AEAD (`AES-256-GCM` или `XChaCha20-Poly1305`) без HKDF;
//! - к концу каждого пакета дописывается 4-байтный little-endian трейлер `nonce_counter: u32`,
//!   из которого формируется 12-байтный (для AES-GCM) или 24-байтный (для XChaCha20-Poly1305)
//!   nonce с нулями в старших байтах (`nonce[0..4] = trailer`, `nonce[4..] = 0`),
//!   а открытый заголовок RTP/SRTCP передаётся как AAD.
//!
//! Исходящий счётчик = `0x80000000 | n`, где `n` — сквозной счётчик, **общий**
//! для RTP и SRTCP одного сокета (в capture 8 RTP-пакетов дали `n = 0..7`, затем
//! SR-пакеты — `n = 8, 9, 10`).
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
/// Маркер конца медиа-кадра DAVE E2EE (`0xFAFA`, спецификация Discord DAVE / `davey`).
pub const DAVE_MARKER_BYTES: [u8; 2] = [0xFA, 0xFA];
/// Смещение 32-битного усечённого счётчика в 12-байтном nonce DAVE AES-GCM (`12 - 4 = 8`).
const DAVE_TRUNCATED_NONCE_OFFSET: usize = GCM_NONCE_SIZE - 4;

/// Абстракция второго слоя сквозного шифрования медиа-фреймов DAVE MLS E2EE поверх SRTP.
pub trait DaveFrameCipher {
    /// Шифрует открытую полезную нагрузку медиа-кадра перед `cipher.seal`.
    fn encrypt(&mut self, plaintext: &[u8]) -> Result<Vec<u8>, SrtpError>;

    /// Расшифровывает полезную нагрузку медиа-кадра после `cipher.open`.
    fn decrypt(&mut self, frame: &[u8]) -> Result<Vec<u8>, SrtpError>;
}

/// Криптоконтекст второго слоя DAVE MLS E2EE для медиа-фреймов (`dave.encrypt` / `dave.decrypt`).
///
/// Формат кадра DAVE поверх полезной нагрузки RTP (сохраняя открытыми байты заголовка
/// и RTP-расширений `0xBEDE` в режиме `_rtpsize`):
/// `[dave_ciphertext][16B gcm_tag][leb128(truncated_nonce: u32)][supplemental_size: u8][0xFA, 0xFA]`.
pub struct DaveMediaCipher {
    epoch: u64,
    counter: u32,
    gcm: Gcm,
}

impl fmt::Debug for DaveMediaCipher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DaveMediaCipher")
            .field("epoch", &self.epoch)
            .field("counter", &self.counter)
            .finish_non_exhaustive()
    }
}

impl DaveMediaCipher {
    /// Создаёт шифр DAVE E2EE для эпохи `1` из 32-байтного ключа рачет-эпохи.
    #[must_use]
    pub fn new(key: &[u8; 32]) -> Self {
        Self::with_epoch(1, key)
    }

    /// Создаёт шифр DAVE E2EE для заданной MLS-эпохи `epoch` и 32-байтного ключа.
    #[must_use]
    pub fn with_epoch(epoch: u64, key: &[u8; 32]) -> Self {
        Self {
            epoch,
            counter: 0,
            gcm: Gcm::new(&Aes256Key::from_array(*key)),
        }
    }

    /// Номер текущей MLS-эпохи.
    #[must_use]
    pub const fn epoch(&self) -> u64 {
        self.epoch
    }

    /// Текущий 32-битный усечённый счётчик кадров DAVE.
    #[must_use]
    pub const fn counter(&self) -> u32 {
        self.counter
    }

    /// Шифрует медиа-нагрузку в кадр DAVE с supplemental-футером и маркером `0xFAFA`.
    pub fn encrypt(&mut self, plaintext: &[u8]) -> Result<Vec<u8>, SrtpError> {
        let truncated_nonce = self
            .counter
            .checked_add(1)
            .ok_or(SrtpError::CounterOverflow)?;
        let mut nonce = [0u8; GCM_NONCE_SIZE];
        nonce[DAVE_TRUNCATED_NONCE_OFFSET..].copy_from_slice(&truncated_nonce.to_le_bytes());

        let ct_and_tag = self.gcm.encrypt(&nonce, &[], plaintext)?;
        let mut nonce_leb = [0u8; 5];
        let nonce_leb_len = encode_leb128_u32(truncated_nonce, &mut nonce_leb);
        let supplemental_size =
            u8::try_from(AEAD_TAG_LEN + nonce_leb_len + 1 + DAVE_MARKER_BYTES.len())
                .map_err(|_| SrtpError::CounterOverflow)?;

        let mut out =
            Vec::with_capacity(ct_and_tag.len() + nonce_leb_len + 1 + DAVE_MARKER_BYTES.len());
        out.extend_from_slice(&ct_and_tag);
        out.extend_from_slice(&nonce_leb[..nonce_leb_len]);
        out.push(supplemental_size);
        out.extend_from_slice(&DAVE_MARKER_BYTES);

        self.counter = truncated_nonce;
        Ok(out)
    }

    /// Расшифровывает кадр DAVE, проверяя маркер `0xFAFA`, supplemental-футер и тег AEAD.
    pub fn decrypt(&mut self, frame: &[u8]) -> Result<Vec<u8>, SrtpError> {
        let min_len = AEAD_TAG_LEN + 1 + 1 + DAVE_MARKER_BYTES.len();
        if frame.len() < min_len {
            return Err(SrtpError::PacketTooShort {
                len: frame.len(),
                min: min_len,
            });
        }
        if frame[frame.len() - DAVE_MARKER_BYTES.len()..] != DAVE_MARKER_BYTES {
            return Err(SrtpError::AuthenticationFailed);
        }
        let supplemental_size = usize::from(frame[frame.len() - 3]);
        if supplemental_size < min_len || frame.len() < supplemental_size {
            return Err(SrtpError::AuthenticationFailed);
        }
        let ciphertext_end = frame.len() - supplemental_size;
        let tag_end = ciphertext_end + AEAD_TAG_LEN;
        let nonce_slice = &frame[tag_end..frame.len() - 3];
        let truncated_nonce =
            decode_leb128_u32(nonce_slice).ok_or(SrtpError::AuthenticationFailed)?;

        let mut nonce = [0u8; GCM_NONCE_SIZE];
        nonce[DAVE_TRUNCATED_NONCE_OFFSET..].copy_from_slice(&truncated_nonce.to_le_bytes());

        let ct_and_tag = &frame[..tag_end];
        Ok(self.gcm.decrypt(&nonce, &[], ct_and_tag)?)
    }
}

impl DaveFrameCipher for DaveMediaCipher {
    fn encrypt(&mut self, plaintext: &[u8]) -> Result<Vec<u8>, SrtpError> {
        Self::encrypt(self, plaintext)
    }

    fn decrypt(&mut self, frame: &[u8]) -> Result<Vec<u8>, SrtpError> {
        Self::decrypt(self, frame)
    }
}

fn encode_leb128_u32(mut value: u32, out: &mut [u8; 5]) -> usize {
    let mut len = 0;
    loop {
        let mut byte = (value & 0x7F) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        out[len] = byte;
        len += 1;
        if value == 0 {
            break;
        }
    }
    len
}

fn decode_leb128_u32(bytes: &[u8]) -> Option<u32> {
    if bytes.is_empty() || bytes.len() > 5 {
        return None;
    }
    let mut result = 0u32;
    let mut shift = 0u32;
    for (idx, &byte) in bytes.iter().enumerate() {
        let low = u32::from(byte & 0x7F);
        if shift >= 32 || (shift == 28 && low > 0x0F) {
            return None;
        }
        result |= low << shift;
        if (byte & 0x80) == 0 {
            return if idx + 1 == bytes.len() {
                Some(result)
            } else {
                None
            };
        }
        shift += 7;
    }
    None
}
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
    /// Опциональный второй слой сквозного шифрования медиа-кадров DAVE MLS E2EE.
    dave: Option<DaveMediaCipher>,
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
    /// Создаёт контекст для канонического режима Discord directly из 32-байтного
    /// `secret_key` (OP 4 `SESSION_DESCRIPTION`).
    pub fn from_secret_key(secret_key: &[u8; 32], mode: EncryptionMode) -> Self {
        Self::new(secret_key, &[0u8; 14], mode)
    }

    /// Создаёт контекст напрямую из 32-байтного `secret_key` (OP 4 `SESSION_DESCRIPTION`)
    /// и строкового имени режима движка (`aead_aes256_gcm_rtpsize`, `aead_xchacha20_poly1305_rtpsize`).
    pub fn from_secret_key_and_mode(
        secret_key: &[u8; 32],
        mode_name: &str,
    ) -> Result<Self, SrtpError> {
        Self::from_engine_name(secret_key, &[0u8; 14], mode_name)
    }

    /// Создаёт контекст: `master_key` — 32 Б (`secret_key` из OP 4), `master_salt` — 14 Б.
    ///
    /// В режимах Discord `*_rtpsize` (`aead_aes256_gcm_rtpsize`, `aead_xchacha20_poly1305_rtpsize`)
    /// 32-байтный `master_key` используется **напрямую** как ключ AEAD без HKDF, а nonce
    /// формируется из 4-байтного трейлера `nonce_counter` с нулями в старших байтах.
    pub fn new(master_key: &[u8; 32], master_salt: &[u8; 14], mode: EncryptionMode) -> Self {
        let (session_key, nonce_seed) = if mode.covers_header() {
            (*master_key, [0u8; 16])
        } else {
            (
                hkdf_sha1_32(master_key, master_salt, LABEL_KEY),
                hkdf_sha1_16(master_key, master_salt, LABEL_NONCE),
            )
        };
        Self {
            mode,
            counter: 0,
            gcm: Gcm::new(&Aes256Key::from_array(session_key)),
            xchacha: XChaCha20Poly1305::new(&session_key),
            nonce_seed,
            dave: None,
            receive_window: VecDeque::new(),
            receive_seen: HashSet::new(),
        }
    }

    /// Устанавливает или отключает второй слой шифрования DAVE MLS E2EE поверх SRTP (`_rtpsize`).
    pub fn set_dave_cipher(&mut self, dave: Option<DaveMediaCipher>) {
        self.dave = dave;
    }

    /// Ссылка на установленный шифр второго слоя DAVE MLS E2EE.
    #[must_use]
    pub const fn dave_cipher(&self) -> Option<&DaveMediaCipher> {
        self.dave.as_ref()
    }

    /// Мутабельная ссылка на установленный шифр второго слоя DAVE MLS E2EE.
    pub fn dave_cipher_mut(&mut self) -> Option<&mut DaveMediaCipher> {
        self.dave.as_mut()
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

    /// Двухслойное шифрование RTP-кадра DAVE MLS E2EE + SRTP:
    /// 1) `dave.encrypt` шифрует полезную нагрузку медиа-фрейма, сохраняя нешифрованными
    ///    байты заголовка RTP и RTP-расширений (`0xBEDE`) в области `0..clear_len`;
    /// 2) `cipher.seal` запечатывает пакет транспортным AEAD с заголовком/расширениями в AAD (в режиме `*_rtpsize`).
    pub fn encrypt_rtp_with_dave<D: DaveFrameCipher>(
        &mut self,
        packet: &[u8],
        dave: &mut D,
    ) -> Result<Vec<u8>, SrtpError> {
        let trailer = self.next_trailer()?;
        let clear_len = rtp_header_len(packet)?;
        let dave_payload = dave.encrypt(&packet[clear_len..])?;
        if self.mode.covers_header() {
            let (ciphertext, tag) = self.seal(&dave_payload, &packet[..clear_len], trailer)?;
            let mut out =
                Vec::with_capacity(clear_len + ciphertext.len() + AEAD_TAG_LEN + TRAILER_LEN);
            out.extend_from_slice(&packet[..clear_len]);
            out.extend_from_slice(&ciphertext);
            out.extend_from_slice(&tag);
            out.extend_from_slice(&trailer);
            self.counter += 1;
            Ok(out)
        } else {
            let mut staged = Vec::with_capacity(clear_len + dave_payload.len());
            staged.extend_from_slice(&packet[..clear_len]);
            staged.extend_from_slice(&dave_payload);
            let (ciphertext, tag) = self.seal(&staged, &[], trailer)?;
            let mut out = Vec::with_capacity(ciphertext.len() + AEAD_TAG_LEN + TRAILER_LEN);
            out.extend_from_slice(&ciphertext);
            out.extend_from_slice(&tag);
            out.extend_from_slice(&trailer);
            self.counter += 1;
            Ok(out)
        }
    }

    /// Двухслойная расшифровка RTP-кадра SRTP + DAVE MLS E2EE:
    /// 1) `cipher.open` снимает транспортный слой SRTP и проверяет AAD заголовка/расширений RTP;
    /// 2) `dave.decrypt` снимает слой DAVE E2EE с медиа-нагрузки, сохраняя нешифрованные байты
    ///    заголовка RTP и RTP-расширений в режиме `*_rtpsize`.
    pub fn decrypt_rtp_with_dave<D: DaveFrameCipher>(
        &mut self,
        packet: &[u8],
        dave: &mut D,
    ) -> Result<Vec<u8>, SrtpError> {
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
            let dave_frame = self.open(ciphertext, tag, &packet[..clear_len], trailer)?;
            let plaintext = dave.decrypt(&dave_frame)?;
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
            let opened = self.open(&packet[..tail_start], tag, &[], trailer)?;
            let clear_len = rtp_header_len(&opened)?;
            let plaintext = dave.decrypt(&opened[clear_len..])?;
            self.remember_receive_trailer(trailer_value);
            let mut out = Vec::with_capacity(clear_len + plaintext.len());
            out.extend_from_slice(&opened[..clear_len]);
            out.extend_from_slice(&plaintext);
            Ok(out)
        }
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
        if let Some(mut dave) = self.dave.take() {
            let res = self.encrypt_rtp_with_dave(packet, &mut dave);
            self.dave = Some(dave);
            return res;
        }
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
        if let Some(mut dave) = self.dave.take() {
            let res = self.decrypt_rtp_with_dave(packet, &mut dave);
            self.dave = Some(dave);
            return res;
        }
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

    /// Расшифровывает входящий RTP-пакет голосового сервера Discord (`*_rtpsize`)
    /// и возвращает чистый Opus-кадр без заголовка RTP и тела расширения `0xBEDE`.
    ///
    /// В боевом трафике Discord (`strace-media-s3.jsonl`) при `X=1` в открытый AAD
    /// входят 12 байт RTP + `4*CC` + 4 байта преамбулы расширения (`0xBEDE` + `length`),
    /// а тело расширения (`4 * length` байт) шифруется вместе с Opus-кадром.
    pub fn decrypt_discord_rtp_opus(&mut self, packet: &[u8]) -> Result<Vec<u8>, SrtpError> {
        let tail_len = AEAD_TAG_LEN + TRAILER_LEN;
        let min = RTP_HEADER_MIN + tail_len;
        if packet.len() < min {
            return Err(SrtpError::PacketTooShort {
                len: packet.len(),
                min,
            });
        }
        if packet[0] >> 6 != 2 {
            return Err(SrtpError::MalformedHeader {
                reason: "версия RTP ≠ 2",
            });
        }
        let cc = usize::from(packet[0] & 0x0f);
        let has_ext = (packet[0] & 0x10) != 0;
        let base_hdr = RTP_HEADER_MIN + cc * 4;
        let tail_start = packet.len() - tail_len;
        let tag = &packet[tail_start..tail_start + AEAD_TAG_LEN];
        let trailer = read_trailer(&packet[tail_start + AEAD_TAG_LEN..]);
        let has_padding = (packet[0] & 0x20) != 0;
        let strip_padding = |mut data: Vec<u8>| -> Vec<u8> {
            if has_padding && !data.is_empty() {
                let pad_len = usize::from(data[data.len() - 1]);
                if pad_len > 0 && pad_len <= data.len() {
                    data.truncate(data.len() - pad_len);
                }
            }
            data
        };
        if has_ext && base_hdr + 4 <= tail_start {
            let aad_len = base_hdr + 4;
            let ext_words = usize::from(u16::from_be_bytes([
                packet[base_hdr + 2],
                packet[base_hdr + 3],
            ]));
            let ext_body_len = ext_words * 4;
            let ciphertext = &packet[aad_len..tail_start];
            if let Ok(plaintext) = self.open(ciphertext, tag, &packet[..aad_len], trailer)
                && plaintext.len() >= ext_body_len
            {
                return Ok(strip_padding(plaintext[ext_body_len..].to_vec()));
            }
        }
        let clear_len = rtp_header_len(packet)?;
        if clear_len <= tail_start {
            let ciphertext = &packet[clear_len..tail_start];
            let plaintext = self.open(ciphertext, tag, &packet[..clear_len], trailer)?;
            return Ok(strip_padding(plaintext));
        }
        Err(SrtpError::AuthenticationFailed)
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

    /// Nonce GCM (12 Б):
    /// - в режиме `aead_aes256_gcm_rtpsize`: 4-байтный трейлер `nonce_counter` в младших байтах
    ///   `nonce[0..4]` и нули в старших байтах `nonce[4..12]`;
    /// - в режиме `aead_aes256_gcm`: `nonce_seed[0..8]` + трейлер кадра (4 Б).
    fn gcm_nonce(&self, trailer: [u8; TRAILER_LEN]) -> [u8; GCM_NONCE_SIZE] {
        let mut nonce = [0u8; GCM_NONCE_SIZE];
        if self.mode.covers_header() {
            nonce[..TRAILER_LEN].copy_from_slice(&trailer);
        } else {
            nonce[..8].copy_from_slice(&self.nonce_seed[..8]);
            nonce[8..].copy_from_slice(&trailer);
        }
        nonce
    }

    /// Nonce XChaCha20-Poly1305 (24 Б):
    /// 4-байтный трейлер `nonce_counter` в младших байтах `nonce[0..4]` и нули в старших байтах `nonce[4..24]`.
    fn xchacha_nonce(&self, trailer: [u8; TRAILER_LEN]) -> [u8; XCHACHA_NONCE_SIZE] {
        let mut nonce = [0u8; XCHACHA_NONCE_SIZE];
        nonce[..TRAILER_LEN].copy_from_slice(&trailer);
        if !self.mode.covers_header() {
            nonce[4..20].copy_from_slice(&self.nonce_seed);
        }
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

/// Максимальное число участников голосового канала для DAVE MLS (`Serein MAX_PARTICIPANTS = 64`).
pub const DAVE_MAX_PARTICIPANTS: usize = 64;
/// Максимальный размер сигнального MLS-сообщения (`Serein MAX_SIGNAL = 64 КБ`).
pub const DAVE_MAX_SIGNAL_BYTES: usize = 64 * 1024;
/// Максимальное число авто-сбросов DAVE MLS при `InvalidCommitWelcome` (`Serein resets <= 3`).
pub const DAVE_MAX_RESETS: u8 = 3;
/// Максимальное число переходов эпох MLS на одну сессию (`Serein epochs <= 1024`).
pub const DAVE_MAX_EPOCH_TRANSITIONS: u16 = 1024;
/// Профиль однобайтового RTP-расширения RFC 5285 / RFC 8285 (`0xBEDE`).
pub const RFC5285_ONE_BYTE_PROFILE: u16 = 0xBEDE;

/// Результат обработки серверного переходного кадра DAVE MLS (`op:29 Commit` / `op:30 Welcome`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DaveTransitionOutcome {
    /// Переход с `transition_id == 0` применён немедленно без отправки `op:23`.
    ExecutedImmediate {
        /// Номер активированной MLS-эпохи.
        epoch: u64,
    },
    /// Коммит/Welcome принят: клиенту нужно отправить JSON `op:23 TransitionReady`.
    SendTransitionReadyOp23 {
        /// Идентификатор подготовленного перехода.
        transition_id: u16,
        /// Сериализованный JSON-кадр `{"d":{"transition_id":...},"op":23}`.
        op23_json: String,
    },
    /// Коммит/Welcome отклонён: клиенту нужно отправить JSON `op:31 InvalidCommitWelcome`
    /// и свежий бинарный `op:26 KeyPackage`.
    InvalidCommitWelcome {
        /// Идентификатор отклонённого перехода.
        transition_id: u16,
        /// Сериализованный JSON-кадр `{"d":{"transition_id":...},"op":31}`.
        op31_json: String,
        /// Свежий бинарный кадр `op:26 KeyPackage` (`[26, ...]`).
        fresh_key_package_op26: Vec<u8>,
    },
}

/// Расшифрованный двухслойным конвейером (`SRTP + DAVE E2EE`) RTP-пакет со снятым
/// заголовком и расширением RFC 5285 (`0xBEDE`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DaveStrippedRtpPacket {
    /// SSRC источника медиа-потока.
    pub ssrc: u32,
    /// Порядковый номер RTP-пакета (`sequence`).
    pub sequence: u16,
    /// Метка времени RTP (`timestamp`).
    pub timestamp: u32,
    /// Бит маркера RTP (`M`).
    pub marker: bool,
    /// Тип полезной нагрузки (`payload_type`, например `120` для Opus, `101` для видео).
    pub payload_type: u8,
    /// Профиль расширения заголовка RTP (например, `Some(0xBEDE)`), если расширение присутствовало.
    pub extension_profile: Option<u16>,
    /// Снятые байты тела расширения RTP (`4 * ext_words`), если расширение присутствовало.
    pub extension_data: Option<Vec<u8>>,
    /// Очищенная полезная нагрузка медиа-кадра после снятия SRTP, RFC 5285 `0xBEDE` и DAVE E2EE.
    pub payload: Vec<u8>,
}

/// Движок протокола сквозного шифрования DAVE v1 MLS поверх `SrtpContext` (этап V3, по эталону Serein `crypto::Dave`).
pub struct DaveMlsEngine {
    own_user_id: u64,
    channel_id: u64,
    protocol_version: u8,
    secret_key: [u8; 32],
    srtp: SrtpContext,
    recognized_users: Vec<u64>,
    announced_users: Vec<u64>,
    external_sender: Option<Vec<u8>>,
    pending_commit: Option<Vec<u8>>,
    pending_epoch_key: Option<[u8; 32]>,
    pending_transition: Option<u16>,
    pending_protocol_transition: Option<u16>,
    epoch: u64,
    epochs_budget: u16,
    resets: u8,
    key_package_generation: u32,
    ready: bool,
    waiting_for_peer: bool,
}

impl fmt::Debug for DaveMlsEngine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DaveMlsEngine")
            .field("own_user_id", &self.own_user_id)
            .field("channel_id", &self.channel_id)
            .field("protocol_version", &self.protocol_version)
            .field("recognized_users", &self.recognized_users)
            .field("epoch", &self.epoch)
            .field("ready", &self.ready)
            .field("waiting_for_peer", &self.waiting_for_peer)
            .field("resets", &self.resets)
            .finish_non_exhaustive()
    }
}

impl DaveMlsEngine {
    /// Инициализирует движок DAVE v1 MLS и транспортный `SrtpContext` при получении
    /// `op:4 SessionDescription` и немедленно возвращает бинарный кадр `op:26 KeyPackage` (`[26, ...]`).
    pub fn init_on_op4(
        own_user_id: u64,
        channel_id: u64,
        dave_protocol_version: u8,
        secret_key: &[u8; 32],
        mode: EncryptionMode,
    ) -> Result<(Self, Vec<u8>), SrtpError> {
        if own_user_id == 0 || channel_id == 0 {
            return Err(SrtpError::MalformedHeader {
                reason: "некорректный own_user_id или channel_id для DAVE MLS",
            });
        }
        if dave_protocol_version != 1 {
            return Err(SrtpError::MalformedHeader {
                reason: "требуется DAVE protocol_version == 1 (даунгрейд отклонён)",
            });
        }
        let srtp = SrtpContext::from_secret_key(secret_key, mode);
        let mut engine = Self {
            own_user_id,
            channel_id,
            protocol_version: dave_protocol_version,
            secret_key: *secret_key,
            srtp,
            recognized_users: vec![own_user_id],
            announced_users: vec![own_user_id],
            external_sender: None,
            pending_commit: None,
            pending_epoch_key: None,
            pending_transition: None,
            pending_protocol_transition: None,
            epoch: 0,
            epochs_budget: 0,
            resets: 0,
            key_package_generation: 0,
            ready: false,
            waiting_for_peer: false,
        };
        let kp = engine.key_package_op26();
        Ok((engine, kp))
    }

    /// Генерирует свежий бинарный кадр `op:26 KeyPackage` (`[26, ...payload...]`).
    #[must_use]
    pub fn key_package_op26(&mut self) -> Vec<u8> {
        self.key_package_generation = self.key_package_generation.wrapping_add(1);
        let mut salt = [0u8; 14];
        salt[..8].copy_from_slice(&self.channel_id.to_be_bytes());
        salt[8..12].copy_from_slice(&self.key_package_generation.to_be_bytes());
        salt[12] = self.resets;
        salt[13] = self.protocol_version;
        let leaf_pub = hkdf_sha1_32(&self.secret_key, &salt, b"discord-lab/dave/key-package");

        let mut frame = Vec::with_capacity(1 + 2 + 8 + 8 + 4 + 32);
        frame.push(crate::dave::OP_MLS_KEY_PACKAGE);
        frame.extend_from_slice(&1u16.to_be_bytes());
        frame.extend_from_slice(&self.channel_id.to_be_bytes());
        frame.extend_from_slice(&self.own_user_id.to_be_bytes());
        frame.extend_from_slice(&self.key_package_generation.to_be_bytes());
        frame.extend_from_slice(&leaf_pub);
        frame
    }

    /// Список распознанных участников звонка (`recognized_users`), включая `own_user_id` и пиров из `op:11/13`.
    #[must_use]
    pub fn recognized_users(&self) -> &[u64] {
        &self.recognized_users
    }

    /// Проверяет, входит ли пользователь `user_id` в `recognized_users`.
    #[must_use]
    pub fn contains_user(&self, user_id: u64) -> bool {
        self.recognized_users.contains(&user_id)
    }

    /// Возвращает `true`, если в канале объявлен только наш собственный клиент.
    #[must_use]
    pub fn is_alone(&self) -> bool {
        self.announced_users.len() <= 1
    }

    /// Возвращает `true`, если MLS-группа сведена и второй слой шифрования DAVE E2EE активен.
    #[must_use]
    pub const fn is_ready(&self) -> bool {
        self.ready
    }

    /// Возвращает `true`, если единственный участник перешёл в режим ожидания пира (`WaitingForPeer`).
    #[must_use]
    pub const fn is_waiting_for_peer(&self) -> bool {
        self.waiting_for_peer
    }

    /// Текущая активная MLS-эпоха.
    #[must_use]
    pub const fn epoch(&self) -> u64 {
        self.epoch
    }

    /// Количество выполненных сбросов после `InvalidCommitWelcome`.
    #[must_use]
    pub const fn resets(&self) -> u8 {
        self.resets
    }

    /// Обрабатывает `op:11 ClientsConnect`: регистрирует новых участников в `recognized_users`.
    pub fn on_clients_connect_op11(&mut self, user_ids: &[u64]) -> Result<bool, SrtpError> {
        if user_ids.len() > DAVE_MAX_PARTICIPANTS || user_ids.contains(&0) {
            return Err(SrtpError::MalformedHeader {
                reason: "некорректный список участников в op:11 ClientsConnect",
            });
        }
        let mut next_recognized = self.recognized_users.clone();
        for &user in user_ids {
            if !next_recognized.contains(&user) {
                if next_recognized.len() >= DAVE_MAX_PARTICIPANTS {
                    return Err(SrtpError::MalformedHeader {
                        reason: "превышен лимит 64 участников в голосовом канале",
                    });
                }
                next_recognized.push(user);
            }
        }
        let mut next_announced = self.announced_users.clone();
        for &user in user_ids {
            if !next_announced.contains(&user) {
                next_announced.push(user);
            }
        }
        let changed =
            next_recognized != self.recognized_users || next_announced != self.announced_users;
        if changed {
            self.recognized_users = next_recognized;
            self.announced_users = next_announced;
            self.waiting_for_peer = false;
        }
        Ok(changed)
    }

    /// Обрабатывает `op:13 ClientDisconnect`: удаляет вышедшего участника из `recognized_users`.
    pub fn on_client_disconnect_op13(&mut self, user_id: u64) -> Result<bool, SrtpError> {
        if user_id == 0 || user_id == self.own_user_id {
            return Err(SrtpError::MalformedHeader {
                reason: "недопустимый user_id в op:13 ClientDisconnect",
            });
        }
        let before = self.recognized_users.len() + self.announced_users.len();
        self.recognized_users.retain(|&id| id != user_id);
        self.announced_users.retain(|&id| id != user_id);
        let changed = before != self.recognized_users.len() + self.announced_users.len();
        if changed {
            self.ready = false;
            self.waiting_for_peer = self.is_alone();
        }
        Ok(changed)
    }

    /// Обрабатывает бинарный `op:25 ExternalSenderPackage` (`[seq_be: u16][25][payload]` или чистый `payload`).
    pub fn on_external_sender_op25(&mut self, frame_or_payload: &[u8]) -> Result<Option<u16>, SrtpError> {
        let (seq, payload) =
            if frame_or_payload.len() >= 3 && frame_or_payload[2] == crate::dave::OP_MLS_EXTERNAL_SENDER {
                let seq = u16::from_be_bytes([frame_or_payload[0], frame_or_payload[1]]);
                (Some(seq), &frame_or_payload[3..])
            } else {
                (None, frame_or_payload)
            };
        if payload.is_empty() || payload.len() > DAVE_MAX_SIGNAL_BYTES {
            return Err(SrtpError::MalformedHeader {
                reason: "пустой или слишком большой op:25 ExternalSenderPackage",
            });
        }
        self.external_sender = Some(payload.to_vec());
        Ok(seq)
    }

    /// Обрабатывает бинарный `op:27 Proposals` (`[seq_be: u16][27][op_type][proposals]` или `[op_type][proposals]`).
    ///
    /// Если `op:25 ExternalSender` ещё не получен, возвращает `Ok(None)` (по эталону Serein).
    /// При успешной обработке формирует и возвращает бинарный кадр `op:28 CommitWelcome` (`[28, ...commit, ...welcome]`).
    pub fn on_proposals_op27(&mut self, frame_or_payload: &[u8]) -> Result<Option<Vec<u8>>, SrtpError> {
        let payload = if frame_or_payload.len() >= 3 && frame_or_payload[2] == crate::dave::OP_MLS_PROPOSALS {
            &frame_or_payload[3..]
        } else {
            frame_or_payload
        };
        if payload.is_empty() || payload.len() > DAVE_MAX_SIGNAL_BYTES {
            return Err(SrtpError::MalformedHeader {
                reason: "обрезанный или превышающий бюджет op:27 Proposals",
            });
        }
        let Some(external_sender) = &self.external_sender else {
            return Ok(None);
        };
        let (&operation_type, proposal_data) = payload.split_first().ok_or(SrtpError::MalformedHeader {
            reason: "пустой op:27 Proposals",
        })?;
        if operation_type > 1 || proposal_data.is_empty() {
            return Err(SrtpError::MalformedHeader {
                reason: "неподдерживаемый тип операции в op:27 Proposals",
            });
        }
        let next_epoch = self.epoch.saturating_add(1);
        let commit_body = self.compute_commit_welcome_body(next_epoch, external_sender);
        self.pending_commit = Some(commit_body.clone());

        let mut frame = Vec::with_capacity(1 + commit_body.len());
        frame.push(crate::dave::OP_MLS_COMMIT_WELCOME);
        frame.extend_from_slice(&commit_body);
        Ok(Some(frame))
    }

    /// Обрабатывает бинарный `op:29 PrepareCommitTransition` или `op:30 Welcome`.
    ///
    /// Поддерживает серверный формат `[seq_be: u16][29|30][transition_id_be: u16][mls_body]`
    /// и сокращённый `[29|30][transition_id_be: u16][mls_body]`.
    /// При успехе возвращает `SendTransitionReadyOp23` (или `ExecutedImmediate` для `transition_id == 0`),
    /// а при ошибке валидации коммита/welcome выполняет `reset` и возвращает `InvalidCommitWelcome`
    /// (`op:31` JSON + свежий бинарный `op:26 KeyPackage`).
    pub fn on_commit_or_welcome_op29_op30(
        &mut self,
        frame: &[u8],
    ) -> Result<DaveTransitionOutcome, SrtpError> {
        let (opcode, transition_id, mls_body) = if frame.len() >= 6
            && (frame[2] == crate::dave::OP_MLS_PREPARE_COMMIT_TRANSITION
                || frame[2] == crate::dave::OP_MLS_WELCOME)
        {
            let tid = u16::from_be_bytes([frame[3], frame[4]]);
            (frame[2], tid, &frame[5..])
        } else if frame.len() >= 4
            && (frame[0] == crate::dave::OP_MLS_PREPARE_COMMIT_TRANSITION
                || frame[0] == crate::dave::OP_MLS_WELCOME)
        {
            let tid = u16::from_be_bytes([frame[1], frame[2]]);
            (frame[0], tid, &frame[3..])
        } else {
            return Err(SrtpError::MalformedHeader {
                reason: "обрезанный бинарный кадр перехода DAVE op:29/30",
            });
        };

        let commit_matches = if opcode == crate::dave::OP_MLS_PREPARE_COMMIT_TRANSITION
            && self.epoch == 0
            && self.pending_commit.is_some()
        {
            self.pending_commit.as_deref() == Some(mls_body)
        } else {
            mls_body.len() >= 16 && mls_body.starts_with(b"DAVE1CMT")
        };

        if !commit_matches || self.external_sender.is_none() {
            self.resets = self.resets.saturating_add(1);
            if self.resets > DAVE_MAX_RESETS {
                return Err(SrtpError::MalformedHeader {
                    reason: "исчерпан лимит восстановления DAVE MLS (resets > 3)",
                });
            }
            self.epoch = 0;
            self.srtp.set_dave_cipher(None);
            self.ready = false;
            self.waiting_for_peer = false;
            self.pending_commit = None;
            self.pending_transition = None;
            self.pending_protocol_transition = None;
            self.pending_epoch_key = None;

            let op31_json = crate::dave::DaveInvalidCommitWelcome { transition_id }.to_json();
            let fresh_key_package_op26 = self.key_package_op26();
            return Ok(DaveTransitionOutcome::InvalidCommitWelcome {
                transition_id,
                op31_json,
                fresh_key_package_op26,
            });
        }

        self.epochs_budget = self.epochs_budget.saturating_add(1);
        if self.epochs_budget > DAVE_MAX_EPOCH_TRANSITIONS {
            return Err(SrtpError::CounterOverflow);
        }

        let next_epoch = self.epoch.saturating_add(1);
        let epoch_key = self.derive_epoch_key(next_epoch, mls_body);
        self.pending_epoch_key = Some(epoch_key);
        self.pending_transition = Some(transition_id);
        self.pending_commit = None;

        if transition_id == 0 {
            let epoch = self.on_execute_transition_op22(0)?;
            Ok(DaveTransitionOutcome::ExecutedImmediate { epoch })
        } else {
            let op23_json = crate::dave::DaveReadyForTransition { transition_id }.to_json();
            Ok(DaveTransitionOutcome::SendTransitionReadyOp23 {
                transition_id,
                op23_json,
            })
        }
    }

    /// Обрабатывает JSON `op:21 PrepareTransition` (`transition_id`, `protocol_version`).
    ///
    /// Отклоняет даунгрейд (`protocol_version != 1`). Для `transition_id != 0` возвращает
    /// JSON `op:23 TransitionReady`.
    pub fn on_prepare_transition_op21(
        &mut self,
        transition_id: u16,
        protocol_version: u8,
    ) -> Result<Option<String>, SrtpError> {
        if protocol_version != 1 {
            return Err(SrtpError::MalformedHeader {
                reason: "Discord запросил даунгрейд шифрования DAVE (protocol_version != 1)",
            });
        }
        self.protocol_version = protocol_version;
        self.pending_protocol_transition = Some(transition_id);
        if transition_id == 0 {
            if self.pending_epoch_key.is_some() || self.ready {
                let _ = self.on_execute_transition_op22(0)?;
            } else if self.is_alone() {
                self.waiting_for_peer = true;
                self.ready = false;
            } else {
                self.pending_protocol_transition = None;
                self.ready = false;
            }
            Ok(None)
        } else {
            Ok(Some(
                crate::dave::DaveReadyForTransition { transition_id }.to_json(),
            ))
        }
    }

    /// Обрабатывает JSON `op:22 ExecuteTransition`: активирует подготовленную эпоху DAVE MLS
    /// и обновляет `DaveMediaCipher` внутри `SrtpContext`.
    pub fn on_execute_transition_op22(&mut self, transition_id: u16) -> Result<u64, SrtpError> {
        let is_mls = self.pending_transition == Some(transition_id);
        let is_proto = self.pending_protocol_transition == Some(transition_id);
        if !is_mls && !is_proto {
            return Err(SrtpError::MalformedHeader {
                reason: "неожиданный transition_id в op:22 ExecuteTransition",
            });
        }
        if is_proto {
            self.pending_protocol_transition = None;
        }
        if let Some(epoch_key) = self.pending_epoch_key.take() {
            self.epoch = self.epoch.saturating_add(1);
            self.srtp
                .set_dave_cipher(Some(DaveMediaCipher::with_epoch(self.epoch, &epoch_key)));
        } else if self.epoch == 0 {
            let next_epoch = 1;
            let fallback_key = self.derive_epoch_key(next_epoch, b"DAVE1CMT-DEFAULT");
            self.epoch = next_epoch;
            self.srtp
                .set_dave_cipher(Some(DaveMediaCipher::with_epoch(self.epoch, &fallback_key)));
        }
        self.pending_transition = None;
        self.ready = true;
        self.waiting_for_peer = false;
        Ok(self.epoch)
    }

    /// Двухслойное шифрование RTP-пакета (`DAVE E2EE + SRTP _rtpsize`).
    pub fn encrypt_rtp_packet(&mut self, packet: &[u8]) -> Result<Vec<u8>, SrtpError> {
        if !self.ready || self.srtp.dave_cipher().is_none() {
            return Err(SrtpError::MalformedHeader {
                reason: "DAVE MLS ещё не готов к шифрованию медиа",
            });
        }
        self.srtp.encrypt_rtp(packet)
    }

    /// Шифрует аудио-кадр демонстрации экрана (soundshare) по эталону Serein `Encryption::seal_soundshare`:
    /// добавляет в AAD 4-байтную преамбулу RFC 5285 `[0xbe, 0xde, 0, 1]`, а 4-байтное тело расширения
    /// `[0x90, 0x04, 0, 0]` шифрует транспортным AEAD вместе с DAVE-кадром.
    pub fn seal_soundshare_rtp(
        &mut self,
        header: &[u8; RTP_HEADER_MIN],
        plaintext_media: &[u8],
    ) -> Result<Vec<u8>, SrtpError> {
        let dave_frame = {
            let dave = self
                .srtp
                .dave_cipher_mut()
                .ok_or(SrtpError::MalformedHeader {
                    reason: "DAVE MLS ещё не инициализирован для soundshare",
                })?;
            dave.encrypt(plaintext_media)?
        };
        let trailer = self.srtp.next_trailer()?;
        let mut extended_aad = [0u8; 16];
        extended_aad[..12].copy_from_slice(header);
        extended_aad[0] |= 0x10;
        extended_aad[12..16].copy_from_slice(&[0xbe, 0xde, 0x00, 0x01]);

        let mut clear_ext_and_dave = Vec::with_capacity(4 + dave_frame.len());
        clear_ext_and_dave.extend_from_slice(&[0x90, 0x04, 0x00, 0x00]);
        clear_ext_and_dave.extend_from_slice(&dave_frame);

        let (ciphertext, tag) = self
            .srtp
            .seal(&clear_ext_and_dave, &extended_aad, trailer)?;
        let mut out =
            Vec::with_capacity(extended_aad.len() + ciphertext.len() + AEAD_TAG_LEN + TRAILER_LEN);
        out.extend_from_slice(&extended_aad);
        out.extend_from_slice(&ciphertext);
        out.extend_from_slice(&tag);
        out.extend_from_slice(&trailer);
        self.srtp.counter += 1;
        Ok(out)
    }

    /// Двухслойное дешифрование входящего RTP-пакета (`SRTP + DAVE E2EE`) со снятием
    /// заголовка RTP, расширения RFC 5285 `0xBEDE` и паддинга RTP.
    pub fn decrypt_rtp_stripping_bede(
        &mut self,
        packet: &[u8],
    ) -> Result<DaveStrippedRtpPacket, SrtpError> {
        let tail_len = AEAD_TAG_LEN + TRAILER_LEN;
        let min = RTP_HEADER_MIN + tail_len;
        if packet.len() < min {
            return Err(SrtpError::PacketTooShort {
                len: packet.len(),
                min,
            });
        }
        if packet[0] >> 6 != 2 {
            return Err(SrtpError::MalformedHeader {
                reason: "версия RTP ≠ 2",
            });
        }
        let cc = usize::from(packet[0] & 0x0f);
        let has_padding = (packet[0] & 0x20) != 0;
        let has_ext = (packet[0] & 0x10) != 0;
        let marker = (packet[1] & 0x80) != 0;
        let payload_type = packet[1] & 0x7f;
        let sequence = u16::from_be_bytes([packet[2], packet[3]]);
        let timestamp = u32::from_be_bytes([packet[4], packet[5], packet[6], packet[7]]);
        let ssrc = u32::from_be_bytes([packet[8], packet[9], packet[10], packet[11]]);

        let base_hdr = RTP_HEADER_MIN + cc * 4;
        let tail_start = packet.len() - tail_len;
        let tag = &packet[tail_start..tail_start + AEAD_TAG_LEN];
        let trailer = read_trailer(&packet[tail_start + AEAD_TAG_LEN..]);
        let trailer_value = u32::from_le_bytes(trailer);
        self.srtp.check_receive_trailer(trailer_value)?;

        // Вариант 1: классический _rtpsize (полный заголовок RTP + 0xBEDE расширение в открытом AAD)
        if let Ok(clear_len) = rtp_header_len(packet)
            && clear_len <= tail_start
            && let Ok(mut dave_frame) =
                self.srtp
                    .open(&packet[clear_len..tail_start], tag, &packet[..clear_len], trailer)
        {
            if has_padding {
                strip_rtp_padding(&mut dave_frame)?;
            }
            let (extension_profile, extension_data) = if has_ext {
                let profile = u16::from_be_bytes([packet[base_hdr], packet[base_hdr + 1]]);
                let ext_words = usize::from(u16::from_be_bytes([
                    packet[base_hdr + 2],
                    packet[base_hdr + 3],
                ]));
                let ext_start = base_hdr + 4;
                let ext_end = ext_start + ext_words * 4;
                (Some(profile), Some(packet[ext_start..ext_end].to_vec()))
            } else {
                (None, None)
            };
            let dave = self
                .srtp
                .dave_cipher_mut()
                .ok_or(SrtpError::AuthenticationFailed)?;
            let plaintext = dave.decrypt(&dave_frame)?;
            self.srtp.remember_receive_trailer(trailer_value);
            return Ok(DaveStrippedRtpPacket {
                ssrc,
                sequence,
                timestamp,
                marker,
                payload_type,
                extension_profile,
                extension_data,
                payload: plaintext,
            });
        }

        // Вариант 2: Serein / боевой Discord _rtpsize (преамбула 0xBEDE в AAD, тело расширения зашифровано с DAVE-кадром)
        if has_ext && base_hdr + 4 <= tail_start {
            let aad_len = base_hdr + 4;
            let profile = u16::from_be_bytes([packet[base_hdr], packet[base_hdr + 1]]);
            let ext_words = usize::from(u16::from_be_bytes([
                packet[base_hdr + 2],
                packet[base_hdr + 3],
            ]));
            let ext_body_len = ext_words * 4;
            let mut opened =
                self.srtp
                    .open(&packet[aad_len..tail_start], tag, &packet[..aad_len], trailer)?;
            if opened.len() < ext_body_len {
                return Err(SrtpError::MalformedHeader {
                    reason: "тело расширения 0xBEDE длиннее расшифрованного кадра",
                });
            }
            let ext_data = opened[..ext_body_len].to_vec();
            opened.drain(..ext_body_len);
            if has_padding {
                strip_rtp_padding(&mut opened)?;
            }
            let dave = self
                .srtp
                .dave_cipher_mut()
                .ok_or(SrtpError::AuthenticationFailed)?;
            let plaintext = dave.decrypt(&opened)?;
            self.srtp.remember_receive_trailer(trailer_value);
            return Ok(DaveStrippedRtpPacket {
                ssrc,
                sequence,
                timestamp,
                marker,
                payload_type,
                extension_profile: Some(profile),
                extension_data: Some(ext_data),
                payload: plaintext,
            });
        }

        Err(SrtpError::AuthenticationFailed)
    }

    fn compute_commit_welcome_body(&self, next_epoch: u64, external_sender: &[u8]) -> Vec<u8> {
        let mut salt = [0u8; 14];
        salt[..8].copy_from_slice(&self.channel_id.to_be_bytes());
        salt[8..14].copy_from_slice(&next_epoch.to_be_bytes()[2..8]);
        let mut seed = self.secret_key;
        for (idx, &b) in external_sender.iter().take(32).enumerate() {
            seed[idx] ^= b;
        }
        let commit_tag = hkdf_sha1_32(&seed, &salt, b"discord-lab/dave/commit-welcome");
        let mut out = Vec::with_capacity(8 + 8 + 32);
        out.extend_from_slice(b"DAVE1CMT");
        out.extend_from_slice(&next_epoch.to_be_bytes());
        out.extend_from_slice(&commit_tag);
        out
    }

    fn derive_epoch_key(&self, epoch: u64, mls_body: &[u8]) -> [u8; 32] {
        let mut salt = [0u8; 14];
        salt[..8].copy_from_slice(&self.channel_id.to_be_bytes());
        salt[8..14].copy_from_slice(&epoch.to_be_bytes()[2..8]);
        let mut seed = self.secret_key;
        for (idx, &b) in mls_body.iter().take(32).enumerate() {
            seed[idx] ^= b;
        }
        hkdf_sha1_32(&seed, &salt, b"discord-lab/dave/epoch-ratchet")
    }
}

fn strip_rtp_padding(frame: &mut Vec<u8>) -> Result<(), SrtpError> {
    let Some(&pad_len) = frame.last() else {
        return Err(SrtpError::MalformedHeader {
            reason: "пустой RTP-кадр с флагом паддинга",
        });
    };
    let pad = usize::from(pad_len);
    if pad == 0 || pad > frame.len() {
        return Err(SrtpError::MalformedHeader {
            reason: "некорректная длина RTP-паддинга",
        });
    }
    frame.truncate(frame.len() - pad);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 32-байтный `secret_key` из OP 4 `SESSION_DESCRIPTION`.
    const OP4_SECRET_KEY: [u8; 32] = [
        0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f,
        0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e,
        0x1f, 0x20,
    ];

    /// Эталонный RTP-пакет Discord: 12 Б базового заголовка + 12 Б RFC 8285 (`0xBEDE`, 2 слова) = 24 Б AAD + Opus payload.
    fn sample_rtp_packet() -> (Vec<u8>, usize, Vec<u8>) {
        let header: [u8; 24] = [
            0x90, 0x78, 0x00, 0x2a, // V=2, X=1, PT=0x78 (Opus), seq=42
            0x00, 0x01, 0x51, 0x80, // timestamp
            0x00, 0x07, 0x9b, 0x23, // SSRC
            0xbe, 0xde, 0x00, 0x02, // RFC 8285 one-byte ext, 2 words (8 bytes)
            0x10, 0xff, 0x22, 0x00, 0x01, 0xab, 0x00, 0x00,
        ];
        let payload: [u8; 16] = [
            0xf8, 0xff, 0xfe, 0x10, 0x20, 0x30, 0x40, 0x50, 0x60, 0x70, 0x80, 0x90, 0xa0, 0xb0,
            0xc0, 0xd0,
        ];
        let mut pkt = Vec::with_capacity(header.len() + payload.len());
        pkt.extend_from_slice(&header);
        pkt.extend_from_slice(&payload);
        (pkt, header.len(), payload.to_vec())
    }

    #[test]
    fn aes256_gcm_rtpsize_byte_format_uses_raw_op4_secret_key_and_trailer_nonce() {
        let (rtp_packet, header_len, payload) = sample_rtp_packet();
        let mut ctx = SrtpContext::new(
            &OP4_SECRET_KEY,
            &[0u8; 14],
            EncryptionMode::Aes256GcmRtpSize,
        );

        let wire = ctx.encrypt_rtp(&rtp_packet).expect("encrypt_rtp");

        // Оракул: прямой 32-байтный secret_key из OP 4 + 12-байтный nonce [trailer_le (4B), 0..0 (8B)] + RTP header как AAD.
        let expected_trailer: [u8; 4] = 0x8000_0000u32.to_le_bytes();
        let mut expected_nonce = [0u8; GCM_NONCE_SIZE];
        expected_nonce[..4].copy_from_slice(&expected_trailer);
        let oracle_gcm = Gcm::new(&Aes256Key::from_array(OP4_SECRET_KEY));
        let expected_ct_and_tag = oracle_gcm
            .encrypt(&expected_nonce, &rtp_packet[..header_len], &payload)
            .expect("oracle gcm encrypt");

        let mut expected_wire =
            Vec::with_capacity(header_len + expected_ct_and_tag.len() + TRAILER_LEN);
        expected_wire.extend_from_slice(&rtp_packet[..header_len]);
        expected_wire.extend_from_slice(&expected_ct_and_tag);
        expected_wire.extend_from_slice(&expected_trailer);

        assert_eq!(
            wire, expected_wire,
            "aead_aes256_gcm_rtpsize должен формировать [RTP header AAD][ciphertext][16B tag][4B trailer] на прямом 32B secret_key и 12B nonce = [trailer, 0..0]"
        );
    }

    #[test]
    fn xchacha20_poly1305_rtpsize_byte_format_uses_raw_op4_secret_key_and_trailer_nonce() {
        let (rtp_packet, header_len, payload) = sample_rtp_packet();
        let mut ctx = SrtpContext::new(
            &OP4_SECRET_KEY,
            &[0u8; 14],
            EncryptionMode::XChaCha20Poly1305RtpSize,
        );

        let wire = ctx.encrypt_rtp(&rtp_packet).expect("encrypt_rtp");

        // Оракул: прямой 32-байтный secret_key из OP 4 + 24-байтный nonce [trailer_le (4B), 0..0 (20B)] + RTP header как AAD.
        let expected_trailer: [u8; 4] = 0x8000_0000u32.to_le_bytes();
        let mut expected_nonce = [0u8; XCHACHA_NONCE_SIZE];
        expected_nonce[..4].copy_from_slice(&expected_trailer);
        let oracle_xchacha = XChaCha20Poly1305::new(&OP4_SECRET_KEY);
        let expected_ct_and_tag = oracle_xchacha
            .encrypt(&expected_nonce, &rtp_packet[..header_len], &payload)
            .expect("oracle xchacha encrypt");

        let mut expected_wire =
            Vec::with_capacity(header_len + expected_ct_and_tag.len() + TRAILER_LEN);
        expected_wire.extend_from_slice(&rtp_packet[..header_len]);
        expected_wire.extend_from_slice(&expected_ct_and_tag);
        expected_wire.extend_from_slice(&expected_trailer);

        assert_eq!(
            wire, expected_wire,
            "aead_xchacha20_poly1305_rtpsize должен формировать [RTP header AAD][ciphertext][16B tag][4B trailer] на прямом 32B secret_key и 24B nonce = [trailer, 0..0]"
        );
    }

    #[test]
    fn rtpsize_roundtrip_between_independent_contexts_with_direct_op4_secret_key() {
        let (rtp_packet, header_len, payload) = sample_rtp_packet();

        for mode in [
            EncryptionMode::Aes256GcmRtpSize,
            EncryptionMode::XChaCha20Poly1305RtpSize,
        ] {
            // Два независимых контекста с одним 32-байтным secret_key из OP 4 (и разными salt,
            // так как в режиме *_rtpsize ключ используется напрямую без HKDF/salt).
            let mut sender = SrtpContext::from_secret_key(&OP4_SECRET_KEY, mode);
            let mut receiver =
                SrtpContext::from_secret_key_and_mode(&OP4_SECRET_KEY, mode.as_engine_name())
                    .expect("from_secret_key_and_mode");
            let mut receiver_with_salt = SrtpContext::new(&OP4_SECRET_KEY, &[0xFF; 14], mode);

            for seq in 0..4u8 {
                let mut pkt = rtp_packet.clone();
                pkt[3] = seq;
                let encrypted = sender.encrypt_rtp(&pkt).expect("sender encrypt_rtp");
                let decrypted = receiver
                    .decrypt_rtp(&encrypted)
                    .expect("receiver decrypt_rtp");
                assert_eq!(decrypted, pkt);
                let decrypted_salt = receiver_with_salt
                    .decrypt_rtp(&encrypted)
                    .expect("receiver_with_salt decrypt_rtp");
                assert_eq!(decrypted_salt, pkt);
            }

            // Проверка расшифровки входящего пакета от медиа-сервера Discord (трейлер без старшего бита: 0x0000_0007).
            let server_trailer = 0x0000_0007u32.to_le_bytes();
            let server_ct_and_tag = match mode {
                EncryptionMode::Aes256GcmRtpSize => {
                    let mut nonce = [0u8; GCM_NONCE_SIZE];
                    nonce[..4].copy_from_slice(&server_trailer);
                    Gcm::new(&Aes256Key::from_array(OP4_SECRET_KEY))
                        .encrypt(&nonce, &rtp_packet[..header_len], &payload)
                        .expect("server gcm encrypt")
                }
                EncryptionMode::XChaCha20Poly1305RtpSize => {
                    let mut nonce = [0u8; XCHACHA_NONCE_SIZE];
                    nonce[..4].copy_from_slice(&server_trailer);
                    XChaCha20Poly1305::new(&OP4_SECRET_KEY)
                        .encrypt(&nonce, &rtp_packet[..header_len], &payload)
                        .expect("server xchacha encrypt")
                }
                EncryptionMode::Aes256Gcm => unreachable!(),
            };
            let mut server_wire = Vec::new();
            server_wire.extend_from_slice(&rtp_packet[..header_len]);
            server_wire.extend_from_slice(&server_ct_and_tag);
            server_wire.extend_from_slice(&server_trailer);

            let opened = receiver
                .decrypt_rtp(&server_wire)
                .expect("decrypt server wire");
            assert_eq!(opened, rtp_packet);
        }
    }

    #[test]
    fn dave_media_cipher_encrypts_and_decrypts_with_fafa_supplemental_footer_and_rejects_tamper() {
        let dave_key: [u8; 32] = [0x42; 32];
        let mut sender_dave = DaveMediaCipher::with_epoch(1, &dave_key);
        let mut receiver_dave = DaveMediaCipher::with_epoch(1, &dave_key);

        let media_plain = b"opus-or-h264-media-frame-payload";
        let dave_encrypted = sender_dave.encrypt(media_plain).expect("dave encrypt");
        assert_eq!(sender_dave.epoch(), 1);
        assert_eq!(sender_dave.counter(), 1);
        assert_eq!(
            &dave_encrypted[dave_encrypted.len() - 2..],
            &DAVE_MARKER_BYTES,
            "DAVE frame must end with 0xFAFA magic marker"
        );
        assert_ne!(&dave_encrypted[..media_plain.len()], media_plain);

        let decrypted = receiver_dave
            .decrypt(&dave_encrypted)
            .expect("dave decrypt");
        assert_eq!(decrypted, media_plain);

        let mut tampered = dave_encrypted.clone();
        tampered[0] ^= 0x01;
        assert_eq!(
            receiver_dave.decrypt(&tampered),
            Err(SrtpError::AuthenticationFailed)
        );
    }

    #[test]
    fn two_layer_dave_and_srtp_rtpsize_pipeline_preserves_unencrypted_rtp_extensions() {
        let (rtp_packet, header_len, media_payload) = sample_rtp_packet();
        let dave_key: [u8; 32] = [0x77; 32];

        for mode in [
            EncryptionMode::XChaCha20Poly1305RtpSize,
            EncryptionMode::Aes256GcmRtpSize,
        ] {
            let mut sender_srtp = SrtpContext::from_secret_key(&OP4_SECRET_KEY, mode);
            let mut receiver_srtp = SrtpContext::from_secret_key(&OP4_SECRET_KEY, mode);
            let mut transport_only_observer = SrtpContext::from_secret_key(&OP4_SECRET_KEY, mode);

            let mut sender_dave = DaveMediaCipher::new(&dave_key);
            let mut receiver_dave = DaveMediaCipher::new(&dave_key);

            let wire = sender_srtp
                .encrypt_rtp_with_dave(&rtp_packet, &mut sender_dave)
                .expect("two-layer encrypt_rtp_with_dave");

            // 1. Заголовок RTP + расширение RFC 8285 (0xBEDE, 24 байта) остаются открытыми на проводе в режиме _rtpsize
            assert_eq!(
                &wire[..header_len],
                &rtp_packet[..header_len],
                "RTP header + extension bytes must remain unencrypted in _rtpsize mode"
            );

            // 2. Промежуточная расшифровка только транспортного слоя SRTP (cipher.open) должна вернуть
            // открытый заголовок + DAVE-шифротекст с маркером 0xFAFA (доказывает порядок: dave.encrypt -> cipher.seal)
            let after_srtp_open = transport_only_observer
                .decrypt_rtp(&wire)
                .expect("outer SRTP decrypt");
            assert_eq!(&after_srtp_open[..header_len], &rtp_packet[..header_len]);
            let inner_dave_payload = &after_srtp_open[header_len..];
            assert_eq!(
                &inner_dave_payload[inner_dave_payload.len() - 2..],
                &DAVE_MARKER_BYTES
            );
            assert_ne!(inner_dave_payload, media_payload.as_slice());

            // 3. Полная двухслойная расшифровка (cipher.open -> dave.decrypt) восстанавливает исходный RTP-пакет
            let opened = receiver_srtp
                .decrypt_rtp_with_dave(&wire, &mut receiver_dave)
                .expect("two-layer decrypt_rtp_with_dave");
            assert_eq!(opened, rtp_packet);

            // 4. Также проверяем встроенный режим set_dave_cipher через стандартные encrypt_rtp / decrypt_rtp
            sender_srtp.set_dave_cipher(Some(DaveMediaCipher::with_epoch(2, &dave_key)));
            receiver_srtp.set_dave_cipher(Some(DaveMediaCipher::with_epoch(2, &dave_key)));
            let wire_2 = sender_srtp
                .encrypt_rtp(&rtp_packet)
                .expect("encrypt_rtp with attached dave cipher");
            assert_eq!(&wire_2[..header_len], &rtp_packet[..header_len]);
            let opened_2 = receiver_srtp
                .decrypt_rtp(&wire_2)
                .expect("decrypt_rtp with attached dave cipher");
            assert_eq!(opened_2, rtp_packet);
        }
    }
}
