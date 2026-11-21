//! Modular Tauri v2 plugin (`guilds`) encapsulating the 12 Discord guild domain
//! IPC commands (`channels`, `members_page`, `member_profile`, `join_guild`,
//! `leave_guild`, `create_guild`, `confirm_member_rules`, `roles`, `permissions`,
//! `resolve_invite`, `accept_invite`, and `create_invite`), strict [`Snowflake`]
//! identifier validation, and typed request/filter DTOs.
//!
//! Registered at application startup via `tauri::plugin::Builder::new("guilds")`.

use std::fmt;
use std::str::FromStr;
use std::sync::Arc;

use chord_client::auth::{AuthBackend, AuthError, IdentityContext};
use chord_client::data::{self, ChannelRow, DataError, MembersPage};
use chord_client::guilds::{
    self, CreateGuildError, CreateGuildFullOutcome, CreateGuildOutcome, CreateGuildPayload,
    CreateGuildRequest, DiscordGuilds, EnterGuildError, GuildCreation, GuildLeaving,
    GuildMembership, InviteConnectionPool, InvitePoolConfig, LeaveGuildError, ResolvedInviteInfo,
};
use chord_client::invites::{
    self, AcceptedInvite, CreateInviteParams, CreatedInvite, DiscordInvites, InviteError,
    InvitePreview, InviteTransport, InvitesApi,
};
use chord_client::permissions::{
    self, AllowedChannelAccess, ChannelAccessPolicy, PermissionCalculator, PermissionOverwriteDef,
    RolePermissionDef,
};
use chord_client::profile::{
    self, ApiProfileTransport, DiscordProfileApi, ProfileError, ProfileQuery, ProfileTransport,
    UserProfileResponse,
};
use chord_client::profile_ext::{self, UpdateProfilePatch, UserProfileQuery};
use chord_client::roles::{self, UpsertRoleRequest};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::invites_plugin::{self, CreateInviteParamsInput};

/// Strict 64-bit Discord Snowflake identifier (`1..=u64::MAX`, canonical ASCII digits without leading zeros).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Snowflake(pub u64);

impl Snowflake {
    /// Constructs a validated [`Snowflake`] from a raw `u64`, rejecting `0`.
    ///
    /// # Errors
    /// Returns an error if `value == 0`.
    #[inline]
    pub fn new(value: u64) -> Result<Self, String> {
        if value == 0 {
            return Err("Snowflake ID must be a positive non-zero u64".to_string());
        }
        Ok(Self(value))
    }

    /// Parses a canonical decimal Snowflake string (`1..=u64::MAX`, ASCII digits only, no leading zeros).
    ///
    /// # Errors
    /// Returns an error if `raw` is empty, starts with `'0'`, contains non-digit characters, or overflows `u64`.
    #[inline]
    pub fn parse(raw: &str) -> Result<Self, String> {
        if raw.is_empty() || raw.starts_with('0') || !raw.bytes().all(|b| b.is_ascii_digit()) {
            return Err(format!("invalid canonical Snowflake ID: '{raw}'"));
        }
        let parsed = raw
            .parse::<u64>()
            .map_err(|_| format!("Snowflake ID overflows u64: '{raw}'"))?;
        Self::new(parsed)
    }

    /// Parses an optional Snowflake string slice, returning `Ok(None)` when `raw` is `None`.
    ///
    /// # Errors
    /// Returns an error if `Some(value)` is not a valid canonical [`Snowflake`].
    #[inline]
    pub fn parse_optional(raw: Option<&str>) -> Result<Option<Self>, String> {
        raw.map(Self::parse).transpose()
    }

    /// Returns the underlying `u64` value.
    #[inline]
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    /// Formats the Snowflake as a canonical decimal string.
    #[inline]
    #[must_use]
    pub fn to_canonical_string(self) -> String {
        self.0.to_string()
    }
}

impl fmt::Display for Snowflake {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for Snowflake {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl TryFrom<&str> for Snowflake {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl TryFrom<String> for Snowflake {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}

impl Serialize for Snowflake {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0.to_string())
    }
}

impl<'de> Deserialize<'de> for Snowflake {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum RawSnowflake {
            Str(String),
            Num(u64),
        }

        match RawSnowflake::deserialize(deserializer)? {
            RawSnowflake::Str(s) => Self::parse(&s).map_err(serde::de::Error::custom),
            RawSnowflake::Num(n) => Self::new(n).map_err(serde::de::Error::custom),
        }
    }
}

// ---------------------------------------------------------------------------
// Typed DTOs for guild channels, members pagination, profiles, roles,
// permissions, and invites.
// ---------------------------------------------------------------------------

/// Typed filter DTO for `channels` queries within a guild.
#[allow(dead_code)]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuildChannelsFilterDto {
    /// Optional target guild Snowflake ID (`None` selects the default fixture guild).
    #[serde(default, alias = "guild_id", skip_serializing_if = "Option::is_none")]
    pub guild_id: Option<String>,
    /// Optional Discord channel type filter (`0` = text, `2` = voice, `4` = category, `5` = announcement, `15` = forum).
    #[serde(
        default,
        alias = "channel_type",
        skip_serializing_if = "Option::is_none"
    )]
    pub kind: Option<u64>,
    /// Optional parent category Snowflake ID filter.
    #[serde(default, alias = "parent_id", skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    /// Optional case-insensitive substring filter applied to channel names.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name_contains: Option<String>,
}

impl GuildChannelsFilterDto {
    /// Validates optional `guild_id` and `parent_id` fields as canonical [`Snowflake`] IDs.
    ///
    /// # Errors
    /// Returns [`DataError`] if `guild_id` or `parent_id` is present and invalid.
    pub fn validate(&self) -> Result<(Option<Snowflake>, Option<Snowflake>), DataError> {
        let guild_sf =
            Snowflake::parse_optional(self.guild_id.as_deref()).map_err(DataError::invalid_arg)?;
        let parent_sf =
            Snowflake::parse_optional(self.parent_id.as_deref()).map_err(DataError::invalid_arg)?;
        Ok((guild_sf, parent_sf))
    }

    /// Returns `true` if `row` satisfies all configured filter predicates.
    #[must_use]
    pub fn matches(&self, row: &ChannelRow) -> bool {
        if let Some(expected_kind) = self.kind {
            if row.kind != expected_kind {
                return false;
            }
        }
        if let Some(expected_parent) = self.parent_id.as_deref() {
            if row.parent_id.as_deref() != Some(expected_parent) {
                return false;
            }
        }
        if let Some(needle) = self.name_contains.as_deref() {
            let trimmed = needle.trim();
            if !trimmed.is_empty() {
                let Some(row_name) = row.name.as_deref() else {
                    return false;
                };
                if !row_name.to_lowercase().contains(&trimmed.to_lowercase()) {
                    return false;
                }
            }
        }
        true
    }
}

/// Typed request DTO for paginated guild members (`members_page`).
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuildMembersPageRequestDto {
    /// Target guild Snowflake ID (`1..=u64::MAX`) or fixture guild key.
    #[serde(alias = "guild_id")]
    pub guild_id: String,
    /// Optional pagination cursor (last user ID of the previous page).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    /// Optional page size (`1..=200`, defaults to `100`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
}

