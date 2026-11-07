//! Движок расчёта битовых масок прав доступа Discord API v9 (TASK-619).
//!
//! Оракулы:
//! - `/home/sanya/re-deep/decoded/4d2a64cb5df5fc4e_0` (`@9770000..9774000`, модуль `136722` `BigFlagUtils`
//!   и константы `Permissions` `eC = Object.freeze({...})`):
//!   - 41 битовый флаг `0..=40` (`CREATE_INSTANT_INVITE = 1 << 0` .. `MODERATE_MEMBERS = 1 << 40`);
//!   - базовые права: `everyone_role_perms | (OR всех ролей участника)`;
//!   - если `user_id == owner_id` или `(base_perms & ADMINISTRATOR) != 0` — возвращается [`ALL_PERMISSIONS`];
//!   - канальные перезаписи (`PermissionOverwrite`):
//!     1) `@everyone` (`overwrite_type == 0 && id == guild_id`): `perms = (perms & !deny) | allow`;
//!     2) роли участника (`overwrite_type == 0 && id != guild_id && member_role_ids.contains(&id)`):
//!        аккумуляция `role_allow |= allow`, `role_deny |= deny`, затем `perms = (perms & !role_deny) | role_allow`;
//!     3) участник (`overwrite_type == 1 && id == user_id`): `perms = (perms & !member_deny) | member_allow`;
//!   - если итоговые права содержат `ADMINISTRATOR` (`(perms & ADMINISTRATOR) != 0`),
//!     возвращается [`ALL_PERMISSIONS`] (полный обход канальных ограничений).

use std::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Not};

/// Маска полных прав доступа (все 64 бита включены, соответствует обходу владельца сервера и `ADMINISTRATOR`).
pub const ALL_PERMISSIONS: u64 = u64::MAX;

/// Маска всех 41 специфицированных флагов Discord API v9 (`0..=40`).
pub const ALL_V9_DEFINED_PERMISSIONS: u64 = (1u64 << 41) - 1;

/// Тип канальной перезаписи: роль (`0`).
pub const OVERWRITE_TYPE_ROLE: u8 = 0;

/// Тип канальной перезаписи: конкретный участник (`1`).
pub const OVERWRITE_TYPE_MEMBER: u8 = 1;

pub const CREATE_INSTANT_INVITE: u64 = 1 << 0;
pub const KICK_MEMBERS: u64 = 1 << 1;
pub const BAN_MEMBERS: u64 = 1 << 2;
pub const ADMINISTRATOR: u64 = 1 << 3;
pub const MANAGE_CHANNELS: u64 = 1 << 4;
pub const MANAGE_GUILD: u64 = 1 << 5;
pub const ADD_REACTIONS: u64 = 1 << 6;
pub const VIEW_AUDIT_LOG: u64 = 1 << 7;
pub const PRIORITY_SPEAKER: u64 = 1 << 8;
pub const STREAM: u64 = 1 << 9;
pub const VIEW_CHANNEL: u64 = 1 << 10;
pub const SEND_MESSAGES: u64 = 1 << 11;
pub const SEND_TTS_MESSAGES: u64 = 1 << 12;
pub const MANAGE_MESSAGES: u64 = 1 << 13;
pub const EMBED_LINKS: u64 = 1 << 14;
pub const ATTACH_FILES: u64 = 1 << 15;
pub const READ_MESSAGE_HISTORY: u64 = 1 << 16;
pub const MENTION_EVERYONE: u64 = 1 << 17;
pub const USE_EXTERNAL_EMOJIS: u64 = 1 << 18;
pub const VIEW_GUILD_INSIGHTS: u64 = 1 << 19;
pub const CONNECT: u64 = 1 << 20;
pub const SPEAK: u64 = 1 << 21;
pub const MUTE_MEMBERS: u64 = 1 << 22;
pub const DEAFEN_MEMBERS: u64 = 1 << 23;
pub const MOVE_MEMBERS: u64 = 1 << 24;
pub const USE_VAD: u64 = 1 << 25;
pub const CHANGE_NICKNAME: u64 = 1 << 26;
pub const MANAGE_NICKNAMES: u64 = 1 << 27;
pub const MANAGE_ROLES: u64 = 1 << 28;
pub const MANAGE_WEBHOOKS: u64 = 1 << 29;
pub const MANAGE_GUILD_EXPRESSIONS: u64 = 1 << 30;
pub const USE_APPLICATION_COMMANDS: u64 = 1 << 31;
pub const REQUEST_TO_SPEAK: u64 = 1 << 32;
pub const MANAGE_EVENTS: u64 = 1 << 33;
pub const MANAGE_THREADS: u64 = 1 << 34;
pub const CREATE_PUBLIC_THREADS: u64 = 1 << 35;
pub const CREATE_PRIVATE_THREADS: u64 = 1 << 36;
pub const USE_EXTERNAL_STICKERS: u64 = 1 << 37;
pub const SEND_MESSAGES_IN_THREADS: u64 = 1 << 38;
pub const USE_EMBEDDED_ACTIVITIES: u64 = 1 << 39;
pub const MODERATE_MEMBERS: u64 = 1 << 40;

