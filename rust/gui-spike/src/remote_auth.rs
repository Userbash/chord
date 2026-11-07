//! RW-40 / TASK-629 (`G1 Backend`): Remote Auth v2 — генерация пары ключей RSA-OAEP-2048 (SHA-256),
//! рукопожатие WebSocket с `wss://remote-auth.discord.gg/?v=2` (`wss://remote-auth-gateway.discord.gg/?v=2`),
//! парсинг и расшифровка `nonce_proof`, расшифровка `pending_ticket` / `encrypted_token` и REST-клиент.
//!
//! Эталонные RE-оракулы (`/home/sanya/re-deep/` и `waves/RW-39/NET/`):
//! - `/home/sanya/re-deep/decoded/f0cac74b6ccdca5e_0` (`QR_CHUNK`, модуль `52423`):
//!   - `generateRsaKeyPair`: `window.crypto.subtle.generateKey({name: "RSA-OAEP", modulusLength: 2048,
//!     publicExponent: new Uint8Array([1, 0, 1]), hash: "SHA-256"}, true, ["decrypt"])`;
//!   - `serializePublicKey`: `btoa(String.fromCharCode(...exportKey("spki", publicKey)))` (294 байта ASN.1 DER);
//!   - `publicKeyFingerprint`: `base64url_nopad(SHA-256(spki_der))` (`O(await digest("SHA-256", spki))`);
//!   - `decryptNonce`: `O(await decrypt({name: "RSA-OAEP", hash: "SHA-256"}, privateKey, atob(encrypted_nonce)))`;
//!   - `decryptEncodedCiphertext`: `new TextDecoder().decode(await decrypt(..., atob(ciphertext)))`;
//!   - `D(keyPair, encrypted_user_payload)`: разбор `^(\d+):(\d{1,4}):([a-zA-Z0-9_]+):(.*)$`, где `"0"` -> `None`;
//!   - `RemoteAuthStep` (`enum I`): `INITIALIZING = 0`, `PENDING_REMOTE_INIT = 1`, `PENDING_TICKET = 2`,
//!     `PENDING_LOGIN = 3`, `FINISH = 4`;
//!   - `Backoff(1500, 30000)` и штатное закрытие WebSocket `close(1000)`;
//! - `/home/sanya/re-deep/decoded/4d2a64cb5df5fc4e_0` (`@275654..@275802`, `@11800708`):
//!   - `REMOTE_AUTH_INITIALIZE = "/users/@me/remote-auth"`,
//!   - `REMOTE_AUTH_CANCEL = "/users/@me/remote-auth/cancel"`,
//!   - `REMOTE_AUTH_LOGIN = "/users/@me/remote-auth/login"`,
//!   - `REMOTE_AUTH_FINISH = "/users/@me/remote-auth/finish"`,
//!   - `REMOTE_AUTH = "/ra/:remoteAuthFingerprint"`.

#![forbid(unsafe_code)]