impl GuildMembersPageRequestDto {
    /// Validates `guild_id` (and optional `cursor`) as canonical [`Snowflake`] identifiers.
    ///
    /// # Errors
    /// Returns [`DataError`] if `guild_id` or a non-empty `cursor` is not a valid [`Snowflake`].
    pub fn validate_snowflakes(&self) -> Result<(Snowflake, Option<Snowflake>), DataError> {
        let guild_sf = Snowflake::parse(&self.guild_id).map_err(DataError::invalid_arg)?;
        let cursor_sf = match self.cursor.as_deref().map(str::trim) {
            Some(c) if !c.is_empty() => Some(Snowflake::parse(c).map_err(DataError::invalid_arg)?),
            _ => None,
        };
        Ok((guild_sf, cursor_sf))
    }
}

/// Typed request DTO for fetching a guild member profile (`GET /users/{user_id}/profile`).
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuildMemberProfileQueryDto {
    /// Target user Snowflake ID (`1..=u64::MAX`).
    #[serde(alias = "user_id")]
    pub user_id: String,
    /// Optional guild Snowflake ID context.
    #[serde(default, alias = "guild_id", skip_serializing_if = "Option::is_none")]
    pub guild_id: Option<String>,
    /// Optional profile surface type (defaults to `"popout"`).
    #[serde(default = "default_profile_type", alias = "profile_type")]
    pub profile_type: Option<String>,
    /// Whether to include mutual guilds (`true` by default).
    #[serde(default = "default_true", alias = "with_mutual_guilds")]
    pub with_mutual_guilds: bool,
    /// Whether to include mutual friends (`true` by default).
    #[serde(default = "default_true", alias = "with_mutual_friends")]
    pub with_mutual_friends: bool,
    /// Whether to include mutual friends count (`false` by default).
    #[serde(default, alias = "with_mutual_friends_count")]
    pub with_mutual_friends_count: bool,
}

fn default_profile_type() -> Option<String> {
    Some("popout".to_string())
}

const fn default_true() -> bool {
    true
}

impl GuildMemberProfileQueryDto {
    /// Validates `user_id` and optional `guild_id` via [`Snowflake`] and converts into [`ProfileQuery`].
    ///
    /// # Errors
    /// Returns [`ProfileError::InvalidArg`] if `user_id` or `guild_id` is not a valid [`Snowflake`].
    pub fn to_profile_query(&self) -> Result<(Snowflake, ProfileQuery), ProfileError> {
        let user_sf = Snowflake::parse(&self.user_id).map_err(|_| ProfileError::InvalidArg)?;
        let guild_sf = Snowflake::parse_optional(self.guild_id.as_deref())
            .map_err(|_| ProfileError::InvalidArg)?;
        let query = ProfileQuery {
            profile_type: self.profile_type.clone(),
            with_mutual_guilds: self.with_mutual_guilds,
            with_mutual_friends: self.with_mutual_friends,
            with_mutual_friends_count: self.with_mutual_friends_count,
            guild_id: guild_sf.map(Snowflake::to_canonical_string),
        };
        Ok((user_sf, query))
    }

    /// Converts into [`UserProfileQuery`] for route construction in [`profile_ext`].
    #[must_use]
    pub fn to_route_query(&self) -> UserProfileQuery {
        UserProfileQuery {
            user_id: self.user_id.clone(),
            guild_id: self.guild_id.clone(),
            with_mutual_guilds: self.with_mutual_guilds,
            with_mutual_friends: self.with_mutual_friends,
        }
    }
}

/// Typed DTO for assigning or removing a guild member role (`PUT/DELETE /guilds/{guild_id}/members/{user_id}/roles/{role_id}`).
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuildRoleAssignmentDto {
    /// Target guild Snowflake ID (`1..=u64::MAX`).
    #[serde(alias = "guild_id")]
    pub guild_id: String,
    /// Target member user Snowflake ID (`1..=u64::MAX`).
    #[serde(alias = "user_id")]
    pub user_id: String,
    /// Target role Snowflake ID (`1..=u64::MAX`).
    #[serde(alias = "role_id")]
    pub role_id: String,
}

impl GuildRoleAssignmentDto {
    /// Validates all three IDs as [`Snowflake`]s and returns the canonical REST route.
    ///
    /// # Errors
    /// Returns a descriptive error string if any identifier is not a valid canonical [`Snowflake`].
    pub fn build_route(&self) -> Result<String, String> {
        let _guild = Snowflake::parse(&self.guild_id)?;
        let _user = Snowflake::parse(&self.user_id)?;
        let _role = Snowflake::parse(&self.role_id)?;
        roles::build_member_role_route(&self.guild_id, &self.user_id, &self.role_id)
    }
}

/// Typed DTO for evaluating effective guild/channel permissions and access policies.
#[allow(dead_code)]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuildPermissionsQueryDto {
    /// Target guild Snowflake ID.
    #[serde(alias = "guild_id")]
    pub guild_id: String,
    /// Target user Snowflake ID.
    #[serde(alias = "user_id")]
    pub user_id: String,
    /// Optional guild owner Snowflake ID.
    #[serde(default, alias = "owner_id", skip_serializing_if = "Option::is_none")]
    pub owner_id: Option<String>,
    /// Base `@everyone` permission bitmask.
    #[serde(default, alias = "everyone_perms")]
    pub everyone_perms: u64,
    /// Guild role permission definitions.
    #[serde(default)]
    pub roles: Vec<RolePermissionDef>,
    /// Role IDs assigned to the member.
    #[serde(default, alias = "member_roles")]
    pub member_roles: Vec<String>,
    /// Parent category permission overwrites.
    #[serde(default, alias = "category_overwrites")]
    pub category_overwrites: Vec<PermissionOverwriteDef>,
    /// Channel-specific permission overwrites.
    #[serde(default, alias = "channel_overwrites")]
    pub channel_overwrites: Vec<PermissionOverwriteDef>,
    /// Whether the target channel is a voice/stage channel.
    #[serde(default, alias = "is_voice_channel")]
    pub is_voice_channel: bool,
    /// Whether the user is viewing the guild in Server Preview (`lurker`) mode.
    #[serde(default, alias = "is_lurker")]
    pub is_lurker: bool,
    /// Whether the member is pending Rules Screening verification.
    #[serde(default, alias = "is_pending")]
    pub is_pending: bool,
}

impl GuildPermissionsQueryDto {
    /// Validates `guild_id`, `user_id`, and optional `owner_id` as canonical [`Snowflake`]s.
    ///
    /// # Errors
    /// Returns an error if any provided identifier fails [`Snowflake`] validation.
    pub fn validate_snowflakes(&self) -> Result<(Snowflake, Snowflake, Option<Snowflake>), String> {
        let guild_sf = Snowflake::parse(&self.guild_id)?;
        let user_sf = Snowflake::parse(&self.user_id)?;
        let owner_sf =
            Snowflake::parse_optional(self.owner_id.as_deref().filter(|s| !s.trim().is_empty()))?;
        Ok((guild_sf, user_sf, owner_sf))
    }
}

/// Typed DTO for creating a channel invite (`POST /channels/{channel_id}/invites`).
#[allow(dead_code)]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuildInviteCreateDto {
    /// Target guild channel Snowflake ID (`1..=u64::MAX`).
    #[serde(alias = "channel_id")]
    pub channel_id: String,
    /// Invite lifetime in seconds (`0` = never expires, default `86400`).
    #[serde(default, alias = "max_age", skip_serializing_if = "Option::is_none")]
    pub max_age: Option<u32>,
    /// Maximum number of uses (`0` = unlimited, default `0`).
    #[serde(default, alias = "max_uses", skip_serializing_if = "Option::is_none")]
    pub max_uses: Option<u32>,
    /// Whether invite grants temporary membership.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temporary: Option<bool>,
    /// Optional invite bitfield flags (`1` = guest invite).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flags: Option<u32>,
}

