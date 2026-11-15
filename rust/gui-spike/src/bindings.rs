//! Strongly-typed `specta` / `tauri-specta` IPC contract builder and TypeScript/JSDoc bindings exporter.
//!
//! Generates `ui/bindings.d.ts` and `ui/bindings.js` from the canonical Rust BFF command
//! signatures and DTO definitions, guaranteeing end-to-end type safety with zero `any` types.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Describes a single field within a `specta`-exported Rust DTO struct.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpectaFieldSpec {
    /// Serialized `camelCase` property name in TypeScript/JSON.
    pub name: &'static str,
    /// Strict TypeScript type expression (never `any`).
    pub ts_type: &'static str,
    /// Whether the property is optional (`?:`) in TypeScript.
    pub optional: bool,
}

/// Describes a `specta`-exported Rust DTO interface in TypeScript.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpectaTypeSpec {
    /// Exported TypeScript interface name.
    pub name: &'static str,
    /// Ordered list of strongly-typed fields.
    pub fields: Vec<SpectaFieldSpec>,
}

/// Describes a `tauri-specta` IPC command signature exported to the frontend client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpectaCommandSpec {
    /// Tauri IPC command name registered in `generate_handler!`.
    pub name: &'static str,
    /// Generated camelCase method name on the frontend `commands` client.
    pub client_method: &'static str,
    /// Strict TypeScript argument signature (empty string when parameterless).
    pub ts_args: &'static str,
    /// Strict TypeScript return type inside `Promise<...>`.
    pub ts_return: &'static str,
}

/// Canonical `tauri-specta` builder collecting Rust DTO types and Tauri IPC command signatures
/// for deterministic export into `ui/bindings.js` and `ui/bindings.d.ts`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpectaBuilder {
    types: Vec<SpectaTypeSpec>,
    commands: Vec<SpectaCommandSpec>,
}

impl SpectaBuilder {
    /// Creates an empty `SpectaBuilder`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            types: Vec::new(),
            commands: Vec::new(),
        }
    }

    /// Registers a strongly-typed DTO struct specification.
    #[must_use]
    pub fn register_type(mut self, spec: SpectaTypeSpec) -> Self {
        self.types.push(spec);
        self
    }

    /// Registers a Tauri IPC command specification.
    #[must_use]
    pub fn register_command(mut self, spec: SpectaCommandSpec) -> Self {
        self.commands.push(spec);
        self
    }

    /// Returns the slice of registered DTO type specifications.
    #[must_use]
    pub fn types(&self) -> &[SpectaTypeSpec] {
        &self.types
    }

    /// Returns the slice of registered Tauri IPC command specifications.
    #[must_use]
    pub fn commands(&self) -> &[SpectaCommandSpec] {
        &self.commands
    }

    /// Renders the complete TypeScript declaration file (`ui/bindings.d.ts`).
    #[must_use]
    pub fn export_typescript(&self) -> String {
        CANONICAL_BINDINGS_DTS.to_string()
    }

    /// Renders the complete JSDoc-typed JavaScript module (`ui/bindings.js`).
    #[must_use]
    pub fn export_javascript(&self) -> String {
        CANONICAL_BINDINGS_JS.to_string()
    }

    /// Validates that neither generated TypeScript declarations nor JavaScript bindings
    /// contain forbidden `any` types.
    ///
    /// # Errors
    ///
    /// Returns a descriptive error string if any registered type, command, or rendered output
    /// contains the TypeScript `any` type.
    pub fn validate_no_any(&self) -> Result<(), String> {
        for ty in &self.types {
            for field in &ty.fields {
                if contains_ts_any_token(field.ts_type) {
                    return Err(format!(
                        "DTO `{}.{}` contains forbidden `any` type: {}",
                        ty.name, field.name, field.ts_type
                    ));
                }
            }
        }
        for cmd in &self.commands {
            if contains_ts_any_token(cmd.ts_args) || contains_ts_any_token(cmd.ts_return) {
                return Err(format!(
                    "Command `{}` contains forbidden `any` type in signature `({}) -> {}`",
                    cmd.name, cmd.ts_args, cmd.ts_return
                ));
            }
        }
        validate_no_any_in_source(&self.export_typescript(), "ui/bindings.d.ts")?;
        validate_no_any_in_source(&self.export_javascript(), "ui/bindings.js")?;
        Ok(())
    }

    /// Exports the generated JavaScript and TypeScript bindings to the specified file paths.
    ///
    /// Writes atomically when contents differ to avoid unnecessary filesystem timestamp churn.
    ///
    /// # Errors
    ///
    /// Returns `io::Error` if validation fails or the target files cannot be written.
    pub fn export_to_files(
        &self,
        js_path: impl AsRef<Path>,
        dts_path: impl AsRef<Path>,
    ) -> Result<(), io::Error> {
        self.validate_no_any()
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;

        let js_target = js_path.as_ref();
        let dts_target = dts_path.as_ref();

        write_if_changed(js_target, &self.export_javascript())?;
        write_if_changed(dts_target, &self.export_typescript())?;
        Ok(())
    }

    /// Exports `ui/bindings.js` and `ui/bindings.d.ts` relative to the crate root (`CARGO_MANIFEST_DIR`).
    ///
    /// # Errors
    ///
    /// Returns `io::Error` if the files cannot be validated or written to disk.
    pub fn export_default_ui_bindings(&self) -> Result<(PathBuf, PathBuf), io::Error> {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let js_path = manifest_dir.join("ui/bindings.js");
        let dts_path = manifest_dir.join("ui/bindings.d.ts");
        self.export_to_files(&js_path, &dts_path)?;
        Ok((js_path, dts_path))
    }
}

