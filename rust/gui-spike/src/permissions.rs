//! Discord Permission Calculator (`twilight-model` + `twilight-util::permission_calculator` semantics)
//! and Server Preview (`lurkerPermissionsMask`) enforcement verified against `/home/sanya/re-deep/decoded/4d2a64cb5df5fc4e_0@9773248`.

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const PERM_CREATE_INSTANT_INVITE: u64 = 1 << 0;
pub const PERM_KICK_MEMBERS: u64 = 1 << 1;
pub const PERM_BAN_MEMBERS: u64 = 1 << 2;
pub const PERM_ADMINISTRATOR: u64 = 1 << 3;
pub const PERM_MANAGE_CHANNELS: u64 = 1 << 4;
pub const PERM_MANAGE_GUILD: u64 = 1 << 5;
pub const PERM_ADD_REACTIONS: u64 = 1 << 6;
pub const PERM_VIEW_AUDIT_LOG: u64 = 1 << 7;
pub const PERM_PRIORITY_SPEAKER: u64 = 1 << 8;
pub const PERM_STREAM: u64 = 1 << 9;
pub const PERM_VIEW_CHANNEL: u64 = 1 << 10;
pub const PERM_SEND_MESSAGES: u64 = 1 << 11;
pub const PERM_SEND_TTS_MESSAGES: u64 = 1 << 12;
pub const PERM_MANAGE_MESSAGES: u64 = 1 << 13;
pub const PERM_EMBED_LINKS: u64 = 1 << 14;
pub const PERM_ATTACH_FILES: u64 = 1 << 15;
pub const PERM_READ_MESSAGE_HISTORY: u64 = 1 << 16;
pub const PERM_MENTION_EVERYONE: u64 = 1 << 17;
pub const PERM_USE_EXTERNAL_EMOJIS: u64 = 1 << 18;
pub const PERM_CONNECT: u64 = 1 << 20;
pub const PERM_SPEAK: u64 = 1 << 21;
pub const PERM_MUTE_MEMBERS: u64 = 1 << 22;
pub const PERM_DEAFEN_MEMBERS: u64 = 1 << 23;
pub const PERM_MOVE_MEMBERS: u64 = 1 << 24;
pub const PERM_USE_VAD: u64 = 1 << 25;
pub const PERM_SEND_MESSAGES_IN_THREADS: u64 = 1 << 38;

/// Discord's official `lurkerPermissionsMask` (`VIEW_CHANNEL | READ_MESSAGE_HISTORY` = `66560`)
/// from `/home/sanya/re-deep/decoded/4d2a64cb5df5fc4e_0@9773248`.
/// When a user is previewing a server (`lurker == true || is_member == false`),
/// all write/voice/reaction bits are stripped regardless of `@everyone` defaults.
pub const LURKER_PERMISSIONS_MASK: u64 = PERM_VIEW_CHANNEL | PERM_READ_MESSAGE_HISTORY;

/// Mask applied when a member is pending Membership Screening (`pending == true`) or timed out.
pub const TIMED_OUT_PERMISSIONS_MASK: u64 = PERM_VIEW_CHANNEL | PERM_READ_MESSAGE_HISTORY;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RolePermissionDef {
    pub id: String,
    pub permissions: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RolePermission {
    pub id: String,
    pub permissions: u64,
    #[serde(default)]
    pub position: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OverwriteKind {
    #[default]
    Role,
    Member,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionOverwriteDef {
    pub id: String,
    pub kind: OverwriteKind,
    pub allow: u64,
    pub deny: u64,
}

pub type ChannelOverwrite = PermissionOverwriteDef;

/// Serde DTO for deserializing a raw Discord channel permission overwrite entry (`TAURI-DTO-06`).
///
/// Supports both string and numeric Snowflake IDs (`id`), numeric (`0`/`1`) or string
/// (`"0"`/`"1"`/`"role"`/`"member"`) overwrite target types (`type` or `kind`), and
/// decimal string or numeric `u64` permission bitmasks (`allow` and `deny`).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct RawPermissionOverwriteDto {
    /// Target role or member Snowflake ID (`id`), parsed from a string or unsigned integer.
    #[serde(deserialize_with = "deserialize_snowflake_id")]
    pub id: String,
    /// Overwrite target kind (`type` or `kind`: `0`/`"role"` = [`OverwriteKind::Role`], `1`/`"member"` = [`OverwriteKind::Member`]).
    #[serde(
        rename = "type",
        alias = "kind",
        deserialize_with = "deserialize_overwrite_kind"
    )]
    pub kind: OverwriteKind,
    /// Allowed permission bitmask (`allow`), parsed from a decimal string or `u64`.
    #[serde(deserialize_with = "deserialize_perm_bits")]
    pub allow: u64,
    /// Denied permission bitmask (`deny`), parsed from a decimal string or `u64`.
    #[serde(deserialize_with = "deserialize_perm_bits")]
    pub deny: u64,
}