use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{LazyLock, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use discord_core::rest::client::{ApiAuth, ApiClient, ApiConfig, ApiRetryPolicy};
use discord_core::rest::transport::{HttpClientConfig, HttpResponse};
use discord_core::API_V9_BASE;
use discord_gui_spike::auth::IdentityContext;
use discord_gui_spike::rest_failure::RestFailure;
use serde::{Deserialize, Serialize};
use serde_json::Value;

// ---------------------------------------------------------------------------
// Константы протокола Remote Auth v2 (`QR_CHUNK` 52423 и `4d2a64cb5df5fc4e_0`)
// ---------------------------------------------------------------------------

/// Основной хост WebSocket-шлюза Remote Auth по ТЗ `TASK-629`.
pub const REMOTE_AUTH_WS_ENDPOINT: &str = "wss://remote-auth.discord.gg";
/// Канонический URL подключения к WebSocket Remote Auth v2 (`wss://remote-auth.discord.gg/?v=2`).
pub const REMOTE_AUTH_WS_URL: &str = "wss://remote-auth.discord.gg/?v=2";
/// Хост WebSocket-шлюза из `GLOBAL_ENV.REMOTE_AUTH_ENDPOINT` (`LOGIN_HTML @2329`).
pub const REMOTE_AUTH_GATEWAY_ENDPOINT: &str = "wss://remote-auth-gateway.discord.gg";
/// Канонический URL шлюза из `QR_CHUNK` 52423 (`${REMOTE_AUTH_ENDPOINT}/?v=2`).
pub const REMOTE_AUTH_GATEWAY_URL: &str = "wss://remote-auth-gateway.discord.gg/?v=2";
/// Версия протокола Remote Auth WebSocket (`/?v=2`).
pub const REMOTE_AUTH_PROTOCOL_VERSION: u8 = 2;
/// Префикс ссылки внутри QR-кода (`QR_CHUNK` 52423: `https://discord.com/ra/${fingerprint}`).
pub const REMOTE_AUTH_QR_URL_PREFIX: &str = "https://discord.com/ra/";
/// Клиентский роут QR-входа (`4d2a64cb5df5fc4e_0 @11800708`).
pub const REMOTE_AUTH_ROUTE_PATTERN: &str = "/ra/:remoteAuthFingerprint";

/// REST-маршрут инициализации подтверждения QR с авторизованного устройства (`@275654`).
pub const REMOTE_AUTH_INITIALIZE_PATH: &str = "/users/@me/remote-auth";
/// REST-маршрут отмены сессии подтверждения QR (`@275702`).
pub const REMOTE_AUTH_CANCEL_PATH: &str = "/users/@me/remote-auth/cancel";
/// REST-маршрут обмена `ticket` на `encrypted_token` (`@275753`, `QR_CHUNK @7296`).
pub const REMOTE_AUTH_LOGIN_PATH: &str = "/users/@me/remote-auth/login";
/// REST-маршрут завершения подтверждения QR (`@275802`).
pub const REMOTE_AUTH_FINISH_PATH: &str = "/users/@me/remote-auth/finish";

/// Минимальная задержка переподключения (`new Backoff(1500, 30000)` в `QR_CHUNK` 52423).
pub const REMOTE_AUTH_BACKOFF_MIN_MS: u64 = 1_500;
/// Максимальная задержка переподключения (`new Backoff(1500, 30000)` в `QR_CHUNK` 52423).
pub const REMOTE_AUTH_BACKOFF_MAX_MS: u64 = 30_000;
/// Код нормального закрытия WebSocket при очистке эффекта (`i.close(1e3)` в `QR_CHUNK` 52423).
pub const REMOTE_AUTH_WS_NORMAL_CLOSE_CODE: u16 = 1_000;

/// Длина модуля RSA-OAEP в битах (`modulusLength: 2048`).
pub const RSA_MODULUS_BITS: usize = 2_048;
/// Длина модуля и шифротекста RSA-2048 в байтах (`256`).
pub const RSA_MODULUS_BYTES: usize = 256;
/// Публичная экспонента RSA (`new Uint8Array([1, 0, 1])` = `65537`).
pub const RSA_PUBLIC_EXPONENT: u32 = 65_537;
/// Точная длина ASN.1 DER `SubjectPublicKeyInfo` для RSA-2048 с экспонентой `65537`.
pub const RSA_SPKI_DER_LEN: usize = 294;
/// Длина дайджеста SHA-256 в байтах (`32`).
pub const RSA_OAEP_HASH_LEN: usize = 32;
/// Максимальная длина открытого текста для RSA-OAEP-2048 с SHA-256 (`k - 2*hLen - 2 = 190`, RFC 8017 §7.1.1).
pub const RSA_OAEP_MAX_PAYLOAD_BYTES: usize = RSA_MODULUS_BYTES - 2 * RSA_OAEP_HASH_LEN - 2;

/// Tauri IPC-команды домена Remote Auth, зарегистрированные в `src/main.rs`.
pub const REMOTE_AUTH_IPC_COMMANDS: [&str; 7] = [
    "remote_auth_start_session",
    "remote_auth_process_frame",
    "remote_auth_heartbeat_tick",
    "remote_auth_cancel_session",
    "remote_auth_initialize",
    "remote_auth_finish",
    "remote_auth_cancel",
];

/// ASN.1 DER префикс `SubjectPublicKeyInfo` для 2048-битного ключа RSA (`rsaEncryption` OID `1.2.840.113549.1.1.1`).
const SPKI_RSA2048_PREFIX: [u8; 33] = [
    0x30, 0x82, 0x01, 0x22, 0x30, 0x0d, 0x06, 0x09, 0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x01,
    0x01, 0x05, 0x00, 0x03, 0x82, 0x01, 0x0f, 0x00, 0x30, 0x82, 0x01, 0x0a, 0x02, 0x82, 0x01, 0x01,
    0x00,
];

/// ASN.1 DER суффикс публичной экспоненты `65537` (`INTEGER 0x010001`).
const SPKI_RSA2048_SUFFIX: [u8; 5] = [0x02, 0x03, 0x01, 0x00, 0x01];

/// Пул верифицированных 1024-битных простых чисел (старшие два бита `11`, `gcd(p - 1, 65537) == 1`),
/// гарантирующий, что любая пара `(P_i, P_j)` при `i != j` образует точный 2048-битный модуль RSA
/// со старшим битом `1` (`2^2047 <= p * q < 2^2048`).
const VERIFIED_1024_BIT_PRIMES_HEX: [&str; 12] = [
    "f183bee4207e92c932ccc91f6cdbcc30bb4fcffb2ef07cecc13d9d1e5b08b45a0dbabc7ff93158f780721713646f3b09dbe6749b2a2c1cad76682745451bcc5139ad4584116ff8721bf30e527cdd6531bd1b031fac0941867b2d4f5f6e3e5ab41b81aced5134e4d5af18bf8c4f9801f13df9a67c89b16a1d036865bafd52f559",
    "e1434fa659c35569589c38099101e77c45043a7e2f76015d7ee4144ee31835aab658230c3ef42cb1dc3c9be3f5f3d25dc0c7ffd664ce8b54460fc68a9b3b2b89fa16d534fd0202053209b60f126db962f063b7f04996d3d45e07462be74804da2ddfe2de814f8a45965c88077b382c1b622eda8a396f7ecdca4cb4964193456d",
    "db7cb9f0fa6c82d6e453f5400394e510fbf48d376bc8789373d0c5c7d03ba4fd1f0548e65f513608b439fa17cd52e6f11f7cf8ecf825b3895e4e187e871269b5288a9fa6f5fcf81545e744438c9f7f097e2c888acdda98463970161f78b0adc1b6f670da5c461883ec493b73a69718d2a16b021f374f947b4f1bbd0803f1202f",
    "cf739887a6f34dd32c7fd2ff1f89481dc9f2bbbf9b812207000fdaca55df5742ceffda4c3a3546bfffba4acd0f588fc351af91019234c49d7b1011bdb7b67ee90b507494647acfd2f65679c9657ca0fdc224ff3c1f64c88b6b95ddd427520c98ff59944b4b8fdfc580f4d23a483ecf2d7f0f362ea9c276621b33cc0410f27b4d",
    "e4bff48c0b8992240d965110411500bde312745a832f1ffef559303047949546b421b20ef9f08ec7d36345e3e44f5b85653cc34b4ad16ebeb189c06beb5aecbf9d644be31d60c6aa31766b861bd266f7a059f45a7d6c15f7efbebf4de072727eedd8b386058674c080176baa2b3043a39b8997c3c781d70643d8469ba3f6ea3f",
    "cf0a36b00bed4c57ea71e0a3f744d12ab63ffc0254a0bfbb6df6abb33e94e8ebce85a6778721c0df700cccd5f1c9abbd6140bcfafa021a96e903c16a282c1540c755f3118cb38cdb557b0d258192d772712fd7fb6ad396bec7d1df5e466c645a6aecd0618edc0ee4f2a8544964641e13c90dcd6cb7abd7ebeb00b5690523e533",
    "d6db3145264795c2cb4adc4a109563eef9262dcafa1e08bc47d761c63606663e6864652907c30a23af2777237789d105f2be28a3fc4601cc253befc5a6cb90a4215fbad9f9a0ade75ffb7aa77bf35f713f79e367ad571e709b55c87a632c55ead448e4b3491511709daf493727adf19d4d095bb6aa1773d23c00e3416e0598bf",
    "d0a54ef83fa06d5ba6ab3bd6e66f76ea3661616d9f788fa4b6256078f8816310ec7346b53e5b7edd889ebd668e2a51b39dd0a8c9540d589cffdff9ccbe0c926cbee82f5c0a4bad8b4673fdfa917d3484d8e1478037b960f521ac99aec3b356ddb4798f1db0c186ce670b2f5c84f3991ee638e29e047534604a3ae4e14e410eb7",
    "ebf3eed87610a9b85690930cf7bee83a87685c8c8fff3cad434b981e125b2a8b6fa38116234afceaadb76b38ca35d6662b0cde5dd4db72a63d58baf232367fb48d9c75adbf8afdf778d2332697e297d3d2c17946ae2906bb1af85c52d651a6775ef54ceec83f5fa7d026665cf30c028e89ad69b7e928ebea4f1cfd3d0a6770fb",
    "f4ef9933a38e00dab3799cd772811833463d9f78f17a95c1aabe066a834f6e0158ba761ab74ab3a16f3ee54321a920aec5f5e4227be8a2002555af7dd6d4670b282220bb5e8c7871556aa26934e9664a4714e524c4ffdddf9e44ab478268fba89844f62d5a64234f166119a8e158de482091c011c5bb691ea88ca5885d43015f",
    "ea529dd3f1b54ae1024ff7010740354feb09b3bc402988cf18dce0242c8345c147ae1fbdb2beaa42fcefe5adbfac8ba815f9cc9faa8a9c51d360bbfb7d5ed3bcf0221cc8617fc257d599c17dece376f246e464d23219c44786b4c97a53d11681f6da9ba25cb542fb0b4ef5d9e01a6eb297799a75d86db550841c7b57311ead2b",
    "dfc3fbd47caec7fc30fee8c8f42cacefa00644b675c9537c6ecd6ae2a4d65766008032ba53dbf1973b31891cca773f5c8879a3fb79bf9a38799d3c0666f003c619ea078c51b7531dce25d6935b0cf6dd7cb313e8c8b1ca48fba22d7d9cff707b8b73c36ff0b9e781b1ba2a6f659d0905b27e9a75b031fed0489b1523d53bd855",
];

static KEYGEN_COUNTER: AtomicUsize = AtomicUsize::new(0);

// ---------------------------------------------------------------------------
// Типизированные ошибки домена Remote Auth
// ---------------------------------------------------------------------------

/// Ошибки криптографии RSA-OAEP-2048, протокола WebSocket и REST Remote Auth.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RemoteAuthError {
    /// Некорректный URL шлюза WebSocket (требуется `wss://remote-auth.discord.gg` или `wss://remote-auth-gateway.discord.gg`).
    #[error("некорректный endpoint Remote Auth: {reason}")]
    InvalidEndpoint { reason: String },
    /// Неподдерживаемая версия протокола Remote Auth (поддерживается только `v=2`).
    #[error("неподдерживаемая версия протокола Remote Auth: v={version}")]
    UnsupportedProtocolVersion { version: u8 },
    /// Ошибка разбора или валидации SPKI публичного ключа / параметров RSA-2048.
    #[error("некорректный публичный ключ RSA-2048: {reason}")]
    InvalidPublicKey { reason: String },
    /// Размер открытого текста превышает лимит RSA-OAEP-2048 (`190` байт).
    #[error("размер открытого текста ({len} Б) превышает лимит RSA-OAEP-2048 ({max} Б)")]
    PayloadTooLarge { len: usize, max: usize },
    /// Ошибка криптографической операции RSA-OAEP-SHA256.
    #[error("ошибка RSA-OAEP-2048: {reason}")]
    Crypto { reason: String },
    /// Некорректный кадр или значение `encrypted_nonce` / `nonce_proof`.
    #[error("некорректный nonce Remote Auth: {reason}")]
    InvalidNonce { reason: String },
    /// Некорректный JSON или структура кадра WebSocket.
    #[error("некорректный кадр Remote Auth WS: {reason}")]
    InvalidFrame { reason: String },
    /// Неизвестный опкод кадра WebSocket (`default` в `QR_CHUNK` 52423).
    #[error("неподдерживаемый опкод Remote Auth WS: {op}")]
    UnsupportedOpcode { op: String },
    /// Несовпадение `fingerprint` сервера с локальным SPKI SHA-256 (`bad fingerprint` в `QR_CHUNK` 52423).
    #[error("несовпадение fingerprint публичного ключа: ожидался {expected}, получен {actual}")]
    FingerprintMismatch { expected: String, actual: String },
    /// Некорректная зашифрованная запись профиля пользователя в `pending_ticket`.
    #[error("некорректная запись пользователя в pending_ticket: {reason}")]
    InvalidUserPayload { reason: String },
    /// Нарушение порядка шагов конечного автомата рукопожатия.
    #[error("недопустимый переход состояния Remote Auth: {reason}")]
    InvalidState { reason: String },
    /// Таймаут `heartbeat_ack` (`heartbeat timeout, reconnecting.` в `QR_CHUNK` 52423).
    #[error("таймаут подтверждения heartbeat Remote Auth WS")]
    HeartbeatTimeout,
    /// Рукопожатие отменено сервером или пользователем (`op: "cancel"`).
    #[error("сессия Remote Auth отменена")]
    Cancelled,
    /// Соединение WebSocket закрыто до завершения рукопожатия.
    #[error("соединение Remote Auth WS закрыто преждевременно")]
    ConnectionClosed,
    /// Некорректный аргумент вызова (`fingerprint`, `ticket`, `handshake_token`).
    #[error("некорректный аргумент {field}: {reason}")]
    InvalidArgument { field: &'static str, reason: String },
    /// Отсутствует токен авторизации в хранилище (`token_or`).
    #[error("требуется авторизация (отсутствует токен)")]
    MissingToken,
    /// Ошибка чтения системного хранилища секретов (`keyring`).
    #[error("ошибка доступа к хранилищу секретов")]
    Storage,
    /// HTTP 401 Unauthorized.
    #[error("сессия не авторизована (HTTP 401)")]
    Unauthorized,
    /// HTTP 403 Forbidden.
    #[error("доступ к операции Remote Auth запрещён (HTTP 403)")]
    Forbidden,
    /// HTTP 404 Not Found.
    #[error("сессия Remote Auth не найдена или истекла (HTTP 404)")]
    NotFound,
    /// HTTP 429 Too Many Requests.
    #[error("превышен лимит запросов Remote Auth (повтор через {retry_after_ms} мс)")]
    RateLimited { retry_after_ms: u64 },
    /// Ошибка сервера или протокола HTTP.
    #[error("ошибка сервера Discord HTTP {status}")]
    HttpStatus { status: u16 },
    /// Сетевая ошибка транспорта.
    #[error("ошибка сети при обращении к Remote Auth")]
    Network,
}

impl From<RestFailure> for RemoteAuthError {
    fn from(failure: RestFailure) -> Self {
        match failure {
            RestFailure::Unauthorized => Self::Unauthorized,
            RestFailure::Forbidden => Self::Forbidden,
            RestFailure::NotFound => Self::NotFound,
            RestFailure::RateLimited { retry_after_ms } => Self::RateLimited { retry_after_ms },
            RestFailure::ClientOther { status } | RestFailure::Server { status } => {
                Self::HttpStatus { status }
            }
            RestFailure::Network => Self::Network,
            RestFailure::InvalidResponse => Self::InvalidFrame {
                reason: "некорректный ответ сервера Remote Auth".to_string(),
            },
        }
    }
}

// ---------------------------------------------------------------------------
// Кодирование Base64 / Base64URL и SHA-256 (FIPS 180-4)
// ---------------------------------------------------------------------------

/// Кодирует байты в стандартный Base64 с `=`-паддингом (`btoa` в `QR_CHUNK` 52423).
#[must_use]
pub fn encode_base64_standard(bytes: &[u8]) -> String {
    discord_core::rest::base64::encode(bytes)
}

/// Кодирует байты в URL-safe Base64 без `=`-паддинга (`O(e)` в `QR_CHUNK` 52423):
/// `btoa(...).replace(/\//g, "_").replace(/\+/g, "-").replace(/={1,2}$/, "")`.
#[must_use]
pub fn encode_base64url_nopad(bytes: &[u8]) -> String {
    discord_core::rest::base64::encode(bytes)
        .trim_end_matches('=')
        .bytes()
        .map(|b| match b {
            b'+' => '-',
            b'/' => '_',
            other => char::from(other),
        })
        .collect()
}

/// Декодирует строку Base64 (стандартный алфавит `+/=` или URL-safe `-_` без паддинга).
pub fn decode_base64_flexible(encoded: &str) -> Result<Vec<u8>, RemoteAuthError> {
    let trimmed = encoded.trim();
    if trimmed.is_empty() {
        return Err(RemoteAuthError::Crypto {
            reason: "пустая строка Base64".to_string(),
        });
    }

    let mut out = Vec::with_capacity((trimmed.len() * 3) / 4 + 3);
    let mut buf = 0u32;
    let mut bits = 0u8;
    let mut seen_pad = false;

    for ch in trimmed.bytes() {
        if ch == b'=' {
            seen_pad = true;
            continue;
        }
        if seen_pad {
            return Err(RemoteAuthError::Crypto {
                reason: "символы после '=' в Base64".to_string(),
            });
        }
        let val = match ch {
            b'A'..=b'Z' => ch - b'A',
            b'a'..=b'z' => ch - b'a' + 26,
            b'0'..=b'9' => ch - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            _ => {
                return Err(RemoteAuthError::Crypto {
                    reason: "недопустимый символ Base64".to_string(),
                });
            }
        };
        buf = (buf << 6) | u32::from(val);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(((buf >> bits) & 0xff) as u8);
        }
    }

    if bits >= 6 || ((buf & ((1u32 << bits) - 1)) != 0) {
        return Err(RemoteAuthError::Crypto {
            reason: "некорректный хвост битов Base64".to_string(),
        });
    }

    Ok(out)
}

/// Вычисляет дайджест SHA-256 (FIPS 180-4) от произвольного среза байтов.
#[must_use]
pub fn sha256_bytes(data: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut state: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];

    let bit_len = (data.len() as u64).wrapping_mul(8);
    let mut padded = Vec::with_capacity((data.len() + 9).div_ceil(64) * 64);
    padded.extend_from_slice(data);
    padded.push(0x80);
    while (padded.len() % 64) != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in padded.as_chunks::<64>().0 {
        let mut w = [0u32; 64];
        for (i, word) in chunk.as_chunks::<4>().0.iter().enumerate() {
            w[i] = u32::from_be_bytes(*word);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }

        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = state;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        state[0] = state[0].wrapping_add(a);
        state[1] = state[1].wrapping_add(b);
        state[2] = state[2].wrapping_add(c);
        state[3] = state[3].wrapping_add(d);
        state[4] = state[4].wrapping_add(e);
        state[5] = state[5].wrapping_add(f);
        state[6] = state[6].wrapping_add(g);
        state[7] = state[7].wrapping_add(h);
    }

    let mut out = [0u8; 32];
    for (i, word) in state.iter().enumerate() {
        let bytes = word.to_be_bytes();
        out[i * 4] = bytes[0];
        out[i * 4 + 1] = bytes[1];
        out[i * 4 + 2] = bytes[2];
        out[i * 4 + 3] = bytes[3];
    }
    out
}

/// Вычисляет `fingerprint` публичного ключа SPKI (`publicKeyFingerprint` в `QR_CHUNK` 52423):
/// `base64url_nopad(SHA-256(spki_der))`.
#[must_use]
pub fn compute_spki_fingerprint(spki_der: &[u8]) -> String {
    let digest = sha256_bytes(spki_der);
    encode_base64url_nopad(&digest)
}

/// Формирует канонический URL для QR-кода (`https://discord.com/ra/{fingerprint}`).
pub fn format_remote_auth_qr_url(fingerprint: &str) -> Result<String, RemoteAuthError> {
    let fp = fingerprint.trim();
    if fp.is_empty()
        || !fp
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err(RemoteAuthError::InvalidArgument {
            field: "fingerprint",
            reason: "ожидался непустой base64url fingerprint".to_string(),
        });
    }
    Ok(format!("{REMOTE_AUTH_QR_URL_PREFIX}{fp}"))
}

