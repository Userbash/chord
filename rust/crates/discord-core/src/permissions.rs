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

use std::fmt;
use std::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Not};
use std::str::FromStr;

/// Маска полных прав доступа (все 64 бита включены, соответствует обходу владельца сервера и `ADMINISTRATOR`).
pub const ALL_PERMISSIONS: u64 = u64::MAX;

/// Маска всех 41 специфицированных флагов Discord API v9 (`0..=40`).
pub const ALL_V9_DEFINED_PERMISSIONS: u64 = (1u64 << 41) - 1;

/// Тип канальной перезаписи: роль (`0`).
pub const OVERWRITE_TYPE_ROLE: u8 = 0;

/// Тип канальной перезаписи: конкретный участник (`1`).
pub const OVERWRITE_TYPE_MEMBER: u8 = 1;

/// Официальная маска `lurkerPermissionsMask` (`VIEW_CHANNEL | READ_MESSAGE_HISTORY = 66560`)
/// из `/home/sanya/re-deep/decoded/4d2a64cb5df5fc4e_0` (`y = a.kg(VIEW_CHANNEL, READ_MESSAGE_HISTORY)`).
pub const LURKER_PERMISSIONS_MASK: u64 = VIEW_CHANNEL | READ_MESSAGE_HISTORY;

/// Маска прав участника в режиме тайм-аута (`communication_disabled_until > now`, `v = 66560`).
pub const TIMED_OUT_PERMISSIONS_MASK: u64 = VIEW_CHANNEL | READ_MESSAGE_HISTORY;

/// Маска прав участника в карантине AutoMod (`b = VIEW_CHANNEL | READ_MESSAGE_HISTORY | CHANGE_NICKNAME = 67175424`).
pub const QUARANTINED_PERMISSIONS_MASK: u64 = VIEW_CHANNEL | READ_MESSAGE_HISTORY | CHANGE_NICKNAME;

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

    /// Добавляет битовую маску `bits` (`BigFlagUtils.add` / `WQ`).
    #[must_use]
    pub const fn add(self, bits: u64) -> Self {
        Self(self.0 | bits)
    }

    /// Снимает биты `bits` (`BigFlagUtils.remove` / `TF`).
    #[must_use]
    pub const fn remove(self, bits: u64) -> Self {
        Self(self.0 & !bits)
    }

    /// Оставляет только биты, входящие в `mask` (`BigFlagUtils.filter` / `pb`).
    #[must_use]
    pub const fn filter(self, mask: u64) -> Self {
        Self(self.0 & mask)
    }

    /// Разбирает каноническую десятичную строку `u64` прав доступа Discord API v9 (`BigFlagUtils.deserialize` / `iu`).
    pub fn from_decimal_str(s: &str) -> Result<Self, PermissionParseError> {
        if s.is_empty() {
            return Err(PermissionParseError::Empty);
        }
        let mut acc = 0u64;
        for b in s.bytes() {
            if !b.is_ascii_digit() {
                return Err(PermissionParseError::InvalidDigit);
            }
            let digit = u64::from(b - b'0');
            acc = acc
                .checked_mul(10)
                .and_then(|v| v.checked_add(digit))
                .ok_or(PermissionParseError::Overflow)?;
        }
        Ok(Self(acc))
    }
}

/// Ошибка разбора десятичной строки битовой маски прав доступа.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PermissionParseError {
    /// Пустая строка.
    Empty,
    /// Строка содержит нецифровой символ (например `-`, пробел или буквы).
    InvalidDigit,
    /// Значение превышает `u64::MAX` (`18446744073709551615`).
    Overflow,
}

impl fmt::Display for PermissionParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("пустая строка битовой маски прав"),
            Self::InvalidDigit => f.write_str("недопустимый символ в десятичной маске прав"),
            Self::Overflow => f.write_str("переполнение 64-битной маски прав"),
        }
    }
}

impl std::error::Error for PermissionParseError {}

impl fmt::Display for Permissions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for Permissions {
    type Err = PermissionParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_decimal_str(s)
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

/// Описание роли гильдии для расчёта базовых прав (`id` и 64-битная маска `permissions`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct RolePermissions {
    /// Snowflake ID роли (`id == guild_id` для роли `@everyone`).
    pub id: u64,
    /// Битовая маска прав роли.
    pub permissions: u64,
}

impl From<crate::ready::Role> for RolePermissions {
    fn from(role: crate::ready::Role) -> Self {
        Self::from(&role)
    }
}