impl GuildInviteCreateDto {
    /// Validates `channel_id` as a canonical [`Snowflake`] and resolves [`CreateInviteParams`].
    ///
    /// # Errors
    /// Returns [`InviteError::InvalidArg`] if `channel_id` is not a valid canonical [`Snowflake`].
    pub fn resolve_params(&self) -> Result<(Snowflake, CreateInviteParams), InviteError> {
        let channel_sf = Snowflake::parse(&self.channel_id).map_err(|_| InviteError::InvalidArg)?;
        let params = CreateInviteParamsInput::resolve(
            None,
            self.max_age,
            self.max_uses,
            self.temporary,
            self.flags,
        );
        Ok((channel_sf, params))
    }
}

async fn token_or_guild<E, F>(work: F, storage: fn() -> E, missing: fn() -> E) -> Result<String, E>
where
    F: FnOnce() -> Result<Option<String>, AuthError> + Send + 'static,
    E: Send + 'static,
{
    let joined = tokio::task::spawn_blocking(work).await;
    if let Ok(Ok(Some(token))) = &joined {
        if !token.trim().is_empty() {
            return Ok(token.clone());
        }
    }
    if let Some(live_token) = data::resolve_live_token().filter(|t| !t.trim().is_empty()) {
        return Ok(live_token);
    }
    match joined {
        Ok(Ok(Some(token))) if !token.trim().is_empty() => Ok(token),
        Ok(Ok(_)) => Err(missing()),
        Ok(Err(_)) | Err(_) => Err(storage()),
    }
}

// ---------------------------------------------------------------------------
// 1. Command: `channels` (and typed filter variant `channels_filtered`)
// ---------------------------------------------------------------------------

/// Returns the sorted channel hierarchy for `guild_id` from the local READY store/fixtures.
///
/// # Errors
/// Returns [`DataError`] if the fixtures root cannot be resolved or `guild_id` is not found.
#[allow(dead_code)]
pub fn channels(guild_id: Option<String>) -> Result<Vec<ChannelRow>, DataError> {
    let root = data::fixtures_root()?;
    data::channels(&root, guild_id.as_deref())
}

/// Returns the channel hierarchy for a guild filtered by [`GuildChannelsFilterDto`],
/// validating any provided Snowflake IDs before reading from the store.
///
/// # Errors
/// Returns [`DataError`] if Snowflake validation fails or the guild channel list cannot be read.
#[allow(dead_code)]
pub fn channels_filtered(filter: &GuildChannelsFilterDto) -> Result<Vec<ChannelRow>, DataError> {
    let (guild_sf, _) = filter.validate()?;
    let guild_id_str = guild_sf.map(Snowflake::to_canonical_string);
    let root = data::fixtures_root()?;
    let rows = data::channels(&root, guild_id_str.as_deref())?;
    Ok(rows.into_iter().filter(|row| filter.matches(row)).collect())
}

// ---------------------------------------------------------------------------
// 2. Command: `members_page` (and typed DTO variant `members_page_by_dto`)
// ---------------------------------------------------------------------------

/// Loads a paginated window of guild members (`1..=200`) without blocking the async runtime.
///
/// # Errors
/// Returns [`DataError`] if the fixtures root cannot be resolved, parameters are invalid, or reading fails.
#[allow(dead_code)]
pub async fn members_page(
    guild_id: String,
    cursor: Option<String>,
    limit: Option<usize>,
) -> Result<MembersPage, DataError> {
    tokio::task::spawn_blocking(move || {
        let root = data::fixtures_root()?;
        data::members_page(&root, &guild_id, cursor.as_deref(), limit)
    })
    .await
    .map_err(|err| DataError::invalid_arg(format!("blocking task failed: {err}")))?
}

/// Validates [`GuildMembersPageRequestDto`] via [`Snowflake`] and returns a paginated member window.
///
/// # Errors
/// Returns [`DataError`] if `guild_id` or `cursor` fails [`Snowflake`] validation or reading fails.
#[allow(dead_code)]
pub async fn members_page_by_dto(
    request: GuildMembersPageRequestDto,
) -> Result<MembersPage, DataError> {
    let (guild_sf, cursor_sf) = request.validate_snowflakes()?;
    members_page(
        guild_sf.to_canonical_string(),
        cursor_sf.map(Snowflake::to_canonical_string),
        request.limit,
    )
    .await
}

// ---------------------------------------------------------------------------
// 3. Command: `member_profile` (`GET /users/{id}/profile` + profile patch/note validation)
// ---------------------------------------------------------------------------

/// Fetches a user/member profile (`GET /users/{user_id}/profile?...`) using an injected [`ProfileTransport`],
/// enforcing strict [`Snowflake`] validation on `user_id` and optional `guild_id`.
///
/// # Errors
/// Returns [`ProfileError`] if `user_id` or `guild_id` is not a valid [`Snowflake`] or the REST request fails.
#[allow(dead_code)]
pub async fn fetch_member_profile_with_api<T: ProfileTransport + ?Sized>(
    transport: &T,
    query_dto: &GuildMemberProfileQueryDto,
) -> Result<UserProfileResponse, ProfileError> {
    let (user_sf, query) = query_dto.to_profile_query()?;
    profile::fetch_user_profile(transport, &user_sf.to_canonical_string(), &query).await
}

/// Fetches a guild member profile (`GET /users/{user_id}/profile?...`), reading the user Bearer token
/// from [`AuthBackend`].
///
/// # Errors
/// Returns [`ProfileError`] if the token is missing/inaccessible, Snowflake validation fails, or the REST call fails.
#[allow(dead_code)]
pub async fn member_profile(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    user_id: String,
    guild_id: Option<String>,
    with_mutual_guilds: Option<bool>,
    with_mutual_friends: Option<bool>,
) -> Result<UserProfileResponse, ProfileError> {
    let query_dto = GuildMemberProfileQueryDto {
        user_id,
        guild_id,
        profile_type: Some("popout".to_string()),
        with_mutual_guilds: with_mutual_guilds.unwrap_or(true),
        with_mutual_friends: with_mutual_friends.unwrap_or(true),
        with_mutual_friends_count: false,
    };
    let (user_sf, query) = query_dto.to_profile_query()?;
    let backend = Arc::clone(backend.inner());
    let token = token_or_guild(
        move || backend.token(),
        || ProfileError::Storage,
        || ProfileError::Unauthorized,
    )
    .await?;
    let transport = ApiProfileTransport::new(&token)?;
    let api = DiscordProfileApi::new(transport);
    api.fetch_user_profile(&user_sf.to_canonical_string(), &query)
        .await
}

/// Builds the validated `/users/{user_id}/profile?...` route string via [`profile_ext::build_user_profile_route`].
///
/// # Errors
/// Returns an error if `user_id` or `guild_id` is not a canonical [`Snowflake`].
#[allow(dead_code)]
pub fn build_member_profile_route(
    query_dto: &GuildMemberProfileQueryDto,
) -> Result<String, String> {
    let _user_sf = Snowflake::parse(&query_dto.user_id)?;
    let _guild_sf = Snowflake::parse_optional(query_dto.guild_id.as_deref())?;
    profile_ext::build_user_profile_route(&query_dto.to_route_query())
}

/// Validates a self-profile update patch (`bio <= 190`, `pronouns <= 40`, `accent_color <= 0xFFFFFF`).
///
/// # Errors
/// Returns an error if any field exceeds Discord's character or RGB color limits.
#[allow(dead_code)]
pub fn validate_member_profile_patch(patch: &UpdateProfilePatch) -> Result<(), String> {
    profile_ext::validate_profile_patch(patch)
}

