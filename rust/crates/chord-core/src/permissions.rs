//! Discord API v9 permission bitmask and channel overwrite calculator (`RW51-EPIC3-T3`).
//!
//! Powered by `twilight_util::permission_calculator::PermissionCalculator` and
//! `twilight_model::guild::Permissions`, providing full 64-bit permission resolution,
//! guild owner and `ADMINISTRATOR` bypass, 3-tier channel overwrite hierarchy
//! (`@everyone` -> `roles` -> `member`), and member restriction masks
//! (`lurker`, `pending`, `timed_out`, `quarantined`).

use std::fmt;
use std::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Not};
use std::str::FromStr;

pub use twilight_model::channel::{
    ChannelType as TwilightChannelType,
    permission_overwrite::{
        PermissionOverwrite as TwilightPermissionOverwrite, PermissionOverwriteType,
    },
};
pub use twilight_model::guild::Permissions as TwilightPermissions;
use twilight_model::id::{
    Id,
    marker::{GenericMarker, GuildMarker, RoleMarker, UserMarker},
};
pub use twilight_util::permission_calculator::PermissionCalculator;

/// Full 64-bit permissions mask (returned on guild owner or `ADMINISTRATOR` bypass).
pub const ALL_PERMISSIONS: u64 = u64::MAX;

/// Mask of all 41 standard Discord API v9 flags (`0..=40`).
pub const ALL_V9_DEFINED_PERMISSIONS: u64 = (1u64 << 41) - 1;

/// Channel overwrite target type: role (`0`).
pub const OVERWRITE_TYPE_ROLE: u8 = 0;

/// Channel overwrite target type: member (`1`).
pub const OVERWRITE_TYPE_MEMBER: u8 = 1;

/// Official `lurkerPermissionsMask` (`VIEW_CHANNEL | READ_MESSAGE_HISTORY = 66560`).
pub const LURKER_PERMISSIONS_MASK: u64 = VIEW_CHANNEL | READ_MESSAGE_HISTORY;

/// Permission mask applied to timed-out guild members (`66560`).
pub const TIMED_OUT_PERMISSIONS_MASK: u64 = VIEW_CHANNEL | READ_MESSAGE_HISTORY;

/// Permission mask applied to AutoMod-quarantined guild members (`67175424`).
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

/// 32 non-root, non-conditional carrier bit positions in [`TwilightPermissions`] used to
/// evaluate arbitrary 32-bit permission slices via [`PermissionCalculator::in_channel`].
const SAFE_CARRIER_BITS: [u64; 32] = [
    1 << 0,
    1 << 4,
    1 << 6,
    1 << 8,
    1 << 9,
    1 << 12,
    1 << 13,
    1 << 14,
    1 << 15,
    1 << 16,
    1 << 17,
    1 << 18,
    1 << 20,
    1 << 21,
    1 << 22,
    1 << 23,
    1 << 24,
    1 << 25,
    1 << 28,
    1 << 29,
    1 << 31,
    1 << 32,
    1 << 33,
    1 << 34,
    1 << 35,
    1 << 36,
    1 << 37,
    1 << 38,
    1 << 39,
    1 << 40,
    1 << 41,
    1 << 42,
];

/// 64-bit Discord v9 permission bitfield wrapper.
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

    /// Creates a `Permissions` wrapper from raw 64-bit flags.
    #[must_use]
    pub const fn from_bits(bits: u64) -> Self {
        Self(bits)
    }

    /// Returns the underlying 64-bit mask.
    #[must_use]
    pub const fn bits(self) -> u64 {
        self.0
    }

    /// Checks whether `required_perm` is granted (including `ADMINISTRATOR` bypass).
    #[must_use]
    pub const fn has(self, required_perm: u64) -> bool {
        has_permission(self.0, required_perm)
    }

    /// Checks exact bitwise containment of `flag` without `ADMINISTRATOR` bypass.
    #[must_use]
    pub const fn contains(self, flag: u64) -> bool {
        (self.0 & flag) == flag
    }

    /// Returns a new `Permissions` with `bits` added.
    #[must_use]
    pub const fn add(self, bits: u64) -> Self {
        Self(self.0 | bits)
    }

    /// Returns a new `Permissions` with `bits` removed.
    #[must_use]
    pub const fn remove(self, bits: u64) -> Self {
        Self(self.0 & !bits)
    }

    /// Returns a new `Permissions` retaining only bits present in `mask`.
    #[must_use]
    pub const fn filter(self, mask: u64) -> Self {
        Self(self.0 & mask)
    }

    /// Parses a canonical decimal `u64` permission string from Discord API v9.
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