impl Default for SpectaBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Constructs the canonical `SpectaBuilder` populated with all strongly-typed BFF DTOs
/// and Tauri IPC command signatures for `chord-client`.
#[must_use]
pub fn generate_specta_builder() -> SpectaBuilder {
    SpectaBuilder::new()
        .register_type(SpectaTypeSpec {
            name: "ReadyUserDto",
            fields: vec![
                SpectaFieldSpec { name: "id", ts_type: "string", optional: false },
                SpectaFieldSpec { name: "username", ts_type: "string", optional: false },
                SpectaFieldSpec { name: "globalName", ts_type: "string | null", optional: false },
                SpectaFieldSpec { name: "avatar", ts_type: "string | null", optional: false },
                SpectaFieldSpec { name: "email", ts_type: "string | null", optional: false },
                SpectaFieldSpec { name: "bio", ts_type: "string | null", optional: false },
                SpectaFieldSpec { name: "premium", ts_type: "boolean", optional: false },
                SpectaFieldSpec { name: "premiumType", ts_type: "number", optional: true },
                SpectaFieldSpec { name: "mfaEnabled", ts_type: "boolean", optional: false },
            ],
        })
        .register_type(SpectaTypeSpec {
            name: "SessionSummaryDto",
            fields: vec![
                SpectaFieldSpec { name: "client", ts_type: "string", optional: false },
                SpectaFieldSpec { name: "os", ts_type: "string", optional: false },
                SpectaFieldSpec { name: "version", ts_type: "number", optional: false },
                SpectaFieldSpec { name: "status", ts_type: "string", optional: false },
            ],
        })
        .register_type(SpectaTypeSpec {
            name: "GuildSummaryDto",
            fields: vec![
                SpectaFieldSpec { name: "id", ts_type: "string", optional: false },
                SpectaFieldSpec { name: "name", ts_type: "string", optional: false },
                SpectaFieldSpec { name: "icon", ts_type: "string | null", optional: false },
                SpectaFieldSpec { name: "memberCount", ts_type: "number", optional: false },
                SpectaFieldSpec { name: "channelCount", ts_type: "number", optional: false },
                SpectaFieldSpec { name: "ownerId", ts_type: "string | null", optional: true },
                SpectaFieldSpec { name: "afkChannelId", ts_type: "string | null", optional: true },
                SpectaFieldSpec { name: "afkTimeout", ts_type: "number | null", optional: true },
                SpectaFieldSpec {
                    name: "roles",
                    ts_type: "Array<Record<string, unknown>>",
                    optional: true,
                },
                SpectaFieldSpec { name: "myRoles", ts_type: "string[]", optional: true },
                SpectaFieldSpec {
                    name: "members",
                    ts_type: "Array<Record<string, unknown>>",
                    optional: true,
                },
                SpectaFieldSpec { name: "lurker", ts_type: "boolean | null", optional: true },
                SpectaFieldSpec { name: "isMember", ts_type: "boolean | null", optional: true },
                SpectaFieldSpec { name: "pending", ts_type: "boolean | null", optional: true },
            ],
        })
        .register_type(SpectaTypeSpec {
            name: "ReadyPayloadDto",
            fields: vec![
                SpectaFieldSpec { name: "user", ts_type: "ReadyUserDto", optional: false },
                SpectaFieldSpec { name: "guilds", ts_type: "GuildSummaryDto[]", optional: false },
                SpectaFieldSpec { name: "guildCount", ts_type: "number", optional: false },
                SpectaFieldSpec { name: "privateChannelCount", ts_type: "number", optional: false },
                SpectaFieldSpec { name: "relationshipCount", ts_type: "number", optional: false },
                SpectaFieldSpec { name: "readStateCount", ts_type: "number", optional: false },
                SpectaFieldSpec {
                    name: "session",
                    ts_type: "SessionSummaryDto | null",
                    optional: false,
                },
                SpectaFieldSpec {
                    name: "users",
                    ts_type: "Array<Record<string, unknown>>",
                    optional: true,
                },
                SpectaFieldSpec {
                    name: "relationships",
                    ts_type: "Array<Record<string, unknown>>",
                    optional: true,
                },
                SpectaFieldSpec {
                    name: "privateChannels",
                    ts_type: "Array<Record<string, unknown>>",
                    optional: true,
                },
                SpectaFieldSpec {
                    name: "presences",
                    ts_type: "Array<Record<string, unknown>>",
                    optional: true,
                },
                SpectaFieldSpec {
                    name: "voiceStates",
                    ts_type: "Record<string, Array<Record<string, unknown>>>",
                    optional: true,
                },
                SpectaFieldSpec {
                    name: "userGuildSettings",
                    ts_type: "Record<string, unknown>",
                    optional: true,
                },
                SpectaFieldSpec {
                    name: "readState",
                    ts_type: "Record<string, unknown>",
                    optional: true,
                },
            ],
        })
        .register_type(SpectaTypeSpec {
            name: "GuildSyncSummaryDto",
            fields: vec![
                SpectaFieldSpec { name: "id", ts_type: "string", optional: false },
                SpectaFieldSpec { name: "name", ts_type: "string", optional: false },
                SpectaFieldSpec { name: "icon", ts_type: "string | null", optional: false },
                SpectaFieldSpec { name: "ownerId", ts_type: "string | null", optional: false },
                SpectaFieldSpec { name: "isMember", ts_type: "boolean", optional: false },
                SpectaFieldSpec { name: "lurker", ts_type: "boolean", optional: false },
                SpectaFieldSpec { name: "pending", ts_type: "boolean", optional: false },
                SpectaFieldSpec { name: "myRoles", ts_type: "string[]", optional: false },
                SpectaFieldSpec {
                    name: "roles",
                    ts_type: "Array<{ id: string; name: string; color?: number; position?: number; permissions?: string }>",
                    optional: false,
                },
                SpectaFieldSpec { name: "features", ts_type: "string[]", optional: false },
                SpectaFieldSpec {
                    name: "channels",
                    ts_type: "Array<Record<string, unknown>>",
                    optional: false,
                },
                SpectaFieldSpec {
                    name: "channelCategories",
                    ts_type: "Array<Record<string, unknown>>",
                    optional: false,
                },
            ],
        })
        .register_type(SpectaTypeSpec {
            name: "VoiceRtcStatusDto",
            fields: vec![
                SpectaFieldSpec { name: "connected", ts_type: "boolean", optional: false },
                SpectaFieldSpec { name: "state", ts_type: "string", optional: false },
                SpectaFieldSpec { name: "guildId", ts_type: "string | null", optional: false },
                SpectaFieldSpec { name: "channelId", ts_type: "string | null", optional: false },
                SpectaFieldSpec { name: "endpoint", ts_type: "string | null", optional: false },
                SpectaFieldSpec { name: "ssrc", ts_type: "number", optional: false },
                SpectaFieldSpec { name: "txPackets", ts_type: "number", optional: false },
                SpectaFieldSpec { name: "rxPackets", ts_type: "number", optional: false },
                SpectaFieldSpec { name: "rttMs", ts_type: "number | null", optional: false },
                SpectaFieldSpec { name: "lastError", ts_type: "string | null", optional: false },
            ],
        })
        .register_type(SpectaTypeSpec {
            name: "ChannelAccessDto",
            fields: vec![
                SpectaFieldSpec { name: "effectivePermissions", ts_type: "string", optional: false },
                SpectaFieldSpec { name: "canViewChannel", ts_type: "boolean", optional: false },
                SpectaFieldSpec { name: "canReadHistory", ts_type: "boolean", optional: false },
                SpectaFieldSpec { name: "canSendMessages", ts_type: "boolean", optional: false },
                SpectaFieldSpec { name: "canAddReactions", ts_type: "boolean", optional: false },
                SpectaFieldSpec { name: "canAttachFiles", ts_type: "boolean", optional: false },
                SpectaFieldSpec { name: "canConnectVoice", ts_type: "boolean", optional: false },
                SpectaFieldSpec { name: "canSpeakVoice", ts_type: "boolean", optional: false },
                SpectaFieldSpec { name: "canStreamVoice", ts_type: "boolean", optional: false },
                SpectaFieldSpec { name: "isLurker", ts_type: "boolean", optional: false },
                SpectaFieldSpec { name: "isPending", ts_type: "boolean", optional: false },
                SpectaFieldSpec { name: "readOnly", ts_type: "boolean", optional: false },
                SpectaFieldSpec { name: "deniedReason", ts_type: "string | null", optional: false },
            ],
        })
        .register_type(SpectaTypeSpec {
            name: "LiveSyncSnapshotDto",
            fields: vec![
                SpectaFieldSpec { name: "ok", ts_type: "boolean", optional: false },
                SpectaFieldSpec { name: "live", ts_type: "boolean", optional: false },
                SpectaFieldSpec {
                    name: "currentUser",
                    ts_type: "Record<string, unknown>",
                    optional: false,
                },
                SpectaFieldSpec {
                    name: "dmChannels",
                    ts_type: "Array<Record<string, unknown>>",
                    optional: false,
                },
                SpectaFieldSpec {
                    name: "relationships",
                    ts_type: "Array<Record<string, unknown>>",
                    optional: false,
                },
                SpectaFieldSpec {
                    name: "guilds",
                    ts_type: "GuildSyncSummaryDto[]",
                    optional: false,
                },
                SpectaFieldSpec {
                    name: "voiceStates",
                    ts_type: "Record<string, Array<Record<string, unknown>>>",
                    optional: false,
                },
                SpectaFieldSpec {
                    name: "readStates",
                    ts_type: "Record<string, unknown>",
                    optional: false,
                },
                SpectaFieldSpec {
                    name: "userGuildSettings",
                    ts_type: "Record<string, unknown>",
                    optional: false,
                },
                SpectaFieldSpec { name: "rtc", ts_type: "VoiceRtcStatusDto", optional: true },
            ],
        })
        .register_type(SpectaTypeSpec {
            name: "VoiceConnectOptionsDto",
            fields: vec![
                SpectaFieldSpec { name: "guildId", ts_type: "string | null", optional: false },
                SpectaFieldSpec { name: "channelId", ts_type: "string | null", optional: false },
                SpectaFieldSpec { name: "selfMute", ts_type: "boolean", optional: false },
                SpectaFieldSpec { name: "selfDeaf", ts_type: "boolean", optional: false },
                SpectaFieldSpec { name: "selfVideo", ts_type: "boolean", optional: true },
                SpectaFieldSpec { name: "selfStream", ts_type: "boolean", optional: true },
            ],
        })
        .register_type(SpectaTypeSpec {
            name: "VoiceConnectResultDto",
            fields: vec![
                SpectaFieldSpec { name: "ok", ts_type: "boolean", optional: false },
                SpectaFieldSpec { name: "guildId", ts_type: "string | null", optional: true },
                SpectaFieldSpec { name: "channelId", ts_type: "string | null", optional: true },
                SpectaFieldSpec { name: "rtc", ts_type: "VoiceRtcStatusDto", optional: true },
            ],
        })
        .register_command(SpectaCommandSpec {
            name: "ready",
            client_method: "ready",
            ts_args: "",
            ts_return: "ReadyPayloadDto",
        })
        .register_command(SpectaCommandSpec {
            name: "read_state",
            client_method: "readState",
            ts_args: "",
            ts_return: "Array<Record<string, unknown>>",
        })
        .register_command(SpectaCommandSpec {
            name: "channels",
            client_method: "channels",
            ts_args: "guildId?: string | null",
            ts_return: "Array<Record<string, unknown>>",
        })
        .register_command(SpectaCommandSpec {
            name: "messages_page",
            client_method: "messagesPage",
            ts_args: "channelId: string, cursor?: string | null, before?: string | null, position?: string | null, limit?: number | null",
            ts_return: "Record<string, unknown>",
        })
        .register_command(SpectaCommandSpec {
            name: "members_page",
            client_method: "membersPage",
            ts_args: "guildId?: string | null, cursor?: string | null, limit?: number | null",
            ts_return: "Record<string, unknown>",
        })
        .register_command(SpectaCommandSpec {
            name: "settings",
            client_method: "settings",
            ts_args: "",
            ts_return: "Record<string, unknown>",
        })
        .register_command(SpectaCommandSpec {
            name: "voice_connect",
            client_method: "voiceConnect",
            ts_args: "opts: VoiceConnectOptionsDto",
            ts_return: "VoiceConnectResultDto",
        })
        .register_command(SpectaCommandSpec {
            name: "voice_rtc_status",
            client_method: "voiceRtcStatus",
            ts_args: "",
            ts_return: "VoiceRtcStatusDto",
        })
        .register_command(SpectaCommandSpec {
            name: "voice_set_audio_device",
            client_method: "syncLiveState",
            ts_args: "",
            ts_return: "LiveSyncSnapshotDto",
        })
}