/// 64-битное поле флагов прав доступа Discord v9.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Permissions(pub u64);

impl Permissions {
    pub const CREATE_INSTANT_INVITE: u64 = CREATE_INSTANT_INVITE;
    pub const KICK_MEMBERS: u64 = KICK_MEMBERS;
    pub const BAN_MEMBERS: u64 = BAN_MEMBERS;
    pub const ADMINISTRATOR: u64 = ADMINISTRATOR;
    pub const MANAGE_CHANNELS: u64 = MANAGE_CHANNELS;
    pub const MANAGE_GUILD: u64 = MANAGE_GUILD;
    pub const ADD_REACTIONS: u64 = ADD_REACTIONS;
    pub const VIEW_AUDIT_LOG: u64 = VIEW_AUDIT_LOG;
    pub const PRIORITY_SPEAKER: u64 = PRIORITY_SPEAKER;
    pub const STREAM: u64 = STREAM;
    pub const VIEW_CHANNEL: u64 = VIEW_CHANNEL;
    pub const SEND_MESSAGES: u64 = SEND_MESSAGES;
    pub const SEND_TTS_MESSAGES: u64 = SEND_TTS_MESSAGES;
    pub const MANAGE_MESSAGES: u64 = MANAGE_MESSAGES;
    pub const EMBED_LINKS: u64 = EMBED_LINKS;
    pub const ATTACH_FILES: u64 = ATTACH_FILES;
    pub const READ_MESSAGE_HISTORY: u64 = READ_MESSAGE_HISTORY;
    pub const MENTION_EVERYONE: u64 = MENTION_EVERYONE;
    pub const USE_EXTERNAL_EMOJIS: u64 = USE_EXTERNAL_EMOJIS;
    pub const VIEW_GUILD_INSIGHTS: u64 = VIEW_GUILD_INSIGHTS;
    pub const CONNECT: u64 = CONNECT;
    pub const SPEAK: u64 = SPEAK;
    pub const MUTE_MEMBERS: u64 = MUTE_MEMBERS;
    pub const DEAFEN_MEMBERS: u64 = DEAFEN_MEMBERS;
    pub const MOVE_MEMBERS: u64 = MOVE_MEMBERS;
    pub const USE_VAD: u64 = USE_VAD;
    pub const CHANGE_NICKNAME: u64 = CHANGE_NICKNAME;
    pub const MANAGE_NICKNAMES: u64 = MANAGE_NICKNAMES;
    pub const MANAGE_ROLES: u64 = MANAGE_ROLES;
    pub const MANAGE_WEBHOOKS: u64 = MANAGE_WEBHOOKS;
    pub const MANAGE_GUILD_EXPRESSIONS: u64 = MANAGE_GUILD_EXPRESSIONS;
    pub const USE_APPLICATION_COMMANDS: u64 = USE_APPLICATION_COMMANDS;
    pub const REQUEST_TO_SPEAK: u64 = REQUEST_TO_SPEAK;
    pub const MANAGE_EVENTS: u64 = MANAGE_EVENTS;
    pub const MANAGE_THREADS: u64 = MANAGE_THREADS;
    pub const CREATE_PUBLIC_THREADS: u64 = CREATE_PUBLIC_THREADS;
    pub const CREATE_PRIVATE_THREADS: u64 = CREATE_PRIVATE_THREADS;
    pub const USE_EXTERNAL_STICKERS: u64 = USE_EXTERNAL_STICKERS;
    pub const SEND_MESSAGES_IN_THREADS: u64 = SEND_MESSAGES_IN_THREADS;
    pub const USE_EMBEDDED_ACTIVITIES: u64 = USE_EMBEDDED_ACTIVITIES;
    pub const MODERATE_MEMBERS: u64 = MODERATE_MEMBERS;