impl RawPermissionOverwriteDto {
    /// Converts the deserialized DTO into a validated [`PermissionOverwriteDef`],
    /// returning `None` if `id` is empty.
    #[inline]
    #[must_use]
    pub fn into_overwrite_def(self) -> Option<PermissionOverwriteDef> {
        if self.id.is_empty() {
            return None;
        }
        Some(PermissionOverwriteDef {
            id: self.id,
            kind: self.kind,
            allow: self.allow,
            deny: self.deny,
        })
    }

    /// Alias for [`Self::into_overwrite_def`].
    #[inline]
    #[must_use]
    pub fn into_permission_overwrite_def(self) -> Option<PermissionOverwriteDef> {
        self.into_overwrite_def()
    }
}

impl TryFrom<RawPermissionOverwriteDto> for PermissionOverwriteDef {
    type Error = &'static str;

    #[inline]
    fn try_from(dto: RawPermissionOverwriteDto) -> Result<Self, Self::Error> {
        dto.into_overwrite_def()
            .ok_or("permission overwrite missing id")
    }
}

impl From<PermissionOverwriteDef> for RawPermissionOverwriteDto {
    #[inline]
    fn from(def: PermissionOverwriteDef) -> Self {
        Self {
            id: def.id,
            kind: def.kind,
            allow: def.allow,
            deny: def.deny,
        }
    }
}

/// Serde DTO for deserializing a raw Discord role permission entry (`TAURI-DTO-06`).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct RawRolePermissionDto {
    /// Role Snowflake ID (`id`), parsed from a string or unsigned integer.
    #[serde(deserialize_with = "deserialize_snowflake_id")]
    pub id: String,
    /// Role permission bitmask (`permissions`), parsed from a decimal string or `u64`.
    #[serde(deserialize_with = "deserialize_perm_bits")]
    pub permissions: u64,
    /// Role hierarchy position (`position`).
    pub position: i64,
}