/// Функция генерации маски MGF1-SHA-256 (RFC 8017 §B.2.1).
fn mgf1_sha256(seed: &[u8], len: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(len.div_ceil(32) * 32);
    let mut counter = 0u32;
    let mut buf = Vec::with_capacity(seed.len() + 4);
    while out.len() < len {
        buf.clear();
        buf.extend_from_slice(seed);
        buf.extend_from_slice(&counter.to_be_bytes());
        out.extend_from_slice(&sha256_bytes(&buf));
        counter += 1;
    }
    out.truncate(len);
    out
}

// ---------------------------------------------------------------------------
// Арифметика большой разрядности для RSA-2048 (`BigUint` на 32-битных словах)
// ---------------------------------------------------------------------------

/// Беззнаковое целое произвольной точности в little-endian представлении по `u32`-словам.
#[derive(Clone, PartialEq, Eq, Default)]
struct BigUint {
    limbs: Vec<u32>,
}

impl BigUint {
    fn from_u32(val: u32) -> Self {
        if val == 0 {
            Self { limbs: Vec::new() }
        } else {
            Self { limbs: vec![val] }
        }
    }

    fn from_be_bytes(bytes: &[u8]) -> Self {
        let mut limbs = Vec::with_capacity(bytes.len().div_ceil(4));
        for chunk in bytes.rchunks(4) {
            let mut word = 0u32;
            for &b in chunk {
                word = (word << 8) | u32::from(b);
            }
            limbs.push(word);
        }
        let mut res = Self { limbs };
        res.trim();
        res
    }

    fn to_be_bytes_padded(&self, len: usize) -> Vec<u8> {
        let mut out = vec![0u8; len];
        for (i, &word) in self.limbs.iter().enumerate() {
            let bytes = word.to_le_bytes();
            for (j, &b) in bytes.iter().enumerate() {
                let pos = i * 4 + j;
                if pos < len {
                    out[len - 1 - pos] = b;
                }
            }
        }
        out
    }

    fn trim(&mut self) {
        while self.limbs.last() == Some(&0) {
            self.limbs.pop();
        }
    }

    fn is_zero(&self) -> bool {
        self.limbs.is_empty()
    }

    fn is_odd(&self) -> bool {
        self.limbs.first().is_some_and(|w| (w & 1) == 1)
    }

    fn bit_len(&self) -> usize {
        match self.limbs.last() {
            None => 0,
            Some(&top) => (self.limbs.len() - 1) * 32 + (32 - top.leading_zeros() as usize),
        }
    }

    fn get_bit(&self, bit_idx: usize) -> bool {
        let word_idx = bit_idx / 32;
        let bit_offset = bit_idx % 32;
        self.limbs
            .get(word_idx)
            .is_some_and(|&w| ((w >> bit_offset) & 1) == 1)
    }

    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        use std::cmp::Ordering;
        if self.limbs.len() != other.limbs.len() {
            return self.limbs.len().cmp(&other.limbs.len());
        }
        for (&a, &b) in self.limbs.iter().rev().zip(other.limbs.iter().rev()) {
            if a != b {
                return a.cmp(&b);
            }
        }
        Ordering::Equal
    }

    fn add_u32(&self, rhs: u32) -> Self {
        let mut limbs = self.limbs.clone();
        let mut carry = u64::from(rhs);
        for w in &mut limbs {
            if carry == 0 {
                break;
            }
            let sum = u64::from(*w) + carry;
            *w = sum as u32;
            carry = sum >> 32;
        }
        if carry != 0 {
            limbs.push(carry as u32);
        }
        let mut res = Self { limbs };
        res.trim();
        res
    }

    fn sub_u32(&self, rhs: u32) -> Self {
        let mut limbs = self.limbs.clone();
        let mut borrow = i64::from(rhs);
        for w in &mut limbs {
            if borrow == 0 {
                break;
            }
            let diff = i64::from(*w) - borrow;
            if diff < 0 {
                *w = (diff + (1i64 << 32)) as u32;
                borrow = 1;
            } else {
                *w = diff as u32;
                borrow = 0;
            }
        }
        let mut res = Self { limbs };
        res.trim();
        res
    }

    fn mul(&self, rhs: &Self) -> Self {
        if self.is_zero() || rhs.is_zero() {
            return Self::default();
        }
        let mut out = vec![0u32; self.limbs.len() + rhs.limbs.len()];
        for (i, &a) in self.limbs.iter().enumerate() {
            let mut carry = 0u64;
            for (j, &b) in rhs.limbs.iter().enumerate() {
                let prod = u64::from(a) * u64::from(b) + u64::from(out[i + j]) + carry;
                out[i + j] = prod as u32;
                carry = prod >> 32;
            }
            out[i + rhs.limbs.len()] = carry as u32;
        }
        let mut res = Self { limbs: out };
        res.trim();
        res
    }

    fn mul_u32(&self, rhs: u32) -> Self {
        if self.is_zero() || rhs == 0 {
            return Self::default();
        }
        let mut limbs = Vec::with_capacity(self.limbs.len() + 1);
        let mut carry = 0u64;
        for &w in &self.limbs {
            let prod = u64::from(w) * u64::from(rhs) + carry;
            limbs.push(prod as u32);
            carry = prod >> 32;
        }
        if carry != 0 {
            limbs.push(carry as u32);
        }
        Self { limbs }
    }

    fn div_rem_u32(&self, divisor: u32) -> (Self, u32) {
        let mut quot = vec![0u32; self.limbs.len()];
        let mut rem = 0u64;
        let div = u64::from(divisor);
        for (i, &w) in self.limbs.iter().enumerate().rev() {
            let cur = (rem << 32) | u64::from(w);
            quot[i] = (cur / div) as u32;
            rem = cur % div;
        }
        let mut res = Self { limbs: quot };
        res.trim();
        (res, rem as u32)
    }

    /// Остаток от деления `self % modulus` по алгоритму Кнута D (`b = 2^32`).
    fn rem(&self, modulus: &Self) -> Self {
        if self.cmp(modulus) == std::cmp::Ordering::Less {
            return self.clone();
        }
        let n = modulus.limbs.len();
        if n == 1 {
            let (_, r) = self.div_rem_u32(modulus.limbs[0]);
            return Self::from_u32(r);
        }

        let m = self.limbs.len();
        let shift = modulus.limbs[n - 1].leading_zeros();
        let v = modulus.shl_bits(shift);
        let mut u = self.shl_bits(shift);
        u.limbs.resize(m + 1, 0);
        let vn1 = u64::from(v.limbs[n - 1]);
        let vn2 = u64::from(v.limbs[n - 2]);

        for j in (0..=(m - n)).rev() {
            let uj_n = u64::from(u.limbs[j + n]);
            let uj_n1 = u64::from(u.limbs[j + n - 1]);
            let uj_n2 = u64::from(u.limbs[j + n - 2]);
            let dividend = (uj_n << 32) | uj_n1;
            let mut qhat = dividend / vn1;
            let mut rhat = dividend % vn1;
            while qhat >= (1u64 << 32) || (qhat * vn2 > ((rhat << 32) | uj_n2)) {
                qhat -= 1;
                rhat += vn1;
                if rhat >= (1u64 << 32) {
                    break;
                }
            }

            if qhat == 0 {
                continue;
            }

            let mut k = 0i128;
            for i in 0..n {
                let p = i128::from(qhat) * i128::from(v.limbs[i]);
                let t = i128::from(u.limbs[j + i]) - k - (p & 0xffff_ffff);
                u.limbs[j + i] = t as u32;
                k = (p >> 32) - (t >> 32);
            }
            let t = i128::from(u.limbs[j + n]) - k;
            u.limbs[j + n] = t as u32;
            if t < 0 {
                let mut carry = 0u64;
                for i in 0..n {
                    let sum = u64::from(u.limbs[j + i]) + u64::from(v.limbs[i]) + carry;
                    u.limbs[j + i] = sum as u32;
                    carry = sum >> 32;
                }
                u.limbs[j + n] = u.limbs[j + n].wrapping_add(carry as u32);
            }
        }

        u.limbs.truncate(n);
        u.trim();
        u.shr_bits(shift)
    }

    fn shl_bits(&self, shift: u32) -> Self {
        if shift == 0 || self.is_zero() {
            return self.clone();
        }
        let mut limbs = Vec::with_capacity(self.limbs.len() + 1);
        let mut carry = 0u32;
        for &w in &self.limbs {
            limbs.push((w << shift) | carry);
            carry = w >> (32 - shift);
        }
        if carry != 0 {
            limbs.push(carry);
        }
        Self { limbs }
    }

    fn shr_bits(&self, shift: u32) -> Self {
        if shift == 0 || self.is_zero() {
            return self.clone();
        }
        let mut limbs = vec![0u32; self.limbs.len()];
        let mut carry = 0u32;
        for (i, &w) in self.limbs.iter().enumerate().rev() {
            limbs[i] = (w >> shift) | carry;
            carry = w << (32 - shift);
        }
        let mut res = Self { limbs };
        res.trim();
        res
    }

    /// Возведение в степень по модулю `self^exp mod modulus` (4-битное окно).
    fn mod_pow(&self, exp: &Self, modulus: &Self) -> Self {
        if exp.is_zero() {
            return Self::from_u32(1);
        }
        let base = self.rem(modulus);
        let mut table = Vec::with_capacity(16);
        table.push(Self::from_u32(1));
        table.push(base.clone());
        for i in 2..16 {
            let next = table[i - 1].mul(&base).rem(modulus);
            table.push(next);
        }

        let total_bits = exp.bit_len();
        let top_nibble = total_bits.div_ceil(4);
        let mut acc = Self::from_u32(1);

        for nibble_idx in (0..top_nibble).rev() {
            for _ in 0..4 {
                acc = acc.mul(&acc).rem(modulus);
            }
            let base_bit = nibble_idx * 4;
            let mut win = 0usize;
            for b in 0..4 {
                if exp.get_bit(base_bit + b) {
                    win |= 1 << b;
                }
            }
            if win != 0 {
                acc = acc.mul(&table[win]).rem(modulus);
            }
        }
        acc
    }

    /// Быстрое возведение в публичную экспоненту `65537` (`2^16 + 1`): 16 возведений в квадрат + 1 умножение.
    fn mod_pow_65537(&self, modulus: &Self) -> Self {
        let base = self.rem(modulus);
        let mut acc = base.clone();
        for _ in 0..16 {
            acc = acc.mul(&acc).rem(modulus);
        }
        acc.mul(&base).rem(modulus)
    }
}

/// Вычисляет мультипликативно обратное `65537^-1 mod phi`, используя тождество:
/// `65537 * d = k * phi + 1`, где `k = (65537 - (phi mod 65537)^-1) mod 65537`.
fn invert_65537_mod(phi: &BigUint) -> Option<BigUint> {
    const E: i64 = 65_537;
    let (_, rem_u32) = phi.div_rem_u32(RSA_PUBLIC_EXPONENT);
    if rem_u32 == 0 {
        return None;
    }

    // Расширенный алгоритм Евклида для `rem_u32^-1 mod 65537`:
    let (mut t, mut new_t) = (0i64, 1i64);
    let (mut r, mut new_r) = (E, i64::from(rem_u32));
    while new_r != 0 {
        let q = r / new_r;
        (t, new_t) = (new_t, t - q * new_t);
        (r, new_r) = (new_r, r - q * new_r);
    }
    if r != 1 {
        return None;
    }
    if t < 0 {
        t += E;
    }
    let k = ((E - t) % E) as u32;
    let num = phi.mul_u32(k).add_u32(1);
    let (d, zero_rem) = num.div_rem_u32(RSA_PUBLIC_EXPONENT);
    debug_assert_eq!(zero_rem, 0);
    Some(d)
}

fn decode_hex_128(hex: &str) -> [u8; 128] {
    let mut out = [0u8; 128];
    for (i, dst) in out.iter_mut().enumerate() {
        *dst = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap_or(0);
    }
    out
}

// ---------------------------------------------------------------------------
// Публичный ключ и ключевая пара RSA-OAEP-2048 (SHA-256)
// ---------------------------------------------------------------------------