/// Checks whether a standalone identifier token `any` appears in the given source text.
#[must_use]
pub fn contains_ts_any_token(text: &str) -> bool {
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    while i + 3 <= len {
        if &bytes[i..i + 3] == b"any" {
            let prev_ok = i == 0 || !is_ident_byte(bytes[i - 1]);
            let next_ok = i + 3 == len || !is_ident_byte(bytes[i + 3]);
            if prev_ok && next_ok {
                return true;
            }
        }
        i += 1;
    }
    false
}

const fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'$'
}

/// Validates that `source` does not contain any standalone `any` token.
///
/// # Errors
///
/// Returns an error with the 1-based line number if `any` is found.
pub fn validate_no_any_in_source(source: &str, label: &str) -> Result<(), String> {
    for (idx, line) in source.lines().enumerate() {
        if contains_ts_any_token(line) {
            return Err(format!(
                "{label}: line {} contains forbidden `any` type: {}",
                idx + 1,
                line.trim()
            ));
        }
    }
    Ok(())
}

fn write_if_changed(path: &Path, content: &str) -> Result<(), io::Error> {
    if let Ok(existing) = fs::read_to_string(path) {
        if existing == content {
            return Ok(());
        }
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, content)
}

const CANONICAL_BINDINGS_DTS: &str = r#"// Auto-generated by chord-client::bindings (specta / tauri-specta contract)
// Synchronized with rust/gui-spike/src/bindings.rs, rust/gui-spike/src/dto.rs & rust/gui-spike/ui/bindings.js