    pub const ALL_PERMISSIONS: u64 = ALL_PERMISSIONS;
    pub const EMPTY: Self = Self(0);
    pub const ALL: Self = Self(ALL_PERMISSIONS);

    /// Создаёт обёртку над битовой маской `bits`.
    #[must_use]
    pub const fn from_bits(bits: u64) -> Self {
        Self(bits)
    }

    /// Возвращает сырое 64-битное значение маски.
    #[must_use]
    pub const fn bits(self) -> u64 {
        self.0
    }

    /// Проверяет наличие требуемого права `required_perm` с учётом флага `ADMINISTRATOR`.
    #[must_use]
    pub const fn has(self, required_perm: u64) -> bool {
        has_permission(self.0, required_perm)
    }

    /// Проверяет точное вхождение битов `flag` без учёта `ADMINISTRATOR`.
    #[must_use]
    pub const fn contains(self, flag: u64) -> bool {
        (self.0 & flag) == flag
    }
}

impl From<u64> for Permissions {
    fn from(bits: u64) -> Self {
        Self(bits)
    }
}

impl From<Permissions> for u64 {
    fn from(perms: Permissions) -> Self {
        perms.0
    }
}

impl BitOr for Permissions {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl BitOr<u64> for Permissions {
    type Output = Self;

    fn bitor(self, rhs: u64) -> Self::Output {
        Self(self.0 | rhs)
    }
}

impl BitOrAssign for Permissions {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl BitOrAssign<u64> for Permissions {
    fn bitor_assign(&mut self, rhs: u64) {
        self.0 |= rhs;
    }
}

impl BitAnd for Permissions {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        Self(self.0 & rhs.0)
    }
}

impl BitAnd<u64> for Permissions {
    type Output = Self;

    fn bitand(self, rhs: u64) -> Self::Output {
        Self(self.0 & rhs)
    }
}

impl BitAndAssign for Permissions {
    fn bitand_assign(&mut self, rhs: Self) {
        self.0 &= rhs.0;
    }
}

impl BitAndAssign<u64> for Permissions {
    fn bitand_assign(&mut self, rhs: u64) {
        self.0 &= rhs;
    }
}

impl BitXor for Permissions {
    type Output = Self;

    fn bitxor(self, rhs: Self) -> Self::Output {
        Self(self.0 ^ rhs.0)
    }
}

impl BitXorAssign for Permissions {
    fn bitxor_assign(&mut self, rhs: Self) {
        self.0 ^= rhs.0;
    }
}

impl Not for Permissions {
    type Output = Self;

    fn not(self) -> Self::Output {
        Self(!self.0)
    }
}

/// Перезапись прав доступа на уровне канала (`permission_overwrites[i]`).
///
/// Крейт `discord-core` не зависит от `serde` (см. `Cargo.toml:10`), поэтому для
/// сериализации/десериализации предоставлены методы поверх `discord_json` и конверсия из [`crate::ready::PermissionOverwrite`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct PermissionOverwrite {
    /// Snowflake ID роли (`overwrite_type == 0`) или участника (`overwrite_type == 1`).
    pub id: u64,
    /// Тип цели перезаписи: `0` — роль (`OVERWRITE_TYPE_ROLE`), `1` — участник (`OVERWRITE_TYPE_MEMBER`).
    pub overwrite_type: u8,
    /// Битовая маска явно разрешённых прав.
    pub allow: u64,
    /// Битовая маска явно запрещённых прав.
    pub deny: u64,
}

impl From<crate::ready::PermissionOverwrite> for PermissionOverwrite {
    fn from(raw: crate::ready::PermissionOverwrite) -> Self {
        Self::from(&raw)
    }
}