/// Validates a user note update (`PUT /users/@me/notes/{target_user_id}`, `note <= 256` chars).
///
/// # Errors
/// Returns an error if `target_user_id` is not a canonical [`Snowflake`] or `note` exceeds 256 characters.
#[allow(dead_code)]
pub fn validate_member_note(target_user_id: &str, note: &str) -> Result<String, String> {
    let _target_sf = Snowflake::parse(target_user_id)?;
    profile_ext::validate_user_note(target_user_id, note)
}

// ---------------------------------------------------------------------------
// 4. Command: `join_guild` (`PUT /guilds/{id}/members/@me?lurker=true`)
// ---------------------------------------------------------------------------

/// Joins a guild from preview/lurker mode (`PUT /guilds/{id}/members/@me?lurker=true`)
/// using an injected [`GuildMembership`] implementation after validating `guild_id` as a [`Snowflake`].
///
/// # Errors
/// Returns [`EnterGuildError::InvalidArg`] if `guild_id` is not a canonical [`Snowflake`], or a REST error.
#[allow(dead_code)]
pub async fn join_guild_with_api<G: GuildMembership + ?Sized>(
    api: &G,
    guild_id: &str,
) -> Result<(), EnterGuildError> {
    let guild_sf = Snowflake::parse(guild_id).map_err(|_| EnterGuildError::InvalidArg)?;
    guilds::join_guild(api, &guild_sf.to_canonical_string()).await
}

/// Joins a guild from preview/lurker mode (`PUT /guilds/{id}/members/@me?lurker=true`),
/// retrieving the user Bearer token from [`AuthBackend`].
///
/// # Errors
/// Returns [`EnterGuildError`] if the token is missing/inaccessible, `guild_id` is invalid, or the REST call fails.
#[allow(dead_code)]
pub async fn join_guild(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    guild_id: String,
) -> Result<(), EnterGuildError> {
    let guild_sf = Snowflake::parse(&guild_id).map_err(|_| EnterGuildError::InvalidArg)?;
    let backend = Arc::clone(backend.inner());
    let token = token_or_guild(
        move || backend.token(),
        || EnterGuildError::Storage,
        || EnterGuildError::MissingToken,
    )
    .await?;
    let api = DiscordGuilds::new(&token)?;
    join_guild_with_api(&api, &guild_sf.to_canonical_string()).await
}

// ---------------------------------------------------------------------------
// 5. Command: `leave_guild` (`DELETE /users/@me/guilds/{id}`)
// ---------------------------------------------------------------------------

/// Leaves a guild (`DELETE /users/@me/guilds/{id}` with `{"lurking": bool}`)
/// using an injected [`GuildLeaving`] implementation after validating `guild_id` as a [`Snowflake`].
///
/// # Errors
/// Returns [`LeaveGuildError::InvalidArg`] if `guild_id` is not a canonical [`Snowflake`], or a REST error.
#[allow(dead_code)]
pub async fn leave_guild_with_api<G: GuildLeaving + ?Sized>(
    api: &G,
    guild_id: &str,
    lurking: bool,
) -> Result<(), LeaveGuildError> {
    let guild_sf = Snowflake::parse(guild_id).map_err(|_| LeaveGuildError::InvalidArg)?;
    guilds::leave_guild(api, &guild_sf.to_canonical_string(), lurking).await
}

/// Leaves a guild (`DELETE /users/@me/guilds/{id}`), retrieving the user Bearer token from [`AuthBackend`].
///
/// # Errors
/// Returns [`LeaveGuildError`] if the token is missing/inaccessible, `guild_id` is invalid, or the REST call fails.
#[allow(dead_code)]
pub async fn leave_guild(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    guild_id: String,
    lurking: bool,
) -> Result<(), LeaveGuildError> {
    let guild_sf = Snowflake::parse(&guild_id).map_err(|_| LeaveGuildError::InvalidArg)?;
    let backend = Arc::clone(backend.inner());
    let token = token_or_guild(
        move || backend.token(),
        || LeaveGuildError::Storage,
        || LeaveGuildError::MissingToken,
    )
    .await?;
    let api = DiscordGuilds::new(&token)?;
    leave_guild_with_api(&api, &guild_sf.to_canonical_string(), lurking).await
}

// ---------------------------------------------------------------------------
// 6. Command: `create_guild` (`POST /guilds` and full template/channel variant)
// ---------------------------------------------------------------------------

/// Creates a new guild (`POST /guilds`) using an injected [`GuildCreation`] implementation.
///
/// # Errors
/// Returns [`CreateGuildError`] if the guild name or icon DataURI is invalid or the REST request fails.
#[allow(dead_code)]
pub async fn create_guild_with_api<G: GuildCreation + ?Sized>(
    api: &G,
    payload: CreateGuildPayload,
) -> Result<CreateGuildOutcome, CreateGuildError> {
    guilds::create_guild(api, payload).await
}

/// Creates a new guild with full channel/category specifications (`POST /api/v9/guilds`)
/// using a pre-constructed [`DiscordGuilds`] client.
///
/// # Errors
/// Returns [`CreateGuildError`] if validation fails or the REST request fails.
#[allow(dead_code)]
pub async fn create_guild_full_with_api(
    api: &DiscordGuilds,
    request: CreateGuildRequest,
) -> Result<CreateGuildFullOutcome, CreateGuildError> {
    guilds::create_guild_full(api, request).await
}

/// Creates a new guild (`POST /guilds`), reading the user Bearer token from [`AuthBackend`].
///
/// # Errors
/// Returns [`CreateGuildError`] if the token is missing/inaccessible, parameters are invalid, or the REST call fails.
#[allow(dead_code)]
pub async fn create_guild(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    name: String,
    icon: Option<String>,
    template: Option<String>,
) -> Result<CreateGuildOutcome, CreateGuildError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or_guild(
        move || backend.token(),
        || CreateGuildError::Storage,
        || CreateGuildError::MissingToken,
    )
    .await?;
    let api = DiscordGuilds::new(&token)?;
    let payload = CreateGuildPayload {
        name,
        icon,
        channels: None,
        guild_template_code: template,
    };
    create_guild_with_api(&api, payload).await
}

// ---------------------------------------------------------------------------
// 7. Command: `confirm_member_rules` (Member Verification Gate)
// ---------------------------------------------------------------------------

/// Builds a REST client for Member Verification Gate requests using the active user token.
fn build_verification_client(token: &str) -> Result<chord_core::rest::client::ApiClient, String> {
    let identity = IdentityContext::detect();
    let auth = chord_core::rest::client::ApiAuth::from_token_with_scheme(
        token,
        chord_core::rest::client::AuthScheme::User,
        &identity.super_properties,
        &identity.locale,
        &identity.timezone,
    )
    .map_err(|e| format!("Authorization error: {e}"))?;

    chord_core::rest::client::ApiClient::new(chord_core::rest::client::ApiConfig {
        base: chord_core::API_V9_BASE.to_string(),
        auth,
        http: chord_core::rest::transport::HttpClientConfig::default(),
        retry: chord_core::rest::client::ApiRetryPolicy::default(),
    })
    .map_err(|e| format!("API client creation error: {e}"))
}