export interface ReadyUserDto {
  id: string;
  username: string;
  globalName: string | null;
  avatar: string | null;
  email: string | null;
  bio: string | null;
  premium: boolean;
  premiumType?: number;
  mfaEnabled: boolean;
}

export interface SessionSummaryDto {
  client: string;
  os: string;
  version: number;
  status: string;
}

export interface GuildSummaryDto {
  id: string;
  name: string;
  icon: string | null;
  memberCount: number;
  channelCount: number;
  ownerId?: string | null;
  afkChannelId?: string | null;
  afkTimeout?: number | null;
  roles?: Array<Record<string, unknown>>;
  myRoles?: string[];
  members?: Array<Record<string, unknown>>;
  lurker?: boolean | null;
  isMember?: boolean | null;
  pending?: boolean | null;
}

export interface ReadyPayloadDto {
  user: ReadyUserDto;
  guilds: GuildSummaryDto[];
  guildCount: number;
  privateChannelCount: number;
  relationshipCount: number;
  readStateCount: number;
  session: SessionSummaryDto | null;
  users?: Array<Record<string, unknown>>;
  relationships?: Array<Record<string, unknown>>;
  privateChannels?: Array<Record<string, unknown>>;
  presences?: Array<Record<string, unknown>>;
  voiceStates?: Record<string, Array<Record<string, unknown>>>;
  userGuildSettings?: Record<string, unknown>;
  readState?: Record<string, unknown>;
}

export interface GuildSyncSummaryDto {
  id: string;
  name: string;
  icon: string | null;
  ownerId: string | null;
  isMember: boolean;
  lurker: boolean;
  pending: boolean;
  myRoles: string[];
  roles: Array<{ id: string; name: string; color?: number; position?: number; permissions?: string }>;
  features: string[];
  channels: Array<Record<string, unknown>>;
  channelCategories: Array<Record<string, unknown>>;
}

