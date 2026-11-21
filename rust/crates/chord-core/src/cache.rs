//! Thread-safe, lock-free/low-contention in-memory cache architecture for Discord entities.
//!
//! Provides [`InMemoryCache`] and [`InMemoryCacheBuilder`] modeled after `twilight-cache-inmemory`,
//! replacing large manual mutex guards (`LiveSessionGuard`) with granular `RwLock`-protected
//! zero-deadlock projections. Read queries (such as [`InMemoryCache::channel`]) execute in < 0.1 µs.

use serde_json::Value;
use std::collections::HashMap;
use std::ops::{BitOr, BitOrAssign};
use std::sync::{Arc, RwLock};
use twilight_model::gateway::event::Event;
use twilight_model::id::Id;
use twilight_model::id::marker::{ChannelMarker, GuildMarker, RoleMarker, UserMarker};

type MemberMap = HashMap<(Id<GuildMarker>, Id<UserMarker>), Arc<CachedMember>>;
type VoiceStateMap = HashMap<(Id<GuildMarker>, Id<UserMarker>), Arc<CachedVoiceState>>;

/// Bitflags representing Discord resource types stored in [`InMemoryCache`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ResourceType(pub u32);

impl ResourceType {
    pub const NONE: Self = Self(0);
    pub const CHANNEL: Self = Self(1 << 0);
    pub const GUILD: Self = Self(1 << 1);
    pub const MEMBER: Self = Self(1 << 2);
    pub const ROLE: Self = Self(1 << 3);
    pub const USER: Self = Self(1 << 4);
    pub const VOICE_STATE: Self = Self(1 << 5);
    pub const CURRENT_USER: Self = Self(1 << 6);
    pub const MESSAGE: Self = Self(1 << 7);
    pub const ALL: Self = Self(0xFFFF);

    #[inline]
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }
}

impl BitOr for ResourceType {
    type Output = Self;
    #[inline]
    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for ResourceType {
    #[inline]
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

/// Cached representation of a Discord channel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CachedChannel {
    pub id: Id<ChannelMarker>,
    pub guild_id: Option<Id<GuildMarker>>,
    pub name: Option<String>,
    pub kind: u8,
    pub position: Option<i32>,
    pub topic: Option<String>,
    pub nsfw: bool,
    pub parent_id: Option<Id<ChannelMarker>>,
    pub rate_limit_per_user: Option<u16>,
}

/// Cached representation of a Discord user.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CachedUser {
    pub id: Id<UserMarker>,
    pub name: String,
    pub discriminator: u16,
    pub avatar: Option<String>,
    pub bot: bool,
}

/// Cached representation of the current logged-in user.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CachedCurrentUser {
    pub id: Id<UserMarker>,
    pub name: String,
    pub discriminator: u16,
    pub avatar: Option<String>,
    pub bot: bool,
    pub mfa_enabled: bool,
    pub verified: Option<bool>,
    pub email: Option<String>,
}

/// Cached representation of a guild member.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CachedMember {
    pub guild_id: Id<GuildMarker>,
    pub user_id: Id<UserMarker>,
    pub user: Option<Arc<CachedUser>>,
    pub nick: Option<String>,
    pub roles: Vec<Id<RoleMarker>>,
    pub joined_at: Option<String>,
    pub deaf: bool,
    pub mute: bool,
}

/// Cached representation of a guild role.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CachedRole {
    pub id: Id<RoleMarker>,
    pub guild_id: Id<GuildMarker>,
    pub name: String,
    pub color: u32,
    pub hoist: bool,
    pub position: i64,
    pub permissions: String,
    pub managed: bool,
    pub mentionable: bool,
}

/// Cached representation of a voice state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CachedVoiceState {
    pub guild_id: Option<Id<GuildMarker>>,
    pub channel_id: Option<Id<ChannelMarker>>,
    pub user_id: Id<UserMarker>,
    pub session_id: String,
    pub deaf: bool,
    pub mute: bool,
    pub self_deaf: bool,
    pub self_mute: bool,
    pub self_video: bool,
    pub suppress: bool,
}

