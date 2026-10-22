//! Манифест релиза и проверка подписи Ed25519.
//!
//! Формат подписанного блоба: `body || signature[64]`, где `signature` — подпись
//! Ed25519 над **исходными байтами** `body`. Канонизация JSON не нужна: подписывается
//! ровно то, что пришло.
//!
//! Проверка подписи использует `ring 0.17` (`ring::signature::ED25519`,
//! `ring::signature::UnparsedPublicKey`, `ring::signature::Ed25519KeyPair`).
//! Публичный ключ передаёт вызывающий код — в этой карточке сеть и установка
//! не выполняются.
//!
//! Контракт: `waves/RW-25/TZ-W26c.md` §3.

use ring::signature::{ED25519, Ed25519KeyPair, UnparsedPublicKey};
use serde_json::Value;
use sha2::{Digest, Sha256};

/// Длина подписи Ed25519 в байтах.
const SIGNATURE_LEN: usize = 64;

/// Длина hex-представления SHA-256.
const SHA256_HEX_LEN: usize = 64;

/// Версия релиза: `major.minor.patch` с необязательным pre-release суффиксом.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version {
    /// Старшая часть версии.
    pub major: u64,
    /// Средняя часть версии.
    pub minor: u64,
    /// Младшая часть версии.
    pub patch: u64,
    /// Pre-release суффикс (`"beta.1"`); `None` — стабильный релиз.
    pub pre: Option<String>,
}

impl Version {
    /// Разбор строки версии. Допускается ровно `major.minor.patch` и
    /// `major.minor.patch-<pre>`; мусор ⇒ ошибка.
    ///
    /// Отвергаются: пустая строка, `1.2`, `1.2.3.4`, `1.2.x`, `v1.2.3`,
    /// ведущие нули в компонентах (`01.2.3`) и пустой pre-release (`1.2.3-`).
    pub fn parse(text: &str) -> Result<Self, ManifestError> {
        let invalid = || ManifestError::InvalidVersion {
            value: text.to_string(),
        };

        if text.is_empty() {
            return Err(invalid());
        }

        // Pre-release отделяется первым дефисом; сам суффикс может содержать дефисы.
        let (core, pre) = match text.split_once('-') {
            Some((core, pre)) => (core, Some(pre)),
            None => (text, None),
        };

        if let Some(pre) = pre
            && pre.is_empty()
        {
            return Err(invalid());
        }

        let mut parts = core.split('.');
        let major = parse_component(parts.next(), &invalid)?;
        let minor = parse_component(parts.next(), &invalid)?;
        let patch = parse_component(parts.next(), &invalid)?;
        if parts.next().is_some() {
            // Лишние компоненты (`1.2.3.4`).
            return Err(invalid());
        }

        Ok(Self {
            major,
            minor,
            patch,
            pre: pre.map(str::to_string),
        })
    }

    /// `true`, если `self` строго новее `other`.
    ///
    /// Pre-release считается младше своей же версии без суффикса
    /// (`1.0.0-beta` < `1.0.0`); pre-суффиксы сравниваются лексикографически.
    pub fn is_newer_than(&self, other: &Self) -> bool {
        match (self.major, self.minor, self.patch).cmp(&(other.major, other.minor, other.patch)) {
            std::cmp::Ordering::Greater => true,
            std::cmp::Ordering::Less => false,
            std::cmp::Ordering::Equal => match (&self.pre, &other.pre) {
                // Стабильный релиз новее своего pre-release.
                (None, None) => false,
                (None, Some(_)) => true,
                (Some(_), None) => false,
                (Some(a), Some(b)) => a > b,
            },
        }
    }
}

/// Разбирает одну числовую компоненту версии: только цифры, без ведущих нулей.
fn parse_component(
    part: Option<&str>,
    invalid: &dyn Fn() -> ManifestError,
) -> Result<u64, ManifestError> {
    let part = part.ok_or_else(invalid)?;
    if part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
        return Err(invalid());
    }
    if part.len() > 1 && part.starts_with('0') {
        // Ведущий ноль (`01.2.3`).
        return Err(invalid());
    }
    part.parse::<u64>().map_err(|_| invalid())
}

/// Один артефакт релиза.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Artifact {
    /// Имя артефакта (имя файла/цели).
    pub name: String,
    /// URL источника (сеть вне карточки — только фиксируется в плане).
    pub url: String,
    /// SHA-256 в hex (нижний регистр, 64 символа).
    pub sha256: String,
    /// Ожидаемый размер в байтах.
    pub size: u64,
}