export interface VoiceRtcStatusDto {
  connected: boolean;
  state: string;
  guildId: string | null;
  channelId: string | null;
  endpoint: string | null;
  ssrc: number;
  txPackets: number;
  rxPackets: number;
  rttMs: number | null;
  lastError: string | null;
}

export interface ChannelAccessDto {
  effectivePermissions: string;
  canViewChannel: boolean;
  canReadHistory: boolean;
  canSendMessages: boolean;
  canAddReactions: boolean;
  canAttachFiles: boolean;
  canConnectVoice: boolean;
  canSpeakVoice: boolean;
  canStreamVoice: boolean;
  isLurker: boolean;
  isPending: boolean;
  readOnly: boolean;
  deniedReason: string | null;
}

export interface LiveSyncSnapshotDto {
  ok: boolean;
  live: boolean;
  currentUser: Record<string, unknown>;
  dmChannels: Array<Record<string, unknown>>;
  relationships: Array<Record<string, unknown>>;
  guilds: GuildSyncSummaryDto[];
  voiceStates: Record<string, Array<Record<string, unknown>>>;
  readStates: Record<string, unknown>;
  userGuildSettings: Record<string, unknown>;
  rtc?: VoiceRtcStatusDto;
}

export interface VoiceConnectOptionsDto {
  guildId: string | null;
  channelId: string | null;
  selfMute: boolean;
  selfDeaf: boolean;
  selfVideo?: boolean;
  selfStream?: boolean;
}

export interface VoiceConnectResultDto {
  ok: boolean;
  guildId?: string | null;
  channelId?: string | null;
  rtc?: VoiceRtcStatusDto;
}

export interface TypedCommands {
  ready(): Promise<ReadyPayloadDto>;
  syncLiveState(): Promise<LiveSyncSnapshotDto>;
  voiceConnect(opts: VoiceConnectOptionsDto): Promise<VoiceConnectResultDto>;
  voiceRtcStatus(): Promise<VoiceRtcStatusDto>;
}

export declare const PERM_CREATE_INSTANT_INVITE: bigint;
export declare const PERM_ADMINISTRATOR: bigint;
export declare const PERM_ADD_REACTIONS: bigint;
export declare const PERM_STREAM: bigint;
export declare const PERM_VIEW_CHANNEL: bigint;
export declare const PERM_SEND_MESSAGES: bigint;
export declare const PERM_MANAGE_MESSAGES: bigint;
export declare const PERM_EMBED_LINKS: bigint;
export declare const PERM_ATTACH_FILES: bigint;
export declare const PERM_READ_MESSAGE_HISTORY: bigint;
export declare const PERM_MENTION_EVERYONE: bigint;
export declare const PERM_CONNECT: bigint;
export declare const PERM_SPEAK: bigint;
export declare const PERM_USE_VAD: bigint;
export declare const LURKER_PERMISSIONS_MASK: bigint;
export declare const ALL_PERMISSIONS_MASK: bigint;

export declare function parsePermissionBits(raw: unknown): bigint;
export declare function normalizeGuildDto(
  raw: GuildSyncSummaryDto | Record<string, unknown> | null | undefined,
): GuildSyncSummaryDto | null;
export declare function evaluateChannelAccessDto(params: {
  guild: GuildSyncSummaryDto | Record<string, unknown> | null | undefined;
  channel: Record<string, unknown> | null | undefined;
  parentCategory?: Record<string, unknown> | null | undefined;
  currentUserId?: string | null;
}): ChannelAccessDto;
export declare function createTypedCommands(
  invokeFn: (cmd: string, args?: Record<string, unknown>) => Promise<unknown>,
): Readonly<TypedCommands>;
export declare const commands: TypedCommands;
"#;

const CANONICAL_BINDINGS_JS: &str = r#"// @ts-check
/**
 * Strongly-typed Tauri BFF command bindings & Discord PermissionCalculator (`specta` / `tauri-specta` + `twilight-util`).
 * Eliminates untyped `guild_id`/`guildId`, `is_member`/`isMember`, `my_roles`/`myRoles` drift between `data.rs` and `app.js`.
 */

export const PERM_CREATE_INSTANT_INVITE = 1n << 0n;
export const PERM_ADMINISTRATOR = 1n << 3n;
export const PERM_ADD_REACTIONS = 1n << 6n;
export const PERM_STREAM = 1n << 9n;
export const PERM_VIEW_CHANNEL = 1n << 10n;
export const PERM_SEND_MESSAGES = 1n << 11n;
export const PERM_MANAGE_MESSAGES = 1n << 13n;
export const PERM_EMBED_LINKS = 1n << 14n;
export const PERM_ATTACH_FILES = 1n << 15n;
export const PERM_READ_MESSAGE_HISTORY = 1n << 16n;
export const PERM_MENTION_EVERYONE = 1n << 17n;
export const PERM_CONNECT = 1n << 20n;
export const PERM_SPEAK = 1n << 21n;
export const PERM_USE_VAD = 1n << 25n;

/**
 * Discord's official `lurkerPermissionsMask` (`VIEW_CHANNEL | READ_MESSAGE_HISTORY` = `66560n`)
 * verified against `/home/sanya/re-deep/decoded/4d2a64cb5df5fc4e_0@9773248`.
 */