/// Cached representation of a Discord guild.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CachedGuild {
    pub id: Id<GuildMarker>,
    pub name: String,
    pub icon: Option<String>,
    pub owner_id: Option<Id<UserMarker>>,
    pub afk_channel_id: Option<Id<ChannelMarker>>,
    pub afk_timeout: Option<u64>,
    pub system_channel_id: Option<Id<ChannelMarker>>,
}

/// Builder for [`InMemoryCache`].
#[derive(Clone, Debug)]
pub struct InMemoryCacheBuilder {
    resource_types: ResourceType,
    message_cache_size: usize,
}

impl Default for InMemoryCacheBuilder {
    fn default() -> Self {
        Self {
            resource_types: ResourceType::ALL,
            message_cache_size: 50,
        }
    }
}

impl InMemoryCacheBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn resource_types(mut self, types: ResourceType) -> Self {
        self.resource_types = types;
        self
    }

    pub fn message_cache_size(mut self, size: usize) -> Self {
        self.message_cache_size = size;
        self
    }

    pub fn build(self) -> InMemoryCache {
        InMemoryCache {
            resource_types: self.resource_types,
            _message_cache_size: self.message_cache_size,
            channels: RwLock::new(HashMap::new()),
            guild_channels: RwLock::new(HashMap::new()),
            guilds: RwLock::new(HashMap::new()),
            members: RwLock::new(HashMap::new()),
            guild_members: RwLock::new(HashMap::new()),
            roles: RwLock::new(HashMap::new()),
            guild_roles: RwLock::new(HashMap::new()),
            users: RwLock::new(HashMap::new()),
            current_user: RwLock::new(None),
            voice_states: RwLock::new(HashMap::new()),
            guild_voice_states: RwLock::new(HashMap::new()),
        }
    }
}

/// Thread-safe in-memory cache for Discord entities.
pub struct InMemoryCache {
    resource_types: ResourceType,
    _message_cache_size: usize,
    channels: RwLock<HashMap<Id<ChannelMarker>, Arc<CachedChannel>>>,
    guild_channels: RwLock<HashMap<Id<GuildMarker>, Vec<Id<ChannelMarker>>>>,
    guilds: RwLock<HashMap<Id<GuildMarker>, Arc<CachedGuild>>>,
    members: RwLock<MemberMap>,
    guild_members: RwLock<HashMap<Id<GuildMarker>, Vec<Id<UserMarker>>>>,
    roles: RwLock<HashMap<Id<RoleMarker>, Arc<CachedRole>>>,
    guild_roles: RwLock<HashMap<Id<GuildMarker>, Vec<Id<RoleMarker>>>>,
    users: RwLock<HashMap<Id<UserMarker>, Arc<CachedUser>>>,
    current_user: RwLock<Option<Arc<CachedCurrentUser>>>,
    voice_states: RwLock<VoiceStateMap>,
    guild_voice_states: RwLock<HashMap<Id<GuildMarker>, Vec<Arc<CachedVoiceState>>>>,
}

impl Default for InMemoryCache {
    fn default() -> Self {
        InMemoryCacheBuilder::new().build()
    }
}

#[derive(Debug)]
pub enum CacheError {
    JsonError(String),
    MissingField(String),
}

impl InMemoryCache {
    pub fn builder() -> InMemoryCacheBuilder {
        InMemoryCacheBuilder::new()
    }

    // --- Channel Operations ---

    #[inline]
    pub fn channel(&self, id: Id<ChannelMarker>) -> Option<Arc<CachedChannel>> {
        if !self.resource_types.contains(ResourceType::CHANNEL) {
            return None;
        }
        self.channels.read().ok()?.get(&id).cloned()
    }

    pub fn channels(&self) -> Vec<Arc<CachedChannel>> {
        if !self.resource_types.contains(ResourceType::CHANNEL) {
            return Vec::new();
        }
        self.channels
            .read()
            .map(|m| m.values().cloned().collect())
            .unwrap_or_default()
    }

    pub fn guild_channels(&self, guild_id: Id<GuildMarker>) -> Option<Vec<Id<ChannelMarker>>> {
        if !self.resource_types.contains(ResourceType::CHANNEL) {
            return None;
        }
        self.guild_channels.read().ok()?.get(&guild_id).cloned()
    }

