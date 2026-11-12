//! P2-08: protobuf wire-декодер настроек (RW-21).
//!
//! Контракт заморожен `waves/RW-21/DESIGN-P2-08.md` §2–§3. Модуль не знает про Discord,
//! состояние и сеть: только bytes → структурные поля. Группы (wire 3/4) не поддержаны —
//! в захватах их нет; неизвестные поля сохраняются как [`ProtoField`], а не отбрасываются.

use std::fmt;

use crate::rest::base64::Base64Error;

/// Верхний предел входного protobuf-сообщения (1 МиБ). Реальные настройки — 3–13 КБ.
pub const MAX_PROTO_BYTES: usize = 1_048_576;

/// Верхний предел числа полей в одном сообщении.
pub const MAX_FIELDS: usize = 262_144;

/// Максимальный номер protobuf-поля (`2^29 - 1`).
const MAX_FIELD_NUMBER: u64 = 0x1FFF_FFFF;

/// Ошибка разбора base64/protobuf.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingsProtoError {
    /// Ошибка base64-обёртки.
    Base64(Base64Error),
    /// Входное сообщение больше [`MAX_PROTO_BYTES`].
    TooLarge { size: usize, limit: usize },
    /// В сообщении больше [`MAX_FIELDS`] полей.
    TooManyFields { limit: usize },
    /// Varint/fixed/length обрывается до конца буфера.
    UnexpectedEnd { offset: usize },
    /// Varint длиннее 10 байт или не помещается в `u64`.
    VarintOverflow { offset: usize },
    /// Номер поля 0 или больше `2^29 - 1`.
    InvalidFieldNumber { number: u64, offset: usize },
    /// Wire type 3/4/6/7 не поддержан.
    UnsupportedWireType {
        field: u32,
        wire_type: u8,
        offset: usize,
    },
    /// Длина length-delimited поля больше остатка буфера.
    LengthOutOfBounds {
        field: u32,
        length: u64,
        remaining: usize,
    },
    /// Известное поле пришло с другим wire type.
    WrongWireType {
        field: u32,
        expected: u8,
        actual: u8,
    },
    /// StringValue содержит невалидный UTF-8.
    InvalidUtf8 { field: u32 },
}

impl fmt::Display for SettingsProtoError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Base64(error) => write!(formatter, "base64: {error:?}"),
            Self::TooLarge { size, limit } => {
                write!(formatter, "protobuf {size} bytes > limit {limit}")
            }
            Self::TooManyFields { limit } => write!(formatter, "protobuf fields > limit {limit}"),
            Self::UnexpectedEnd { offset } => write!(formatter, "unexpected end at {offset}"),
            Self::VarintOverflow { offset } => write!(formatter, "varint overflow at {offset}"),
            Self::InvalidFieldNumber { number, offset } => {
                write!(formatter, "invalid field {number} at {offset}")
            }
            Self::UnsupportedWireType {
                field,
                wire_type,
                offset,
            } => write!(
                formatter,
                "unsupported wire type {wire_type} for field {field} at {offset}"
            ),
            Self::LengthOutOfBounds {
                field,
                length,
                remaining,
            } => write!(
                formatter,
                "length {length} of field {field} > remaining {remaining}"
            ),
            Self::WrongWireType {
                field,
                expected,
                actual,
            } => write!(
                formatter,
                "field {field}: expected wire {expected}, got {actual}"
            ),
            Self::InvalidUtf8 { field } => write!(formatter, "field {field}: invalid utf-8"),
        }
    }
}

impl std::error::Error for SettingsProtoError {}

impl From<Base64Error> for SettingsProtoError {
    fn from(error: Base64Error) -> Self {
        Self::Base64(error)
    }
}

/// Значение protobuf-поля без интерпретации схемы.
#[derive(Clone, PartialEq, Eq)]
pub enum ProtoValue {
    /// Varint (включая bool/int32/int64/enum).
    Varint(u64),
    /// 64-битное fixed little-endian.
    Fixed64(u64),
    /// Length-delimited bytes (вложенное сообщение/строка/map).
    LengthDelimited(Vec<u8>),
    /// 32-битное fixed little-endian.
    Fixed32(u32),
}

