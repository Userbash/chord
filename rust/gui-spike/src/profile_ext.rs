//! RW-40 / task-221 (BE-P1-19): контракт профиля пользователя, bio, pronouns,
//! mutuals и заметок (`GET /users/{id}/profile`, `PATCH /users/@me/profile`,
//! `PUT /users/@me/notes/{target_user_id}`).
//!
//! Оракул: `/home/sanya/re-deep/rest-static.json` (`Endpoints.USER_PROFILE`, `Endpoints.NOTE`).

use serde::{Deserialize, Serialize};

/// Максимальная длина описания профиля (`bio`) в символах Unicode.
pub const MAX_BIO_CHARS: usize = 190;

/// Максимальная длина местоимений (`pronouns`) в символах Unicode.
pub const MAX_PRONOUNS_CHARS: usize = 40;

/// Максимальная длина заметки о пользователе (`note`) в символах Unicode.
pub const MAX_NOTE_CHARS: usize = 256;

/// Максимальное 24-битное RGB-значение цвета акцента (`0xFF_FFFF`).
const MAX_ACCENT_COLOR: u32 = 0xFF_FFFF;

/// Параметры запроса маршрута профиля пользователя (`GET /users/{user_id}/profile`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserProfileQuery {
    /// Снежинка целевого пользователя (`1..=u64::MAX`, без ведущих нулей).
    pub user_id: String,
    /// Опциональная снежинка гильдии (`1..=u64::MAX`, без ведущих нулей).
    pub guild_id: Option<String>,
    /// Запрашивать ли общие серверы (`with_mutual_guilds`).
    pub with_mutual_guilds: bool,
    /// Запрашивать ли общих друзей (`with_mutual_friends`).
    pub with_mutual_friends: bool,
}

/// Патч обновления собственного профиля (`PATCH /users/@me/profile`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateProfilePatch {
    /// Описание профиля (`<= 190` символов Unicode).
    pub bio: Option<String>,
    /// Местоимения (`<= 40` символов Unicode).
    pub pronouns: Option<String>,
    /// 24-битный RGB-цвет акцента (`0..=0xFF_FFFF`).
    pub accent_color: Option<u32>,
}

/// Проверяет, что строка является каноническим положительным `u64` Snowflake ID
/// (`1..=u64::MAX`, только ASCII-цифры, без ведущих нулей).
fn validate_canonical_snowflake(field: &str, value: &str) -> Result<(), String> {
    if value.is_empty() || value.starts_with('0') || !value.bytes().all(|b| b.is_ascii_digit()) {
        return Err(format!("некорректный Snowflake ID в поле {field}"));
    }
    match value.parse::<u64>() {
        Ok(n) if n > 0 => Ok(()),
        _ => Err(format!("некорректный Snowflake ID в поле {field}")),
    }
}

/// Валидирует [`UserProfileQuery`] и строит маршрут
/// `/users/{user_id}/profile?with_mutual_guilds={true|false}&with_mutual_friends={true|false}[&guild_id={gid}]`.
pub fn build_user_profile_route(q: &UserProfileQuery) -> Result<String, String> {
    validate_canonical_snowflake("user_id", &q.user_id)?;

    let mut query = format!(
        "with_mutual_guilds={}&with_mutual_friends={}",
        q.with_mutual_guilds, q.with_mutual_friends
    );

    if let Some(gid) = q.guild_id.as_deref() {
        validate_canonical_snowflake("guild_id", gid)?;
        query.push_str(&format!("&guild_id={gid}"));
    }

    Ok(format!("/users/{}/profile?{}", q.user_id, query))
}

/// Валидирует [`UpdateProfilePatch`] по лимитам Discord:
/// - `bio`: не более [`MAX_BIO_CHARS`] (`190`) символов Unicode;
/// - `pronouns`: не более [`MAX_PRONOUNS_CHARS`] (`40`) символов Unicode;
/// - `accent_color`: не более `0xFF_FFFF` (24-битный RGB).
pub fn validate_profile_patch(patch: &UpdateProfilePatch) -> Result<(), String> {
    if let Some(bio) = patch.bio.as_deref() {
        let chars = bio.chars().count();
        if chars > MAX_BIO_CHARS {
            return Err(format!(
                "bio превышает лимит {MAX_BIO_CHARS} символов (получено {chars})"
            ));
        }
    }

    if let Some(pronouns) = patch.pronouns.as_deref() {
        let chars = pronouns.chars().count();
        if chars > MAX_PRONOUNS_CHARS {
            return Err(format!(
                "pronouns превышает лимит {MAX_PRONOUNS_CHARS} символов (получено {chars})"
            ));
        }
    }

    if let Some(color) = patch.accent_color {
        if color > MAX_ACCENT_COLOR {
            return Err(format!(
                "accent_color 0x{color:X} превышает 24-битный диапазон 0x{MAX_ACCENT_COLOR:X}"
            ));
        }
    }

    Ok(())
}

/// Валидирует `target_user_id` (канонический Snowflake) и длину заметки `note`
/// (`<= 256` символов Unicode), возвращая маршрут `PUT /users/@me/notes/{target_user_id}`.
pub fn validate_user_note(target_user_id: &str, note: &str) -> Result<String, String> {
    validate_canonical_snowflake("target_user_id", target_user_id)?;

    let chars = note.chars().count();
    if chars > MAX_NOTE_CHARS {
        return Err(format!(
            "note превышает лимит {MAX_NOTE_CHARS} символов (получено {chars})"
        ));
    }

    Ok(format!("/users/@me/notes/{target_user_id}"))
}