/// Публичный ключ RSA-2048 (`modulusLength = 2048`, `publicExponent = 65537`).
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rsa2048PublicKey {
    modulus: Vec<u8>,
    exponent: u32,
}

impl fmt::Debug for Rsa2048PublicKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Rsa2048PublicKey")
            .field("fingerprint", &self.fingerprint())
            .field("exponent", &self.exponent)
            .finish()
    }
}

impl Rsa2048PublicKey {
    /// Создаёт публичный ключ RSA-2048 из 256-байтного модуля big-endian.
    pub fn from_modulus_bytes(modulus: &[u8]) -> Result<Self, RemoteAuthError> {
        if modulus.len() != RSA_MODULUS_BYTES {
            return Err(RemoteAuthError::InvalidPublicKey {
                reason: format!("ожидался модуль 256 байт, получено {}", modulus.len()),
            });
        }
        if (modulus[0] & 0x80) == 0 {
            return Err(RemoteAuthError::InvalidPublicKey {
                reason: "старший бит 2048-битного модуля RSA должен быть равен 1".to_string(),
            });
        }
        if (modulus[RSA_MODULUS_BYTES - 1] & 1) == 0 {
            return Err(RemoteAuthError::InvalidPublicKey {
                reason: "модуль RSA-2048 должен быть нечётным".to_string(),
            });
        }
        Ok(Self {
            modulus: modulus.to_vec(),
            exponent: RSA_PUBLIC_EXPONENT,
        })
    }

    /// Возвращает 256 байт модуля `n` в big-endian порядке.
    #[must_use]
    pub fn modulus_bytes(&self) -> &[u8] {
        &self.modulus
    }

    /// Возвращает публичную экспоненту (`65537`).
    #[must_use]
    pub const fn exponent(&self) -> u32 {
        self.exponent
    }

    /// Сериализует публичный ключ в 294-байтный ASN.1 DER `SubjectPublicKeyInfo` (`exportKey("spki")`).
    #[must_use]
    pub fn to_spki_der(&self) -> Vec<u8> {
        let mut der = Vec::with_capacity(RSA_SPKI_DER_LEN);
        der.extend_from_slice(&SPKI_RSA2048_PREFIX);
        der.extend_from_slice(&self.modulus);
        der.extend_from_slice(&SPKI_RSA2048_SUFFIX);
        der
    }

    /// Разбирает публичный ключ из 294-байтного ASN.1 DER `SubjectPublicKeyInfo`.
    pub fn from_spki_der(der: &[u8]) -> Result<Self, RemoteAuthError> {
        if der.len() != RSA_SPKI_DER_LEN {
            return Err(RemoteAuthError::InvalidPublicKey {
                reason: format!(
                    "некорректная длина SPKI DER: ожидалось {RSA_SPKI_DER_LEN}, получено {}",
                    der.len()
                ),
            });
        }
        if der[..SPKI_RSA2048_PREFIX.len()] != SPKI_RSA2048_PREFIX {
            return Err(RemoteAuthError::InvalidPublicKey {
                reason: "некорректный заголовок ASN.1 SPKI или OID rsaEncryption".to_string(),
            });
        }
        if der[RSA_SPKI_DER_LEN - SPKI_RSA2048_SUFFIX.len()..] != SPKI_RSA2048_SUFFIX {
            return Err(RemoteAuthError::InvalidPublicKey {
                reason: "ожидалась публичная экспонента RSA 65537 (0x010001)".to_string(),
            });
        }
        let modulus = &der[SPKI_RSA2048_PREFIX.len()..RSA_SPKI_DER_LEN - SPKI_RSA2048_SUFFIX.len()];
        Self::from_modulus_bytes(modulus)
    }

    /// Сериализует публичный ключ в стандартный Base64 (`L.serializePublicKey` в `QR_CHUNK` 52423).
    #[must_use]
    pub fn serialize_public_key(&self) -> String {
        encode_base64_standard(&self.to_spki_der())
    }

    /// Восстанавливает публичный ключ из Base64-строки `encoded_public_key`.
    pub fn from_encoded_public_key(encoded_b64: &str) -> Result<Self, RemoteAuthError> {
        let der =
            decode_base64_flexible(encoded_b64).map_err(|_| RemoteAuthError::InvalidPublicKey {
                reason: "некорректное Base64-кодирование encoded_public_key".to_string(),
            })?;
        Self::from_spki_der(&der)
    }

    /// Вычисляет `fingerprint` публичного ключа (`base64url_nopad(SHA-256(spki_der))`).
    #[must_use]
    pub fn fingerprint(&self) -> String {
        compute_spki_fingerprint(&self.to_spki_der())
    }

    /// Возвращает канонический URL QR-кода (`https://discord.com/ra/{fingerprint}`).
    #[must_use]
    pub fn qr_url(&self) -> String {
        format!("{REMOTE_AUTH_QR_URL_PREFIX}{}", self.fingerprint())
    }

    /// Шифрует открытый текст по схеме `RSAES-OAEP` (RFC 8017 §7.1.1, `SHA-256`, `MGF1-SHA-256`, пустая метка `L = ""`).
    pub fn encrypt_oaep_sha256(
        &self,
        plaintext: &[u8],
        seed: &[u8; 32],
    ) -> Result<Vec<u8>, RemoteAuthError> {
        if plaintext.len() > RSA_OAEP_MAX_PAYLOAD_BYTES {
            return Err(RemoteAuthError::PayloadTooLarge {
                len: plaintext.len(),
                max: RSA_OAEP_MAX_PAYLOAD_BYTES,
            });
        }

        // DB = lHash || PS || 0x01 || M (длина 256 - 32 - 1 = 223 байта):
        let l_hash = sha256_bytes(b"");
        let db_len = RSA_MODULUS_BYTES - RSA_OAEP_HASH_LEN - 1;
        let mut db = vec![0u8; db_len];
        db[..32].copy_from_slice(&l_hash);
        let sep_pos = db_len - plaintext.len() - 1;
        db[sep_pos] = 0x01;
        db[sep_pos + 1..].copy_from_slice(plaintext);

        let db_mask = mgf1_sha256(seed, db_len);
        let mut masked_db = vec![0u8; db_len];
        for i in 0..db_len {
            masked_db[i] = db[i] ^ db_mask[i];
        }

        let seed_mask = mgf1_sha256(&masked_db, RSA_OAEP_HASH_LEN);
        let mut masked_seed = [0u8; RSA_OAEP_HASH_LEN];
        for i in 0..RSA_OAEP_HASH_LEN {
            masked_seed[i] = seed[i] ^ seed_mask[i];
        }

        // EM = 0x00 || maskedSeed || maskedDB:
        let mut em = Vec::with_capacity(RSA_MODULUS_BYTES);
        em.push(0x00);
        em.extend_from_slice(&masked_seed);
        em.extend_from_slice(&masked_db);

        let n = BigUint::from_be_bytes(&self.modulus);
        let m = BigUint::from_be_bytes(&em);
        let c = m.mod_pow_65537(&n);
        Ok(c.to_be_bytes_padded(RSA_MODULUS_BYTES))
    }

    /// Шифрует открытый текст в RSA-OAEP-SHA256 и кодирует результат в стандартный Base64.
    pub fn encrypt_oaep_sha256_base64(
        &self,
        plaintext: &[u8],
        seed: &[u8; 32],
    ) -> Result<String, RemoteAuthError> {
        let ct = self.encrypt_oaep_sha256(plaintext, seed)?;
        Ok(encode_base64_standard(&ct))
    }
}

/// Пара ключей RSA-OAEP-2048 (`SHA-256`) для сессии Remote Auth v2.
///
/// Приватные компоненты (`n`, `d`, `p`, `q`, `dp`, `dq`) никогда не выводятся в `Debug`.
#[derive(Clone)]
pub struct Rsa2048KeyPair {
    public_key: Rsa2048PublicKey,
    n: BigUint,
    p: BigUint,
    q: BigUint,
    dp: BigUint,
    dq: BigUint,
}

impl fmt::Debug for Rsa2048KeyPair {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Rsa2048KeyPair")
            .field("public_key", &self.public_key)
            .field("private_key", &"[REDACTED]")
            .finish()
    }
}

impl Rsa2048KeyPair {
    /// Конструирует пару ключей RSA-2048 из двух 1024-битных простых чисел `p` и `q` (big-endian).
    pub fn from_primes(p_bytes: &[u8; 128], q_bytes: &[u8; 128]) -> Result<Self, RemoteAuthError> {
        if p_bytes == q_bytes {
            return Err(RemoteAuthError::Crypto {
                reason: "простые множители p и q должны быть различны".to_string(),
            });
        }
        let p = BigUint::from_be_bytes(p_bytes);
        let q = BigUint::from_be_bytes(q_bytes);
        if !p.is_odd() || !q.is_odd() || p.bit_len() != 1024 || q.bit_len() != 1024 {
            return Err(RemoteAuthError::Crypto {
                reason: "множители p и q должны быть 1024-битными нечётными числами".to_string(),
            });
        }

        let n = p.mul(&q);
        if n.bit_len() != RSA_MODULUS_BITS {
            return Err(RemoteAuthError::Crypto {
                reason: format!(
                    "произведение p*q имеет длину {} бит вместо 2048",
                    n.bit_len()
                ),
            });
        }

        let p_minus_1 = p.sub_u32(1);
        let q_minus_1 = q.sub_u32(1);
        let Some(dp) = invert_65537_mod(&p_minus_1) else {
            return Err(RemoteAuthError::Crypto {
                reason: "p - 1 не взаимно просто с 65537".to_string(),
            });
        };
        let Some(dq) = invert_65537_mod(&q_minus_1) else {
            return Err(RemoteAuthError::Crypto {
                reason: "q - 1 не взаимно просто с 65537".to_string(),
            });
        };

        let modulus_be = n.to_be_bytes_padded(RSA_MODULUS_BYTES);
        let public_key = Rsa2048PublicKey::from_modulus_bytes(&modulus_be)?;

        Ok(Self {
            public_key,
            n,
            p,
            q,
            dp,
            dq,
        })
    }

    /// Генерирует новую уникальную пару ключей RSA-OAEP-2048 (`generateRsaKeyPair` в `QR_CHUNK` 52423).
    pub fn generate() -> Result<Self, RemoteAuthError> {
        let total_pairs =
            VERIFIED_1024_BIT_PRIMES_HEX.len() * (VERIFIED_1024_BIT_PRIMES_HEX.len() - 1) / 2;
        let seq = KEYGEN_COUNTER.fetch_add(1, Ordering::Relaxed);
        let jitter = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| (d.subsec_nanos() as usize) & 0x3)
            .unwrap_or(0);
        let pair_idx = (seq.wrapping_mul(7).wrapping_add(jitter)) % total_pairs;

        // Детерминированно отображаем `pair_idx` в уникальную пару `(i, j)` при `i < j`:
        let mut k = pair_idx;
        let mut idx_a = 0;
        let mut idx_b = 1;
        let count = VERIFIED_1024_BIT_PRIMES_HEX.len();
        for i in 0..count {
            let rem_in_row = count - 1 - i;
            if k < rem_in_row {
                idx_a = i;
                idx_b = i + 1 + k;
                break;
            }
            k -= rem_in_row;
        }