impl RawRolePermissionDto {
    /// Converts the deserialized DTO into a validated [`RolePermissionDef`],
    /// returning `None` if `id` is empty.
    #[inline]
    #[must_use]
    pub fn into_role_permission_def(self) -> Option<RolePermissionDef> {
        if self.id.is_empty() {
            return None;
        }
        Some(RolePermissionDef {
            id: self.id,
            permissions: self.permissions,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AllowedChannelAccess {
    pub effective_permissions: u64,
    pub can_view: bool,
    pub can_send: bool,
    pub can_connect: bool,
    pub can_speak: bool,
    pub can_stream: bool,
    pub is_lurker: bool,
    pub is_pending: bool,
    pub denied_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelAccessPolicy {
    pub effective_permissions: String,
    pub can_view_channel: bool,
    pub can_read_history: bool,
    pub can_send_messages: bool,
    pub can_add_reactions: bool,
    pub can_attach_files: bool,
    pub can_connect_voice: bool,
    pub can_speak_voice: bool,
    pub can_stream_voice: bool,
    pub is_lurker: bool,
    pub is_pending: bool,
    pub read_only: bool,
    pub denied_reason: Option<String>,
}

/// Strict `twilight-util::permission_calculator::PermissionCalculator` implementation
/// extended with Discord's `lurkerPermissionsMask` and `pending` member restrictions.
#[derive(Debug, Clone)]
pub struct PermissionCalculator<'a> {
    guild_id: &'a str,
    user_id: &'a str,
    owner_id: Option<&'a str>,
    everyone_perms: u64,
    roles: &'a [RolePermissionDef],
    member_roles: &'a [String],
    is_lurker: bool,
    is_pending: bool,
    is_timed_out: bool,
}

impl<'a> PermissionCalculator<'a> {
    pub fn new(
        guild_id: &'a str,
        user_id: &'a str,
        everyone_perms: u64,
        roles: &'a [RolePermissionDef],
        member_roles: &'a [String],
    ) -> Self {
        Self {
            guild_id,
            user_id,
            owner_id: None,
            everyone_perms,
            roles,
            member_roles,
            is_lurker: false,
            is_pending: false,
            is_timed_out: false,
        }
    }

    pub const fn owner_id(mut self, owner_id: Option<&'a str>) -> Self {
        self.owner_id = owner_id;
        self
    }

    pub const fn is_lurker(mut self, is_lurker: bool) -> Self {
        self.is_lurker = is_lurker;
        self
    }

    pub const fn is_pending(mut self, is_pending: bool) -> Self {
        self.is_pending = is_pending;
        self
    }

    pub const fn is_timed_out(mut self, is_timed_out: bool) -> Self {
        self.is_timed_out = is_timed_out;
        self
    }

    /// Computes guild-level root permissions (`twilight_util::permission_calculator::PermissionCalculator::root`).
    pub fn root(&self) -> u64 {
        if !self.is_lurker
            && self
                .owner_id
                .is_some_and(|owner| !owner.is_empty() && owner == self.user_id)
        {
            return u64::MAX;
        }

        let mut perms = self.everyone_perms;
        if !self.is_lurker {
            for role in self.roles {
                if role.id == self.guild_id {
                    perms |= role.permissions;
                } else if self.member_roles.iter().any(|r| r == &role.id) {
                    perms |= role.permissions;
                }
            }
        } else {
            for role in self.roles {
                if role.id == self.guild_id {
                    perms |= role.permissions;
                }
            }
        }

        if (perms & PERM_ADMINISTRATOR) == PERM_ADMINISTRATOR && !self.is_lurker {
            return u64::MAX;
        }

        if self.is_lurker {
            perms &= LURKER_PERMISSIONS_MASK;
        } else if self.is_pending || self.is_timed_out {
            perms &= TIMED_OUT_PERMISSIONS_MASK;
        }

        perms
    }

    /// Computes effective permissions in a channel given optional parent category overwrites
    /// and channel-specific overwrites (`twilight_util::permission_calculator::PermissionCalculator::in_channel`).
    pub fn in_channel(
        &self,
        category_overwrites: &[PermissionOverwriteDef],
        channel_overwrites: &[PermissionOverwriteDef],
    ) -> u64 {
        if !self.is_lurker
            && self
                .owner_id
                .is_some_and(|owner| !owner.is_empty() && owner == self.user_id)
        {
            return u64::MAX;
        }

        let mut perms = self.everyone_perms;
        for role in self.roles {
            if role.id == self.guild_id {
                perms |= role.permissions;
            } else if !self.is_lurker && self.member_roles.iter().any(|r| r == &role.id) {
                perms |= role.permissions;
            }
        }

        if (perms & PERM_ADMINISTRATOR) == PERM_ADMINISTRATOR && !self.is_lurker {
            return u64::MAX;
        }

        perms = self.apply_overwrites(perms, category_overwrites);
        perms = self.apply_overwrites(perms, channel_overwrites);

        // Implicit Discord denials per official specification:
        if (perms & PERM_VIEW_CHANNEL) == 0 {
            return 0;
        }

        if (perms & PERM_SEND_MESSAGES) == 0 {
            perms &= !(PERM_SEND_TTS_MESSAGES
                | PERM_MENTION_EVERYONE
                | PERM_ATTACH_FILES
                | PERM_EMBED_LINKS);
        }

        if (perms & PERM_CONNECT) == 0 {
            perms &= !(PERM_SPEAK
                | PERM_STREAM
                | PERM_USE_VAD
                | PERM_MUTE_MEMBERS
                | PERM_DEAFEN_MEMBERS
                | PERM_MOVE_MEMBERS
                | PERM_PRIORITY_SPEAKER);
        }

        if self.is_lurker {
            perms &= LURKER_PERMISSIONS_MASK;
        } else if self.is_pending || self.is_timed_out {
            perms &= TIMED_OUT_PERMISSIONS_MASK;
        }

        perms
    }

    fn apply_overwrites(&self, mut perms: u64, overwrites: &[PermissionOverwriteDef]) -> u64 {
        if overwrites.is_empty() {
            return perms;
        }

        // 1. @everyone role overwrite (id == guild_id)
        for ow in overwrites {
            if ow.kind == OverwriteKind::Role && ow.id == self.guild_id {
                perms &= !ow.deny;
                perms |= ow.allow;
            }
        }

        // 2. Member role overwrites (accumulated allow/deny)
        if !self.is_lurker {
            let mut allow_roles = 0u64;
            let mut deny_roles = 0u64;
            for ow in overwrites {
                if ow.kind == OverwriteKind::Role
                    && ow.id != self.guild_id
                    && self.member_roles.iter().any(|r| r == &ow.id)
                {
                    allow_roles |= ow.allow;
                    deny_roles |= ow.deny;
                }
            }
            perms &= !deny_roles;
            perms |= allow_roles;

            // 3. Member-specific overwrite (id == user_id)
            if !self.user_id.is_empty() {
                for ow in overwrites {
                    if ow.kind == OverwriteKind::Member && ow.id == self.user_id {
                        perms &= !ow.deny;
                        perms |= ow.allow;
                    }
                }
            }
        }

        perms
    }

    pub fn allowed_access(
        &self,
        category_overwrites: &[PermissionOverwriteDef],
        channel_overwrites: &[PermissionOverwriteDef],
    ) -> AllowedChannelAccess {
        let effective_permissions = self.in_channel(category_overwrites, channel_overwrites);
        let can_view = (effective_permissions & PERM_VIEW_CHANNEL) != 0;
        let can_send = can_view
            && !self.is_lurker
            && !self.is_pending
            && (effective_permissions & PERM_SEND_MESSAGES) != 0;
        let can_connect = can_view
            && !self.is_lurker
            && !self.is_pending
            && (effective_permissions & PERM_CONNECT) != 0;
        let can_speak = can_connect && (effective_permissions & PERM_SPEAK) != 0;
        let can_stream = can_connect && (effective_permissions & PERM_STREAM) != 0;

        let denied_reason = if self.is_lurker {
            Some("Сервер в режиме просмотра".to_string())
        } else if self.is_pending {
            Some("Требуется принять правила сервера".to_string())
        } else if !can_connect {
            Some("Роль не даёт права Подключаться (CONNECT)".to_string())
        } else {
            None
        };

        AllowedChannelAccess {
            effective_permissions,
            can_view,
            can_send,
            can_connect,
            can_speak,
            can_stream,
            is_lurker: self.is_lurker,
            is_pending: self.is_pending,
            denied_reason,
        }
    }
}

impl PermissionCalculator<'static> {
    pub fn evaluate_allowed_channel_access(
        guild_id: &str,
        owner_id: &str,
        user_id: &str,
        member_roles: &[String],
        guild_roles: &[RolePermission],
        channel_overwrites: &[ChannelOverwrite],
        is_lurker: bool,
        is_pending: bool,
    ) -> AllowedChannelAccess {
        let mut everyone_perms = PERM_VIEW_CHANNEL | PERM_READ_MESSAGE_HISTORY;
        let mut role_defs = Vec::with_capacity(guild_roles.len());
        for r in guild_roles {
            if r.id == guild_id {
                everyone_perms = r.permissions;
            }
            role_defs.push(RolePermissionDef {
                id: r.id.clone(),
                permissions: r.permissions,
            });
        }

        let calc =
            PermissionCalculator::new(guild_id, user_id, everyone_perms, &role_defs, member_roles)
                .owner_id((!owner_id.is_empty()).then_some(owner_id))
                .is_lurker(is_lurker)
                .is_pending(is_pending);
        calc.allowed_access(&[], channel_overwrites)
    }

    pub fn compute_guild_channel_permissions(
        guild_id: &str,
        owner_id: Option<&str>,
        user_id: &str,
        member_roles: &[String],
        roles_json: &[Value],
        channels_json: &[Value],
        is_lurker: bool,
        is_pending: bool,
    ) -> std::collections::HashMap<String, AllowedChannelAccess> {
        let mut everyone_perms = PERM_VIEW_CHANNEL | PERM_READ_MESSAGE_HISTORY;
        let mut role_defs = Vec::with_capacity(roles_json.len());
        for r in roles_json {
            let Ok(role_dto) = RawRolePermissionDto::deserialize(r) else {
                continue;
            };
            let Some(role_def) = role_dto.into_role_permission_def() else {
                continue;
            };
            if role_def.id == guild_id {
                everyone_perms = role_def.permissions;
            }
            role_defs.push(role_def);
        }

        let calc =
            PermissionCalculator::new(guild_id, user_id, everyone_perms, &role_defs, member_roles)
                .owner_id(owner_id.filter(|s| !s.is_empty()))
                .is_lurker(is_lurker)
                .is_pending(is_pending);

        let mut category_overwrites_by_id: std::collections::HashMap<
            String,
            Vec<PermissionOverwriteDef>,
        > = std::collections::HashMap::new();
        for ch in channels_json {
            let ch_type = ch.get("type").and_then(Value::as_u64).unwrap_or(0);
            if ch_type == 4 {
                let cid = parse_snowflake_id(ch.get("id"));
                if !cid.is_empty() {
                    let ows = parse_overwrites_from_json(
                        ch.get("permission_overwrites")
                            .or_else(|| ch.get("permissionOverwrites")),
                    );
                    category_overwrites_by_id.insert(cid, ows);
                }
            }
        }

        let mut map = std::collections::HashMap::with_capacity(channels_json.len());
        for ch in channels_json {
            let cid = parse_snowflake_id(ch.get("id"));
            if cid.is_empty() {
                continue;
            }
            let parent_id =
                parse_snowflake_id(ch.get("parent_id").or_else(|| ch.get("parentId")));
            let cat_ows = category_overwrites_by_id
                .get(&parent_id)
                .map(Vec::as_slice)
                .unwrap_or(&[]);
            let ch_ows = parse_overwrites_from_json(
                ch.get("permission_overwrites")
                    .or_else(|| ch.get("permissionOverwrites")),
            );
            map.insert(cid, calc.allowed_access(cat_ows, &ch_ows));
        }
        map
    }
}

/// Parses a JSON array of raw Discord permission overwrites into validated [`PermissionOverwriteDef`] items
/// via Serde deserialization of [`RawPermissionOverwriteDto`].
#[inline]
#[must_use]
pub fn parse_overwrites_from_json(raw: Option<&Value>) -> Vec<PermissionOverwriteDef> {
    let Some(arr) = raw.and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut out = Vec::with_capacity(arr.len());
    for item in arr {
        if let Ok(dto) = RawPermissionOverwriteDto::deserialize(item) {
            if let Some(def) = dto.into_overwrite_def() {
                out.push(def);
            }
        }
    }
    out
}

/// Custom Serde deserializer for Snowflake IDs accepting either string or unsigned integer JSON/ETF values.
///
/// Returns an empty string if the field is null, empty, `"nil"`, `"null"`, or non-scalar.
#[inline]
pub fn deserialize_snowflake_id<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let val = Option::<Value>::deserialize(deserializer)?;
    Ok(parse_snowflake_id(val.as_ref()))
}

/// Extracts a Snowflake ID string from an optional JSON value (string or non-negative integer).
#[inline]
#[must_use]
pub fn parse_snowflake_id(val: Option<&Value>) -> String {
    match val {
        Some(Value::String(s)) => {
            let trimmed = s.trim();
            if trimmed.is_empty() || trimmed == "nil" || trimmed == "null" {
                String::new()
            } else {
                trimmed.to_string()
            }
        }
        Some(Value::Number(n)) => {
            if let Some(u) = n.as_u64() {
                u.to_string()
            } else if let Some(i) = n.as_i64() {
                if i >= 0 {
                    i.to_string()
                } else {
                    String::new()
                }
            } else {
                String::new()
            }
        }
        _ => String::new(),
    }
}

/// Custom Serde deserializer for [`OverwriteKind`] accepting numeric (`0`/`1`) or string (`"0"`/`"1"`/`"role"`/`"member"`) values.
#[inline]
pub fn deserialize_overwrite_kind<'de, D>(deserializer: D) -> Result<OverwriteKind, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let val = Option::<Value>::deserialize(deserializer)?;
    let kind = match val {
        Some(Value::Number(n)) if n.as_u64() == Some(1) => OverwriteKind::Member,
        Some(Value::String(s)) => {
            let trimmed = s.trim();
            if trimmed == "1" || trimmed.eq_ignore_ascii_case("member") {
                OverwriteKind::Member
            } else {
                OverwriteKind::Role
            }
        }
        _ => OverwriteKind::Role,
    };
    Ok(kind)
}

/// Custom Serde deserializer for a 64-bit permission bitmask (`allow`, `deny`, `permissions`) from a decimal string or `u64`.
#[inline]
pub fn deserialize_perm_bits<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let val = Option::<Value>::deserialize(deserializer)?;
    Ok(parse_perm_bits(val.as_ref()))
}