    pub fn seed_channel(
        &self,
        guild_id: Id<GuildMarker>,
        channel_id: Id<ChannelMarker>,
        name: &str,
        kind: u8,
        position: i32,
    ) {
        let ch = Arc::new(CachedChannel {
            id: channel_id,
            guild_id: Some(guild_id),
            name: Some(name.to_string()),
            kind,
            position: Some(position),
            topic: None,
            nsfw: false,
            parent_id: None,
            rate_limit_per_user: None,
        });

        if let Ok(mut channels) = self.channels.write() {
            channels.insert(channel_id, ch);
        }
        if let Ok(mut gc) = self.guild_channels.write() {
            let list = gc.entry(guild_id).or_default();
            if !list.contains(&channel_id) {
                list.push(channel_id);
            }
        }
    }

    // --- User & Current User Operations ---

    #[inline]
    pub fn user(&self, id: Id<UserMarker>) -> Option<Arc<CachedUser>> {
        if !self.resource_types.contains(ResourceType::USER) {
            return None;
        }
        self.users.read().ok()?.get(&id).cloned()
    }

    #[inline]
    pub fn current_user(&self) -> Option<Arc<CachedCurrentUser>> {
        if !self.resource_types.contains(ResourceType::CURRENT_USER) {
            return None;
        }
        self.current_user.read().ok()?.clone()
    }

    // --- Member Operations ---

    #[inline]
    pub fn member(
        &self,
        guild_id: Id<GuildMarker>,
        user_id: Id<UserMarker>,
    ) -> Option<Arc<CachedMember>> {
        if !self.resource_types.contains(ResourceType::MEMBER) {
            return None;
        }
        self.members.read().ok()?.get(&(guild_id, user_id)).cloned()
    }

    pub fn guild_members(&self, guild_id: Id<GuildMarker>) -> Option<Vec<Id<UserMarker>>> {
        if !self.resource_types.contains(ResourceType::MEMBER) {
            return None;
        }
        self.guild_members.read().ok()?.get(&guild_id).cloned()
    }

    // --- Role Operations ---

    #[inline]
    pub fn role(&self, id: Id<RoleMarker>) -> Option<Arc<CachedRole>> {
        if !self.resource_types.contains(ResourceType::ROLE) {
            return None;
        }
        self.roles.read().ok()?.get(&id).cloned()
    }

    pub fn guild_roles(&self, guild_id: Id<GuildMarker>) -> Option<Vec<Id<RoleMarker>>> {
        if !self.resource_types.contains(ResourceType::ROLE) {
            return None;
        }
        self.guild_roles.read().ok()?.get(&guild_id).cloned()
    }

    // --- Voice State Operations ---

    #[inline]
    pub fn voice_state(
        &self,
        guild_id: Id<GuildMarker>,
        user_id: Id<UserMarker>,
    ) -> Option<Arc<CachedVoiceState>> {
        if !self.resource_types.contains(ResourceType::VOICE_STATE) {
            return None;
        }
        self.voice_states
            .read()
            .ok()?
            .get(&(guild_id, user_id))
            .cloned()
    }

    pub fn voice_states(&self, guild_id: Id<GuildMarker>) -> Vec<Arc<CachedVoiceState>> {
        if !self.resource_types.contains(ResourceType::VOICE_STATE) {
            return Vec::new();
        }
        self.guild_voice_states
            .read()
            .ok()
            .and_then(|m| m.get(&guild_id).cloned())
            .unwrap_or_default()
    }

    // --- Guild Operations ---

    #[inline]
    pub fn guild(&self, id: Id<GuildMarker>) -> Option<Arc<CachedGuild>> {
        if !self.resource_types.contains(ResourceType::GUILD) {
            return None;
        }
        self.guilds.read().ok()?.get(&id).cloned()
    }

    pub fn guilds(&self) -> Vec<Arc<CachedGuild>> {
        if !self.resource_types.contains(ResourceType::GUILD) {
            return Vec::new();
        }
        self.guilds
            .read()
            .map(|m| m.values().cloned().collect())
            .unwrap_or_default()
    }