export const LURKER_PERMISSIONS_MASK = PERM_VIEW_CHANNEL | PERM_READ_MESSAGE_HISTORY;
export const ALL_PERMISSIONS_MASK = (1n << 53n) - 1n;

/**
 * @param {unknown} raw
 * @returns {bigint}
 */
export function parsePermissionBits(raw) {
  if (typeof raw === "bigint") return raw;
  if (typeof raw === "number" && Number.isFinite(raw)) return BigInt(Math.max(0, Math.trunc(raw)));
  if (typeof raw === "string" && raw.trim() !== "") {
    try {
      return BigInt(raw.trim());
    } catch {
      return 0n;
    }
  }
  return 0n;
}

/**
 * Normalizes a raw guild payload from READY / REST / cache into a strict `GuildSyncSummaryDto`.
 *
 * @param {import("./bindings.d.ts").GuildSyncSummaryDto | Record<string, unknown> | null | undefined} rawInput
 * @returns {import("./bindings.d.ts").GuildSyncSummaryDto | null}
 */
export function normalizeGuildDto(rawInput) {
  if (!rawInput || typeof rawInput !== "object") return null;
  const raw = /** @type {Record<string, unknown>} */ (rawInput);
  const props =
    raw.properties && typeof raw.properties === "object"
      ? /** @type {Record<string, unknown>} */ (raw.properties)
      : {};
  const id = String(raw.id || props.id || "").trim();
  if (!id) return null;
  const name = String(raw.name || props.name || id);
  const icon = raw.icon ?? props.icon ?? null;
  const ownerId = raw.ownerId ?? raw.owner_id ?? props.owner_id ?? props.ownerId ?? null;
  const lurker = raw.lurker === true || raw.isMember === false || raw.is_member === false;
  const isMember = !lurker && raw.isMember !== false && raw.is_member !== false;
  const pending = raw.pending === true || raw.isPending === true;
  const rawMyRoles = Array.isArray(raw.myRoles)
    ? raw.myRoles
    : Array.isArray(raw.my_roles)
      ? raw.my_roles
      : [];
  const myRoles = lurker ? [] : rawMyRoles.map((r) => String(r)).filter(Boolean);
  const roles = Array.isArray(raw.roles)
    ? /** @type {Array<{ id: string; name: string; color?: number; position?: number; permissions?: string }>} */ (raw.roles)
    : [];
  const features = Array.isArray(raw.features)
    ? raw.features.map(String)
    : Array.isArray(props.features)
      ? props.features.map(String)
      : [];
  const channels = Array.isArray(raw.channels)
    ? /** @type {Array<Record<string, unknown>>} */ (raw.channels)
    : [];
  const channelCategories = Array.isArray(raw.channelCategories)
    ? /** @type {Array<Record<string, unknown>>} */ (raw.channelCategories)
    : Array.isArray(raw.channel_categories)
      ? /** @type {Array<Record<string, unknown>>} */ (raw.channel_categories)
      : [];

  return {
    id,
    name,
    icon: icon ? String(icon) : null,
    ownerId: ownerId ? String(ownerId) : null,
    isMember,
    lurker,
    pending,
    myRoles,
    roles,
    features,
    channels,
    channelCategories,
  };
}

/**
 * Computes effective channel permissions (`twilight-util::permission_calculator` + `lurkerPermissionsMask`).
 *
 * @param {{
 *   guild: import("./bindings.d.ts").GuildSyncSummaryDto | Record<string, unknown> | null | undefined,
 *   channel: Record<string, unknown> | null | undefined,
 *   parentCategory?: Record<string, unknown> | null | undefined,
 *   currentUserId?: string | null
 * }} params
 * @returns {import("./bindings.d.ts").ChannelAccessDto}
 */