/// Error returned when parsing a decimal permission bitmask string fails.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PermissionParseError {
    /// Input string is empty.
    Empty,
    /// Input string contains a non-ASCII-digit character.
    InvalidDigit,
    /// Parsed value exceeds `u64::MAX`.
    Overflow,
}

impl fmt::Display for PermissionParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("empty permission bitmask string"),
            Self::InvalidDigit => f.write_str("invalid character in decimal permission bitmask"),
            Self::Overflow => f.write_str("64-bit permission bitmask overflow"),
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

/// Channel-level permission overwrite entry (`permission_overwrites[i]`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct PermissionOverwrite {
    /// Target role (`overwrite_type == 0`) or member (`overwrite_type == 1`) snowflake ID.
    pub id: u64,
    /// Target kind: `0` for role (`OVERWRITE_TYPE_ROLE`), `1` for member (`OVERWRITE_TYPE_MEMBER`).
    pub overwrite_type: u8,
    /// Explicitly allowed permission bits.
    pub allow: u64,
    /// Explicitly denied permission bits.
    pub deny: u64,
}

impl PermissionOverwrite {
    /// Converts this overwrite into a `twilight_model` [`TwilightPermissionOverwrite`],
    /// substituting `zero_sentinel` if `self.id == 0`.
    #[must_use]
    pub fn to_twilight_overwrite(self, zero_sentinel: u64) -> TwilightPermissionOverwrite {
        TwilightPermissionOverwrite {
            allow: TwilightPermissions::from_bits_truncate(self.allow),
            deny: TwilightPermissions::from_bits_truncate(self.deny),
            id: map_nonzero_id::<GenericMarker>(self.id, zero_sentinel),
            kind: PermissionOverwriteType::from(self.overwrite_type),
        }
    }
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

fn find_zero_sentinel(guild_id: u64, user_id: u64, roles: &[u64], ows: &[PermissionOverwrite]) -> u64 {
    let mut candidate = 1u64;
    while candidate == guild_id
        || candidate == user_id
        || roles.contains(&candidate)
        || ows.iter().any(|o| o.id == candidate)
    {
        candidate = candidate.wrapping_add(1).max(1);
    }
    candidate
}

fn map_nonzero_id<M>(raw: u64, zero_sentinel: u64) -> Id<M> {
    Id::new(if raw == 0 { zero_sentinel.max(1) } else { raw })
}

const fn encode_half_to_twilight(half: u32) -> TwilightPermissions {
    let mut bits = 0u64;
    let mut idx = 0;
    while idx < 32 {
        if (half & (1u32 << idx)) != 0 {
            bits |= SAFE_CARRIER_BITS[idx];
        }
        idx += 1;
    }
    TwilightPermissions::from_bits_truncate(bits)
}

const fn decode_half_from_twilight(perms: TwilightPermissions) -> u32 {
    let raw = perms.bits();
    let mut out = 0u32;
    let mut idx = 0;
    while idx < 32 {
        if (raw & SAFE_CARRIER_BITS[idx]) != 0 {
            out |= 1u32 << idx;
        }
        idx += 1;
    }
    out
}

#[allow(clippy::cast_possible_truncation)]
fn compute_half_via_twilight(
    base_half: u32,
    tw_guild_id: Id<GuildMarker>,
    tw_user_id: Id<UserMarker>,
    tw_roles: &[(Id<RoleMarker>, TwilightPermissions)],
    overwrites: &[PermissionOverwrite],
    shift: u32,
    zero_sentinel: u64,
) -> u32 {
    let everyone = encode_half_to_twilight(base_half) | TwilightPermissions::VIEW_CHANNEL;
    let tw_overwrites: Vec<TwilightPermissionOverwrite> = overwrites
        .iter()
        .map(|ov| TwilightPermissionOverwrite {
            allow: encode_half_to_twilight((ov.allow >> shift) as u32),
            deny: encode_half_to_twilight((ov.deny >> shift) as u32),
            id: map_nonzero_id::<GenericMarker>(ov.id, zero_sentinel),
            kind: PermissionOverwriteType::from(ov.overwrite_type),
        })
        .collect();
    let calc = PermissionCalculator::new(tw_guild_id, tw_user_id, everyone, tw_roles);
    let resolved = calc.in_channel(TwilightChannelType::GuildCategory, &tw_overwrites);
    decode_half_from_twilight(resolved)
}

/// Computes guild-level base permissions for a member using [`PermissionCalculator::root`].
#[must_use]
pub fn compute_base_permissions(
    owner_id: u64,
    user_id: u64,
    everyone_role_perms: u64,
    member_roles_perms: &[u64],
) -> u64 {
    let sentinel = find_zero_sentinel(1, user_id, &[owner_id], &[]);
    let tw_owner_id = map_nonzero_id::<UserMarker>(owner_id, sentinel);
    let tw_user_id = map_nonzero_id::<UserMarker>(user_id, sentinel);
    let tw_roles: Vec<(Id<RoleMarker>, TwilightPermissions)> = member_roles_perms
        .iter()
        .enumerate()
        .map(|(i, &p)| {
            (
                Id::new((i as u64).saturating_add(2)),
                TwilightPermissions::from_bits_truncate(p),
            )
        })
        .collect();

    let calc = PermissionCalculator::new(
        Id::new(1),
        tw_user_id,
        TwilightPermissions::from_bits_truncate(everyone_role_perms),
        &tw_roles,
    )
    .owner_id(tw_owner_id);

    let root = calc.root();
    if root.is_all() || root.contains(TwilightPermissions::ADMINISTRATOR) {
        return ALL_PERMISSIONS;
    }

    let mut high_bits = everyone_role_perms & !TwilightPermissions::all().bits();
    for &role_perms in member_roles_perms {
        high_bits |= role_perms & !TwilightPermissions::all().bits();
    }
    root.bits() | high_bits
}

/// Computes effective channel permissions using [`PermissionCalculator::in_channel`].
#[must_use]
#[allow(clippy::cast_possible_truncation)]
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

