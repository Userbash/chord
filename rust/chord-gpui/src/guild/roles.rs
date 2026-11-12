//! Иерархия ролей сервера, drag-and-drop перетаскивание `PATCH /guilds/{id}/roles`
//! и 64-битный редактор битовой маски прав `permissions` (`TASK-1007`).
//!
//! Оракулы:
//! - `waves/kanban-board.json` (`TASK-1007`): `Roles Manager Dialog` (`720px x 540px`,
//!   `padding: 16px`, `spacing: 12px`), дизайн-токены (`#313338`, `#2b2d31`, `#ffffff`,
//!   `#5865f2`, `#3f4147`), состояния `role-item` (`row -> reorder icon -> selected role`).
//! - `docs/re/gateway-schema.json` и `docs/re/gateway-schema.md`: `READY.d.guilds[].roles[]`
//!   (132 роли / 13 ключей, `permissions` как десятичная строка `u64`).
//! - `docs/re/rest-d02.json`, `/home/sanya/re-deep/rest-static.json` (`GUILD_ROLES`, `GUILD_ROLE`),
//!   `rust/crates/discord-core/src/permissions.rs` (41 флаг `0..=40`) и `rust/gui-spike/src/roles.rs`.

#![forbid(unsafe_code)]
#![allow(dead_code)]

use std::fmt;

pub const ROLES_DIALOG_CONTAINER: &str = "Roles Manager Dialog";
pub const ROLES_DIALOG_DIMENSIONS: &str = "720px x 540px";
pub const ROLES_DIALOG_WIDTH_PX: u32 = 720;
pub const ROLES_DIALOG_HEIGHT_PX: u32 = 540;
pub const ROLES_DIALOG_PADDING_PX: u32 = 16;
pub const ROLES_DIALOG_SPACING_PX: u32 = 12;
pub const ROLES_SIDEBAR_WIDTH_PX: u32 = 320;

pub const TOKEN_BACKGROUND: &str = "#313338";
pub const TOKEN_SURFACE: &str = "#2b2d31";
pub const TOKEN_TEXT_PRIMARY: &str = "#ffffff";
pub const TOKEN_ACCENT: &str = "#5865f2";
pub const TOKEN_BORDER: &str = "#3f4147";
pub const DEFAULT_ROLE_COLOR_HEX: &str = "#99AAB5";

pub const ROLE_ITEM_ELEMENT: &str = "role-item";
pub const ROLE_ITEM_STATE_DEFAULT: &str = "row";
pub const ROLE_ITEM_STATE_HOVER: &str = "reorder icon";
pub const ROLE_ITEM_STATE_ACTIVE: &str = "selected role";

pub const MAX_ROLE_NAME_CHARS: usize = 100;
pub const MAX_RGB_COLOR: u32 = 0x00FF_FFFF;
pub const MAX_GUILD_ROLES: usize = 250;
pub const TOTAL_V9_PERMISSIONS_COUNT: usize = 41;
pub const ALL_V9_DEFINED_PERMISSIONS: u64 = (1u64 << 41) - 1;

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
pub const PERM_VIEW_GUILD_INSIGHTS: u64 = 1 << 19;
pub const PERM_CONNECT: u64 = 1 << 20;
pub const PERM_SPEAK: u64 = 1 << 21;
pub const PERM_MUTE_MEMBERS: u64 = 1 << 22;
pub const PERM_DEAFEN_MEMBERS: u64 = 1 << 23;
pub const PERM_MOVE_MEMBERS: u64 = 1 << 24;
pub const PERM_USE_VAD: u64 = 1 << 25;
pub const PERM_CHANGE_NICKNAME: u64 = 1 << 26;
pub const PERM_MANAGE_NICKNAMES: u64 = 1 << 27;
pub const PERM_MANAGE_ROLES: u64 = 1 << 28;
pub const PERM_MANAGE_WEBHOOKS: u64 = 1 << 29;
pub const PERM_MANAGE_GUILD_EXPRESSIONS: u64 = 1 << 30;
pub const PERM_USE_APPLICATION_COMMANDS: u64 = 1 << 31;
pub const PERM_REQUEST_TO_SPEAK: u64 = 1 << 32;
pub const PERM_MANAGE_EVENTS: u64 = 1 << 33;
pub const PERM_MANAGE_THREADS: u64 = 1 << 34;
pub const PERM_CREATE_PUBLIC_THREADS: u64 = 1 << 35;
pub const PERM_CREATE_PRIVATE_THREADS: u64 = 1 << 36;
pub const PERM_USE_EXTERNAL_STICKERS: u64 = 1 << 37;
pub const PERM_SEND_MESSAGES_IN_THREADS: u64 = 1 << 38;
pub const PERM_USE_EMBEDDED_ACTIVITIES: u64 = 1 << 39;
pub const PERM_MODERATE_MEMBERS: u64 = 1 << 40;