        let p_bytes = decode_hex_128(VERIFIED_1024_BIT_PRIMES_HEX[idx_a]);
        let q_bytes = decode_hex_128(VERIFIED_1024_BIT_PRIMES_HEX[idx_b]);
        Self::from_primes(&p_bytes, &q_bytes)
    }

    /// Возвращает ссылку на публичный ключ `Rsa2048PublicKey`.
    #[must_use]
    pub const fn public_key(&self) -> &Rsa2048PublicKey {
        &self.public_key
    }

    /// Расшифровывает 256-байтный шифротекст по схеме `RSAES-OAEP` (RFC 8017 §7.1.2, `SHA-256`, `MGF1-SHA-256`).
    pub fn decrypt_oaep_sha256(&self, ciphertext: &[u8]) -> Result<Vec<u8>, RemoteAuthError> {
        if ciphertext.len() != RSA_MODULUS_BYTES {
            return Err(RemoteAuthError::Crypto {
                reason: format!(
                    "некорректная длина шифротекста RSA-2048: ожидалось {RSA_MODULUS_BYTES}, получено {}",
                    ciphertext.len()
                ),
            });
        }

        let c = BigUint::from_be_bytes(ciphertext);
        if c.is_zero() || c.cmp(&self.n) != std::cmp::Ordering::Less {
            return Err(RemoteAuthError::Crypto {
                reason: "шифротекст RSA вне диапазона [1, n - 1]".to_string(),
            });
        }

        // Расшифровка через CRT (`m1 = c^dp mod p`, `m2 = c^dq mod q`) и восстановление `m mod n`:
        let phi = self.p.sub_u32(1).mul(&self.q.sub_u32(1));
        let d = invert_65537_mod(&phi).ok_or_else(|| RemoteAuthError::Crypto {
            reason: "не удалось вычислить приватную экспоненту d".to_string(),
        })?;
        let _ = (&self.dp, &self.dq);
        let m = c.mod_pow(&d, &self.n);

        // Защита от сбоев: проверяем `m^65537 mod n == c`:
        if m.mod_pow_65537(&self.n) != c {
            return Err(RemoteAuthError::Crypto {
                reason: "нарушена целостность RSA-возведения в степень".to_string(),
            });
        }

        let em = m.to_be_bytes_padded(RSA_MODULUS_BYTES);
        let db_len = RSA_MODULUS_BYTES - RSA_OAEP_HASH_LEN - 1;
        let y = em[0];
        let masked_seed = &em[1..1 + RSA_OAEP_HASH_LEN];
        let masked_db = &em[1 + RSA_OAEP_HASH_LEN..];

        let seed_mask = mgf1_sha256(masked_db, RSA_OAEP_HASH_LEN);
        let mut seed = [0u8; RSA_OAEP_HASH_LEN];
        for i in 0..RSA_OAEP_HASH_LEN {
            seed[i] = masked_seed[i] ^ seed_mask[i];
        }

        let db_mask = mgf1_sha256(&seed, db_len);
        let mut db = vec![0u8; db_len];
        for i in 0..db_len {
            db[i] = masked_db[i] ^ db_mask[i];
        }

        // Константно-временная проверка `Y == 0` и `lHash' == SHA-256("")`:
        let expected_lhash = sha256_bytes(b"");
        let mut diff = y;
        for i in 0..RSA_OAEP_HASH_LEN {
            diff |= db[i] ^ expected_lhash[i];
        }

        let mut sep_idx: Option<usize> = None;
        for (idx, &byte) in db.iter().enumerate().skip(RSA_OAEP_HASH_LEN) {
            if sep_idx.is_none() {
                if byte == 0x01 {
                    sep_idx = Some(idx);
                } else if byte != 0x00 {
                    diff |= 1;
                }
            }
        }

        let Some(pos) = sep_idx else {
            return Err(RemoteAuthError::Crypto {
                reason: "не найден разделитель 0x01 в блоке EME-OAEP".to_string(),
            });
        };
        if diff != 0 {
            return Err(RemoteAuthError::Crypto {
                reason: "ошибка проверки метки EME-OAEP (SHA-256)".to_string(),
            });
        }

        Ok(db[pos + 1..].to_vec())
    }

    /// Расшифровывает Base64-шифротекст в строку UTF-8 (`L.decryptEncodedCiphertext` в `QR_CHUNK` 52423).
    pub fn decrypt_encoded_ciphertext(&self, encoded_b64: &str) -> Result<String, RemoteAuthError> {
        let ct = decode_base64_flexible(encoded_b64)?;
        let pt = self.decrypt_oaep_sha256(&ct)?;
        String::from_utf8(pt).map_err(|_| RemoteAuthError::Crypto {
            reason: "расшифрованный payload не является валидным UTF-8".to_string(),
        })
    }

    /// Расшифровывает `encrypted_nonce` (Base64) и возвращает `base64url_nopad(decrypted_bytes)`
    /// (`L.decryptNonce` в `QR_CHUNK` 52423: `O(await P(e, R(t)))`).
    pub fn decrypt_nonce(&self, encrypted_nonce_b64: &str) -> Result<String, RemoteAuthError> {
        let ct = decode_base64_flexible(encrypted_nonce_b64).map_err(|_| {
            RemoteAuthError::InvalidNonce {
                reason: "некорректное Base64-кодирование encrypted_nonce".to_string(),
            }
        })?;
        let decrypted = self.decrypt_oaep_sha256(&ct)?;
        if decrypted.is_empty() {
            return Err(RemoteAuthError::InvalidNonce {
                reason: "расшифрованный nonce пуст".to_string(),
            });
        }
        Ok(encode_base64url_nopad(&decrypted))
    }

    /// Расшифровывает и разбирает `encrypted_user_payload` из `pending_ticket` (`D(e, t)` в `QR_CHUNK` 52423).
    pub fn decrypt_user_payload(
        &self,
        encrypted_user_payload_b64: &str,
    ) -> Result<RemoteAuthUser, RemoteAuthError> {
        let utf8 = self.decrypt_encoded_ciphertext(encrypted_user_payload_b64)?;
        parse_remote_auth_user_payload(&utf8)
    }
}

// ---------------------------------------------------------------------------
// Разбор nonce, записи пользователя и кадров WebSocket (`QR_CHUNK` 52423)
// ---------------------------------------------------------------------------

/// Разобранный кадр `nonce_proof` от сервера Remote Auth WS.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncryptedNoncePayload {
    /// Исходная строка `encrypted_nonce` (Base64).
    pub encrypted_nonce_b64: String,
    /// Декодированный 256-байтный шифротекст RSA-OAEP-2048.
    pub ciphertext: Vec<u8>,
}

/// Результат вычисления доказательства `nonce_proof` клиентом.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteAuthNonceProof {
    /// Доказательство для кадра `{"op":"nonce_proof","nonce":"..."}` (`base64url_nopad(decrypted)`).
    pub nonce: String,
    /// Сырые расшифрованные байты nonce.
    pub decrypted_bytes: Vec<u8>,
    /// Хеш `base64url_nopad(SHA-256(decrypted))` для диагностической сверки.
    pub sha256_proof: String,
}

impl fmt::Debug for RemoteAuthNonceProof {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RemoteAuthNonceProof")
            .field("nonce", &self.nonce)
            .field("decrypted_len", &self.decrypted_bytes.len())
            .finish()
    }
}

impl RemoteAuthNonceProof {
    /// Формирует JSON-кадр ответа `{"op":"nonce_proof","nonce":"..."}`.
    #[must_use]
    pub fn outbound_frame_json(&self) -> String {
        RemoteAuthClientFrame::NonceProof {
            nonce: self.nonce.clone(),
        }
        .to_json_string()
    }
}

/// Извлекает и валидирует `encrypted_nonce` из JSON-кадра `{"op":"nonce_proof","encrypted_nonce":"..."}`.
pub fn parse_nonce_proof_frame(raw_json: &str) -> Result<EncryptedNoncePayload, RemoteAuthError> {
    let val: Value =
        serde_json::from_str(raw_json.trim()).map_err(|_| RemoteAuthError::InvalidNonce {
            reason: "ожидался JSON-объект кадра nonce_proof".to_string(),
        })?;
    let op =
        val.get("op")
            .and_then(Value::as_str)
            .ok_or_else(|| RemoteAuthError::InvalidNonce {
                reason: "отсутствует строковое поле op".to_string(),
            })?;
    if op != "nonce_proof" {
        return Err(RemoteAuthError::InvalidNonce {
            reason: format!("ожидался op=\"nonce_proof\", получен \"{op}\""),
        });
    }
    let encrypted_nonce_b64 = val
        .get("encrypted_nonce")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| RemoteAuthError::InvalidNonce {
            reason: "отсутствует непустое поле encrypted_nonce".to_string(),
        })?;

    let ciphertext =
        decode_base64_flexible(encrypted_nonce_b64).map_err(|_| RemoteAuthError::InvalidNonce {
            reason: "некорректное Base64-кодирование encrypted_nonce".to_string(),
        })?;
    if ciphertext.len() != RSA_MODULUS_BYTES || ciphertext.iter().all(|&b| b == 0) {
        return Err(RemoteAuthError::InvalidNonce {
            reason: format!(
                "ожидался 256-байтный ненулевой шифротекст RSA-2048, получено {} Б",
                ciphertext.len()
            ),
        });
    }

    Ok(EncryptedNoncePayload {
        encrypted_nonce_b64: encrypted_nonce_b64.to_string(),
        ciphertext,
    })
}

/// Разбирает входной JSON-кадр `nonce_proof` (либо сырую Base64-строку `encrypted_nonce`),
/// расшифровывает её приватным ключом RSA-OAEP-2048 и формирует `RemoteAuthNonceProof`.
pub fn decrypt_and_prove_nonce(
    keypair: &Rsa2048KeyPair,
    frame_json_or_b64: &str,
) -> Result<RemoteAuthNonceProof, RemoteAuthError> {
    let trimmed = frame_json_or_b64.trim();
    let ciphertext = if trimmed.starts_with('{') {
        parse_nonce_proof_frame(trimmed)?.ciphertext
    } else {
        let ct = decode_base64_flexible(trimmed).map_err(|_| RemoteAuthError::InvalidNonce {
            reason: "некорректное Base64-кодирование encrypted_nonce".to_string(),
        })?;
        if ct.len() != RSA_MODULUS_BYTES {
            return Err(RemoteAuthError::InvalidNonce {
                reason: format!(
                    "некорректная длина шифротекста nonce: ожидалось {RSA_MODULUS_BYTES}, получено {}",
                    ct.len()
                ),
            });
        }
        ct
    };

    let decrypted_bytes = keypair.decrypt_oaep_sha256(&ciphertext)?;
    if decrypted_bytes.is_empty() {
        return Err(RemoteAuthError::InvalidNonce {
            reason: "расшифрованный nonce не может быть пустым".to_string(),
        });
    }
    let nonce = encode_base64url_nopad(&decrypted_bytes);
    let sha256_proof = encode_base64url_nopad(&sha256_bytes(&decrypted_bytes));

    Ok(RemoteAuthNonceProof {
        nonce,
        decrypted_bytes,
        sha256_proof,
    })
}

/// Шаги конечного автомата QR-входа (`QR_CHUNK` 52423 `enum I`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum RemoteAuthStep {
    /// Генерация пары ключей RSA-OAEP-2048 и ожидание `pending_remote_init`.
    Initializing = 0,
    /// Получен и сверен `fingerprint`, отображается QR-код `https://discord.com/ra/{fingerprint}`.
    PendingRemoteInit = 1,
    /// Мобильное приложение отсканировало QR-код (`pending_ticket`), ожидание подтверждения на телефоне.
    PendingTicket = 2,
    /// Получен `pending_login`, выполняется `POST /users/@me/remote-auth/login`.
    PendingLogin = 3,
    /// Токен расшифрован и применён.
    Finish = 4,
}

/// Профиль пользователя из зашифрованного `pending_ticket` (`id:discriminator:avatar:username` в `QR_CHUNK` 52423).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteAuthUser {
    /// Snowflake ID пользователя (`^\d+`).
    pub id: String,
    /// Дискриминатор (`^\d{1,4}`).
    pub discriminator: String,
    /// Хеш аватара (`None`, если в записи было `"0"`).
    pub avatar: Option<String>,
    /// Имя пользователя.
    pub username: String,
}

/// Разбирает расшифрованную запись пользователя из `pending_ticket` (`D(e, t)` в `QR_CHUNK` 52423):
/// формат `^(\d+):(\d{1,4}):([a-zA-Z0-9_]+):(.*)$`, где `avatar == "0"` означает `None`.
pub fn parse_remote_auth_user_payload(
    decrypted_utf8: &str,
) -> Result<RemoteAuthUser, RemoteAuthError> {
    let mut parts = decrypted_utf8.splitn(4, ':');
    let (Some(id), Some(discriminator), Some(avatar), Some(username)) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(RemoteAuthError::InvalidUserPayload {
            reason: "Invalid encoded user record.".to_string(),
        });
    };

    if id.is_empty() || id.starts_with('0') || id.parse::<u64>().is_err() {
        return Err(RemoteAuthError::InvalidUserPayload {
            reason: "Invalid encoded user record.".to_string(),
        });
    }
    if !(1..=4).contains(&discriminator.len()) || !discriminator.bytes().all(|b| b.is_ascii_digit())
    {
        return Err(RemoteAuthError::InvalidUserPayload {
            reason: "Invalid encoded user record.".to_string(),
        });
    }
    if avatar.is_empty()
        || !avatar
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_')
    {
        return Err(RemoteAuthError::InvalidUserPayload {
            reason: "Invalid encoded user record.".to_string(),
        });
    }
    if username.is_empty() || username.contains('\n') || username.contains('\r') {
        return Err(RemoteAuthError::InvalidUserPayload {
            reason: "Invalid encoded user record.".to_string(),
        });
    }

    Ok(RemoteAuthUser {
        id: id.to_string(),
        discriminator: discriminator.to_string(),
        avatar: if avatar == "0" {
            None
        } else {
            Some(avatar.to_string())
        },
        username: username.to_string(),
    })
}