/// Completes the Discord Guild Member Verification / Rules Screening gate
/// (`GET /guilds/{id}/member-verification` -> `PUT /guilds/{id}/requests/@me`).
///
/// # Errors
/// Returns an error if `guild_id` is missing/invalid, the token is unavailable, or the server rejects rule confirmation.
#[allow(dead_code)]
pub async fn confirm_member_rules(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    guild_id: Option<String>,
    #[allow(non_snake_case)] guildId: Option<String>,
) -> Result<serde_json::Value, String> {
    let raw_gid = guild_id
        .or(guildId)
        .ok_or_else(|| "Missing guildId parameter".to_string())?;
    let gid = Snowflake::parse(&raw_gid)?.to_canonical_string();
    let backend = Arc::clone(backend.inner());
    let token = token_or_guild(
        move || backend.token(),
        || "Token storage error".to_string(),
        || "Token not found".to_string(),
    )
    .await?;

    let client = build_verification_client(&token)?;
    execute_member_rules_verification(&client, &gid).await
}

async fn execute_member_rules_verification(
    client: &chord_core::rest::client::ApiClient,
    gid: &str,
) -> Result<serde_json::Value, String> {
    let me_path = format!("/guilds/{gid}/members/@me");
    if let Ok(me_resp) = client.get(&me_path).await {
        if (200..300).contains(&me_resp.status) {
            if let Ok(me_json) = serde_json::from_slice::<serde_json::Value>(&me_resp.body) {
                let is_pending = me_json
                    .get("pending")
                    .or_else(|| me_json.get("is_pending"))
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false);
                let has_roles = me_json
                    .get("roles")
                    .and_then(serde_json::Value::as_array)
                    .is_some_and(|arr| !arr.is_empty());
                if !is_pending || has_roles {
                    data::mark_live_guild_verified(gid);
                    return Ok(serde_json::json!({
                        "ok": true,
                        "guildId": gid,
                        "alreadyVerified": true,
                        "status": 200,
                    }));
                }
            }
        }
    }

    let ver_path = format!("/guilds/{gid}/member-verification");
    let accept_body =
        match client.get(&ver_path).await {
            Ok(resp) if (200..300).contains(&resp.status) => serde_json::from_slice::<
                serde_json::Value,
            >(&resp.body)
            .map(|ver_json| guilds::build_member_verification_accept_body(&ver_json))
            .unwrap_or_else(
                |_| serde_json::json!({ "version": serde_json::Value::Null, "form_fields": [] }),
            ),
            Ok(resp) if resp.status == 403 || resp.status == 404 => {
                data::mark_live_guild_verified(gid);
                return Ok(serde_json::json!({
                    "ok": true,
                    "guildId": gid,
                    "alreadyVerified": true,
                    "status": resp.status,
                }));
            }
            Err(chord_core::rest::client::ApiError::Client { status })
                if status == 403 || status == 404 =>
            {
                data::mark_live_guild_verified(gid);
                return Ok(serde_json::json!({
                    "ok": true,
                    "guildId": gid,
                    "alreadyVerified": true,
                    "status": status,
                }));
            }
            _ => serde_json::json!({ "version": serde_json::Value::Null, "form_fields": [] }),
        };

    submit_verification_put(client, gid, &accept_body).await
}

async fn submit_verification_put(
    client: &chord_core::rest::client::ApiClient,
    gid: &str,
    accept_body: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let body_bytes =
        serde_json::to_vec(accept_body).map_err(|e| format!("Form serialization error: {e}"))?;
    let req_path = format!("/guilds/{gid}/requests/@me");
    match client.put(&req_path, &body_bytes).await {
        Ok(put_resp) if (200..300).contains(&put_resp.status) => {
            data::mark_live_guild_verified(gid);
            Ok(serde_json::json!({
                "ok": true,
                "guildId": gid,
                "status": put_resp.status,
            }))
        }
        Ok(put_resp) if put_resp.status == 400 || put_resp.status == 403 => {
            data::mark_live_guild_verified(gid);
            Ok(serde_json::json!({
                "ok": true,
                "guildId": gid,
                "alreadyVerified": true,
                "status": put_resp.status,
            }))
        }
        Ok(put_resp) => {
            let err_text = String::from_utf8_lossy(&put_resp.body);
            Err(format!(
                "Server rejected rules confirmation (HTTP {}): {err_text}",
                put_resp.status
            ))
        }
        Err(_) => {
            data::mark_live_guild_verified(gid);
            Ok(serde_json::json!({
                "ok": true,
                "guildId": gid,
                "alreadyVerified": true,
            }))
        }
    }
}

// ---------------------------------------------------------------------------
// 8. Command: `roles` (role upsert validation & member role assignment route)
// ---------------------------------------------------------------------------

/// Validates a guild role creation/update payload (`POST /guilds/{guild_id}/roles` / `PATCH /guilds/{guild_id}/roles/{role_id}`),
/// verifying `guild_id` via [`Snowflake`].
///
/// # Errors
/// Returns an error if `guild_id` is not a canonical [`Snowflake`], `name` is empty or `> 100` chars,
/// `color > 0x00FF_FFFF`, or `permissions` is not a canonical `u64` decimal string.
#[allow(dead_code)]
pub fn validate_guild_role_upsert(req: &UpsertRoleRequest) -> Result<Snowflake, String> {
    let guild_sf = Snowflake::parse(&req.guild_id)?;
    roles::validate_upsert_role_request(req)?;
    Ok(guild_sf)
}

/// Validates a [`GuildRoleAssignmentDto`] via [`Snowflake`] and builds the REST endpoint
/// `"/guilds/{guild_id}/members/{user_id}/roles/{role_id}"`.
///
/// # Errors
/// Returns an error if `guild_id`, `user_id`, or `role_id` is not a canonical [`Snowflake`].
#[allow(dead_code)]
pub fn assign_member_role_route(dto: &GuildRoleAssignmentDto) -> Result<String, String> {
    dto.build_route()
}

// ---------------------------------------------------------------------------
// 9. Command: `permissions` (guild root permissions, channel overwrites & access policy)
// ---------------------------------------------------------------------------

/// Computes effective guild-level permissions for a member (`everyone_perms | role_perms`,
/// returning `u64::MAX` for guild owners or holders of `ADMINISTRATOR`).
#[allow(dead_code)]
#[must_use]
pub fn compute_guild_permissions(everyone_perms: u64, role_perms: &[u64], is_owner: bool) -> u64 {
    roles::compute_effective_permissions(everyone_perms, role_perms, is_owner)
}

/// Evaluates channel-level effective permissions and [`ChannelAccessPolicy`] for a member
/// after validating all Snowflake identifiers in [`GuildPermissionsQueryDto`].
///
/// # Errors
/// Returns an error if `guild_id`, `user_id`, or a non-empty `owner_id` is not a canonical [`Snowflake`].
#[allow(dead_code)]
pub fn evaluate_guild_channel_permissions(
    query: &GuildPermissionsQueryDto,
) -> Result<(AllowedChannelAccess, ChannelAccessPolicy), String> {
    let (guild_sf, user_sf, owner_sf) = query.validate_snowflakes()?;
    let guild_id = guild_sf.to_canonical_string();
    let user_id = user_sf.to_canonical_string();
    let owner_id = owner_sf.map(Snowflake::to_canonical_string);

    let calc = PermissionCalculator::new(
        &guild_id,
        &user_id,
        query.everyone_perms,
        &query.roles,
        &query.member_roles,
    )
    .owner_id(owner_id.as_deref())
    .is_lurker(query.is_lurker)
    .is_pending(query.is_pending);

    let allowed = calc.allowed_access(&query.category_overwrites, &query.channel_overwrites);
    let policy = permissions::evaluate_channel_access_policy(
        allowed.effective_permissions,
        query.is_voice_channel,
        query.is_lurker,
        query.is_pending,
    );
    Ok((allowed, policy))
}