impl From<&crate::ready::Role> for RolePermissions {
    fn from(role: &crate::ready::Role) -> Self {
        Self {
            id: role.id,
            permissions: role.permissions,
        }
    }
}

impl From<(u64, u64)> for RolePermissions {
    fn from((id, permissions): (u64, u64)) -> Self {
        Self { id, permissions }
    }
}

/// Ограничительные флаги состояния участника в гильдии (`4d2a64cb5df5fc4e_0@9771400`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct MemberRestrictions {
    /// Режим предпросмотра сервера (`lurker == true`).
    pub is_lurker: bool,
    /// Не пройдено принятие правил (`isPending == true`).
    pub is_pending: bool,
    /// Активен тайм-аут модерации (`communicationDisabledUntil > now`).
    pub is_timed_out: bool,
    /// Участник помещён в карантин AutoMod (`AUTOMOD_QUARANTINED_*`).
    pub is_quarantined: bool,
}

/// Полный результат резолвинга прав доступа пользователя в канале гильдии
/// (`Base Guild Permissions -> @everyone Overwrite -> Role Overwrites -> Member Overwrite`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ResolvedPermissions {
    /// Базовые права на уровне гильдии (`compute_base_permissions`).
    pub base_bits: u64,
    /// Сырые права после наложения канальных перезаписей (`compute_channel_permissions`).
    pub channel_bits: u64,
    /// Эффективные права в канале с учётом неявных зависимостей (`!VIEW_CHANNEL`, `!SEND_MESSAGES`, `!CONNECT`)
    /// и ограничительных масок (`lurker`, `pending`, `timed_out`, `quarantined`).
    pub effective_bits: u64,
    /// Доступ к просмотру канала (`VIEW_CHANNEL`).
    pub can_view: bool,
    /// Чтение истории сообщений в канале (`VIEW_CHANNEL && READ_MESSAGE_HISTORY`).
    pub can_read_history: bool,
    /// Отправка сообщений в канал (`VIEW_CHANNEL && SEND_MESSAGES`).
    pub can_send: bool,
    /// Прикрепление файлов в канал (`VIEW_CHANNEL && SEND_MESSAGES && ATTACH_FILES`).
    pub can_attach: bool,
    /// Встраивание ссылок (`VIEW_CHANNEL && SEND_MESSAGES && EMBED_LINKS`).
    pub can_embed_links: bool,
    /// Добавление реакций (`VIEW_CHANNEL && ADD_REACTIONS`).
    pub can_add_reactions: bool,
    /// Удаление чужих сообщений в канале (`VIEW_CHANNEL && MANAGE_MESSAGES`).
    pub can_delete_others: bool,
    /// Исключение участников из гильдии (`KICK_MEMBERS`).
    pub can_kick: bool,
    /// Блокировка участников в гильдии (`BAN_MEMBERS`).
    pub can_ban: bool,
    /// Управление каналом (`VIEW_CHANNEL && MANAGE_CHANNELS`).
    pub can_manage_channels: bool,
    /// Управление правами/ролями (`VIEW_CHANNEL && MANAGE_ROLES`).
    pub can_manage_roles: bool,
    /// Подключение к голосовому каналу (`VIEW_CHANNEL && CONNECT`).
    pub can_connect: bool,
    /// Голос в голосовом канале (`VIEW_CHANNEL && CONNECT && SPEAK`).
    pub can_speak: bool,
    /// Трансляция экрана / видео в голосовом канале (`VIEW_CHANNEL && CONNECT && STREAM`).
    pub can_stream: bool,
    /// Полный административный доступ (`ADMINISTRATOR`).
    pub is_admin: bool,
}

/// Вычисляет базовые права участника по полному списку ролей гильдии `guild_roles` и списку ID ролей участника `member_role_ids`.
/// Роль `@everyone` определяется совпадением `role.id == guild_id`.
#[must_use]
pub fn resolve_guild_base_permissions(
    guild_id: u64,
    owner_id: u64,
    user_id: u64,
    guild_roles: &[RolePermissions],
    member_role_ids: &[u64],
) -> u64 {
    let mut everyone_perms = 0u64;
    let mut combined_member_perms = 0u64;
    for role in guild_roles {
        if role.id == guild_id {
            everyone_perms |= role.permissions;
        } else if member_role_ids.contains(&role.id) {
            combined_member_perms |= role.permissions;
        }
    }

    compute_base_permissions(owner_id, user_id, everyone_perms, &[combined_member_perms])
}