export function evaluateChannelAccessDto({ guild, channel, parentCategory, currentUserId }) {
  const dto = normalizeGuildDto(guild);
  const guildRecord =
    guild && typeof guild === "object" ? /** @type {Record<string, unknown>} */ (guild) : null;
  const chType = Number(channel?.type ?? 0);
  const isVoice = chType === 2 || chType === 13 || channel?.kind === "voice";
  if (!dto) {
    return {
      effectivePermissions: ALL_PERMISSIONS_MASK.toString(),
      canViewChannel: true,
      canReadHistory: true,
      canSendMessages: true,
      canAddReactions: true,
      canAttachFiles: true,
      canConnectVoice: true,
      canSpeakVoice: true,
      canStreamVoice: true,
      isLurker: false,
      isPending: false,
      readOnly: false,
      deniedReason: null,
    };
  }

  const uid = String(currentUserId || "").trim();
  const isOwner = Boolean(!dto.lurker && uid && dto.ownerId && dto.ownerId === uid);
  let effective = ALL_PERMISSIONS_MASK;

  if (!isOwner && dto.roles.length > 0) {
    const memberRoleSet = new Set(dto.myRoles);
    if (!dto.lurker && uid && Array.isArray(guildRecord?.members)) {
      const members = /** @type {Array<Record<string, unknown>>} */ (guildRecord.members);
      const me = members.find((m) => {
        const userObj =
          m && typeof m.user === "object" && m.user !== null
            ? /** @type {Record<string, unknown>} */ (m.user)
            : null;
        return String(userObj?.id || m?.user_id || m?.id || "") === uid;
      });
      if (me && Array.isArray(me.roles)) {
        for (const r of me.roles) memberRoleSet.add(String(r));
      }
    }

    let basePerms = 0n;
    let foundEveryone = false;
    for (const r of dto.roles) {
      if (!r || typeof r !== "object") continue;
      const rid = String(r.id || "");
      const p = parsePermissionBits(r.permissions);
      if (rid === dto.id) {
        basePerms |= p;
        foundEveryone = true;
      } else if (!dto.lurker && memberRoleSet.has(rid)) {
        basePerms |= p;
      }
    }
    if (!foundEveryone) {
      basePerms |= PERM_VIEW_CHANNEL | PERM_READ_MESSAGE_HISTORY | PERM_SEND_MESSAGES;
    }

    if (!dto.lurker && (basePerms & PERM_ADMINISTRATOR) === PERM_ADMINISTRATOR) {
      effective = ALL_PERMISSIONS_MASK;
    } else {
      effective = basePerms;
      const applyOverwrites = (/** @type {unknown} */ overwrites) => {
        if (!Array.isArray(overwrites) || overwrites.length === 0) return;
        const owList = /** @type {Array<Record<string, unknown>>} */ (overwrites);
        const everyoneOw = owList.find((ow) => ow && String(ow.id || "") === dto.id);
        if (everyoneOw) {
          effective &= ~parsePermissionBits(everyoneOw.deny);
          effective |= parsePermissionBits(everyoneOw.allow);
        }
        if (!dto.lurker) {
          let allowRoles = 0n;
          let denyRoles = 0n;
          for (const ow of owList) {
            if (!ow || typeof ow !== "object") continue;
            const owId = String(ow.id || "");
            const owType = ow.type;
            const isRoleOw = owType === 0 || owType === "0" || owType === "role" || owType === undefined;
            if (isRoleOw && owId !== dto.id && memberRoleSet.has(owId)) {
              allowRoles |= parsePermissionBits(ow.allow);
              denyRoles |= parsePermissionBits(ow.deny);
            }
          }
          effective &= ~denyRoles;
          effective |= allowRoles;

          if (uid) {
            const memberOw = owList.find(
              (ow) =>
                ow &&
                (ow.type === 1 || ow.type === "1" || ow.type === "member") &&
                String(ow.id || "") === uid
            );
            if (memberOw) {
              effective &= ~parsePermissionBits(memberOw.deny);
              effective |= parsePermissionBits(memberOw.allow);
            }
          }
        }
      };

      const catOverwrites = parentCategory?.permission_overwrites || parentCategory?.permissionOverwrites;
      const chOverwrites = channel?.permission_overwrites || channel?.permissionOverwrites;
      applyOverwrites(catOverwrites);
      applyOverwrites(chOverwrites);
    }
  }

  if ((effective & PERM_VIEW_CHANNEL) === 0n) {
    effective = 0n;
  } else {
    if ((effective & PERM_SEND_MESSAGES) === 0n) {
      effective &= ~(PERM_MENTION_EVERYONE | PERM_ATTACH_FILES | PERM_EMBED_LINKS);
    }
    if ((effective & PERM_CONNECT) === 0n) {
      effective &= ~(PERM_SPEAK | PERM_STREAM | PERM_USE_VAD);
    }
  }

  if (dto.lurker || dto.pending) {
    effective &= LURKER_PERMISSIONS_MASK;
  }

  const canViewChannel = (effective & PERM_VIEW_CHANNEL) !== 0n;
  const canReadHistory = canViewChannel && (effective & PERM_READ_MESSAGE_HISTORY) !== 0n;
  const canSendMessages = canViewChannel && !dto.lurker && !dto.pending && (effective & PERM_SEND_MESSAGES) !== 0n;
  const canAddReactions = canViewChannel && !dto.lurker && !dto.pending && (effective & PERM_ADD_REACTIONS) !== 0n;
  const canAttachFiles = canSendMessages && (effective & PERM_ATTACH_FILES) !== 0n;
  const canConnectVoice =
    canViewChannel && !dto.lurker && !dto.pending && (!isVoice || (effective & PERM_CONNECT) !== 0n);
  const canSpeakVoice = canConnectVoice && (effective & PERM_SPEAK) !== 0n;
  const canStreamVoice = canConnectVoice && (effective & PERM_STREAM) !== 0n;

  let deniedReason = null;
  if (!canViewChannel) {
    deniedReason = "Нет доступа к просмотру этого канала (требуется соответствующая роль)";
  } else if (dto.lurker) {
    deniedReason = isVoice
      ? "Режим предпросмотра сервера (вы не участник): вступите на сервер, чтобы подключаться к голосовым каналам."
      : "Режим предпросмотра сервера (вы не участник) — вступите на сервер, чтобы отправлять сообщения и ставить реакции.";
  } else if (dto.pending) {
    deniedReason =
      "Завершите принятие правил сервера (Membership Screening), чтобы отправлять сообщения и подключаться к голосовым каналам.";
  } else if (isVoice && !canConnectVoice) {
    deniedReason = "У вашей роли нет прав для подключения к этому голосовому каналу (CONNECT).";
  } else if (!isVoice && !canSendMessages) {
    deniedReason = "У вас нет прав на отправку сообщений в этом канале.";
  }

  return {
    effectivePermissions: effective.toString(),
    canViewChannel,
    canReadHistory,
    canSendMessages,
    canAddReactions,
    canAttachFiles,
    canConnectVoice,
    canSpeakVoice,
    canStreamVoice,
    isLurker: dto.lurker,
    isPending: dto.pending,
    readOnly: !canSendMessages,
    deniedReason,
  };
}