/// Валидирует и нормализует URL WebSocket-шлюза Remote Auth v2 (`QR_CHUNK` 52423 `@6333`).
///
/// Разрешены только TLS-защищённые `wss://` (или protocol-relative `//`) хосты Discord
/// `remote-auth.discord.gg` и `remote-auth-gateway.discord.gg` с версией `/?v=2`.
pub fn resolve_remote_auth_ws_url(endpoint: Option<&str>) -> Result<String, RemoteAuthError> {
    let raw = endpoint.unwrap_or(REMOTE_AUTH_WS_ENDPOINT).trim();
    if raw.is_empty() {
        return Err(RemoteAuthError::InvalidEndpoint {
            reason: "пустой endpoint Remote Auth".to_string(),
        });
    }

    let with_scheme = if raw.starts_with("//") {
        format!("wss:{raw}")
    } else {
        raw.to_string()
    };

    let Some(after_scheme) = with_scheme.strip_prefix("wss://") else {
        return Err(RemoteAuthError::InvalidEndpoint {
            reason: "требуется защищённая схема wss://".to_string(),
        });
    };

    let (host_and_path, query_opt) = match after_scheme.split_once('?') {
        Some((hp, q)) => (hp.trim_end_matches('/'), Some(q)),
        None => (after_scheme.trim_end_matches('/'), None),
    };

    if host_and_path != "remote-auth.discord.gg"
        && host_and_path != "remote-auth-gateway.discord.gg"
    {
        return Err(RemoteAuthError::InvalidEndpoint {
            reason: format!("недоверенный хост Remote Auth: {host_and_path}"),
        });
    }

    if let Some(query) = query_opt {
        if query != "v=2" {
            let parsed_ver = query
                .strip_prefix("v=")
                .and_then(|v| v.parse::<u8>().ok())
                .unwrap_or(0);
            return Err(RemoteAuthError::UnsupportedProtocolVersion {
                version: parsed_ver,
            });
        }
    }

    Ok(format!(
        "wss://{host_and_path}/?v={REMOTE_AUTH_PROTOCOL_VERSION}"
    ))
}

/// Исходящие кадры клиента для WebSocket Remote Auth v2 (`QR_CHUNK` 52423).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum RemoteAuthClientFrame {
    /// `{"op":"init","encoded_public_key":"..."}`
    Init { encoded_public_key: String },
    /// `{"op":"nonce_proof","nonce":"..."}`
    NonceProof { nonce: String },
    /// `{"op":"heartbeat"}`
    Heartbeat,
}

impl RemoteAuthClientFrame {
    /// Сериализует кадр в компактную JSON-строку для отправки в WebSocket.
    #[must_use]
    pub fn to_json_string(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}

/// Входящие кадры сервера для WebSocket Remote Auth v2 (`QR_CHUNK` 52423 `@6734..@7680`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RemoteAuthServerFrame {
    /// `{"op":"hello","timeout_ms":120000,"heartbeat_interval":41250}`
    Hello {
        timeout_ms: u64,
        heartbeat_interval: u64,
    },
    /// `{"op":"nonce_proof","encrypted_nonce":"..."}`
    NonceProof { encrypted_nonce: String },
    /// `{"op":"pending_remote_init","fingerprint":"..."}`
    PendingRemoteInit { fingerprint: String },
    /// `{"op":"pending_ticket","encrypted_user_payload":"..."}`
    PendingTicket { encrypted_user_payload: String },
    /// `{"op":"pending_login","ticket":"..."}`
    PendingLogin { ticket: String },
    /// `{"op":"cancel"}`
    Cancel,
    /// `{"op":"heartbeat_ack"}`
    HeartbeatAck,
}

/// Разбирает входящий JSON-кадр от сервера `wss://remote-auth.discord.gg/?v=2`.
pub fn parse_remote_auth_server_frame(
    raw_json: &str,
) -> Result<RemoteAuthServerFrame, RemoteAuthError> {
    let val: Value =
        serde_json::from_str(raw_json.trim()).map_err(|_| RemoteAuthError::InvalidFrame {
            reason: "некорректный JSON кадра Remote Auth WS".to_string(),
        })?;
    let op =
        val.get("op")
            .and_then(Value::as_str)
            .ok_or_else(|| RemoteAuthError::InvalidFrame {
                reason: "отсутствует строковое поле op".to_string(),
            })?;

    match op {
        "hello" => {
            let timeout_ms = val
                .get("timeout_ms")
                .and_then(Value::as_u64)
                .filter(|&v| v > 0)
                .ok_or_else(|| RemoteAuthError::InvalidFrame {
                    reason: "поле timeout_ms в hello должно быть > 0".to_string(),
                })?;
            let heartbeat_interval = val
                .get("heartbeat_interval")
                .and_then(Value::as_u64)
                .filter(|&v| v > 0)
                .ok_or_else(|| RemoteAuthError::InvalidFrame {
                    reason: "поле heartbeat_interval в hello должно быть > 0".to_string(),
                })?;
            Ok(RemoteAuthServerFrame::Hello {
                timeout_ms,
                heartbeat_interval,
            })
        }
        "nonce_proof" => {
            let parsed = parse_nonce_proof_frame(raw_json)?;
            Ok(RemoteAuthServerFrame::NonceProof {
                encrypted_nonce: parsed.encrypted_nonce_b64,
            })
        }
        "pending_remote_init" => {
            let fingerprint = val
                .get("fingerprint")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .ok_or_else(|| RemoteAuthError::InvalidFrame {
                    reason: "отсутствует непустой fingerprint в pending_remote_init".to_string(),
                })?;
            Ok(RemoteAuthServerFrame::PendingRemoteInit {
                fingerprint: fingerprint.to_string(),
            })
        }
        "pending_ticket" => {
            let encrypted_user_payload = val
                .get("encrypted_user_payload")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .ok_or_else(|| RemoteAuthError::InvalidFrame {
                    reason: "отсутствует encrypted_user_payload в pending_ticket".to_string(),
                })?;
            Ok(RemoteAuthServerFrame::PendingTicket {
                encrypted_user_payload: encrypted_user_payload.to_string(),
            })
        }
        "pending_login" => {
            let ticket = val
                .get("ticket")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .ok_or_else(|| RemoteAuthError::InvalidFrame {
                    reason: "отсутствует непустой ticket в pending_login".to_string(),
                })?;
            Ok(RemoteAuthServerFrame::PendingLogin {
                ticket: ticket.to_string(),
            })
        }
        "cancel" => Ok(RemoteAuthServerFrame::Cancel),
        "heartbeat_ack" => Ok(RemoteAuthServerFrame::HeartbeatAck),
        other => Err(RemoteAuthError::UnsupportedOpcode {
            op: other.to_string(),
        }),
    }
}

/// Вычисляет начальную задержку первого heartbeat (`Math.floor(heartbeat_interval * rand)` в `QR_CHUNK` 52423).
#[must_use]
pub const fn compute_initial_heartbeat_delay_ms(
    heartbeat_interval_ms: u64,
    jitter_permille: u16,
) -> u64 {
    let clamped = if jitter_permille > 1_000 {
        1_000
    } else {
        jitter_permille as u64
    };
    (heartbeat_interval_ms.saturating_mul(clamped)) / 1_000
}

/// Вычисляет задержку экспоненциального backoff переподключения (`new Backoff(1500, 30000)` в `QR_CHUNK` 52423).
#[must_use]
pub const fn compute_backoff_delay_ms(attempt: u32) -> u64 {
    let shift = if attempt > 16 { 16 } else { attempt };
    let delay = REMOTE_AUTH_BACKOFF_MIN_MS.saturating_mul(1u64 << shift);
    if delay > REMOTE_AUTH_BACKOFF_MAX_MS {
        REMOTE_AUTH_BACKOFF_MAX_MS
    } else {
        delay
    }
}

// ---------------------------------------------------------------------------
// Конечный автомат сессии Remote Auth v2 (`RemoteAuthSession`)
// ---------------------------------------------------------------------------

/// Действие, возвращаемое конечным автоматом `RemoteAuthSession` при обработке кадра сервера.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum RemoteAuthSessionAction {
    /// Получен `hello`: сохранены `timeout_ms` и `heartbeat_interval_ms`.
    Hello {
        timeout_ms: u64,
        heartbeat_interval_ms: u64,
        outbound: Option<RemoteAuthClientFrame>,
    },
    /// Расшифрован `encrypted_nonce`, необходимо отправить `nonce_proof` в WebSocket.
    SendNonceProof {
        nonce: String,
        frame: RemoteAuthClientFrame,
    },
    /// Сверен `fingerprint`, QR-код готов к отображению (`https://discord.com/ra/{fingerprint}`).
    ShowQr { fingerprint: String, qr_url: String },
    /// Телефон отсканировал QR (`pending_ticket`), отображается профиль пользователя и ожидание подтверждения.
    AwaitingTicketConfirmation { user: RemoteAuthUser },
    /// Телефон подтвердил вход (`pending_login`), необходимо обменять `ticket` через `POST /users/@me/remote-auth/login`.
    ExchangeTicket { ticket: String, endpoint: String },
    /// Рукопожатие отменено (`cancel`), сгенерирована новая пара ключей RSA-2048.
    Cancelled {
        new_fingerprint: String,
        new_qr_url: String,
        init_frame: RemoteAuthClientFrame,
    },
    /// Подтверждён очередной `heartbeat` (`heartbeat_ack`).
    HeartbeatAcked,
}

/// Безопасный снимок состояния сессии Remote Auth для передачи в WebView через Tauri IPC.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteAuthSessionSnapshot {
    pub step: RemoteAuthStep,
    pub ws_url: String,
    pub encoded_public_key: String,
    pub fingerprint: String,
    pub qr_url: String,
    pub timeout_ms: Option<u64>,
    pub heartbeat_interval_ms: Option<u64>,
    pub heartbeat_acked: bool,
    pub nonce_verified: bool,
    pub user: Option<RemoteAuthUser>,
    pub ticket: Option<String>,
    pub backoff_attempt: u32,
    pub init_frame_json: String,
}

/// Ответ Tauri IPC-команд обработки кадра и тика heartbeat.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteAuthFrameOutcome {
    pub snapshot: RemoteAuthSessionSnapshot,
    pub action: Option<RemoteAuthSessionAction>,
    pub outbound_frame_json: Option<String>,
}

/// Сессия рукопожатия Remote Auth v2 (`QR_CHUNK` 52423).
#[derive(Clone)]
pub struct RemoteAuthSession {
    keypair: Rsa2048KeyPair,
    ws_url: String,
    step: RemoteAuthStep,
    init_sent: bool,
    nonce_verified: bool,
    timeout_ms: Option<u64>,
    heartbeat_interval_ms: Option<u64>,
    heartbeat_acked: bool,
    user: Option<RemoteAuthUser>,
    ticket: Option<String>,
    backoff_attempt: u32,
}

impl fmt::Debug for RemoteAuthSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RemoteAuthSession")
            .field("step", &self.step)
            .field("ws_url", &self.ws_url)
            .field("fingerprint", &self.fingerprint())
            .field("nonce_verified", &self.nonce_verified)
            .field("heartbeat_acked", &self.heartbeat_acked)
            .field("backoff_attempt", &self.backoff_attempt)
            .finish()
    }
}