impl ProtoValue {
    /// Wire type значения.
    #[must_use]
    pub fn wire_type(&self) -> u8 {
        match self {
            Self::Varint(_) => 0,
            Self::Fixed64(_) => 1,
            Self::LengthDelimited(_) => 2,
            Self::Fixed32(_) => 5,
        }
    }

    /// Varint, если значение этого типа.
    #[must_use]
    pub fn as_varint(&self) -> Option<u64> {
        match self {
            Self::Varint(value) => Some(*value),
            _ => None,
        }
    }

    /// Length-delimited bytes, если значение этого типа.
    #[must_use]
    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Self::LengthDelimited(bytes) => Some(bytes),
            _ => None,
        }
    }
}

impl fmt::Debug for ProtoValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Varint(value) => write!(formatter, "Varint({value})"),
            Self::Fixed64(value) => write!(formatter, "Fixed64({value})"),
            Self::LengthDelimited(bytes) => {
                write!(formatter, "LengthDelimited(<{} bytes>)", bytes.len())
            }
            Self::Fixed32(value) => write!(formatter, "Fixed32({value})"),
        }
    }
}

/// Одно поле protobuf.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtoField {
    /// Номер поля.
    pub number: u32,
    /// Значение.
    pub value: ProtoValue,
}

impl ProtoField {
    /// Закодировать поле в protobuf wire-буфер.
    pub fn encode_into(&self, out: &mut Vec<u8>) {
        let tag = (u64::from(self.number) << 3) | u64::from(self.value.wire_type());
        write_varint(tag, out);
        match &self.value {
            ProtoValue::Varint(val) => {
                write_varint(*val, out);
            }
            ProtoValue::Fixed64(val) => {
                out.extend_from_slice(&val.to_le_bytes());
            }
            ProtoValue::LengthDelimited(bytes) => {
                write_varint(bytes.len() as u64, out);
                out.extend_from_slice(bytes);
            }
            ProtoValue::Fixed32(val) => {
                out.extend_from_slice(&val.to_le_bytes());
            }
        }
    }
}

/// Разобранное protobuf-сообщение (один уровень).
#[derive(Clone, PartialEq, Eq)]
pub struct ProtoMessage {
    /// Поля в порядке провода; повторения сохранены.
    pub fields: Vec<ProtoField>,
}

impl fmt::Debug for ProtoMessage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProtoMessage")
            .field("fields", &self.fields.len())
            .finish()
    }
}