    let sentinel = find_zero_sentinel(guild_id, user_id, member_role_ids, overwrites);
    let tw_guild_id = map_nonzero_id::<GuildMarker>(guild_id, sentinel);
    let tw_user_id = map_nonzero_id::<UserMarker>(user_id, sentinel);
    let tw_roles: Vec<(Id<RoleMarker>, TwilightPermissions)> = member_role_ids
        .iter()
        .map(|&rid| (map_nonzero_id::<RoleMarker>(rid, sentinel), TwilightPermissions::empty()))
        .collect();

    let low = compute_half_via_twilight(
        base_perms as u32,
        tw_guild_id,
        tw_user_id,
        &tw_roles,
        overwrites,
        0,
        sentinel,
    );
    let high = compute_half_via_twilight(
        (base_perms >> 32) as u32,
        tw_guild_id,
        tw_user_id,
        &tw_roles,
        overwrites,
        32,
        sentinel,
    );

    let perms = u64::from(low) | (u64::from(high) << 32);
    if (perms & ADMINISTRATOR) != 0 {
        return ALL_PERMISSIONS;
    }
    perms
}

/// Checks whether `computed_perms` grants `required_perm` (including `ADMINISTRATOR` bypass).
#[must_use]
pub const fn has_permission(computed_perms: u64, required_perm: u64) -> bool {
    if (computed_perms & ADMINISTRATOR) != 0 {
        return true;
    }
    (computed_perms & required_perm) == required_perm
}

/// Guild role permission descriptor (`id` and 64-bit `permissions` mask).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct RolePermissions {
    /// Role snowflake ID (`id == guild_id` for `@everyone`).
    pub id: u64,
    /// Role 64-bit permission bitmask.
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