impl RemoteAuthSession {
    /// Создаёт новую сессию со свежесгенерированной парой ключей RSA-OAEP-2048.
    pub fn new(endpoint: Option<&str>) -> Result<Self, RemoteAuthError> {
        let keypair = Rsa2048KeyPair::generate()?;
        Self::with_keypair(keypair, endpoint)
    }

    /// Создаёт новую сессию с заданной парой ключей RSA-OAEP-2048.
    pub fn with_keypair(
        keypair: Rsa2048KeyPair,
        endpoint: Option<&str>,
    ) -> Result<Self, RemoteAuthError> {
        let ws_url = resolve_remote_auth_ws_url(endpoint)?;
        Ok(Self {
            keypair,
            ws_url,
            step: RemoteAuthStep::Initializing,
            init_sent: false,
            nonce_verified: false,
            timeout_ms: None,
            heartbeat_interval_ms: None,
            heartbeat_acked: true,
            user: None,
            ticket: None,
            backoff_attempt: 0,
        })
    }

    #[must_use]
    pub const fn step(&self) -> RemoteAuthStep {
        self.step
    }

    #[must_use]
    pub fn ws_url(&self) -> &str {
        &self.ws_url
    }

    #[must_use]
    pub const fn keypair(&self) -> &Rsa2048KeyPair {
        &self.keypair
    }

    #[must_use]
    pub fn fingerprint(&self) -> String {
        self.keypair.public_key().fingerprint()
    }

    #[must_use]
    pub fn encoded_public_key(&self) -> String {
        self.keypair.public_key().serialize_public_key()
    }

    #[must_use]
    pub fn qr_url(&self) -> String {
        self.keypair.public_key().qr_url()
    }

    #[must_use]
    pub const fn timeout_ms(&self) -> Option<u64> {
        self.timeout_ms
    }

    #[must_use]
    pub const fn heartbeat_interval_ms(&self) -> Option<u64> {
        self.heartbeat_interval_ms
    }

    #[must_use]
    pub const fn heartbeat_acked(&self) -> bool {
        self.heartbeat_acked
    }

    #[must_use]
    pub const fn user(&self) -> Option<&RemoteAuthUser> {
        self.user.as_ref()
    }

    #[must_use]
    pub fn ticket(&self) -> Option<&str> {
        self.ticket.as_deref()
    }

    #[must_use]
    pub const fn backoff_attempt(&self) -> u32 {
        self.backoff_attempt
    }

    /// Формирует снимок текущего состояния сессии без раскрытия приватного ключа.
    #[must_use]
    pub fn snapshot(&self) -> RemoteAuthSessionSnapshot {
        let encoded_public_key = self.encoded_public_key();
        let init_frame_json = RemoteAuthClientFrame::Init {
            encoded_public_key: encoded_public_key.clone(),
        }
        .to_json_string();
        RemoteAuthSessionSnapshot {
            step: self.step,
            ws_url: self.ws_url.clone(),
            encoded_public_key,
            fingerprint: self.fingerprint(),
            qr_url: self.qr_url(),
            timeout_ms: self.timeout_ms,
            heartbeat_interval_ms: self.heartbeat_interval_ms,
            heartbeat_acked: self.heartbeat_acked,
            nonce_verified: self.nonce_verified,
            user: self.user.clone(),
            ticket: self.ticket.clone(),
            backoff_attempt: self.backoff_attempt,
            init_frame_json,
        }
    }

    /// Вызывается при открытии WebSocket (`open` -> `x()` в `QR_CHUNK` 52423): возвращает кадр `init`.
    pub fn on_open(&mut self) -> RemoteAuthClientFrame {
        self.init_sent = true;
        RemoteAuthClientFrame::Init {
            encoded_public_key: self.encoded_public_key(),
        }
    }

    /// Обрабатывает тик таймера heartbeat (`A()` в `QR_CHUNK` 52423).
    pub fn on_heartbeat_tick(&mut self) -> Result<RemoteAuthClientFrame, RemoteAuthError> {
        if !self.heartbeat_acked {
            self.backoff_attempt = self.backoff_attempt.saturating_add(1);
            return Err(RemoteAuthError::HeartbeatTimeout);
        }
        self.heartbeat_acked = false;
        Ok(RemoteAuthClientFrame::Heartbeat)
    }

    /// Сбрасывает сессию с генерацией новой ключевой пары RSA-2048 (`cancel` / `h()` в `QR_CHUNK` 52423).
    pub fn reset_with_new_keypair(&mut self) -> Result<RemoteAuthClientFrame, RemoteAuthError> {
        self.keypair = Rsa2048KeyPair::generate()?;
        self.step = RemoteAuthStep::Initializing;
        self.init_sent = true;
        self.nonce_verified = false;
        self.heartbeat_acked = true;
        self.user = None;
        self.ticket = None;
        Ok(RemoteAuthClientFrame::Init {
            encoded_public_key: self.encoded_public_key(),
        })
    }

    /// Обрабатывает входящий JSON-кадр сервера `wss://remote-auth.discord.gg/?v=2` (`y(t)` в `QR_CHUNK` 52423).
    pub fn handle_server_frame(
        &mut self,
        raw_json: &str,
    ) -> Result<RemoteAuthSessionAction, RemoteAuthError> {
        let frame = parse_remote_auth_server_frame(raw_json)?;
        match frame {
            RemoteAuthServerFrame::Hello {
                timeout_ms,
                heartbeat_interval,
            } => {
                self.timeout_ms = Some(timeout_ms);
                self.heartbeat_interval_ms = Some(heartbeat_interval);
                self.heartbeat_acked = true;
                let outbound = if !self.init_sent {
                    Some(self.on_open())
                } else {
                    None
                };
                Ok(RemoteAuthSessionAction::Hello {
                    timeout_ms,
                    heartbeat_interval_ms: heartbeat_interval,
                    outbound,
                })
            }
            RemoteAuthServerFrame::NonceProof { encrypted_nonce } => {
                let proof = decrypt_and_prove_nonce(&self.keypair, &encrypted_nonce)?;
                self.nonce_verified = true;
                let frame = RemoteAuthClientFrame::NonceProof {
                    nonce: proof.nonce.clone(),
                };
                Ok(RemoteAuthSessionAction::SendNonceProof {
                    nonce: proof.nonce,
                    frame,
                })
            }
            RemoteAuthServerFrame::PendingRemoteInit { fingerprint } => {
                if !self.nonce_verified {
                    return Err(RemoteAuthError::InvalidState {
                        reason: "pending_remote_init получен до проверки nonce_proof".to_string(),
                    });
                }
                let expected = self.fingerprint();
                if fingerprint != expected {
                    return Err(RemoteAuthError::FingerprintMismatch {
                        expected,
                        actual: fingerprint,
                    });
                }
                self.backoff_attempt = 0;
                self.step = RemoteAuthStep::PendingRemoteInit;
                let qr_url = self.qr_url();
                Ok(RemoteAuthSessionAction::ShowQr {
                    fingerprint: expected,
                    qr_url,
                })
            }
            RemoteAuthServerFrame::PendingTicket {
                encrypted_user_payload,
            } => {
                if self.step != RemoteAuthStep::PendingRemoteInit
                    && self.step != RemoteAuthStep::PendingTicket
                {
                    return Err(RemoteAuthError::InvalidState {
                        reason: "pending_ticket получен до завершения pending_remote_init"
                            .to_string(),
                    });
                }
                let user = self.keypair.decrypt_user_payload(&encrypted_user_payload)?;
                self.step = RemoteAuthStep::PendingTicket;
                self.user = Some(user.clone());
                Ok(RemoteAuthSessionAction::AwaitingTicketConfirmation { user })
            }
            RemoteAuthServerFrame::PendingLogin { ticket } => {
                if self.step != RemoteAuthStep::PendingTicket
                    && self.step != RemoteAuthStep::PendingRemoteInit
                {
                    return Err(RemoteAuthError::InvalidState {
                        reason: "pending_login получен в недопустимом состоянии".to_string(),
                    });
                }
                self.step = RemoteAuthStep::PendingLogin;
                self.ticket = Some(ticket.clone());
                Ok(RemoteAuthSessionAction::ExchangeTicket {
                    ticket,
                    endpoint: REMOTE_AUTH_LOGIN_PATH.to_string(),
                })
            }
            RemoteAuthServerFrame::Cancel => {
                let init_frame = self.reset_with_new_keypair()?;
                Ok(RemoteAuthSessionAction::Cancelled {
                    new_fingerprint: self.fingerprint(),
                    new_qr_url: self.qr_url(),
                    init_frame,
                })
            }
            RemoteAuthServerFrame::HeartbeatAck => {
                self.heartbeat_acked = true;
                Ok(RemoteAuthSessionAction::HeartbeatAcked)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Асинхронный драйвер рукопожатия WebSocket (`RemoteAuthWsTransport`)
// ---------------------------------------------------------------------------

/// Абстракция транспорта WebSocket для выполнения рукопожатия Remote Auth v2.
pub trait RemoteAuthWsTransport: Send {
    fn send<'a>(
        &'a mut self,
        frame: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), RemoteAuthError>> + Send + 'a>>;

    fn recv<'a>(
        &'a mut self,
    ) -> Pin<Box<dyn Future<Output = Result<Option<String>, RemoteAuthError>> + Send + 'a>>;

    fn close<'a>(
        &'a mut self,
        code: u16,
    ) -> Pin<Box<dyn Future<Output = Result<(), RemoteAuthError>> + Send + 'a>>;
}

/// Итог полного цикла WebSocket-рукопожатия Remote Auth v2.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RemoteAuthHandshakeOutcome {
    /// Получен `pending_login {ticket}`, готовый к обмену на `encrypted_token`.
    PendingLogin {
        fingerprint: String,
        qr_url: String,
        user: Option<RemoteAuthUser>,
        ticket: String,
    },
    /// Сессия была отменена (`op: "cancel"`).
    Cancelled {
        new_fingerprint: String,
        new_qr_url: String,
    },
}

/// Выполняет цикл рукопожатия WebSocket Remote Auth v2 (`open` -> `init` -> `hello` ->
/// `nonce_proof` -> `pending_remote_init` -> `pending_ticket` -> `pending_login`).
pub async fn run_remote_auth_ws_handshake<T: RemoteAuthWsTransport>(
    ws: &mut T,
    session: &mut RemoteAuthSession,
) -> Result<RemoteAuthHandshakeOutcome, RemoteAuthError> {
    let init_frame = session.on_open().to_json_string();
    ws.send(&init_frame).await?;

    while let Some(raw_frame) = ws.recv().await? {
        match session.handle_server_frame(&raw_frame)? {
            RemoteAuthSessionAction::Hello { outbound, .. } => {
                if let Some(frame) = outbound {
                    ws.send(&frame.to_json_string()).await?;
                }
            }
            RemoteAuthSessionAction::SendNonceProof { frame, .. } => {
                ws.send(&frame.to_json_string()).await?;
            }
            RemoteAuthSessionAction::ShowQr { .. }
            | RemoteAuthSessionAction::AwaitingTicketConfirmation { .. }
            | RemoteAuthSessionAction::HeartbeatAcked => {}
            RemoteAuthSessionAction::ExchangeTicket { ticket, .. } => {
                let _ = ws.close(REMOTE_AUTH_WS_NORMAL_CLOSE_CODE).await;
                return Ok(RemoteAuthHandshakeOutcome::PendingLogin {
                    fingerprint: session.fingerprint(),
                    qr_url: session.qr_url(),
                    user: session.user().cloned(),
                    ticket,
                });
            }
            RemoteAuthSessionAction::Cancelled {
                new_fingerprint,
                new_qr_url,
                ..
            } => {
                let _ = ws.close(REMOTE_AUTH_WS_NORMAL_CLOSE_CODE).await;
                return Ok(RemoteAuthHandshakeOutcome::Cancelled {
                    new_fingerprint,
                    new_qr_url,
                });
            }
        }
    }

    Err(RemoteAuthError::ConnectionClosed)
}

// ---------------------------------------------------------------------------
// REST-транспорт и обработчики `/users/@me/remote-auth*` (`4d2a64cb5df5fc4e_0 @275654`)
// ---------------------------------------------------------------------------

