//! RW-40 / task-229 (`BE-P1-22`): BFF-контракт управления ролями и правами гильдии
//! (`POST /guilds/{guild_id}/roles`, `PATCH/DELETE /guilds/{guild_id}/roles/{role_id}`,
//! `PUT/DELETE /guilds/{guild_id}/members/{user_id}/roles/{role_id}`) и расчёт эффективных прав.
//!
//! Источники контракта:
//! - `/home/sanya/re-deep/rest-static.json:886-896` (`Endpoints.GUILD_ROLES: "/guilds/{arg}/roles"`);
//! - `/home/sanya/re-deep/rest-static.json:936-948` (`Endpoints.GUILD_ROLE: "/guilds/{arg}/roles/{arg}"`);
//! - Discord API v9: бит `ADMINISTRATOR` (`1 << 3`), цвет роли RGB (`0..=0x00FF_FFFF`),
//!   длина имени роли (`1..=100` символов Unicode).

use serde::{Deserialize, Serialize};

/// Максимальная длина названия роли в символах Unicode (`1..=100`).
pub const MAX_ROLE_NAME_CHARS: usize = 100;

/// Максимальное допустимое значение RGB-цвета роли (`0x00FF_FFFF`, 24-битный RGB без альфа-канала).
pub const MAX_RGB_COLOR: u32 = 0x00FF_FFFF;

/// Битовая маска права `ADMINISTRATOR` (`1 << 3 = 8`).
pub const PERM_ADMINISTRATOR: u64 = 1 << 3;

/// Запрос на создание или обновление роли гильдии (`POST /guilds/{guild_id}/roles` / `PATCH /guilds/{guild_id}/roles/{role_id}`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpsertRoleRequest {
    /// Snowflake ID гильдии (`1..=u64::MAX`, без ведущих нулей).
    pub guild_id: String,
    /// Название роли (`1..=MAX_ROLE_NAME_CHARS` символов Unicode после `trim()`).
    pub name: String,
    /// 24-битный RGB-цвет роли (`0..=MAX_RGB_COLOR`).
    pub color: u32,
    /// Отображать ли участников с этой ролью отдельно в списке (`hoist`).
    pub hoist: bool,
    /// Разрешено ли упоминать роль (`@role`).
    pub mentionable: bool,
    /// Десятичная строка битовой маски прав `u64` (`"0"`..=`u64::MAX`, без ведущих нулей).
    pub permissions: String,
}

/// Проверяет, что строка является каноническим положительным `u64` Snowflake ID
/// (`1..=u64::MAX`, только ASCII-цифры, без ведущих нулей).
fn validate_snowflake(field: &str, value: &str) -> Result<(), String> {
    if value.is_empty() || value.starts_with('0') || !value.bytes().all(|b| b.is_ascii_digit()) {
        return Err(format!("некорректный Snowflake ID в поле {field}"));
    }
    match value.parse::<u64>() {
        Ok(n) if n > 0 => Ok(()),
        _ => Err(format!("некорректный Snowflake ID в поле {field}")),
    }
}

/// Проверяет, что строка является каноническим десятичным представлением `u64`
/// (`0..=u64::MAX`, только ASCII-цифры, без ведущих нулей кроме `"0"`).
fn validate_u64_decimal(field: &str, value: &str) -> Result<u64, String> {
    if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
        return Err(format!("некорректное u64 значение в поле {field}"));
    }
    if value.len() > 1 && value.starts_with('0') {
        return Err(format!(
            "ведущие нули не допускаются в десятичном u64 поле {field}"
        ));
    }
    value
        .parse::<u64>()
        .map_err(|_| format!("значение поля {field} выходит за пределы u64"))
}

/// Валидирует запрос создания/обновления роли [`UpsertRoleRequest`]:
/// - проверяет `guild_id` как канонический положительный `u64` Snowflake (`1..=u64::MAX`, без ведущих нулей);
/// - проверяет, что `req.name.trim()` содержит от `1` до [`MAX_ROLE_NAME_CHARS`] символов Unicode;
/// - проверяет, что `req.color <= MAX_RGB_COLOR`;
/// - проверяет, что `req.permissions` парсится как каноническая десятичная строка `u64`
///   (`0..=u64::MAX`, без ведущих нулей, кроме `"0"`).
pub fn validate_upsert_role_request(req: &UpsertRoleRequest) -> Result<(), String> {
    validate_snowflake("guild_id", &req.guild_id)?;

    let trimmed_len = req.name.trim().chars().count();
    if trimmed_len == 0 {
        return Err("название роли не может быть пустым".to_string());
    }
    if trimmed_len > MAX_ROLE_NAME_CHARS {
        return Err(format!(
            "длина названия роли ({trimmed_len}) превышает максимум {MAX_ROLE_NAME_CHARS} символов"
        ));
    }

    if req.color > MAX_RGB_COLOR {
        return Err(format!(
            "цвет роли 0x{:08X} превышает максимально допустимый RGB 0x{MAX_RGB_COLOR:06X}",
            req.color
        ));
    }

    validate_u64_decimal("permissions", &req.permissions)?;
    Ok(())
}

/// Вычисляет эффективную битовую маску прав участника гильдии на уровне сервера:
/// - если `is_owner == true`, немедленно возвращает `u64::MAX`;
/// - побитово объединяет (`|`) базовые права `@everyone` (`everyone_perms`) и права всех ролей участника (`role_perms`);
/// - если в итоговой маске установлен бит [`PERM_ADMINISTRATOR`] (`1 << 3`), возвращает `u64::MAX`, иначе — накопленную маску.
#[must_use]
pub fn compute_effective_permissions(
    everyone_perms: u64,
    role_perms: &[u64],
    is_owner: bool,
) -> u64 {
    if is_owner {
        return u64::MAX;
    }
    let acc = role_perms
        .iter()
        .copied()
        .fold(everyone_perms, |acc, perm| acc | perm);
    if (acc & PERM_ADMINISTRATOR) != 0 {
        u64::MAX
    } else {
        acc
    }
}

/// Валидирует `guild_id`, `user_id` и `role_id` как канонические положительные `u64` Snowflake ID
/// и возвращает маршрут назначения/снятия роли участника:
/// `"/guilds/{guild_id}/members/{user_id}/roles/{role_id}"`.
pub fn build_member_role_route(
    guild_id: &str,
    user_id: &str,
    role_id: &str,
) -> Result<String, String> {
    validate_snowflake("guild_id", guild_id)?;
    validate_snowflake("user_id", user_id)?;
    validate_snowflake("role_id", role_id)?;
    Ok(format!(
        "/guilds/{guild_id}/members/{user_id}/roles/{role_id}"
    ))
}