/// Манифест релиза.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    /// Версия релиза.
    pub version: Version,
    /// Дата выпуска (ISO-8601), хранится как строка.
    pub released_at: String,
    /// Список артефактов релиза.
    pub artifacts: Vec<Artifact>,
}

/// Манифест, прошедший проверку подписи: получить его можно только из `verify_manifest`.
///
/// Поля приватные: так тип нельзя сконструировать вручную — единственный путь получения
/// подписанного манифеста — успешная проверка подписи Ed25519. Именно этот тип принимает
/// [`crate::plan::plan_update`], поэтому неподписанный манифест нельзя подсунуть в план.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedManifest {
    /// Версия релиза.
    version: Version,
    /// Дата выпуска (ISO-8601).
    released_at: String,
    /// Артефакты релиза.
    artifacts: Vec<Artifact>,
}

impl VerifiedManifest {
    /// Версия релиза.
    pub fn version(&self) -> &Version {
        &self.version
    }

    /// Дата выпуска (ISO-8601).
    pub fn released_at(&self) -> &str {
        &self.released_at
    }

    /// Все артефакты релиза.
    pub fn artifacts(&self) -> &[Artifact] {
        &self.artifacts
    }

    /// Артефакт по имени; `None`, если такого артефакта нет.
    pub fn artifact(&self, name: &str) -> Option<&Artifact> {
        self.artifacts.iter().find(|artifact| artifact.name == name)
    }
}

/// Ошибка манифеста.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestError {
    /// JSON не разобран.
    InvalidJson {
        /// Текст ошибки разбора.
        message: String,
    },
    /// Обязательное поле отсутствует.
    MissingField {
        /// Имя отсутствующего поля.
        field: &'static str,
    },
    /// Версия не разобрана.
    InvalidVersion {
        /// Исходное значение версии.
        value: String,
    },
    /// Хэш артефакта невалиден.
    InvalidDigest {
        /// Имя артефакта.
        artifact: String,
        /// Исходное значение хэша.
        value: String,
    },
    /// Дублирующееся имя артефакта.
    DuplicateArtifact {
        /// Имя артефакта.
        name: String,
    },
    /// Подпись не совпала.
    InvalidSignature,
    /// В блобе нет 64 байт подписи.
    SignatureMissing,
    /// Артефакт с таким именем не найден.
    UnknownArtifact {
        /// Имя артефакта.
        name: String,
    },
    /// Текущая версия не старше кандидата.
    NotNewer {
        /// Текущая версия.
        current: String,
        /// Версия-кандидат.
        candidate: String,
    },
    /// Хэш фактических данных не совпал с ожидаемым.
    DigestMismatch {
        /// Имя артефакта.
        artifact: String,
    },
    /// Ошибка ввода-вывода (с путем и текстом).
    Io {
        /// Путь, на котором возникла ошибка.
        path: String,
        /// Текст ошибки.
        message: String,
    },
    /// Откат не удался.
    RollbackFailed {
        /// Путь, на котором возникла ошибка.
        path: String,
        /// Текст ошибки.
        message: String,
    },
}