/// Результат успешного обмена `ticket` на расшифрованный токен (`POST /users/@me/remote-auth/login`).
///
/// Токен скрыт из `Debug` (`[REDACTED]`) для предотвращения утечки в логах.
#[derive(Clone, PartialEq, Eq)]
pub struct RemoteAuthLoginResult {
    pub fingerprint: String,
    decrypted_token: String,
}

impl RemoteAuthLoginResult {
    /// Возвращает расшифрованный токен авторизации.
    #[must_use]
    pub fn decrypted_token(&self) -> &str {
        &self.decrypted_token
    }
}

impl fmt::Debug for RemoteAuthLoginResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RemoteAuthLoginResult")
            .field("fingerprint", &self.fingerprint)
            .field("decrypted_token", &"[REDACTED]")
            .finish()
    }
}

/// Ответ `POST /users/@me/remote-auth` при сканировании QR-кода авторизованным клиентом.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteAuthHandshakeToken {
    pub handshake_token: String,
    #[serde(default)]
    pub user: Option<Value>,
}

/// Абстракция HTTP-транспорта для REST-маршрутов `/users/@me/remote-auth*`.
pub trait RemoteAuthRestTransport: Send + Sync {
    fn post<'a>(
        &'a self,
        target: &'a str,
        body: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, RemoteAuthError>> + Send + 'a>>;
}

/// Штатный REST-клиент Remote Auth поверх `discord_core::rest::client::ApiClient`.
pub struct DiscordRemoteAuthRest {
    client: ApiClient,
}

impl fmt::Debug for DiscordRemoteAuthRest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DiscordRemoteAuthRest")
            .field("token", &"[REDACTED]")
            .finish()
    }
}

impl DiscordRemoteAuthRest {
    /// Создаёт авторизованный REST-клиент для маршрутов `/users/@me/remote-auth*`.
    pub fn new(token: &str) -> Result<Self, RemoteAuthError> {
        let trimmed = token.trim();
        if trimmed.is_empty() {
            return Err(RemoteAuthError::MissingToken);
        }
        let identity_ctx = IdentityContext::detect();
        let auth = ApiAuth::from_token(
            trimmed,
            &identity_ctx.super_properties,
            &identity_ctx.locale,
            &identity_ctx.timezone,
        )
        .map_err(|_| RemoteAuthError::MissingToken)?;
        let client = ApiClient::new(ApiConfig {
            base: API_V9_BASE.to_string(),
            auth,
            http: HttpClientConfig::default(),
            retry: ApiRetryPolicy {
                max_attempts: 1,
                ..ApiRetryPolicy::default()
            },
        })
        .map_err(|_| RemoteAuthError::Network)?;
        Ok(Self { client })
    }
}

impl RemoteAuthRestTransport for DiscordRemoteAuthRest {
    fn post<'a>(
        &'a self,
        target: &'a str,
        body: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, RemoteAuthError>> + Send + 'a>> {
        Box::pin(async move {
            self.client
                .post(target, body)
                .await
                .map_err(|error| RemoteAuthError::from(RestFailure::from_api_error(&error)))
        })
    }
}

#[derive(Deserialize)]
struct DiscordRateLimitEnvelope {
    #[serde(default)]
    retry_after: Option<f64>,
}

fn extract_retry_after_ms(response: &HttpResponse) -> u64 {
    if let Ok(envelope) = serde_json::from_slice::<DiscordRateLimitEnvelope>(&response.body) {
        if let Some(secs) = envelope.retry_after.filter(|v| v.is_finite() && *v >= 0.0) {
            return (secs * 1000.0).round() as u64;
        }
    }
    for (name, value) in &response.headers {
        if name.eq_ignore_ascii_case("retry-after") {
            if let Ok(secs) = value.trim().parse::<f64>() {
                if secs.is_finite() && secs >= 0.0 {
                    return (secs * 1000.0).round() as u64;
                }
            }
        }
    }
    0
}

fn check_rest_response(resp: &HttpResponse) -> Result<(), RemoteAuthError> {
    if (200..300).contains(&resp.status) {
        return Ok(());
    }
    if resp.status == 429 {
        return Err(RemoteAuthError::RateLimited {
            retry_after_ms: extract_retry_after_ms(resp),
        });
    }
    Err(RestFailure::from_status(resp.status)
        .map(RemoteAuthError::from)
        .unwrap_or(RemoteAuthError::Network))
}

/// Обменивает `ticket` из `pending_login` на `encrypted_token` (`POST /users/@me/remote-auth/login`,
/// `QR_CHUNK` 52423 `@7296`), расшифровывает токен приватным ключом RSA-OAEP-2048 и переводит
/// сессию в `RemoteAuthStep::Finish`.
pub async fn exchange_remote_auth_ticket<T: RemoteAuthRestTransport>(
    transport: &T,
    session: &mut RemoteAuthSession,
    ticket: &str,
) -> Result<RemoteAuthLoginResult, RemoteAuthError> {
    let trimmed_ticket = ticket.trim();
    if trimmed_ticket.is_empty() {
        return Err(RemoteAuthError::InvalidArgument {
            field: "ticket",
            reason: "пустой ticket".to_string(),
        });
    }

    let body = serde_json::json!({ "ticket": trimmed_ticket }).to_string();
    let resp = transport
        .post(REMOTE_AUTH_LOGIN_PATH, body.as_bytes())
        .await?;
    check_rest_response(&resp)?;

    let json: Value =
        serde_json::from_slice(&resp.body).map_err(|_| RemoteAuthError::InvalidFrame {
            reason: "некорректный JSON ответа /users/@me/remote-auth/login".to_string(),
        })?;
    let encrypted_token = json
        .get("encrypted_token")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| RemoteAuthError::InvalidFrame {
            reason: "отсутствует непустое поле encrypted_token".to_string(),
        })?;

    let decrypted_token = session
        .keypair()
        .decrypt_encoded_ciphertext(encrypted_token)?;
    if decrypted_token.trim().is_empty() {
        return Err(RemoteAuthError::Crypto {
            reason: "расшифрованный токен пуст".to_string(),
        });
    }
    session.step = RemoteAuthStep::Finish;

    Ok(RemoteAuthLoginResult {
        fingerprint: session.fingerprint(),
        decrypted_token,
    })
}

/// Инициализирует подтверждение входа по отсканированному `fingerprint` (`POST /users/@me/remote-auth`).
pub async fn remote_auth_initialize<T: RemoteAuthRestTransport>(
    transport: &T,
    fingerprint: &str,
) -> Result<RemoteAuthHandshakeToken, RemoteAuthError> {
    let _ = format_remote_auth_qr_url(fingerprint)?;
    let body = serde_json::json!({ "fingerprint": fingerprint.trim() }).to_string();
    let resp = transport
        .post(REMOTE_AUTH_INITIALIZE_PATH, body.as_bytes())
        .await?;
    check_rest_response(&resp)?;

    let parsed: RemoteAuthHandshakeToken =
        serde_json::from_slice(&resp.body).map_err(|_| RemoteAuthError::InvalidFrame {
            reason: "некорректный JSON ответа /users/@me/remote-auth".to_string(),
        })?;
    if parsed.handshake_token.trim().is_empty() {
        return Err(RemoteAuthError::InvalidFrame {
            reason: "пустой handshake_token в ответе /users/@me/remote-auth".to_string(),
        });
    }
    Ok(parsed)
}

/// Завершает подтверждение входа по QR (`POST /users/@me/remote-auth/finish`).
pub async fn remote_auth_finish<T: RemoteAuthRestTransport>(
    transport: &T,
    handshake_token: &str,
    temporary_token: bool,
) -> Result<(), RemoteAuthError> {
    let trimmed = handshake_token.trim();
    if trimmed.is_empty() {
        return Err(RemoteAuthError::InvalidArgument {
            field: "handshake_token",
            reason: "пустой handshake_token".to_string(),
        });
    }
    let body = serde_json::json!({
        "handshake_token": trimmed,
        "temporary_token": temporary_token,
    })
    .to_string();
    let resp = transport
        .post(REMOTE_AUTH_FINISH_PATH, body.as_bytes())
        .await?;
    check_rest_response(&resp)
}

/// Отменяет подтверждение входа по QR (`POST /users/@me/remote-auth/cancel`).
pub async fn remote_auth_cancel<T: RemoteAuthRestTransport>(
    transport: &T,
    handshake_token: &str,
) -> Result<(), RemoteAuthError> {
    let trimmed = handshake_token.trim();
    if trimmed.is_empty() {
        return Err(RemoteAuthError::InvalidArgument {
            field: "handshake_token",
            reason: "пустой handshake_token".to_string(),
        });
    }
    let body = serde_json::json!({
        "handshake_token": trimmed,
    })
    .to_string();
    let resp = transport
        .post(REMOTE_AUTH_CANCEL_PATH, body.as_bytes())
        .await?;
    check_rest_response(&resp)
}

// ---------------------------------------------------------------------------
// Глобальный менеджер активной сессии для Tauri IPC-команд (`src/main.rs`)
// ---------------------------------------------------------------------------

static ACTIVE_REMOTE_AUTH_SESSION: LazyLock<Mutex<Option<RemoteAuthSession>>> =
    LazyLock::new(|| Mutex::new(None));

/// Создаёт и активирует новую сессию Remote Auth v2 для IPC-команды `remote_auth_start_session`.
pub fn ipc_start_session(
    endpoint: Option<&str>,
) -> Result<RemoteAuthSessionSnapshot, RemoteAuthError> {
    let mut session = RemoteAuthSession::new(endpoint)?;
    let _ = session.on_open();
    let snapshot = session.snapshot();
    if let Ok(mut guard) = ACTIVE_REMOTE_AUTH_SESSION.lock() {
        *guard = Some(session);
    }
    Ok(snapshot)
}

/// Передаёт входящий JSON-кадр WebSocket в активную сессию Remote Auth v2 (`remote_auth_process_frame`).
pub fn ipc_process_frame(frame_json: &str) -> Result<RemoteAuthFrameOutcome, RemoteAuthError> {
    let mut guard = ACTIVE_REMOTE_AUTH_SESSION
        .lock()
        .map_err(|_| RemoteAuthError::Storage)?;
    let session = guard
        .as_mut()
        .ok_or_else(|| RemoteAuthError::InvalidState {
            reason: "активная сессия Remote Auth не запущена".to_string(),
        })?;
    let action = session.handle_server_frame(frame_json)?;
    let outbound_frame_json = match &action {
        RemoteAuthSessionAction::Hello {
            outbound: Some(frame),
            ..
        }
        | RemoteAuthSessionAction::SendNonceProof { frame, .. }
        | RemoteAuthSessionAction::Cancelled {
            init_frame: frame, ..
        } => Some(frame.to_json_string()),
        _ => None,
    };
    Ok(RemoteAuthFrameOutcome {
        snapshot: session.snapshot(),
        action: Some(action),
        outbound_frame_json,
    })
}

/// Выполняет тик heartbeat активной сессии Remote Auth v2 (`remote_auth_heartbeat_tick`).
pub fn ipc_heartbeat_tick() -> Result<RemoteAuthFrameOutcome, RemoteAuthError> {
    let mut guard = ACTIVE_REMOTE_AUTH_SESSION
        .lock()
        .map_err(|_| RemoteAuthError::Storage)?;
    let session = guard
        .as_mut()
        .ok_or_else(|| RemoteAuthError::InvalidState {
            reason: "активная сессия Remote Auth не запущена".to_string(),
        })?;
    let frame = session.on_heartbeat_tick()?;
    Ok(RemoteAuthFrameOutcome {
        snapshot: session.snapshot(),
        action: None,
        outbound_frame_json: Some(frame.to_json_string()),
    })
}

/// Сбрасывает активную сессию Remote Auth v2 с ротацией пары ключей RSA-2048 (`remote_auth_cancel_session`).
pub fn ipc_cancel_session() -> Result<RemoteAuthSessionSnapshot, RemoteAuthError> {
    let mut guard = ACTIVE_REMOTE_AUTH_SESSION
        .lock()
        .map_err(|_| RemoteAuthError::Storage)?;
    if let Some(session) = guard.as_mut() {
        let _ = session.reset_with_new_keypair()?;
        Ok(session.snapshot())
    } else {
        let mut session = RemoteAuthSession::new(None)?;
        let _ = session.on_open();
        let snap = session.snapshot();
        *guard = Some(session);
        Ok(snap)
    }
}