/// Member restriction flags in a guild (`lurker`, `is_pending`, `is_timed_out`, `is_quarantined`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct MemberRestrictions {
    /// Server preview mode (`lurker == true`).
    pub is_lurker: bool,
    /// Pending membership screening (`isPending == true`).
    pub is_pending: bool,
    /// Active moderation timeout (`communicationDisabledUntil > now`).
    pub is_timed_out: bool,
    /// AutoMod quarantine active (`AUTOMOD_QUARANTINED_*`).
    pub is_quarantined: bool,
}

/// Complete channel permission resolution output for a guild member.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ResolvedPermissions {
    pub base_bits: u64,
    pub channel_bits: u64,
    pub effective_bits: u64,
    pub can_view: bool,
    pub can_read_history: bool,
    pub can_send: bool,
    pub can_attach: bool,
    pub can_embed_links: bool,
    pub can_add_reactions: bool,
    pub can_delete_others: bool,
    pub can_kick: bool,
    pub can_ban: bool,
    pub can_manage_channels: bool,
    pub can_manage_roles: bool,
    pub can_connect: bool,
    pub can_speak: bool,
    pub can_stream: bool,
    pub is_admin: bool,
}

/// Resolves guild base permissions from the full `guild_roles` list and `member_role_ids`.
#[must_use]
pub fn resolve_guild_base_permissions(
    guild_id: u64,
    owner_id: u64,
    user_id: u64,
    guild_roles: &[RolePermissions],
    member_role_ids: &[u64],
) -> u64 {
    let mut everyone_perms = 0u64;
    let mut member_perms = Vec::new();
    for role in guild_roles {
        if role.id == guild_id {
            everyone_perms |= role.permissions;
        } else if member_role_ids.contains(&role.id) {
            member_perms.push(role.permissions);
        }
    }
    compute_base_permissions(owner_id, user_id, everyone_perms, &member_perms)
}

/// Resolves channel permissions with default (unrestricted) member flags.
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

fn apply_implicit_and_restriction_masks(
    channel_bits: u64,
    is_admin: bool,
    restrictions: MemberRestrictions,
) -> u64 {
    if is_admin {
        return ALL_PERMISSIONS;
    }
    let mut eff = channel_bits;
    if restrictions.is_quarantined {
        eff &= QUARANTINED_PERMISSIONS_MASK;
    }
    if restrictions.is_timed_out {
        eff &= TIMED_OUT_PERMISSIONS_MASK;
    }
    if restrictions.is_lurker || restrictions.is_pending {
        eff &= LURKER_PERMISSIONS_MASK;
    }
    if (eff & VIEW_CHANNEL) == 0 {
        return 0;
    }
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

/// Resolves channel permissions taking into account member restriction masks and implicit dependencies.
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
    let is_lurker_or_pending = restrictions.is_lurker || restrictions.is_pending;
    let is_admin = (channel_bits & ADMINISTRATOR) != 0 && !is_lurker_or_pending;
    let effective_bits = apply_implicit_and_restriction_masks(channel_bits, is_admin, restrictions);

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
    let can_send = can_view && (effective_bits & SEND_MESSAGES) != 0;
    let can_connect = can_view && (effective_bits & CONNECT) != 0;

    ResolvedPermissions {
        base_bits: base_perms,
        channel_bits,
        effective_bits,
        can_view,
        can_read_history: can_view && (effective_bits & READ_MESSAGE_HISTORY) != 0,
        can_send,
        can_attach: can_send && (effective_bits & ATTACH_FILES) != 0,
        can_embed_links: can_send && (effective_bits & EMBED_LINKS) != 0,
        can_add_reactions: can_view && (effective_bits & ADD_REACTIONS) != 0,
        can_delete_others: can_view && (effective_bits & MANAGE_MESSAGES) != 0,
        can_kick: (guild_mod_bits & KICK_MEMBERS) != 0,
        can_ban: (guild_mod_bits & BAN_MEMBERS) != 0,
        can_manage_channels: can_view && (effective_bits & MANAGE_CHANNELS) != 0,
        can_manage_roles: can_view && (effective_bits & MANAGE_ROLES) != 0,
        can_connect,
        can_speak: can_connect && (effective_bits & SPEAK) != 0,
        can_stream: can_connect && (effective_bits & STREAM) != 0,
        is_admin,
    }
}