impl ProtoMessage {
    /// Закодировать все поля сообщения в protobuf wire-байты.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        for field in &self.fields {
            field.encode_into(&mut out);
        }
        out
    }

    /// Установить значение поля с номером `number`.
    ///
    /// Если поле уже присутствует, заменяет его значение (и удаляет любые последующие дубликаты);
    /// иначе добавляет поле в конец списка полей.
    pub fn set_field(&mut self, number: u32, value: ProtoValue) {
        if let Some(pos) = self.fields.iter().position(|field| field.number == number) {
            self.fields[pos].value = value;
            let mut i = pos + 1;
            while i < self.fields.len() {
                if self.fields[i].number == number {
                    self.fields.remove(i);
                } else {
                    i += 1;
                }
            }
        } else {
            self.fields.push(ProtoField { number, value });
        }
    }

    /// Разобрать одно protobuf-сообщение.
    pub fn decode(bytes: &[u8]) -> Result<Self, SettingsProtoError> {
        if bytes.len() > MAX_PROTO_BYTES {
            return Err(SettingsProtoError::TooLarge {
                size: bytes.len(),
                limit: MAX_PROTO_BYTES,
            });
        }

        let mut fields = Vec::new();
        let mut offset = 0;
        while offset < bytes.len() {
            if fields.len() >= MAX_FIELDS {
                return Err(SettingsProtoError::TooManyFields { limit: MAX_FIELDS });
            }
            let tag_offset = offset;
            let tag = read_varint(bytes, &mut offset)?;
            let number = tag >> 3;
            if number == 0 || number > MAX_FIELD_NUMBER {
                return Err(SettingsProtoError::InvalidFieldNumber {
                    number,
                    offset: tag_offset,
                });
            }
            let number =
                u32::try_from(number).map_err(|_| SettingsProtoError::InvalidFieldNumber {
                    number,
                    offset: tag_offset,
                })?;
            let wire_type = (tag & 0x07) as u8;
            let value = match wire_type {
                0 => ProtoValue::Varint(read_varint(bytes, &mut offset)?),
                1 => {
                    let remaining = bytes.len().saturating_sub(offset);
                    if remaining < 8 {
                        return Err(SettingsProtoError::UnexpectedEnd { offset });
                    }
                    let mut raw = [0_u8; 8];
                    raw.copy_from_slice(&bytes[offset..offset + 8]);
                    offset += 8;
                    ProtoValue::Fixed64(u64::from_le_bytes(raw))
                }
                2 => {
                    let length = read_varint(bytes, &mut offset)?;
                    let remaining = bytes.len().saturating_sub(offset);
                    if length > remaining as u64 {
                        return Err(SettingsProtoError::LengthOutOfBounds {
                            field: number,
                            length,
                            remaining,
                        });
                    }
                    let length = usize::try_from(length).map_err(|_| {
                        SettingsProtoError::LengthOutOfBounds {
                            field: number,
                            length,
                            remaining,
                        }
                    })?;
                    let value = bytes[offset..offset + length].to_vec();
                    offset += length;
                    ProtoValue::LengthDelimited(value)
                }
                5 => {
                    let remaining = bytes.len().saturating_sub(offset);
                    if remaining < 4 {
                        return Err(SettingsProtoError::UnexpectedEnd { offset });
                    }
                    let mut raw = [0_u8; 4];
                    raw.copy_from_slice(&bytes[offset..offset + 4]);
                    offset += 4;
                    ProtoValue::Fixed32(u32::from_le_bytes(raw))
                }
                other => {
                    return Err(SettingsProtoError::UnsupportedWireType {
                        field: number,
                        wire_type: other,
                        offset: tag_offset,
                    });
                }
            };
            fields.push(ProtoField { number, value });
        }
        Ok(Self { fields })
    }

    /// Последнее поле с номером (protobuf singular last-wins).
    #[must_use]
    pub fn field(&self, number: u32) -> Option<&ProtoField> {
        self.fields
            .iter()
            .rev()
            .find(|field| field.number == number)
    }

    /// Все поля с номером в порядке провода.
    pub fn fields_of(&self, number: u32) -> impl Iterator<Item = &ProtoField> {
        self.fields
            .iter()
            .filter(move |field| field.number == number)
    }

    /// Вложенное сообщение поля; `None` — поле отсутствует.
    pub fn message(&self, number: u32) -> Result<Option<Self>, SettingsProtoError> {
        let Some(field) = self.field(number) else {
            return Ok(None);
        };
        let bytes = field
            .value
            .as_bytes()
            .ok_or(SettingsProtoError::WrongWireType {
                field: number,
                expected: 2,
                actual: field.value.wire_type(),
            })?;
        Self::decode(bytes).map(Some)
    }

    /// Length-delimited bytes поля; `None` — поле отсутствует.
    pub fn length_delimited(&self, number: u32) -> Result<Option<&[u8]>, SettingsProtoError> {
        let Some(field) = self.field(number) else {
            return Ok(None);
        };
        field
            .value
            .as_bytes()
            .map(Some)
            .ok_or(SettingsProtoError::WrongWireType {
                field: number,
                expected: 2,
                actual: field.value.wire_type(),
            })
    }

    /// StringValue (wrapper field 1); отсутствие поля → `None`, пустая строка валидна.
    pub fn string(&self, number: u32) -> Result<Option<String>, SettingsProtoError> {
        let Some(bytes) = self.length_delimited(number)? else {
            return Ok(None);
        };
        let wrapper = Self::decode(bytes)?;
        let Some(field) = wrapper.field(1) else {
            return Ok(Some(String::new()));
        };
        let text = field
            .value
            .as_bytes()
            .ok_or(SettingsProtoError::WrongWireType {
                field: 1,
                expected: 2,
                actual: field.value.wire_type(),
            })?;
        String::from_utf8(text.to_vec())
            .map(Some)
            .map_err(|_| SettingsProtoError::InvalidUtf8 { field: number })
    }

    /// BoolValue: отсутствие → `None`, пустой wrapper → `Some(false)`.
    pub fn bool_wrapper(&self, number: u32) -> Result<Option<bool>, SettingsProtoError> {
        let Some(wrapper) = self.message(number)? else {
            return Ok(None);
        };
        Ok(Some(wrapper.scalar_bool(1)?.unwrap_or(false)))
    }

    /// Int32Value: отсутствие → `None`, пустой wrapper → `Some(0)`.
    pub fn int32_wrapper(&self, number: u32) -> Result<Option<i32>, SettingsProtoError> {
        let Some(wrapper) = self.message(number)? else {
            return Ok(None);
        };
        Ok(Some(wrapper.scalar_i32(1)?.unwrap_or(0)))
    }

    /// Int64Value: отсутствие → `None`, пустой wrapper → `Some(0)`.
    pub fn int64_wrapper(&self, number: u32) -> Result<Option<i64>, SettingsProtoError> {
        let Some(wrapper) = self.message(number)? else {
            return Ok(None);
        };
        Ok(Some(wrapper.scalar_i64(1)?.unwrap_or(0)))
    }

    /// UInt64Value: отсутствие → `None`, пустой wrapper → `Some(0)`.
    pub fn uint64_wrapper(&self, number: u32) -> Result<Option<u64>, SettingsProtoError> {
        let Some(wrapper) = self.message(number)? else {
            return Ok(None);
        };
        Ok(Some(wrapper.scalar_u64(1)?.unwrap_or(0)))
    }

    /// Scalar varint как `u64`; отсутствие → `None`.
    pub fn scalar_u64(&self, number: u32) -> Result<Option<u64>, SettingsProtoError> {
        let Some(field) = self.field(number) else {
            return Ok(None);
        };
        field
            .value
            .as_varint()
            .map(Some)
            .ok_or(SettingsProtoError::WrongWireType {
                field: number,
                expected: 0,
                actual: field.value.wire_type(),
            })
    }

    /// Scalar varint как `i64` (two's complement); отсутствие → `None`.
    pub fn scalar_i64(&self, number: u32) -> Result<Option<i64>, SettingsProtoError> {
        Ok(self.scalar_u64(number)?.map(|value| value as i64))
    }

    /// Scalar varint как `i32` (two's complement); отсутствие → `None`.
    pub fn scalar_i32(&self, number: u32) -> Result<Option<i32>, SettingsProtoError> {
        Ok(self.scalar_u64(number)?.map(|value| value as i32))
    }

    /// Scalar varint как bool; отсутствие → `None`.
    pub fn scalar_bool(&self, number: u32) -> Result<Option<bool>, SettingsProtoError> {
        Ok(self.scalar_u64(number)?.map(|value| value != 0))
    }

    /// Scalar fixed64; отсутствие → `None`.
    pub fn scalar_fixed64(&self, number: u32) -> Result<Option<u64>, SettingsProtoError> {
        let Some(field) = self.field(number) else {
            return Ok(None);
        };
        match &field.value {
            ProtoValue::Fixed64(value) => Ok(Some(*value)),
            other => Err(SettingsProtoError::WrongWireType {
                field: number,
                expected: 1,
                actual: other.wire_type(),
            }),
        }
    }
}

fn read_varint(bytes: &[u8], offset: &mut usize) -> Result<u64, SettingsProtoError> {
    let start = *offset;
    let mut value = 0_u64;
    let mut shift = 0_u32;
    loop {
        let Some(&byte) = bytes.get(*offset) else {
            return Err(SettingsProtoError::UnexpectedEnd { offset: *offset });
        };
        *offset += 1;
        if shift == 63 && byte > 1 {
            return Err(SettingsProtoError::VarintOverflow { offset: start });
        }
        value |= u64::from(byte & 0x7F) << shift;
        if byte & 0x80 == 0 {
            return Ok(value);
        }
        shift += 7;
        if shift >= 64 {
            return Err(SettingsProtoError::VarintOverflow { offset: start });
        }
    }
}

fn write_varint(mut value: u64, out: &mut Vec<u8>) {
    while value >= 0x80 {
        out.push(((value & 0x7F) as u8) | 0x80);
        value >>= 7;
    }
    out.push((value & 0x7F) as u8);
}