/// Parses a 64-bit permission bitfield from an optional JSON value (either numeric or decimal string).
///
/// Handles Discord REST/Gateway variations where permissions may arrive as stringified integers
/// or native JSON numbers. Returns `0` if absent or malformed.
#[inline]
#[must_use]
pub fn parse_perm_bits(val: Option<&Value>) -> u64 {
    match val {
        Some(Value::String(s)) => s.trim().parse::<u64>().unwrap_or(0),
        Some(Value::Number(n)) => n.as_u64().unwrap_or(0),
        _ => 0,
    }
}

/// Evaluates high-level channel access flags according to Discord permission semantics.
///
/// Applies `lurkerPermissionsMask` when viewing servers without membership,
/// or `TIMED_OUT_PERMISSIONS_MASK` for pending/quarantined members.
pub fn evaluate_channel_access_policy(
    effective_perms: u64,
    is_voice_channel: bool,
    is_lurker: bool,
    is_pending: bool,
) -> ChannelAccessPolicy {
    let masked = if is_lurker {
        effective_perms & LURKER_PERMISSIONS_MASK
    } else if is_pending {
        effective_perms & TIMED_OUT_PERMISSIONS_MASK
    } else {
        effective_perms
    };

    let can_view_channel = (masked & PERM_VIEW_CHANNEL) != 0;
    let can_read_history = can_view_channel && (masked & PERM_READ_MESSAGE_HISTORY) != 0;
    let can_send_messages =
        can_view_channel && !is_lurker && !is_pending && (masked & PERM_SEND_MESSAGES) != 0;
    let can_add_reactions =
        can_view_channel && !is_lurker && !is_pending && (masked & PERM_ADD_REACTIONS) != 0;
    let can_attach_files = can_send_messages && (masked & PERM_ATTACH_FILES) != 0;
    let can_connect_voice = can_view_channel
        && !is_lurker
        && !is_pending
        && (!is_voice_channel || (masked & PERM_CONNECT) != 0);
    let can_speak_voice = can_connect_voice && (masked & PERM_SPEAK) != 0;
    let can_stream_voice = can_connect_voice && (masked & PERM_STREAM) != 0;

    let read_only = !can_send_messages;
    let denied_reason = if is_lurker {
        Some("Сервер в режиме просмотра".to_string())
    } else if is_pending {
        Some("Требуется принять правила сервера".to_string())
    } else if is_voice_channel && !can_connect_voice {
        Some("Роль не даёт права Подключаться (CONNECT)".to_string())
    } else if !can_view_channel {
        Some("Нет доступа к просмотру этого канала (требуется соответствующая роль)".to_string())
    } else if !is_voice_channel && !can_send_messages {
        Some("У вас нет прав на отправку сообщений в этом канале".to_string())
    } else {
        None
    };

    ChannelAccessPolicy {
        effective_permissions: masked.to_string(),
        can_view_channel,
        can_read_history,
        can_send_messages,
        can_add_reactions,
        can_attach_files,
        can_connect_voice,
        can_speak_voice,
        can_stream_voice,
        is_lurker,
        is_pending,
        read_only,
        denied_reason,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lurker_mask_strips_send_messages_reactions_and_voice_connect() {
        let everyone = PERM_VIEW_CHANNEL
            | PERM_READ_MESSAGE_HISTORY
            | PERM_SEND_MESSAGES
            | PERM_ADD_REACTIONS
            | PERM_CONNECT
            | PERM_SPEAK;
        let calc = PermissionCalculator::new("g1", "u1", everyone, &[], &[]).is_lurker(true);
        let perms = calc.in_channel(&[], &[]);
        assert_eq!(perms, LURKER_PERMISSIONS_MASK);
        assert_ne!(perms & PERM_VIEW_CHANNEL, 0);
        assert_ne!(perms & PERM_READ_MESSAGE_HISTORY, 0);
        assert_eq!(perms & PERM_SEND_MESSAGES, 0);
        assert_eq!(perms & PERM_ADD_REACTIONS, 0);
        assert_eq!(perms & PERM_CONNECT, 0);

        let text_policy = evaluate_channel_access_policy(perms, false, true, false);
        assert!(text_policy.can_view_channel);
        assert!(!text_policy.can_send_messages);
        assert!(!text_policy.can_add_reactions);
        assert!(text_policy.read_only);

        let voice_policy = evaluate_channel_access_policy(perms, true, true, false);
        assert!(!voice_policy.can_connect_voice);
    }

    #[test]
    fn member_role_overwrites_override_everyone_deny() {
        let everyone = PERM_VIEW_CHANNEL | PERM_READ_MESSAGE_HISTORY;
        let roles = vec![RolePermissionDef {
            id: "role_verified".to_string(),
            permissions: PERM_SEND_MESSAGES | PERM_CONNECT | PERM_SPEAK,
        }];
        let member_roles = vec!["role_verified".to_string()];
        let overwrites = vec![
            PermissionOverwriteDef {
                id: "g1".to_string(),
                kind: OverwriteKind::Role,
                allow: 0,
                deny: PERM_SEND_MESSAGES,
            },
            PermissionOverwriteDef {
                id: "role_verified".to_string(),
                kind: OverwriteKind::Role,
                allow: PERM_SEND_MESSAGES,
                deny: 0,
            },
        ];

        let calc_no_role = PermissionCalculator::new("g1", "u1", everyone, &roles, &[]);
        assert_eq!(
            calc_no_role.in_channel(&[], &overwrites) & PERM_SEND_MESSAGES,
            0
        );

        let calc_with_role = PermissionCalculator::new("g1", "u1", everyone, &roles, &member_roles);
        assert_ne!(
            calc_with_role.in_channel(&[], &overwrites) & PERM_SEND_MESSAGES,
            0
        );
    }

    #[test]
    fn raw_permission_overwrite_dto_deserializes_string_and_numeric_snowflakes_and_bitmasks() {
        let json_str_id = serde_json::json!({
            "id": "1057598992449552425",
            "type": 0,
            "allow": "274877908992",
            "deny": "2048"
        });
        let dto_str: RawPermissionOverwriteDto =
            serde_json::from_value(json_str_id).expect("deserialize string snowflake");
        assert_eq!(dto_str.id, "1057598992449552425");
        assert_eq!(dto_str.kind, OverwriteKind::Role);
        assert_eq!(dto_str.allow, PERM_SEND_MESSAGES_IN_THREADS | PERM_SEND_MESSAGES);
        assert_eq!(dto_str.deny, PERM_SEND_MESSAGES);

        let json_int_id = serde_json::json!({
            "id": 999888777666555444u64,
            "type": 1,
            "allow": 3145728u64,
            "deny": 0
        });
        let dto_int: RawPermissionOverwriteDto =
            serde_json::from_value(json_int_id).expect("deserialize integer snowflake");
        assert_eq!(dto_int.id, "999888777666555444");
        assert_eq!(dto_int.kind, OverwriteKind::Member);
        assert_eq!(dto_int.allow, PERM_CONNECT | PERM_SPEAK);
        assert_eq!(dto_int.deny, 0);

        let json_kind_alias = serde_json::json!({
            "id": "777",
            "kind": "member",
            "allow": "1024",
            "deny": "0"
        });
        let dto_alias: RawPermissionOverwriteDto =
            serde_json::from_value(json_kind_alias).expect("deserialize kind alias");
        assert_eq!(dto_alias.id, "777");
        assert_eq!(dto_alias.kind, OverwriteKind::Member);
        assert_eq!(dto_alias.allow, PERM_VIEW_CHANNEL);

        let empty_dto: RawPermissionOverwriteDto =
            serde_json::from_value(serde_json::json!({ "id": "nil", "type": 0 }))
                .expect("deserialize nil id");
        assert!(empty_dto.into_overwrite_def().is_none());
    }

    #[test]
    fn parse_overwrites_from_json_and_guild_channel_permissions_support_etf_numeric_ids() {
        let raw_overwrites = serde_json::json!([
            {
                "id": 100000000000000001u64,
                "type": "0",
                "allow": "0",
                "deny": "2048"
            },
            {
                "id": "200000000000000002",
                "type": "1",
                "allow": 2048,
                "deny": 0
            },
            {
                "id": "",
                "type": 0,
                "allow": 1024,
                "deny": 0
            }
        ]);
        let parsed = parse_overwrites_from_json(Some(&raw_overwrites));
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].id, "100000000000000001");
        assert_eq!(parsed[0].kind, OverwriteKind::Role);
        assert_eq!(parsed[0].deny, PERM_SEND_MESSAGES);
        assert_eq!(parsed[1].id, "200000000000000002");
        assert_eq!(parsed[1].kind, OverwriteKind::Member);
        assert_eq!(parsed[1].allow, PERM_SEND_MESSAGES);

        let roles_json = vec![serde_json::json!({
            "id": 100000000000000001u64,
            "permissions": "68608"
        })];
        let channels_json = vec![serde_json::json!({
            "id": 300000000000000003u64,
            "type": 0,
            "permission_overwrites": raw_overwrites
        })];
        let perms_map = PermissionCalculator::compute_guild_channel_permissions(
            "100000000000000001",
            None,
            "200000000000000002",
            &[],
            &roles_json,
            &channels_json,
            false,
            false,
        );
        let access = perms_map
            .get("300000000000000003")
            .expect("channel access computed");
        assert!(access.can_view);
        assert!(access.can_send);
    }
}