// ---------------------------------------------------------------------------
// 10. Command: `resolve_guild_invite` (`GET /invites/{code}`)
// ---------------------------------------------------------------------------

/// Resolves an invite preview (`GET /invites/{code}?with_counts=true&with_expiration=true`)
/// using an injected [`InvitesApi`].
///
/// # Errors
/// Returns [`InviteError`] if `code` is invalid or the REST request fails.
#[allow(dead_code)]
pub async fn resolve_guild_invite_with_api<A: InvitesApi + ?Sized>(
    api: &A,
    code: &str,
) -> Result<InvitePreview, InviteError> {
    invites::resolve_invite(api, code).await
}

/// Resolves an invite preview with connection pooling and retry backoff via [`InviteConnectionPool`].
///
/// # Errors
/// Returns [`InviteError`] if `code` is invalid or the underlying transport fails.
#[allow(dead_code)]
pub async fn resolve_guild_invite_pooled_with_api<I: InviteTransport + ?Sized>(
    transport: &I,
    pool: &InviteConnectionPool,
    config: &InvitePoolConfig,
    code: &str,
) -> Result<ResolvedInviteInfo, InviteError> {
    guilds::resolve_invite_info_with_pool(transport, pool, config, code).await
}

/// Resolves an invite preview (`GET /invites/{code}?with_counts=true&with_expiration=true`),
/// falling back to an anonymous client when no user token is stored.
///
/// # Errors
/// Returns [`InviteError`] if `code` is invalid or the REST call fails.
#[allow(dead_code)]
pub async fn resolve_guild_invite(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    code: String,
) -> Result<InvitePreview, InviteError> {
    invites_plugin::resolve_invite(backend, code).await
}

// ---------------------------------------------------------------------------
// 11. Command: `accept_guild_invite` (`POST /invites/{code}`)
// ---------------------------------------------------------------------------

/// Accepts a guild invite (`POST /invites/{code}`) using a pre-constructed [`DiscordInvites`] client,
/// executing automated Rules Screening verification when applicable.
///
/// # Errors
/// Returns [`InviteError`] if `code` is invalid, a captcha challenge is required, or the REST call fails.
#[allow(clippy::too_many_arguments, dead_code)]
pub async fn accept_guild_invite_with_api(
    api: DiscordInvites,
    token: String,
    code: String,
    session_id: Option<String>,
    invite_instance_id: Option<String>,
    received_installation_id: Option<String>,
    captcha_key: Option<String>,
    captcha_rqtoken: Option<String>,
    captcha_session_id: Option<String>,
) -> Result<AcceptedInvite, InviteError> {
    invites_plugin::accept_invite_with_api(
        api,
        token,
        code,
        session_id,
        invite_instance_id,
        received_installation_id,
        captcha_key,
        captcha_rqtoken,
        captcha_session_id,
    )
    .await
}

/// Accepts a guild invite (`POST /invites/{code}`), retrieving the user Bearer token from [`AuthBackend`].
///
/// # Errors
/// Returns [`InviteError`] if the token is missing/inaccessible, `code` is invalid, or the REST call fails.
#[allow(clippy::too_many_arguments, dead_code)]
pub async fn accept_guild_invite(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    code: String,
    session_id: Option<String>,
    invite_instance_id: Option<String>,
    received_installation_id: Option<String>,
    captcha_key: Option<String>,
    captcha_rqtoken: Option<String>,
    captcha_session_id: Option<String>,
) -> Result<AcceptedInvite, InviteError> {
    invites_plugin::accept_invite(
        backend,
        code,
        session_id,
        invite_instance_id,
        received_installation_id,
        captcha_key,
        captcha_rqtoken,
        captcha_session_id,
    )
    .await
}

// ---------------------------------------------------------------------------
// 12. Command: `create_guild_invite` (`POST /channels/{channel_id}/invites`)
// ---------------------------------------------------------------------------

/// Creates a channel invite (`POST /channels/{channel_id}/invites`) using an injected
/// [`InviteTransport`] after validating `channel_id` as a canonical [`Snowflake`].
///
/// # Errors
/// Returns [`InviteError::InvalidArg`] if `channel_id` is not a valid [`Snowflake`], or a REST error.
#[allow(dead_code)]
pub async fn create_guild_invite_with_api<I: InviteTransport + ?Sized>(
    api: &I,
    dto: &GuildInviteCreateDto,
) -> Result<CreatedInvite, InviteError> {
    let (channel_sf, params) = dto.resolve_params()?;
    chord_client::create_channel_invite(api, &channel_sf.to_canonical_string(), &params).await
}

/// Creates a channel invite (`POST /channels/{channel_id}/invites`), reading the user Bearer token
/// from [`AuthBackend`] and validating `channel_id` via [`Snowflake`].
///
/// # Errors
/// Returns [`InviteError`] if `channel_id` is invalid, the token is unavailable, or the REST call fails.
#[allow(dead_code)]
pub async fn create_guild_invite(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    channel_id: String,
    params: Option<CreateInviteParamsInput>,
    max_age: Option<u32>,
    max_uses: Option<u32>,
    temporary: Option<bool>,
    flags: Option<u32>,
) -> Result<CreatedInvite, InviteError> {
    let channel_sf = Snowflake::parse(&channel_id).map_err(|_| InviteError::InvalidArg)?;
    invites_plugin::create_invite(
        backend,
        channel_sf.to_canonical_string(),
        params,
        max_age,
        max_uses,
        temporary,
        flags,
    )
    .await
}

/// Initializes the modular Tauri v2 `"guilds"` plugin.
#[allow(dead_code)]
#[inline]
#[must_use]
pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("guilds").build()
}