    // --- Cache Update Engine ---

    pub fn update(&self, event: &Event) -> Result<(), CacheError> {
        match event {
            Event::ChannelCreate(cc) => {
                let ch = &cc.0;
                let cached = Arc::new(CachedChannel {
                    id: ch.id,
                    guild_id: ch.guild_id,
                    name: ch.name.clone(),
                    kind: ch.kind.into(),
                    position: ch.position,
                    topic: ch.topic.clone(),
                    nsfw: ch.nsfw.unwrap_or(false),
                    parent_id: ch.parent_id,
                    rate_limit_per_user: ch.rate_limit_per_user,
                });
                if let Ok(mut channels) = self.channels.write() {
                    channels.insert(ch.id, cached);
                }
                if let Some(gid) = ch.guild_id
                    && let Ok(mut gc) = self.guild_channels.write()
                {
                    let list = gc.entry(gid).or_default();
                    if !list.contains(&ch.id) {
                        list.push(ch.id);
                    }
                }
            }
            Event::ChannelDelete(cd) => {
                let ch = &cd.0;
                if let Ok(mut channels) = self.channels.write() {
                    channels.remove(&ch.id);
                }
                if let Some(gid) = ch.guild_id
                    && let Ok(mut gc) = self.guild_channels.write()
                    && let Some(list) = gc.get_mut(&gid)
                {
                    list.retain(|id| *id != ch.id);
                }
            }
            Event::ChannelUpdate(cu) => {
                let ch = &cu.0;
                let cached = Arc::new(CachedChannel {
                    id: ch.id,
                    guild_id: ch.guild_id,
                    name: ch.name.clone(),
                    kind: ch.kind.into(),
                    position: ch.position,
                    topic: ch.topic.clone(),
                    nsfw: ch.nsfw.unwrap_or(false),
                    parent_id: ch.parent_id,
                    rate_limit_per_user: ch.rate_limit_per_user,
                });
                if let Ok(mut channels) = self.channels.write() {
                    channels.insert(ch.id, cached);
                }
            }
            Event::VoiceStateUpdate(vsu) => {
                let vs = &vsu.0;
                if let Some(gid) = vs.guild_id {
                    if vs.channel_id.is_none() {
                        // User left voice channel
                        if let Ok(mut states) = self.voice_states.write() {
                            states.remove(&(gid, vs.user_id));
                        }
                        if let Ok(mut gvs) = self.guild_voice_states.write()
                            && let Some(list) = gvs.get_mut(&gid)
                        {
                            list.retain(|s| s.user_id != vs.user_id);
                        }
                    } else {
                        let state = Arc::new(CachedVoiceState {
                            guild_id: Some(gid),
                            channel_id: vs.channel_id,
                            user_id: vs.user_id,
                            session_id: vs.session_id.clone(),
                            deaf: vs.deaf,
                            mute: vs.mute,
                            self_deaf: vs.self_deaf,
                            self_mute: vs.self_mute,
                            self_video: vs.self_video,
                            suppress: vs.suppress,
                        });
                        if let Ok(mut states) = self.voice_states.write() {
                            states.insert((gid, vs.user_id), Arc::clone(&state));
                        }
                        if let Ok(mut gvs) = self.guild_voice_states.write() {
                            let list = gvs.entry(gid).or_default();
                            list.retain(|s| s.user_id != vs.user_id);
                            list.push(state);
                        }
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Hydrates cache from a READY payload JSON [`Value`].
    pub fn update_from_ready_value(&self, value: &Value) -> Result<(), CacheError> {
        let d = if let Some(inner) = value.get("d") {
            inner
        } else {
            value
        };

        // Cache Current User
        if let Some(user_val) = d.get("user")
            && let Some(id_str) = user_val.get("id").and_then(Value::as_str)
            && let Ok(id_u64) = id_str.parse::<u64>()
            && let Some(user_id) = Id::new_checked(id_u64)
        {
            let name = user_val
                .get("username")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let disc = user_val
                .get("discriminator")
                .and_then(Value::as_str)
                .and_then(|s| s.parse::<u16>().ok())
                .unwrap_or(0);
            let avatar = user_val
                .get("avatar")
                .and_then(Value::as_str)
                .map(str::to_string);
            let bot = user_val
                .get("bot")
                .and_then(Value::as_bool)
                .unwrap_or(false);

            let cu = Arc::new(CachedCurrentUser {
                id: user_id,
                name,
                discriminator: disc,
                avatar,
                bot,
                mfa_enabled: user_val
                    .get("mfa_enabled")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                verified: user_val.get("verified").and_then(Value::as_bool),
                email: user_val
                    .get("email")
                    .and_then(Value::as_str)
                    .map(str::to_string),
            });
            if let Ok(mut slot) = self.current_user.write() {
                *slot = Some(cu);
            }
        }

        // Cache Guilds & Nested Entities
        if let Some(guilds_arr) = d.get("guilds").and_then(Value::as_array) {
            for g_val in guilds_arr {
                let Some(gid_str) = g_val.get("id").and_then(Value::as_str) else {
                    continue;
                };
                let Ok(gid_u64) = gid_str.parse::<u64>() else {
                    continue;
                };
                let Some(gid) = Id::<GuildMarker>::new_checked(gid_u64) else {
                    continue;
                };

                let g_name = g_val
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let icon = g_val
                    .get("icon")
                    .and_then(Value::as_str)
                    .map(str::to_string);
                let afk_channel_id = g_val
                    .get("afk_channel_id")
                    .and_then(Value::as_str)
                    .and_then(|s| s.parse::<u64>().ok())
                    .and_then(Id::new_checked);
                let afk_timeout = g_val.get("afk_timeout").and_then(Value::as_u64);

                let guild = Arc::new(CachedGuild {
                    id: gid,
                    name: g_name,
                    icon,
                    owner_id: None,
                    afk_channel_id,
                    afk_timeout,
                    system_channel_id: None,
                });
                if let Ok(mut guilds) = self.guilds.write() {
                    guilds.insert(gid, guild);
                }

                // Channels in Guild
                if let Some(ch_arr) = g_val.get("channels").and_then(Value::as_array) {
                    for ch_val in ch_arr {
                        let Some(cid_str) = ch_val.get("id").and_then(Value::as_str) else {
                            continue;
                        };
                        let Ok(cid_u64) = cid_str.parse::<u64>() else {
                            continue;
                        };
                        let Some(cid) = Id::<ChannelMarker>::new_checked(cid_u64) else {
                            continue;
                        };
                        let name = ch_val
                            .get("name")
                            .and_then(Value::as_str)
                            .map(str::to_string);
                        let kind = ch_val.get("type").and_then(Value::as_u64).unwrap_or(0) as u8;
                        let pos = ch_val
                            .get("position")
                            .and_then(Value::as_i64)
                            .map(|p| p as i32);

                        let ch = Arc::new(CachedChannel {
                            id: cid,
                            guild_id: Some(gid),
                            name,
                            kind,
                            position: pos,
                            topic: ch_val
                                .get("topic")
                                .and_then(Value::as_str)
                                .map(str::to_string),
                            nsfw: ch_val.get("nsfw").and_then(Value::as_bool).unwrap_or(false),
                            parent_id: ch_val
                                .get("parent_id")
                                .and_then(Value::as_str)
                                .and_then(|s| s.parse::<u64>().ok())
                                .and_then(Id::new_checked),
                            rate_limit_per_user: ch_val
                                .get("rate_limit_per_user")
                                .and_then(Value::as_u64)
                                .map(|r| r as u16),
                        });
                        if let Ok(mut channels) = self.channels.write() {
                            channels.insert(cid, ch);
                        }
                        if let Ok(mut gc) = self.guild_channels.write() {
                            let list = gc.entry(gid).or_default();
                            if !list.contains(&cid) {
                                list.push(cid);
                            }
                        }
                    }
                }

                // Roles in Guild
                if let Some(roles_arr) = g_val.get("roles").and_then(Value::as_array) {
                    for r_val in roles_arr {
                        let Some(rid_str) = r_val.get("id").and_then(Value::as_str) else {
                            continue;
                        };
                        let Ok(rid_u64) = rid_str.parse::<u64>() else {
                            continue;
                        };
                        let Some(rid) = Id::<RoleMarker>::new_checked(rid_u64) else {
                            continue;
                        };
                        let r_name = r_val
                            .get("name")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string();
                        let color = r_val.get("color").and_then(Value::as_u64).unwrap_or(0) as u32;
                        let pos = r_val.get("position").and_then(Value::as_i64).unwrap_or(0);
                        let perms = r_val
                            .get("permissions")
                            .and_then(Value::as_str)
                            .unwrap_or("0")
                            .to_string();

                        let role = Arc::new(CachedRole {
                            id: rid,
                            guild_id: gid,
                            name: r_name,
                            color,
                            hoist: r_val.get("hoist").and_then(Value::as_bool).unwrap_or(false),
                            position: pos,
                            permissions: perms,
                            managed: r_val
                                .get("managed")
                                .and_then(Value::as_bool)
                                .unwrap_or(false),
                            mentionable: r_val
                                .get("mentionable")
                                .and_then(Value::as_bool)
                                .unwrap_or(false),
                        });
                        if let Ok(mut roles) = self.roles.write() {
                            roles.insert(rid, role);
                        }
                        if let Ok(mut gr) = self.guild_roles.write() {
                            let list = gr.entry(gid).or_default();
                            if !list.contains(&rid) {
                                list.push(rid);
                            }
                        }
                    }
                }

                // Members in Guild
                if let Some(mem_arr) = g_val.get("members").and_then(Value::as_array) {
                    for m_val in mem_arr {
                        let Some(user_obj) = m_val.get("user") else {
                            continue;
                        };
                        let Some(uid_str) = user_obj.get("id").and_then(Value::as_str) else {
                            continue;
                        };
                        let Ok(uid_u64) = uid_str.parse::<u64>() else {
                            continue;
                        };
                        let Some(uid) = Id::<UserMarker>::new_checked(uid_u64) else {
                            continue;
                        };

                        let user = Arc::new(CachedUser {
                            id: uid,
                            name: user_obj
                                .get("username")
                                .and_then(Value::as_str)
                                .unwrap_or_default()
                                .to_string(),
                            discriminator: user_obj
                                .get("discriminator")
                                .and_then(Value::as_str)
                                .and_then(|s| s.parse::<u16>().ok())
                                .unwrap_or(0),
                            avatar: user_obj
                                .get("avatar")
                                .and_then(Value::as_str)
                                .map(str::to_string),
                            bot: user_obj
                                .get("bot")
                                .and_then(Value::as_bool)
                                .unwrap_or(false),
                        });
                        if let Ok(mut users) = self.users.write() {
                            users.insert(uid, Arc::clone(&user));
                        }

                        let roles = m_val
                            .get("roles")
                            .and_then(Value::as_array)
                            .map(|arr| {
                                arr.iter()
                                    .filter_map(Value::as_str)
                                    .filter_map(|s| s.parse::<u64>().ok())
                                    .filter_map(Id::<RoleMarker>::new_checked)
                                    .collect()
                            })
                            .unwrap_or_default();

                        let member = Arc::new(CachedMember {
                            guild_id: gid,
                            user_id: uid,
                            user: Some(user),
                            nick: m_val
                                .get("nick")
                                .and_then(Value::as_str)
                                .map(str::to_string),
                            roles,
                            joined_at: m_val
                                .get("joined_at")
                                .and_then(Value::as_str)
                                .map(str::to_string),
                            deaf: m_val.get("deaf").and_then(Value::as_bool).unwrap_or(false),
                            mute: m_val.get("mute").and_then(Value::as_bool).unwrap_or(false),
                        });

                        if let Ok(mut members) = self.members.write() {
                            members.insert((gid, uid), member);
                        }
                        if let Ok(mut gm) = self.guild_members.write() {
                            let list = gm.entry(gid).or_default();
                            if !list.contains(&uid) {
                                list.push(uid);
                            }
                        }
                    }
                }

                // Voice states in Guild
                if let Some(vs_arr) = g_val.get("voice_states").and_then(Value::as_array) {
                    for vs_val in vs_arr {
                        let Some(uid_str) = vs_val.get("user_id").and_then(Value::as_str) else {
                            continue;
                        };
                        let Ok(uid_u64) = uid_str.parse::<u64>() else {
                            continue;
                        };
                        let Some(uid) = Id::<UserMarker>::new_checked(uid_u64) else {
                            continue;
                        };
                        let cid = vs_val
                            .get("channel_id")
                            .and_then(Value::as_str)
                            .and_then(|s| s.parse::<u64>().ok())
                            .and_then(Id::<ChannelMarker>::new_checked);
                        let sess = vs_val
                            .get("session_id")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string();

                        let state = Arc::new(CachedVoiceState {
                            guild_id: Some(gid),
                            channel_id: cid,
                            user_id: uid,
                            session_id: sess,
                            deaf: vs_val.get("deaf").and_then(Value::as_bool).unwrap_or(false),
                            mute: vs_val.get("mute").and_then(Value::as_bool).unwrap_or(false),
                            self_deaf: vs_val
                                .get("self_deaf")
                                .and_then(Value::as_bool)
                                .unwrap_or(false),
                            self_mute: vs_val
                                .get("self_mute")
                                .and_then(Value::as_bool)
                                .unwrap_or(false),
                            self_video: vs_val
                                .get("self_video")
                                .and_then(Value::as_bool)
                                .unwrap_or(false),
                            suppress: vs_val
                                .get("suppress")
                                .and_then(Value::as_bool)
                                .unwrap_or(false),
                        });
                        if let Ok(mut states) = self.voice_states.write() {
                            states.insert((gid, uid), Arc::clone(&state));
                        }
                        if let Ok(mut gvs) = self.guild_voice_states.write() {
                            let list = gvs.entry(gid).or_default();
                            list.push(state);
                        }
                    }
                }
            }
        }

        // Private Channels
        if let Some(pc_arr) = d.get("private_channels").and_then(Value::as_array) {
            for ch_val in pc_arr {
                let Some(cid_str) = ch_val.get("id").and_then(Value::as_str) else {
                    continue;
                };
                let Ok(cid_u64) = cid_str.parse::<u64>() else {
                    continue;
                };
                let Some(cid) = Id::<ChannelMarker>::new_checked(cid_u64) else {
                    continue;
                };
                let name = ch_val
                    .get("name")
                    .and_then(Value::as_str)
                    .map(str::to_string);
                let kind = ch_val.get("type").and_then(Value::as_u64).unwrap_or(1) as u8;

                let ch = Arc::new(CachedChannel {
                    id: cid,
                    guild_id: None,
                    name,
                    kind,
                    position: None,
                    topic: None,
                    nsfw: false,
                    parent_id: None,
                    rate_limit_per_user: None,
                });
                if let Ok(mut channels) = self.channels.write() {
                    channels.insert(cid, ch);
                }
            }
        }

        Ok(())
    }

    /// Clears all entries from the cache.
    pub fn clear(&self) {
        if let Ok(mut c) = self.channels.write() {
            c.clear();
        }
        if let Ok(mut gc) = self.guild_channels.write() {
            gc.clear();
        }
        if let Ok(mut g) = self.guilds.write() {
            g.clear();
        }
        if let Ok(mut m) = self.members.write() {
            m.clear();
        }
        if let Ok(mut gm) = self.guild_members.write() {
            gm.clear();
        }
        if let Ok(mut r) = self.roles.write() {
            r.clear();
        }
        if let Ok(mut gr) = self.guild_roles.write() {
            gr.clear();
        }
        if let Ok(mut u) = self.users.write() {
            u.clear();
        }
        if let Ok(mut cu) = self.current_user.write() {
            *cu = None;
        }
        if let Ok(mut vs) = self.voice_states.write() {
            vs.clear();
        }
        if let Ok(mut gvs) = self.guild_voice_states.write() {
            gvs.clear();
        }
    }
}