impl From<&crate::ready::PermissionOverwrite> for PermissionOverwrite {
    fn from(raw: &crate::ready::PermissionOverwrite) -> Self {
        Self {
            id: raw.id,
            overwrite_type: u8::try_from(raw.target_type).unwrap_or(u8::MAX),
            allow: raw.allow,
            deny: raw.deny,
        }
    }
}

/// Вычисляет базовые права участника на уровне гильдии (шаги 1–2 официального алгоритма Discord v9):
/// 1. `base_perms = everyone_role_perms | (OR всех member_roles_perms)`.
/// 2. Если `user_id == owner_id` или `(base_perms & ADMINISTRATOR) != 0`, возвращается [`ALL_PERMISSIONS`].
#[must_use]
pub fn compute_base_permissions(
    owner_id: u64,
    user_id: u64,
    everyone_role_perms: u64,
    member_roles_perms: &[u64],
) -> u64 {
    if user_id == owner_id {
        return ALL_PERMISSIONS;
    }

    let mut base_perms = everyone_role_perms;
    for &role_perms in member_roles_perms {
        base_perms |= role_perms;
    }

    if (base_perms & ADMINISTRATOR) != 0 {
        return ALL_PERMISSIONS;
    }

    base_perms
}

/// Вычисляет эффективные права участника в канале с учётом канальных перезаписей
/// (шаги 3–4 официального алгоритма Discord v9):
/// - Если `(base_perms & ADMINISTRATOR) != 0`, немедленно возвращается [`ALL_PERMISSIONS`].
/// - Шаг 3a: применяется перезапись `@everyone` (`overwrite_type == 0 && id == guild_id`):
///   `perms = (perms & !deny) | allow`.
/// - Шаг 3b: суммируются побитовым ИЛИ `role_allow` и `role_deny` для всех ролей из `member_role_ids`
///   (`overwrite_type == 0 && id != guild_id`), затем `perms = (perms & !role_deny) | role_allow`.
/// - Шаг 3c: применяется персональная перезапись участника (`overwrite_type == 1 && id == user_id`):
///   `perms = (perms & !member_deny) | member_allow`.
/// - Шаг 4: если `(perms & ADMINISTRATOR) != 0`, возвращается [`ALL_PERMISSIONS`].
#[must_use]
pub fn compute_channel_permissions(
    base_perms: u64,
    guild_id: u64,
    user_id: u64,
    member_role_ids: &[u64],
    overwrites: &[PermissionOverwrite],
) -> u64 {
    if (base_perms & ADMINISTRATOR) != 0 {
        return ALL_PERMISSIONS;
    }

    let mut perms = base_perms;

    // 3a) @everyone overwrite (id == guild_id, overwrite_type == 0)
    for ov in overwrites {
        if ov.overwrite_type == OVERWRITE_TYPE_ROLE && ov.id == guild_id {
            perms = (perms & !ov.deny) | ov.allow;
        }
    }

    // 3b) Role overwrites: accumulate all role_allow (OR) and role_deny (OR) for member's roles
    let mut role_allow = 0u64;
    let mut role_deny = 0u64;
    for ov in overwrites {
        if ov.overwrite_type == OVERWRITE_TYPE_ROLE
            && ov.id != guild_id
            && member_role_ids.contains(&ov.id)
        {
            role_allow |= ov.allow;
            role_deny |= ov.deny;
        }
    }
    perms = (perms & !role_deny) | role_allow;

    // 3c) Member overwrite (id == user_id, overwrite_type == 1)
    for ov in overwrites {
        if ov.overwrite_type == OVERWRITE_TYPE_MEMBER && ov.id == user_id {
            perms = (perms & !ov.deny) | ov.allow;
        }
    }

    // Step 4: ADMINISTRATOR bypass
    if (perms & ADMINISTRATOR) != 0 {
        return ALL_PERMISSIONS;
    }

    perms
}

/// Проверяет, содержит ли вычисленная маска `computed_perms` требуемое право (или набор прав) `required_perm`.
/// Наличие бита [`ADMINISTRATOR`] в `computed_perms` автоматически разрешает любое право.
#[must_use]
pub const fn has_permission(computed_perms: u64, required_perm: u64) -> bool {
    if (computed_perms & ADMINISTRATOR) != 0 {
        return true;
    }
    (computed_perms & required_perm) == required_perm
}