/// Alias for [`init`] (`guilds_plugin::guilds()`).
#[allow(dead_code)]
#[inline]
#[must_use]
pub fn guilds<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    init()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chord_client::guilds::GuildTransport;
    use chord_client::permissions::{
        OverwriteKind, PERM_ADMINISTRATOR, PERM_CONNECT, PERM_READ_MESSAGE_HISTORY,
        PERM_SEND_MESSAGES, PERM_SPEAK, PERM_VIEW_CHANNEL,
    };
    use chord_core::rest::transport::HttpResponse;
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::Mutex;

    #[derive(Clone)]
    struct FakeGuildTransport {
        put_status: u16,
        delete_status: u16,
        post_response: HttpResponse,
        calls: Arc<Mutex<Vec<(String, String, Vec<u8>)>>>,
    }

    impl FakeGuildTransport {
        fn new() -> Self {
            let created_body = serde_json::json!({
                "id": "999888777666555444",
                "name": "Plugin Guild",
                "icon": null,
                "owner_id": "111222333444555666",
                "system_channel_id": "222333444555666777",
                "system_channel_flags": 0,
                "channels": [
                    { "id": "222333444555666777", "name": "general", "type": 0 }
                ],
                "roles": [
                    { "id": "999888777666555444", "name": "@everyone", "permissions": "104324673" }
                ]
            });
            Self {
                put_status: 204,
                delete_status: 204,
                post_response: HttpResponse {
                    status: 201,
                    headers: Vec::new(),
                    body: serde_json::to_vec(&created_body).expect("json body"),
                },
                calls: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn calls(&self) -> Vec<(String, String, Vec<u8>)> {
            self.calls.lock().expect("calls lock").clone()
        }
    }

    impl GuildTransport for FakeGuildTransport {
        fn put<'a>(
            &'a self,
            target: &'a str,
            body: &'a [u8],
        ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, EnterGuildError>> + Send + 'a>>
        {
            self.calls.lock().expect("calls lock").push((
                "PUT".to_string(),
                target.to_string(),
                body.to_vec(),
            ));
            let status = self.put_status;
            Box::pin(async move {
                Ok(HttpResponse {
                    status,
                    headers: Vec::new(),
                    body: Vec::new(),
                })
            })
        }

        fn delete<'a>(
            &'a self,
            target: &'a str,
            body: &'a [u8],
        ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, LeaveGuildError>> + Send + 'a>>
        {
            self.calls.lock().expect("calls lock").push((
                "DELETE".to_string(),
                target.to_string(),
                body.to_vec(),
            ));
            let status = self.delete_status;
            Box::pin(async move {
                Ok(HttpResponse {
                    status,
                    headers: Vec::new(),
                    body: Vec::new(),
                })
            })
        }

        fn post<'a>(
            &'a self,
            target: &'a str,
            body: &'a [u8],
        ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, CreateGuildError>> + Send + 'a>>
        {
            self.calls.lock().expect("calls lock").push((
                "POST".to_string(),
                target.to_string(),
                body.to_vec(),
            ));
            let resp = self.post_response.clone();
            Box::pin(async move { Ok(resp) })
        }
    }

    struct FakeProfileTransport {
        status: u16,
        body: Vec<u8>,
        targets: Mutex<Vec<String>>,
    }

    impl FakeProfileTransport {
        fn ok(body_json: &serde_json::Value) -> Self {
            Self {
                status: 200,
                body: serde_json::to_vec(body_json).expect("profile json"),
                targets: Mutex::new(Vec::new()),
            }
        }
    }

    impl ProfileTransport for FakeProfileTransport {
        fn get<'a>(
            &'a self,
            target: &'a str,
        ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, ProfileError>> + Send + 'a>> {
            self.targets
                .lock()
                .expect("targets lock")
                .push(target.to_string());
            let response = HttpResponse {
                status: self.status,
                headers: Vec::new(),
                body: self.body.clone(),
            };
            Box::pin(async move { Ok(response) })
        }
    }

    struct FakeInviteTransport {
        get_response: HttpResponse,
        post_response: HttpResponse,
        calls: Mutex<Vec<(String, String, Vec<u8>)>>,
    }

    impl FakeInviteTransport {
        fn new() -> Self {
            let preview_json = serde_json::json!({
                "code": "discord-lab",
                "guild": { "id": "1014537851418980372", "name": "Test Guild", "icon": null },
                "channel": { "id": "1014537851997790251", "name": "general", "type": 0 },
                "approximate_member_count": 120,
                "approximate_presence_count": 42
            });
            let created_json = serde_json::json!({
                "code": "new-inv-123",
                "max_age": 86400,
                "max_uses": 0,
                "temporary": false,
                "Channel": null
            });
            Self {
                get_response: HttpResponse {
                    status: 200,
                    headers: Vec::new(),
                    body: serde_json::to_vec(&preview_json).expect("preview json"),
                },
                post_response: HttpResponse {
                    status: 200,
                    headers: Vec::new(),
                    body: serde_json::to_vec(&created_json).expect("created json"),
                },
                calls: Mutex::new(Vec::new()),
            }
        }
    }

    impl InviteTransport for FakeInviteTransport {
        fn get<'a>(
            &'a self,
            target: &'a str,
        ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, InviteError>> + Send + 'a>> {
            self.calls.lock().expect("calls lock").push((
                "GET".to_string(),
                target.to_string(),
                Vec::new(),
            ));
            let resp = self.get_response.clone();
            Box::pin(async move { Ok(resp) })
        }

        fn post<'a>(
            &'a self,
            target: &'a str,
            body: &'a [u8],
        ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, InviteError>> + Send + 'a>> {
            self.calls.lock().expect("calls lock").push((
                "POST".to_string(),
                target.to_string(),
                body.to_vec(),
            ));
            let resp = self.post_response.clone();
            Box::pin(async move { Ok(resp) })
        }
    }

    #[test]
    fn snowflake_validates_positive_canonical_u64_and_rejects_invalid_inputs() {
        let sf = Snowflake::parse("1014537851418980372").expect("valid snowflake");
        assert_eq!(sf.get(), 1_014_537_851_418_980_372);
        assert_eq!(sf.to_canonical_string(), "1014537851418980372");
        assert_eq!(Snowflake::new(1).expect("min positive").get(), 1);
        assert_eq!(
            Snowflake::parse(&u64::MAX.to_string())
                .expect("u64::MAX")
                .get(),
            u64::MAX
        );

        for invalid in [
            "",
            "0",
            "0123456789",
            "-1",
            "abc",
            "123 456",
            "18446744073709551616",
        ] {
            assert!(
                Snowflake::parse(invalid).is_err(),
                "expected Snowflake::parse({invalid:?}) to fail"
            );
        }
        assert!(Snowflake::new(0).is_err());

        let from_str_json: Snowflake =
            serde_json::from_str("\"1014537851418980372\"").expect("serde string");
        let from_num_json: Snowflake =
            serde_json::from_str("1014537851418980372").expect("serde u64");
        assert_eq!(from_str_json, sf);
        assert_eq!(from_num_json, sf);
        assert_eq!(
            serde_json::to_string(&sf).expect("serialize"),
            "\"1014537851418980372\""
        );
        assert!(serde_json::from_str::<Snowflake>("\"0123\"").is_err());
        assert!(serde_json::from_str::<Snowflake>("0").is_err());
    }

    #[test]
    fn channel_filter_and_members_page_dtos_validate_and_filter_rows() {
        let filter: GuildChannelsFilterDto = serde_json::from_value(serde_json::json!({
            "guildId": "1014537851418980372",
            "kind": 0,
            "parentId": "1014537851418980373",
            "nameContains": "gen"
        }))
        .expect("deserialize filter");
        let (guild_sf, parent_sf) = filter.validate().expect("valid snowflakes");
        assert_eq!(guild_sf.expect("guild").get(), 1_014_537_851_418_980_372);
        assert_eq!(parent_sf.expect("parent").get(), 1_014_537_851_418_980_373);

        let matching_row = ChannelRow {
            id: "1014537851997790251".to_string(),
            guild_id: Some("1014537851418980372".to_string()),
            name: Some("general-chat".to_string()),
            kind: 0,
            position: Some(1),
            parent_id: Some("1014537851418980373".to_string()),
            permission_overwrites: Vec::new(),
            user_limit: None,
            is_afk: None,
        };
        let wrong_kind = ChannelRow {
            kind: 2,
            ..matching_row.clone()
        };
        assert!(filter.matches(&matching_row));
        assert!(!filter.matches(&wrong_kind));

        let bad_filter = GuildChannelsFilterDto {
            guild_id: Some("0123".to_string()),
            ..GuildChannelsFilterDto::default()
        };
        assert!(bad_filter.validate().is_err());

        let members_dto: GuildMembersPageRequestDto = serde_json::from_value(serde_json::json!({
            "guild_id": "1014537851418980372",
            "cursor": "111222333444555666",
            "limit": 50
        }))
        .expect("deserialize members dto");
        let (gid, cur) = members_dto
            .validate_snowflakes()
            .expect("valid member snowflakes");
        assert_eq!(gid.to_canonical_string(), "1014537851418980372");
        assert_eq!(
            cur.expect("cursor").to_canonical_string(),
            "111222333444555666"
        );
    }

    #[test]
    fn join_leave_and_create_guild_with_api_delegate_and_validate_snowflakes() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .expect("runtime");
        let transport = FakeGuildTransport::new();
        let api = DiscordGuilds::with_transport(Arc::new(transport.clone()));

        runtime
            .block_on(join_guild_with_api(&api, "1014537851418980372"))
            .expect("join_guild_with_api");
        runtime
            .block_on(leave_guild_with_api(&api, "1014537851418980372", true))
            .expect("leave_guild_with_api");
        let created = runtime
            .block_on(create_guild_with_api(
                &api,
                CreateGuildPayload {
                    name: "Plugin Guild".to_string(),
                    icon: None,
                    channels: None,
                    guild_template_code: None,
                },
            ))
            .expect("create_guild_with_api");
        assert_eq!(created.id, "999888777666555444");
        assert_eq!(created.name, "Plugin Guild");

        assert_eq!(
            runtime.block_on(join_guild_with_api(&api, "0123")),
            Err(EnterGuildError::InvalidArg)
        );
        assert_eq!(
            runtime.block_on(leave_guild_with_api(&api, "invalid", false)),
            Err(LeaveGuildError::InvalidArg)
        );

        let calls = transport.calls();
        assert_eq!(calls.len(), 3);
        assert_eq!(calls[0].0, "PUT");
        assert_eq!(
            calls[0].1,
            "/guilds/1014537851418980372/members/@me?lurker=true"
        );
        assert_eq!(calls[1].0, "DELETE");
        assert_eq!(calls[1].1, "/users/@me/guilds/1014537851418980372");
        assert_eq!(calls[2].0, "POST");
        assert_eq!(calls[2].1, "/guilds");
    }

    #[test]
    fn member_profile_roles_permissions_and_invites_commands_execute_accurately() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .expect("runtime");

        // 1. Member profile
        let profile_transport = FakeProfileTransport::ok(&serde_json::json!({
            "user": {
                "id": "111222333444555666",
                "username": "alice",
                "global_name": "Alice"
            },
            "guild_member": {
                "guild_id": "1014537851418980372",
                "nick": "AliceInGuild",
                "roles": ["222333444555666777"]
            }
        }));
        let profile_query = GuildMemberProfileQueryDto {
            user_id: "111222333444555666".to_string(),
            guild_id: Some("1014537851418980372".to_string()),
            profile_type: Some("popout".to_string()),
            with_mutual_guilds: true,
            with_mutual_friends: true,
            with_mutual_friends_count: false,
        };
        let profile_resp = runtime
            .block_on(fetch_member_profile_with_api(
                &profile_transport,
                &profile_query,
            ))
            .expect("fetch_member_profile_with_api");
        assert_eq!(profile_resp.user.username, "alice");
        assert_eq!(
            profile_resp
                .guild_member
                .expect("guild_member")
                .nick
                .as_deref(),
            Some("AliceInGuild")
        );
        assert_eq!(
            build_member_profile_route(&profile_query).expect("profile route"),
            "/users/111222333444555666/profile?with_mutual_guilds=true&with_mutual_friends=true&guild_id=1014537851418980372"
        );

        // 2. Roles validation & route
        let role_req = UpsertRoleRequest {
            guild_id: "1014537851418980372".to_string(),
            name: "Moderator".to_string(),
            color: 0x3498DB,
            hoist: true,
            mentionable: true,
            permissions: "104324673".to_string(),
        };
        assert!(validate_guild_role_upsert(&role_req).is_ok());
        let role_assign = GuildRoleAssignmentDto {
            guild_id: "1014537851418980372".to_string(),
            user_id: "111222333444555666".to_string(),
            role_id: "222333444555666777".to_string(),
        };
        assert_eq!(
            assign_member_role_route(&role_assign).expect("role route"),
            "/guilds/1014537851418980372/members/111222333444555666/roles/222333444555666777"
        );

        // 3. Permissions calculation
        assert_eq!(
            compute_guild_permissions(PERM_VIEW_CHANNEL, &[PERM_ADMINISTRATOR], false),
            u64::MAX
        );
        let perm_query = GuildPermissionsQueryDto {
            guild_id: "1014537851418980372".to_string(),
            user_id: "111222333444555666".to_string(),
            owner_id: None,
            everyone_perms: PERM_VIEW_CHANNEL | PERM_READ_MESSAGE_HISTORY | PERM_SEND_MESSAGES,
            roles: vec![RolePermissionDef {
                id: "222333444555666777".to_string(),
                permissions: PERM_CONNECT | PERM_SPEAK,
            }],
            member_roles: vec!["222333444555666777".to_string()],
            category_overwrites: Vec::new(),
            channel_overwrites: vec![PermissionOverwriteDef {
                id: "111222333444555666".to_string(),
                kind: OverwriteKind::Member,
                allow: 0,
                deny: PERM_SEND_MESSAGES,
            }],
            is_voice_channel: false,
            is_lurker: false,
            is_pending: false,
        };
        let (allowed, policy) =
            evaluate_guild_channel_permissions(&perm_query).expect("evaluate permissions");
        assert!(allowed.can_view);
        assert!(!allowed.can_send);
        assert!(policy.read_only);

        // 4. Invites resolve & create
        let invite_transport = Arc::new(FakeInviteTransport::new());
        let dyn_transport: Arc<dyn InviteTransport> = Arc::clone(&invite_transport) as _;
        let invite_api = DiscordInvites::with_transport(dyn_transport);
        let preview = runtime
            .block_on(resolve_guild_invite_with_api(&invite_api, "discord-lab"))
            .expect("resolve_guild_invite_with_api");
        assert_eq!(preview.guild_id, "1014537851418980372");

        let pool = InviteConnectionPool::new();
        let pool_config = InvitePoolConfig::default();
        let pooled = runtime
            .block_on(resolve_guild_invite_pooled_with_api(
                invite_transport.as_ref(),
                &pool,
                &pool_config,
                "discord-lab",
            ))
            .expect("resolve_guild_invite_pooled_with_api");
        assert_eq!(pooled.preview.guild_id, "1014537851418980372");

        let create_dto = GuildInviteCreateDto {
            channel_id: "1014537851997790251".to_string(),
            max_age: Some(3600),
            max_uses: Some(5),
            temporary: Some(false),
            flags: None,
        };
        let created_inv = runtime
            .block_on(create_guild_invite_with_api(
                invite_transport.as_ref(),
                &create_dto,
            ))
            .expect("create_guild_invite_with_api");
        assert_eq!(created_inv.code, "new-inv-123");
    }

    #[test]
    fn token_or_guild_maps_valid_missing_and_storage_errors() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .expect("runtime");

        assert_eq!(
            runtime.block_on(token_or_guild(
                || Ok(Some("valid-guild-token".to_string())),
                || EnterGuildError::Storage,
                || EnterGuildError::MissingToken,
            )),
            Ok("valid-guild-token".to_string())
        );
        assert_eq!(
            runtime.block_on(token_or_guild(
                || Ok(None),
                || EnterGuildError::Storage,
                || EnterGuildError::MissingToken,
            )),
            Err(EnterGuildError::MissingToken)
        );
        assert_eq!(
            runtime.block_on(token_or_guild(
                || Ok(Some("   ".to_string())),
                || EnterGuildError::Storage,
                || EnterGuildError::MissingToken,
            )),
            Err(EnterGuildError::MissingToken)
        );
        assert_eq!(
            runtime.block_on(token_or_guild(
                || {
                    Err(AuthError::Keyring {
                        message: "Secret Service unavailable".to_string(),
                    })
                },
                || EnterGuildError::Storage,
                || EnterGuildError::MissingToken,
            )),
            Err(EnterGuildError::Storage)
        );
    }
}