/// Разбирает манифест из JSON-байтов **без** проверки подписи.
///
/// Проверяется: валидный JSON, наличие `version`/`artifacts`, корректность каждого
/// `sha256` (ровно 64 hex-символа), уникальность имён артефактов, `size >= 0`.
pub fn parse_manifest(bytes: &[u8]) -> Result<Manifest, ManifestError> {
    let value: Value = serde_json::from_slice(bytes).map_err(|e| ManifestError::InvalidJson {
        message: e.to_string(),
    })?;

    let root = value
        .as_object()
        .ok_or_else(|| ManifestError::InvalidJson {
            message: "ожидался JSON-объект в корне".to_string(),
        })?;

    let version_text = match root.get("version") {
        Some(Value::String(text)) => text.clone(),
        Some(other) => {
            return Err(ManifestError::InvalidVersion {
                value: other.to_string(),
            });
        }
        None => return Err(ManifestError::MissingField { field: "version" }),
    };
    let version = Version::parse(&version_text)?;

    let released_at = match root.get("released_at") {
        Some(Value::String(text)) => text.clone(),
        Some(other) => {
            return Err(ManifestError::InvalidJson {
                message: format!("поле released_at должно быть строкой, получено {other}"),
            });
        }
        None => String::new(),
    };

    let artifacts_value = root
        .get("artifacts")
        .ok_or(ManifestError::MissingField { field: "artifacts" })?;
    let artifacts_array = artifacts_value
        .as_array()
        .ok_or_else(|| ManifestError::InvalidJson {
            message: "поле artifacts должно быть массивом".to_string(),
        })?;

    let mut artifacts: Vec<Artifact> = Vec::with_capacity(artifacts_array.len());
    for item in artifacts_array {
        let item = item.as_object().ok_or_else(|| ManifestError::InvalidJson {
            message: "элемент artifacts должен быть объектом".to_string(),
        })?;

        let name = match item.get("name") {
            Some(Value::String(text)) => text.clone(),
            Some(other) => {
                return Err(ManifestError::InvalidJson {
                    message: format!("поле name должно быть строкой, получено {other}"),
                });
            }
            None => return Err(ManifestError::MissingField { field: "name" }),
        };

        let url = match item.get("url") {
            Some(Value::String(text)) => text.clone(),
            Some(other) => {
                return Err(ManifestError::InvalidJson {
                    message: format!("поле url должно быть строкой, получено {other}"),
                });
            }
            None => return Err(ManifestError::MissingField { field: "url" }),
        };

        let sha256 = match item.get("sha256") {
            Some(Value::String(text)) => text.clone(),
            Some(other) => {
                return Err(ManifestError::InvalidJson {
                    message: format!("поле sha256 должно быть строкой, получено {other}"),
                });
            }
            None => return Err(ManifestError::MissingField { field: "sha256" }),
        };
        if !is_sha256_hex(&sha256) {
            return Err(ManifestError::InvalidDigest {
                artifact: name,
                value: sha256,
            });
        }

        let size = match item.get("size") {
            Some(Value::Number(number)) => {
                // `size` — неотрицательное целое; отрицательное или дробное ⇒ ошибка.
                number.as_u64().ok_or_else(|| ManifestError::InvalidJson {
                    message: format!(
                        "поле size должно быть неотрицательным целым, получено {number}"
                    ),
                })?
            }
            Some(other) => {
                return Err(ManifestError::InvalidJson {
                    message: format!("поле size должно быть числом, получено {other}"),
                });
            }
            None => return Err(ManifestError::MissingField { field: "size" }),
        };

        if artifacts.iter().any(|a| a.name == name) {
            return Err(ManifestError::DuplicateArtifact { name });
        }

        artifacts.push(Artifact {
            name,
            url,
            sha256,
            size,
        });
    }

    Ok(Manifest {
        version,
        released_at,
        artifacts,
    })
}

/// `true`, если строка — ровно 64 hex-символа (регистр не важен).
fn is_sha256_hex(text: &str) -> bool {
    text.len() == SHA256_HEX_LEN && text.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Проверяет подпись и разбирает манифест. Ошибка подписи ⇒ `InvalidSignature`.
///
/// Блоб короче 64 байт ⇒ `SignatureMissing`. Подпись проверяется **до** разбора тела,
/// поэтому подменённое тело даёт `InvalidSignature`, а не ошибку разбора JSON.
///
/// Возвращает [`VerifiedManifest`] — подписанный манифест, который можно передать в
/// [`crate::plan::plan_update`].
pub fn verify_manifest(
    signed: &[u8],
    public_key: &[u8; 32],
) -> Result<VerifiedManifest, ManifestError> {
    if signed.len() < SIGNATURE_LEN {
        return Err(ManifestError::SignatureMissing);
    }
    let (body, signature) = signed.split_at(signed.len() - SIGNATURE_LEN);

    let key = UnparsedPublicKey::new(&ED25519, public_key);
    key.verify(body, signature)
        .map_err(|_| ManifestError::InvalidSignature)?;

    let manifest = parse_manifest(body)?;
    Ok(VerifiedManifest {
        version: manifest.version,
        released_at: manifest.released_at,
        artifacts: manifest.artifacts,
    })
}

/// Подписывает тело манифеста (для выпуска и для тестов).
///
/// `seed` — 32-байтовый seed Ed25519-ключа (`ring::signature::Ed25519KeyPair`).
/// Возвращает `body || signature[64]`.
pub fn sign_manifest(body: &[u8], seed: &[u8; 32]) -> Result<Vec<u8>, ManifestError> {
    let pair =
        Ed25519KeyPair::from_seed_unchecked(seed).map_err(|_| ManifestError::InvalidSignature)?;
    let signature = pair.sign(body);
    let mut signed = Vec::with_capacity(body.len() + SIGNATURE_LEN);
    signed.extend_from_slice(body);
    signed.extend_from_slice(signature.as_ref());
    Ok(signed)
}

/// SHA-256 артефакта в hex (нижний регистр, ровно 64 символа).
pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut hex = String::with_capacity(SHA256_HEX_LEN);
    for byte in digest {
        hex.push(char::from_digit(u32::from(byte >> 4), 16).unwrap_or('0'));
        hex.push(char::from_digit(u32::from(byte & 0x0f), 16).unwrap_or('0'));
    }
    hex
}
