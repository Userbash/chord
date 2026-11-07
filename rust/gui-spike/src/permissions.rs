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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OverwriteKind {
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
            perms &= !(PERM_SEND_TTS_MESSAGES | PERM_MENTION_EVERYONE | PERM_ATTACH_FILES | PERM_EMBED_LINKS);
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
        let can_send =
            can_view && !self.is_lurker && !self.is_pending && (effective_permissions & PERM_SEND_MESSAGES) != 0;
        let can_connect =
            can_view && !self.is_lurker && !self.is_pending && (effective_permissions & PERM_CONNECT) != 0;
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

        let calc = PermissionCalculator::new(guild_id, user_id, everyone_perms, &role_defs, member_roles)
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
            let rid = r.get("id").and_then(Value::as_str).unwrap_or_default();
            if rid.is_empty() {
                continue;
            }
            let perms = parse_perm_bits(r.get("permissions"));
            if rid == guild_id {
                everyone_perms = perms;
            }
            role_defs.push(RolePermissionDef {
                id: rid.to_string(),
                permissions: perms,
            });
        }

        let calc = PermissionCalculator::new(guild_id, user_id, everyone_perms, &role_defs, member_roles)
            .owner_id(owner_id.filter(|s| !s.is_empty()))
            .is_lurker(is_lurker)
            .is_pending(is_pending);

        let mut category_overwrites_by_id: std::collections::HashMap<String, Vec<PermissionOverwriteDef>> =
            std::collections::HashMap::new();
        for ch in channels_json {
            let ch_type = ch.get("type").and_then(Value::as_u64).unwrap_or(0);
            if ch_type == 4 {
                if let Some(cid) = ch.get("id").and_then(Value::as_str) {
                    let ows = parse_overwrites_from_json(
                        ch.get("permission_overwrites")
                            .or_else(|| ch.get("permissionOverwrites")),
                    );
                    category_overwrites_by_id.insert(cid.to_string(), ows);
                }
            }
        }

        let mut map = std::collections::HashMap::with_capacity(channels_json.len());
        for ch in channels_json {
            let Some(cid) = ch.get("id").and_then(Value::as_str) else {
                continue;
            };
            let parent_id = ch
                .get("parent_id")
                .or_else(|| ch.get("parentId"))
                .and_then(Value::as_str)
                .unwrap_or_default();
            let cat_ows = category_overwrites_by_id
                .get(parent_id)
                .map(Vec::as_slice)
                .unwrap_or(&[]);
            let ch_ows = parse_overwrites_from_json(
                ch.get("permission_overwrites")
                    .or_else(|| ch.get("permissionOverwrites")),
            );
            map.insert(cid.to_string(), calc.allowed_access(cat_ows, &ch_ows));
        }
        map
    }
}

pub fn parse_overwrites_from_json(raw: Option<&Value>) -> Vec<PermissionOverwriteDef> {
    let Some(arr) = raw.and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut out = Vec::with_capacity(arr.len());
    for item in arr {
        let id = item
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        if id.is_empty() {
            continue;
        }
        let kind = match item.get("type") {
            Some(Value::Number(n)) if n.as_u64() == Some(1) => OverwriteKind::Member,
            Some(Value::String(s)) if s == "1" || s.eq_ignore_ascii_case("member") => {
                OverwriteKind::Member
            }
            _ => OverwriteKind::Role,
        };
        let allow = parse_perm_bits(item.get("allow"));
        let deny = parse_perm_bits(item.get("deny"));
        out.push(PermissionOverwriteDef {
            id,
            kind,
            allow,
            deny,
        });
    }
    out
}

pub fn parse_perm_bits(val: Option<&Value>) -> u64 {
    match val {
        Some(Value::String(s)) => s.trim().parse::<u64>().unwrap_or(0),
        Some(Value::Number(n)) => n.as_u64().unwrap_or(0),
        _ => 0,
    }
}

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
    let can_send_messages = can_view_channel && !is_lurker && !is_pending && (masked & PERM_SEND_MESSAGES) != 0;
    let can_add_reactions = can_view_channel && !is_lurker && !is_pending && (masked & PERM_ADD_REACTIONS) != 0;
    let can_attach_files = can_send_messages && (masked & PERM_ATTACH_FILES) != 0;
    let can_connect_voice =
        can_view_channel && !is_lurker && !is_pending && (!is_voice_channel || (masked & PERM_CONNECT) != 0);
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
        let everyone = PERM_VIEW_CHANNEL | PERM_READ_MESSAGE_HISTORY | PERM_SEND_MESSAGES | PERM_ADD_REACTIONS | PERM_CONNECT | PERM_SPEAK;
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
        assert_eq!(calc_no_role.in_channel(&[], &overwrites) & PERM_SEND_MESSAGES, 0);

        let calc_with_role = PermissionCalculator::new("g1", "u1", everyone, &roles, &member_roles);
        assert_ne!(calc_with_role.in_channel(&[], &overwrites) & PERM_SEND_MESSAGES, 0);
    }
}