pub const DANGEROUS_PERMISSIONS_MASK: u64 = PERM_ADMINISTRATOR
    | PERM_MANAGE_GUILD
    | PERM_MANAGE_ROLES
    | PERM_MANAGE_CHANNELS
    | PERM_KICK_MEMBERS
    | PERM_BAN_MEMBERS
    | PERM_MANAGE_WEBHOOKS
    | PERM_MANAGE_MESSAGES
    | PERM_MODERATE_MEMBERS;

pub const MAX_MESSAGE_CHARS: usize = 2_000;
pub const MAX_ATTACHMENTS_COUNT: usize = 10;
pub const MAX_ATTACHMENT_BYTES: u64 = 25 * 1024 * 1024;
pub const MAX_SEARCH_OFFSET: u32 = 9_975;
pub const MAX_VOLUME_PERCENT: u16 = 200;
pub const MAX_BADGE_CAP: u32 = 99;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoleItemInteractiveState {
    Row,
    ReorderIcon,
    SelectedRole,
}

impl RoleItemInteractiveState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Row => ROLE_ITEM_STATE_DEFAULT,
            Self::ReorderIcon => ROLE_ITEM_STATE_HOVER,
            Self::SelectedRole => ROLE_ITEM_STATE_ACTIVE,
        }
    }

    #[must_use]
    pub const fn resolve(selected: bool, hovered: bool) -> Self {
        if selected {
            Self::SelectedRole
        } else if hovered {
            Self::ReorderIcon
        } else {
            Self::Row
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropPlacement {
    Before,
    After,
}

impl DropPlacement {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Before => "before",
            Self::After => "after",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionCategory {
    General,
    Membership,
    Text,
    Voice,
    Advanced,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoleError {
    InvalidSnowflake(String),
    EmptyRoleName,
    RoleNameTooLong(usize),
    InvalidRgbColor(u32),
    InvalidHexColor(String),
    InvalidPermissionsDecimal(String),
    RoleNotFound(String),
    EveryoneRoleImmutable,
    ManagedRoleImmutable(String),
    MissingManageRolesPermission,
    HierarchyViolation {
        role_pos: i32,
        actor_highest_pos: i32,
    },
    PrivilegeEscalationDenied(u64),
    AgeGatedRestricted,
    MessageTooLong(usize),
    TooManyAttachments(usize),
    AttachmentTooLarge(u64),
    SearchOffsetOutOfBounds(u32),
    VolumeOutOfBounds(u16),
    HttpFailure {
        status: u16,
        retry_after_ms: u64,
    },
}

impl fmt::Display for RoleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for RoleError {}

pub fn validate_snowflake(raw: &str) -> Result<u64, RoleError> {
    let trimmed = raw.trim();
    if trimmed.is_empty()
        || trimmed.starts_with('0')
        || !trimmed.bytes().all(|b| b.is_ascii_digit())
    {
        return Err(RoleError::InvalidSnowflake(raw.to_string()));
    }
    let val = trimmed
        .parse::<u64>()
        .map_err(|_| RoleError::InvalidSnowflake(raw.to_string()))?;
    if val == 0 {
        return Err(RoleError::InvalidSnowflake(raw.to_string()));
    }
    Ok(val)
}

pub fn parse_permissions_decimal(raw: &str) -> Result<u64, RoleError> {
    if raw.is_empty() || !raw.bytes().all(|b| b.is_ascii_digit()) {
        return Err(RoleError::InvalidPermissionsDecimal(raw.to_string()));
    }
    if raw.len() > 1 && raw.starts_with('0') {
        return Err(RoleError::InvalidPermissionsDecimal(raw.to_string()));
    }
    raw.parse::<u64>()
        .map_err(|_| RoleError::InvalidPermissionsDecimal(raw.to_string()))
}

pub fn parse_role_hex_color(hex: &str) -> Result<u32, RoleError> {
    let clean = hex.trim().strip_prefix('#').unwrap_or(hex.trim());
    if clean.len() != 6 || !clean.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(RoleError::InvalidHexColor(hex.to_string()));
    }
    let val =
        u32::from_str_radix(clean, 16).map_err(|_| RoleError::InvalidHexColor(hex.to_string()))?;
    if val > MAX_RGB_COLOR {
        return Err(RoleError::InvalidRgbColor(val));
    }
    Ok(val)
}

#[must_use]
pub fn format_role_hex_color(color: u32) -> String {
    if color == 0 {
        DEFAULT_ROLE_COLOR_HEX.to_string()
    } else {
        format!("#{:06X}", color & MAX_RGB_COLOR)
    }
}

#[must_use]
pub fn format_member_badge(count: u32) -> String {
    if count > MAX_BADGE_CAP {
        "99+".to_string()
    } else {
        count.to_string()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuildRole {
    pub id: String,
    pub name: String,
    pub position: i32,
    pub color: u32,
    pub hoist: bool,
    pub managed: bool,
    pub mentionable: bool,
    pub permissions: u64,
    pub member_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActorHierarchyContext {
    pub effective_permissions: u64,
    pub highest_position: i32,
    pub is_owner: bool,
}

impl ActorHierarchyContext {
    #[must_use]
    pub const fn has_manage_roles(self) -> bool {
        self.is_owner
            || (self.effective_permissions & PERM_ADMINISTRATOR) != 0
            || (self.effective_permissions & PERM_MANAGE_ROLES) != 0
    }

    #[must_use]
    pub const fn can_grant_permission(self, bit: u64) -> bool {
        if self.is_owner || (self.effective_permissions & PERM_ADMINISTRATOR) != 0 {
            return true;
        }
        self.has_manage_roles() && (self.effective_permissions & bit) == bit
    }
}

pub fn ensure_actor_can_modify_position(
    actor: ActorHierarchyContext,
    role_pos: i32,
) -> Result<(), RoleError> {
    if !actor.has_manage_roles() {
        return Err(RoleError::MissingManageRolesPermission);
    }
    if !actor.is_owner && role_pos >= actor.highest_position {
        return Err(RoleError::HierarchyViolation {
            role_pos,
            actor_highest_pos: actor.highest_position,
        });
    }
    Ok(())
}

#[must_use]
pub fn sort_guild_roles_hierarchy(roles: &[GuildRole], guild_id: &str) -> Vec<GuildRole> {
    let mut copy = roles.to_vec();
    copy.sort_by(|a, b| {
        let a_everyone = a.id == guild_id || a.name == "@everyone";
        let b_everyone = b.id == guild_id || b.name == "@everyone";
        match (a_everyone, b_everyone) {
            (true, false) => std::cmp::Ordering::Greater,
            (false, true) => std::cmp::Ordering::Less,
            _ => b.position.cmp(&a.position).then_with(|| b.id.cmp(&a.id)),
        }
    });
    copy
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RolePositionPatchEntry {
    pub id: String,
    pub position: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoleHierarchyReorderResult {
    pub method: &'static str,
    pub route: String,
    pub drop_indicator_line: String,
    pub ordered_roles: Vec<GuildRole>,
    pub patch_payload: Vec<RolePositionPatchEntry>,
}

pub fn reorder_guild_roles_drag_and_drop(
    roles: &[GuildRole],
    guild_id: &str,
    dragged_role_id: &str,
    target_role_id: &str,
    placement: DropPlacement,
    actor: ActorHierarchyContext,
) -> Result<RoleHierarchyReorderResult, RoleError> {
    validate_snowflake(guild_id)?;
    validate_snowflake(dragged_role_id)?;
    validate_snowflake(target_role_id)?;
    if dragged_role_id == guild_id || target_role_id == guild_id {
        return Err(RoleError::EveryoneRoleImmutable);
    }
    let sorted = sort_guild_roles_hierarchy(roles, guild_id);
    let dragged = sorted
        .iter()
        .find(|r| r.id == dragged_role_id)
        .ok_or_else(|| RoleError::RoleNotFound(dragged_role_id.to_string()))?;
    let target = sorted
        .iter()
        .find(|r| r.id == target_role_id)
        .ok_or_else(|| RoleError::RoleNotFound(target_role_id.to_string()))?;
    if dragged.managed {
        return Err(RoleError::ManagedRoleImmutable(dragged.id.clone()));
    }
    ensure_actor_can_modify_position(actor, dragged.position)?;
    ensure_actor_can_modify_position(actor, target.position)?;
    build_reordered_hierarchy_payload(
        &sorted,
        guild_id,
        dragged_role_id,
        target_role_id,
        placement,
    )
}

fn build_reordered_hierarchy_payload(
    sorted: &[GuildRole],
    guild_id: &str,
    dragged_role_id: &str,
    target_role_id: &str,
    placement: DropPlacement,
) -> Result<RoleHierarchyReorderResult, RoleError> {
    let mut non_ev: Vec<GuildRole> = sorted
        .iter()
        .filter(|r| r.id != guild_id && r.name != "@everyone")
        .cloned()
        .collect();
    let d_idx = non_ev
        .iter()
        .position(|r| r.id == dragged_role_id)
        .ok_or_else(|| RoleError::RoleNotFound(dragged_role_id.to_string()))?;
    let dragged = non_ev.remove(d_idx);
    let t_idx = non_ev
        .iter()
        .position(|r| r.id == target_role_id)
        .ok_or_else(|| RoleError::RoleNotFound(target_role_id.to_string()))?;
    non_ev.insert(
        t_idx + usize::from(placement == DropPlacement::After),
        dragged,
    );
    let total = non_ev.len() as i32;
    for (idx, r) in non_ev.iter_mut().enumerate() {
        r.position = total - (idx as i32);
    }
    let patch_payload = non_ev
        .iter()
        .map(|r| RolePositionPatchEntry {
            id: r.id.clone(),
            position: r.position,
        })
        .collect();
    if let Some(mut ev) = sorted.iter().find(|r| r.id == guild_id).cloned() {
        ev.position = 0;
        non_ev.push(ev);
    }
    Ok(RoleHierarchyReorderResult {
        method: "PATCH",
        route: format!("/guilds/{guild_id}/roles"),
        drop_indicator_line: format!("{}:{target_role_id}", placement.as_str()),
        ordered_roles: non_ev,
        patch_payload,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionsBitmaskDraft {
    pub guild_id: String,
    pub role_id: String,
    pub name: String,
    pub color: u32,
    pub hoist: bool,
    pub mentionable: bool,
    pub position: i32,
    pub managed: bool,
    pub original_bits: u64,
    pub current_bits: u64,
}

impl PermissionsBitmaskDraft {
    #[must_use]
    pub fn from_role(guild_id: &str, role: &GuildRole) -> Self {
        Self {
            guild_id: guild_id.to_string(),
            role_id: role.id.clone(),
            name: role.name.clone(),
            color: role.color,
            hoist: role.hoist,
            mentionable: role.mentionable,
            position: role.position,
            managed: role.managed,
            original_bits: role.permissions,
            current_bits: role.permissions,
        }
    }

    #[must_use]
    pub const fn is_dirty(&self) -> bool {
        self.current_bits != self.original_bits
    }

    #[must_use]
    pub const fn has_administrator_warning(&self) -> bool {
        (self.current_bits & PERM_ADMINISTRATOR) != 0
    }

    pub fn toggle_permission_bit(
        &mut self,
        bit: u64,
        actor: ActorHierarchyContext,
    ) -> Result<u64, RoleError> {
        if self.managed {
            return Err(RoleError::ManagedRoleImmutable(self.role_id.clone()));
        }
        ensure_actor_can_modify_position(actor, self.position)?;
        if !actor.can_grant_permission(bit) {
            return Err(RoleError::PrivilegeEscalationDenied(bit));
        }
        self.current_bits ^= bit;
        Ok(self.current_bits)
    }

    pub fn reset(&mut self) {
        self.current_bits = self.original_bits;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RolePatchSaveRequest {
    pub method: &'static str,
    pub route: String,
    pub permissions_decimal: String,
}

pub fn build_role_save_patch_request(
    draft: &PermissionsBitmaskDraft,
) -> Result<RolePatchSaveRequest, RoleError> {
    validate_snowflake(&draft.guild_id)?;
    validate_snowflake(&draft.role_id)?;
    let name_len = draft.name.trim().chars().count();
    if name_len == 0 {
        return Err(RoleError::EmptyRoleName);
    }
    if name_len > MAX_ROLE_NAME_CHARS {
        return Err(RoleError::RoleNameTooLong(name_len));
    }
    if draft.color > MAX_RGB_COLOR {
        return Err(RoleError::InvalidRgbColor(draft.color));
    }
    Ok(RolePatchSaveRequest {
        method: "PATCH",
        route: format!("/guilds/{}/roles/{}", draft.guild_id, draft.role_id),
        permissions_decimal: draft.current_bits.to_string(),
    })
}

#[derive(Debug, Clone)]
pub struct OptimisticRolesReorderState {
    pub guild_id: String,
    pub roles: Vec<GuildRole>,
    backup: Option<Vec<GuildRole>>,
}

impl OptimisticRolesReorderState {
    #[must_use]
    pub fn new(guild_id: &str, roles: Vec<GuildRole>) -> Self {
        Self {
            guild_id: guild_id.to_string(),
            roles: sort_guild_roles_hierarchy(&roles, guild_id),
            backup: None,
        }
    }

    pub fn apply_optimistic_reorder(
        &mut self,
        dragged_id: &str,
        target_id: &str,
        placement: DropPlacement,
        actor: ActorHierarchyContext,
    ) -> Result<RoleHierarchyReorderResult, RoleError> {
        let res = reorder_guild_roles_drag_and_drop(
            &self.roles,
            &self.guild_id,
            dragged_id,
            target_id,
            placement,
            actor,
        )?;
        self.backup = Some(self.roles.clone());
        self.roles = res.ordered_roles.clone();
        Ok(res)
    }

    pub fn rollback(&mut self) -> bool {
        let Some(prev) = self.backup.take() else {
            return false;
        };
        self.roles = prev;
        true
    }
}

pub fn validate_system_boundaries(
    message_chars: usize,
    attachments_count: usize,
    attachment_bytes: u64,
    search_offset: u32,
    volume_pct: u16,
) -> Result<(), RoleError> {
    if message_chars > MAX_MESSAGE_CHARS {
        return Err(RoleError::MessageTooLong(message_chars));
    }
    if attachments_count > MAX_ATTACHMENTS_COUNT {
        return Err(RoleError::TooManyAttachments(attachments_count));
    }
    if attachment_bytes > MAX_ATTACHMENT_BYTES {
        return Err(RoleError::AttachmentTooLarge(attachment_bytes));
    }
    if search_offset > MAX_SEARCH_OFFSET {
        return Err(RoleError::SearchOffsetOutOfBounds(search_offset));
    }
    if volume_pct > MAX_VOLUME_PERCENT {
        return Err(RoleError::VolumeOutOfBounds(volume_pct));
    }
    Ok(())
}

#[must_use]
pub fn redact_roles_diagnostic(raw: &str) -> String {
    let mut out = raw.to_string();
    for marker in ["mfa.", "Bearer "] {
        if let Some(pos) = out.find(marker) {
            let start = pos + marker.len();
            let end = out[start..]
                .find(|c: char| c.is_whitespace() || c == '"' || c == '\'')
                .map_or(out.len(), |i| start + i);
            out.replace_range(start..end, "[REDACTED_TOKEN]");
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn role(id: u64, name: &str, pos: i32, color: u32, perms: u64, count: u32) -> GuildRole {
        GuildRole {
            id: id.to_string(),
            name: name.to_string(),
            position: pos,
            color,
            hoist: pos > 0,
            managed: false,
            mentionable: pos > 0 && pos < 4,
            permissions: perms,
            member_count: count,
        }
    }

    fn sample_guild_roles() -> Vec<GuildRole> {
        let base = 100_000_000_000_000_000u64;
        vec![
            role(base + 1, "@everyone", 0, 0, PERM_VIEW_CHANNEL, 150),
            role(base + 2, "Developer", 1, 0x3498DB, PERM_VIEW_CHANNEL, 12),
            role(base + 3, "Moderator", 2, 0x2ECC71, PERM_MANAGE_ROLES, 4),
            role(base + 5, "Server Admin", 4, 0xE74C3C, PERM_ADMINISTRATOR, 2),
        ]
    }

    #[test]
    fn test_sub_1007_1_oracle_tokens_and_permission_constants() {
        assert_eq!(ROLES_DIALOG_CONTAINER, "Roles Manager Dialog");
        assert_eq!(ROLES_DIALOG_DIMENSIONS, "720px x 540px");
        assert_eq!(ROLES_DIALOG_PADDING_PX, 16);
        assert_eq!(ROLES_DIALOG_SPACING_PX, 12);
        assert_eq!(ROLES_SIDEBAR_WIDTH_PX, 320);
        assert_eq!(TOKEN_BACKGROUND, "#313338");
        assert_eq!(TOKEN_SURFACE, "#2b2d31");
        assert_eq!(TOKEN_TEXT_PRIMARY, "#ffffff");
        assert_eq!(TOKEN_ACCENT, "#5865f2");
        assert_eq!(TOKEN_BORDER, "#3f4147");
        assert_eq!(PERM_ADMINISTRATOR, 8);
        assert_eq!(PERM_MANAGE_ROLES, 1 << 28);
        assert_eq!(PERM_MODERATE_MEMBERS, 1 << 40);
        assert_eq!(ALL_V9_DEFINED_PERMISSIONS.count_ones(), 41);
        assert_eq!(
            RoleItemInteractiveState::resolve(false, false).as_str(),
            "row"
        );
        assert_eq!(
            RoleItemInteractiveState::resolve(false, true).as_str(),
            "reorder icon"
        );
        assert_eq!(
            RoleItemInteractiveState::resolve(true, false).as_str(),
            "selected role"
        );
    }

    #[test]
    fn test_sub_1007_2_hierarchy_reorder_and_anti_escalation() {
        let guild_id = "100000000000000001";
        let roles = sample_guild_roles();
        let sorted = sort_guild_roles_hierarchy(&roles, guild_id);
        assert_eq!(sorted[0].position, 4);
        assert_eq!(sorted[1].position, 2);
        assert_eq!(sorted[2].position, 1);
        assert_eq!(sorted[3].position, 0);

        let admin = ActorHierarchyContext {
            effective_permissions: PERM_MANAGE_ROLES | PERM_ADMINISTRATOR,
            highest_position: 4,
            is_owner: false,
        };
        let reordered = reorder_guild_roles_drag_and_drop(
            &roles,
            guild_id,
            "100000000000000002",
            "100000000000000003",
            DropPlacement::Before,
            admin,
        )
        .unwrap();
        assert_eq!(reordered.method, "PATCH");
        assert_eq!(reordered.route, "/guilds/100000000000000001/roles");
        assert_eq!(reordered.ordered_roles[1].id, "100000000000000002");

        let moderator = ActorHierarchyContext {
            effective_permissions: PERM_MANAGE_ROLES,
            highest_position: 2,
            is_owner: false,
        };
        let denied = reorder_guild_roles_drag_and_drop(
            &roles,
            guild_id,
            "100000000000000002",
            "100000000000000003",
            DropPlacement::Before,
            moderator,
        );
        assert!(matches!(denied, Err(RoleError::HierarchyViolation { .. })));
        assert_eq!(parse_role_hex_color("#5865f2").unwrap(), 0x5865F2);
        assert_eq!(format_role_hex_color(0), "#99AAB5");
    }

    #[test]
    fn test_sub_1007_3_bitmask_editor_and_dialog_spec() {
        let roles = sample_guild_roles();
        let mut draft = PermissionsBitmaskDraft::from_role("100000000000000001", &roles[1]);
        assert!(!draft.is_dirty());
        let limited_actor = ActorHierarchyContext {
            effective_permissions: PERM_MANAGE_ROLES | PERM_MODERATE_MEMBERS,
            highest_position: 3,
            is_owner: false,
        };
        draft
            .toggle_permission_bit(PERM_MODERATE_MEMBERS, limited_actor)
            .unwrap();
        assert!(draft.is_dirty());
        assert_ne!(draft.current_bits & PERM_MODERATE_MEMBERS, 0);

        let esc = draft.toggle_permission_bit(PERM_ADMINISTRATOR, limited_actor);
        assert_eq!(
            esc,
            Err(RoleError::PrivilegeEscalationDenied(PERM_ADMINISTRATOR))
        );

        let owner = ActorHierarchyContext {
            effective_permissions: 0,
            highest_position: 1,
            is_owner: true,
        };
        draft
            .toggle_permission_bit(PERM_ADMINISTRATOR, owner)
            .unwrap();
        assert!(draft.has_administrator_warning());
        let req = build_role_save_patch_request(&draft).unwrap();
        assert_eq!(
            req.route,
            "/guilds/100000000000000001/roles/100000000000000002"
        );
        assert_eq!(MAX_BADGE_CAP, 99);
        assert_eq!(format_member_badge(150), "99+");
    }

    #[test]
    fn test_sub_1007_4_nine_dimensions_and_rollback() {
        assert!(validate_snowflake("0").is_err());
        assert_eq!(
            validate_snowflake("18446744073709551615").unwrap(),
            u64::MAX
        );
        assert!(validate_system_boundaries(2_000, 10, 25 * 1024 * 1024, 9_975, 200).is_ok());
        assert!(validate_system_boundaries(2_001, 10, 100, 0, 100).is_err());

        let guild_id = "100000000000000001";
        let mut state = OptimisticRolesReorderState::new(guild_id, sample_guild_roles());
        let owner = ActorHierarchyContext {
            effective_permissions: ALL_V9_DEFINED_PERMISSIONS,
            highest_position: 10,
            is_owner: true,
        };
        state
            .apply_optimistic_reorder(
                "100000000000000002",
                "100000000000000003",
                DropPlacement::Before,
                owner,
            )
            .unwrap();
        assert_eq!(state.roles[1].id, "100000000000000002");
        assert!(state.rollback());
        assert_eq!(state.roles[1].id, "100000000000000003");

        let redacted = redact_roles_diagnostic("Authorization: Bearer mfa.secret_token_999");
        assert!(!redacted.contains("secret_token_999"));
        assert!(redacted.contains("[REDACTED_TOKEN]"));
    }
}