/// Выполняет полный цикл расчёта прав пользователя в канале (`base -> @everyone -> roles -> member`)
/// со всеми неявными зависимостями Discord v9 и возвращает структуру [`ResolvedPermissions`].
#[must_use]
pub fn resolve_permissions(
    base_perms: u64,
    guild_id: u64,
    user_id: u64,
    member_role_ids: &[u64],
    overwrites: &[PermissionOverwrite],
) -> ResolvedPermissions {
    resolve_permissions_with_restrictions(
        base_perms,
        guild_id,
        user_id,
        member_role_ids,
        overwrites,
        MemberRestrictions::default(),
    )
}

/// Выполняет полный цикл расчёта прав пользователя в канале с учётом ограничительных флагов
/// состояния участника (`lurker`, `is_pending`, `is_timed_out`, `is_quarantined`)
/// по эталонному алгоритму `w()` / `U()` из `/home/sanya/re-deep/decoded/4d2a64cb5df5fc4e_0`.
#[must_use]
pub fn resolve_permissions_with_restrictions(
    base_perms: u64,
    guild_id: u64,
    user_id: u64,
    member_role_ids: &[u64],
    overwrites: &[PermissionOverwrite],
    restrictions: MemberRestrictions,
) -> ResolvedPermissions {
    let channel_bits =
        compute_channel_permissions(base_perms, guild_id, user_id, member_role_ids, overwrites);

    let has_admin = (channel_bits & ADMINISTRATOR) != 0;
    let is_lurker_or_pending = restrictions.is_lurker || restrictions.is_pending;
    let is_admin = has_admin && !is_lurker_or_pending;

    let effective_bits = if is_admin {
        ALL_PERMISSIONS
    } else {
        let mut eff = channel_bits;
        if restrictions.is_quarantined {
            eff &= QUARANTINED_PERMISSIONS_MASK;
        }
        if restrictions.is_timed_out {
            eff &= TIMED_OUT_PERMISSIONS_MASK;
        }
        if is_lurker_or_pending {
            eff &= LURKER_PERMISSIONS_MASK;
        }

        if (eff & VIEW_CHANNEL) == 0 {
            0
        } else {
            if (eff & SEND_MESSAGES) == 0 {
                eff &= !(SEND_TTS_MESSAGES | MENTION_EVERYONE | ATTACH_FILES | EMBED_LINKS);
            }
            if (eff & CONNECT) == 0 {
                eff &= !(SPEAK
                    | STREAM
                    | USE_VAD
                    | MUTE_MEMBERS
                    | DEAFEN_MEMBERS
                    | MOVE_MEMBERS
                    | PRIORITY_SPEAKER);
            }
            eff
        }
    };

    let is_restricted =
        is_lurker_or_pending || restrictions.is_timed_out || restrictions.is_quarantined;
    let guild_mod_bits = if is_admin {
        ALL_PERMISSIONS
    } else if is_restricted {
        0
    } else {
        base_perms | effective_bits
    };

    let can_view = (effective_bits & VIEW_CHANNEL) != 0;
    let can_read_history = can_view && (effective_bits & READ_MESSAGE_HISTORY) != 0;
    let can_send = can_view && (effective_bits & SEND_MESSAGES) != 0;
    let can_attach = can_send && (effective_bits & ATTACH_FILES) != 0;
    let can_embed_links = can_send && (effective_bits & EMBED_LINKS) != 0;
    let can_add_reactions = can_view && (effective_bits & ADD_REACTIONS) != 0;
    let can_delete_others = can_view && (effective_bits & MANAGE_MESSAGES) != 0;
    let can_kick = (guild_mod_bits & KICK_MEMBERS) != 0;
    let can_ban = (guild_mod_bits & BAN_MEMBERS) != 0;
    let can_manage_channels = can_view && (effective_bits & MANAGE_CHANNELS) != 0;
    let can_manage_roles = can_view && (effective_bits & MANAGE_ROLES) != 0;
    let can_connect = can_view && (effective_bits & CONNECT) != 0;
    let can_speak = can_connect && (effective_bits & SPEAK) != 0;
    let can_stream = can_connect && (effective_bits & STREAM) != 0;

    ResolvedPermissions {
        base_bits: base_perms,
        channel_bits,
        effective_bits,
        can_view,
        can_read_history,
        can_send,
        can_attach,
        can_embed_links,
        can_add_reactions,
        can_delete_others,
        can_kick,
        can_ban,
        can_manage_channels,
        can_manage_roles,
        can_connect,
        can_speak,
        can_stream,
        is_admin,
    }
}