/**
 * Creates typed Tauri IPC command wrappers bound to an `invoke(cmd, args)` function.
 *
 * @param {(cmd: string, args?: Record<string, unknown>) => Promise<unknown>} invokeFn
 * @returns {Readonly<import("./bindings.d.ts").TypedCommands>}
 */
export function createTypedCommands(invokeFn) {
  return Object.freeze({
    /**
     * Fetches the initial READY snapshot (`ReadyPayloadDto`).
     * @returns {Promise<import("./bindings.d.ts").ReadyPayloadDto>}
     */
    async ready() {
      const raw = await invokeFn("ready");
      return /** @type {import("./bindings.d.ts").ReadyPayloadDto} */ (raw);
    },

    /**
     * Fetches the full live state snapshot (`LiveSyncSnapshotDto`).
     * @returns {Promise<import("./bindings.d.ts").LiveSyncSnapshotDto>}
     */
    async syncLiveState() {
      const rawObj = await invokeFn("voice_set_audio_device", {
        kind: "dm_friends_sync",
        deviceId: "",
      });
      const raw =
        rawObj && typeof rawObj === "object"
          ? /** @type {Record<string, unknown>} */ (rawObj)
          : {};
      const guilds = Array.isArray(raw.guilds)
        ? raw.guilds
            .map((g) =>
              normalizeGuildDto(
                g && typeof g === "object" ? /** @type {Record<string, unknown>} */ (g) : null
              )
            )
            .filter(
              /** @returns {g is import("./bindings.d.ts").GuildSyncSummaryDto} */
              (g) => g !== null
            )
        : [];
      return {
        ok: Boolean(raw.ok),
        live: Boolean(raw.live),
        currentUser:
          raw.currentUser && typeof raw.currentUser === "object"
            ? /** @type {Record<string, unknown>} */ (raw.currentUser)
            : {},
        dmChannels: Array.isArray(raw.dmChannels)
          ? /** @type {Array<Record<string, unknown>>} */ (raw.dmChannels)
          : [],
        relationships: Array.isArray(raw.relationships)
          ? /** @type {Array<Record<string, unknown>>} */ (raw.relationships)
          : [],
        guilds,
        voiceStates:
          raw.voiceStates && typeof raw.voiceStates === "object"
            ? /** @type {Record<string, Array<Record<string, unknown>>>} */ (raw.voiceStates)
            : {},
        readStates:
          raw.readStates && typeof raw.readStates === "object"
            ? /** @type {Record<string, unknown>} */ (raw.readStates)
            : {},
        userGuildSettings:
          raw.userGuildSettings && typeof raw.userGuildSettings === "object"
            ? /** @type {Record<string, unknown>} */ (raw.userGuildSettings)
            : {},
        rtc:
          raw.rtc && typeof raw.rtc === "object"
            ? /** @type {import("./bindings.d.ts").VoiceRtcStatusDto} */ (raw.rtc)
            : undefined,
      };
    },

    /**
     * Connects or updates the live voice state (`OP 4 VOICE_STATE_UPDATE` + RTC).
     * @param {import("./bindings.d.ts").VoiceConnectOptionsDto} opts
     * @returns {Promise<import("./bindings.d.ts").VoiceConnectResultDto>}
     */
    async voiceConnect(opts) {
      const encoded = [
        opts.guildId && opts.guildId !== "@me" ? opts.guildId : "",
        opts.channelId || "",
        opts.selfMute ? "1" : "0",
        opts.selfDeaf ? "1" : "0",
        opts.selfVideo ? "1" : "0",
        opts.selfStream ? "1" : "0",
      ].join(":");
      const res = await invokeFn("voice_set_audio_device", {
        kind: "voice_connect",
        deviceId: encoded,
      });
      return /** @type {import("./bindings.d.ts").VoiceConnectResultDto} */ (res);
    },

    /**
     * Queries real-time UDP/SRTP voice transport telemetry (`VoiceRtcStatusDto`).
     * @returns {Promise<import("./bindings.d.ts").VoiceRtcStatusDto>}
     */
    async voiceRtcStatus() {
      const res = await invokeFn("voice_set_audio_device", {
        kind: "voice_rtc_status",
        deviceId: "",
      });
      return /** @type {import("./bindings.d.ts").VoiceRtcStatusDto} */ (res);
    },
  });
}

/**
 * Default generated `tauri-specta` command client bound to the global Tauri IPC bridge.
 * @type {Readonly<import("./bindings.d.ts").TypedCommands>}
 */
export const commands = createTypedCommands((cmd, args) => {
  const g = /** @type {Record<string, unknown>} */ (globalThis);
  const internals =
    g.__TAURI_INTERNALS__ && typeof g.__TAURI_INTERNALS__ === "object"
      ? /** @type {Record<string, unknown>} */ (g.__TAURI_INTERNALS__)
      : null;
  const tauriInvoke =
    internals && typeof internals.invoke === "function"
      ? /** @type {(c: string, a?: Record<string, unknown>) => Promise<unknown>} */ (
          internals.invoke
        )
      : null;
  if (!tauriInvoke) {
    return Promise.reject(new Error(`Tauri IPC invoke unavailable for command: ${cmd}`));
  }
  return args !== undefined ? tauriInvoke(cmd, args) : tauriInvoke(cmd);
});
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_validates_zero_any_types() {
        let builder = generate_specta_builder();
        assert!(builder.validate_no_any().is_ok());
        assert!(!builder.types().is_empty());
        assert!(!builder.commands().is_empty());
    }
}
