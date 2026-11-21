# Notification Defaults Protocol (D-06) — Client Decision Heuristics, Cascades, Mention Sound Rules, and Mute Inheritance

Normative specification for **Task D-06** (`Notification Defaults Protocol`) in the Notification Engine (`chord-platform::notifications`). Synthesized from clean-room empirical captures (`fixtures/gateway/by-type/002-READY.json`, `docs/re/gateway-schema.md`, `docs/re/backend-ipc.md`, `docs/re/main-arch.md`, `docs/re/native-modules.md`) and bundle traces (`waves/RW-40/NOTIFICATIONS-RE.md`, `waves/RW-30/RE-NOTIFICATIONS.md`).

---

## 0. Quantitative Oracle Summary

| Metric | Value | Source Oracle |
|---|---:|---|
| `READY.d.user_guild_settings.entries[]` | **58** | `002-READY.json` (`version = 734`, `partial = false`) |
| `READY.d.user_guild_settings.entries[]` field count | **13** | `docs/re/gateway-schema.md:255-271` |
| `channel_overrides[]` total entries across 58 guilds | **1,328** | `002-READY.json` (52 guilds non-empty, max 185 in one guild) |
| `channel_overrides[]` field count | **6** | `docs/re/gateway-schema.md:273-282` |
| Guild `message_notifications` distribution | `0: 19, 1: 36, 2: 3` | `002-READY.json` (`3 / NULL` never occurs at guild level) |
| Channel `message_notifications` distribution | `1: 1, 3: 1327` | `002-READY.json` (`3 / NULL` = inherit parent/server) |
| Guild `notify_highlights` distribution | `0: 56, 1: 2, 2: 0` | `002-READY.json` (`0 = NULL`, `1 = DISABLED`, `2 = ENABLED`) |
| Guild `flags` distribution | `0: 52, 16384: 6` | `002-READY.json` (`16384 = OPT_IN_CHANNELS_ON`) |
| Channel `flags` distribution | `4096: 1252, 6144: 1, null: 75` | `002-READY.json` (`4096 = OPT_IN_ENABLED`, `2048 = FAVORITED`) |
| Muted guilds (`muted = true`) | **22** (`21` with `mute_config`, `1` `null`) | `002-READY.json` |
| Muted channels (`muted = true`) | **42** (`38` with `mute_config`, `4` `null`, `3` expired 2021) | `002-READY.json` |
| Global account `d.notification_settings.flags` | `0` | `002-READY.json` (`USE_NEW_NOTIFICATIONS = 16`, `MENTION_ON_ALL_MESSAGES = 32`) |
| Canonical `mute_config.selected_time_window` presets | **6** (`900, 3600, 10800, 28800, 86400, -1`) | `web.4d4b2f75fdea5470.js@8349873` (`Xx`), `@6225743` (`b8()`) |
| Legacy `DISCORD_NOTIFICATIONS_*` IPC channels | **7** | `docs/re/backend-ipc.md:55` (`core/#7592`) |

---

## 1. Platform Decision Heuristics (`isPlatformEmbedded` & `NotificationSettingsStore`)

### 1.1 `isPlatformEmbedded` Detection & Default Notification States

In the official client (`NotificationSettingsStore`, `persistKey = "notifications"`, `@9225345` in module `803224`, and `isPlatformEmbedded` platform helper), default desktop notification states are determined by runtime embedding and OS notification backend availability:

1. **Embedded Desktop Runtime (`isPlatformEmbedded == true`)**:
   - True when running inside the native desktop shell (`window.DiscordNative != null` in Electron preload `core/mainScreenPreload.js`, or native Rust `chord-gpui` / `chord-platform` runtime).
   - **Default `desktopType`**: `ALL` (`DesktopNotificationType::All`) — desktop notifications are enabled by default without requiring a browser Web Notifications permission prompt.
   - **Default `permissionState`**: `ENABLED` (`NotificationPermissionState::Enabled`) on supported desktop backends (`org.freedesktop.Notifications` D-Bus on Linux in Chord, `NSUserNotificationCenter` / `UNUserNotificationCenter` on macOS `kernelMajor >= 21`, Windows Toast / `notificationstate.node` on Windows). Note: while Electron Discord 1.0.162 left `discord_notifications` unloaded on Linux (`main-arch.md` §3.2), Chord's native `chord-platform` provides full D-Bus `org.freedesktop.Notifications` support (`NOTIFY_SIGNATURE = "susssasa{sv}i"`).
   - **Default `taskbarFlash`**: ` true` (`OnlyWhenUnfocused` / flash frame via `WINDOW_FLASH_FRAME` when unfocused).
   - **Default `disableUnreadBadge`**: `false` (OS dock / taskbar / Unity Launcher `APP_BADGE_SET` unread counter active).
2. **Browser / Unembedded Runtime (`isPlatformEmbedded == false`)**:
   - **Default `desktopType`**: `NEVER` (`DesktopNotificationType::Never`) until explicit Web Notification API permission is granted by the user.
   - **Default `permissionState`**: `BLOCKED` or `NEVER` depending on `Notification.permission`.
3. **Local `NotificationSettingsStore` Schema (`@9225345`, module `803224`)**:
   - `desktopType`: `NEVER | ALL` (`@243859`);
   - `disableAllSounds`: `bool` (default `false`);
   - `disabledSounds`: `Vec<String>` (default `[]`, per-cue mute list e.g. `["message1", "mention"]`);
   - `ttsType`: `NEVER | ALL_CHANNELS | SELECTED_CHANNEL` (`@243899`, default `NEVER`);
   - `disableUnreadBadge`: `bool` (default `false`);
   - `taskbarFlash`: `bool` (default `true`);
   - `notifyMessagesInSelectedChannel`: `bool` (default `false` — when `app_focused && active_channel_id == event.channel_id`, notifications are suppressed unless explicitly overridden);
   - `screenDowntimeReminder`: `bool` (default `false`).

### 1.2 Global Account Flags (`d.notification_settings.flags`, `@6216165`, module `406535`)

- `USE_NEW_NOTIFICATIONS = 16` (`1 << 4`): switches between the legacy 4-preset notification modal (`all_messages`, `hybrid`, `mentions`, `nothing`) and the 3-level modal (`0`, `1`, `2`) with independent unread flags.
- `MENTION_ON_ALL_MESSAGES = 32` (`1 << 5`): account-wide flag treating regular messages as mention-worthy highlights when enabled.

---

## 2. Server-Level vs Channel-Level Notification Cascades

### 2.1 Enumeration Wire Codes (`message_notifications`, `@313815`, module `652215` export `orn`)

| Wire Code | Constant | Valid Scopes | Semantics |
|---:|---|---|---|
| `0` | `ALL_MESSAGES` | Guild, Category, Channel | Alert (sound + desktop banner) on every message unless muted or opt-in suppressed |
| `1` | `ONLY_MENTIONS` | Guild, Category, Channel | Alert only on `@user`, `@role` (if `!suppress_roles`), and `@everyone`/`@here` (if `!suppress_everyone`) |
| `2` | `NO_MESSAGES` | Guild, Category, Channel | Never emit sound or desktop notifications (direct `@user` still increments `.mention-badge`) |
| `3` | `NULL` (`Inherit`) | **Category & Channel only** | Inherit notification level from parent category or guild (`3` is invalid at guild scope) |

### 2.2 Four-Stage Level Resolution Cascade (`resolvedMessageNotifications`, `@9387678`)

For any guild channel `channel` (with optional `parent_id` pointing to a category channel of type `4`) in `guild`:

1. **Stage 1 — Channel Override**: If `channel_overrides[channel.id]` exists and its `message_notifications ∈ {0, 1, 2}` (`!= 3`), resolve to `channel_override.message_notifications` (`CascadeSource::ChannelOverride`).
2. **Stage 2 — Parent Category Override**: Else if `channel.parent_id` has an entry `channel_overrides[parent_id]` and its `message_notifications ∈ {0, 1, 2}` (`!= 3`), resolve to `category_override.message_notifications` (`CascadeSource::CategoryOverride`).
3. **Stage 3 — User Guild Settings Entry**: Else if `user_guild_settings.entries[guild.id]` has an explicit `message_notifications ∈ {0, 1, 2}`, resolve to `guild_entry.message_notifications` (`CascadeSource::GuildSettings`).
4. **Stage 4 — Server Default (`guild.default_message_notifications`)**: Else fall back to `guild.default_message_notifications` (`0 = ALL_MESSAGES` or `1 = ONLY_MENTIONS`, defaulting to `1 = ONLY_MENTIONS` if out of range; `CascadeSource::GuildDefault`). For guilds with `member_count >= 2500` (`LARGE_GUILD_MEMBER_THRESHOLD = 2500`), Discord forces default message notifications to `1` (`ONLY_MENTIONS`) and warns on selecting `0` (`i18n` key `Dh5p5j`).

### 2.3 Channel & Guild Unread Flag Cascade (`flags`, `@8352891`, `@8352746`, `@6231609`)

- **Guild `flags` (`n3`)**:
  - `UNREADS_ALL_MESSAGES = 2048` (`1 << 11`)
  - `UNREADS_ONLY_MENTIONS = 4096` (`1 << 12`)
  - `OPT_IN_CHANNELS_OFF = 8192` (`1 << 13`)
  - `OPT_IN_CHANNELS_ON = 16384` (`1 << 14`)
  - `GUILD_UNREAD_MASK = 2048 | 4096 = 6144`
- **Channel `flags` (`vv`)**:
  - `UNREADS_ONLY_MENTIONS = 512` (`1 << 9`)
  - `UNREADS_ALL_MESSAGES = 1024` (`1 << 10`)
  - `FAVORITED = 2048` (`1 << 11`)
  - `OPT_IN_ENABLED = 4096` (`1 << 12`)
  - `NEW_FORUM_THREADS_OFF = 8192` (`1 << 13`)
  - `NEW_FORUM_THREADS_ON = 16384` (`1 << 14`)
  - `CHANNEL_UNREAD_MASK = 512 | 1024 = 1536`
- **Bitwise Helpers (`@11923745`, module `665260`)**:
  - `mD(flags, bit) = (flags & !UNREAD_MASK) | (bit & UNREAD_MASK)` (clears unread pair before setting target unread bit, preserving `OPT_IN_*`, `FAVORITED`, `NEW_FORUM_THREADS_*`).
  - `PW(flags) = flags & !UNREAD_MASK` (resets unread bits to `UNSET = 0` on override reset `{ message_notifications: 3, muted: false, flags: PW(flags) }`).
- **Four Combined Presets (`@6231609`, module `24873`)**:
  1. `all_messages`: `unread = ALL_MESSAGES`, `message_notifications = 0`;
  2. `hybrid`: `unread = ALL_MESSAGES`, `message_notifications = 1` (unread dot lights up on all messages, sound/desktop alert only on `@mentions`);
  3. `mentions`: `unread ∈ {UNSET, ONLY_MENTIONS}`, `message_notifications = 1`;
  4. `nothing`: `unread ∈ {UNSET, ONLY_MENTIONS}`, `message_notifications = 2`.

---

## 3. Mute Inheritance, Time Windows, and Opt-In Suppression

### 3.1 Temporal Mute Evaluation (`isMuted` & `isTemporarilyMuted`, `@2364848`, module `544743`)

For any guild, category, or channel override entry at epoch timestamp `now_unix`:

```text
isMuted            = entry.muted && (mute_config == null || mute_config.end_time == null || mute_config.end_time == "nil" || parse_iso8601(end_time) >= now_unix)
isTemporarilyMuted = entry.muted && mute_config != null && mute_config.end_time != null && mute_config.end_time != "nil" && parse_iso8601(end_time) >= now_unix
```

1. **Indefinite Mute without `mute_config`**: `muted: true` with `mute_config: null` (or `selected_time_window: -1, end_time: null/"nil"`) is an active indefinite mute (`isMuted = true`, `isTemporarilyMuted = false`). Verified in `002-READY.json`: 22 muted guilds (21 with `mute_config`, 1 `null`) and 42 muted channels (38 with `mute_config`, 4 `null`).
2. **Expired Mute (`end_time < now_unix`)**: Automatically evaluates to `isMuted = false` and `isTemporarilyMuted = false` without requiring a network round-trip, and schedules local `GUILD_MUTE_EXPIRED` / `CHANNEL_MUTE_EXPIRED` timer transitions (`@9384776`, `@9392282`). Verified in `002-READY.json`: 3 channels have historical 2021 `end_time` timestamps (`2021-02-27T17:14:08.418000+00:00`) and evaluate to unmuted.
3. **Six Canonical Time Windows (`VALID_MUTE_TIME_WINDOWS`, `@8349873`)**:
   - `900` (15 minutes)
   - `3600` (1 hour)
   - `10800` (3 hours)
   - `28800` (8 hours)
   - `86400` (24 hours)
   - `-1` (Until I turn it back on -> `end_time: null`)

### 3.2 Hierarchical Mute Inheritance & `allowNoMessages` (`@9388508`)

1. **Effective Mute Inheritance**:
   - `guild_muted = isMuted(guild_entry, now_unix)`
   - `category_muted = isMuted(category_override, now_unix)`
   - `channel_muted = isMuted(channel_override, now_unix)`
   - `effective_muted = guild_muted || category_muted || channel_muted`
   - **Rule**: A child channel cannot un-mute a muted parent category or muted server for sound/desktop notifications (`allowNoMessages`), even if the child channel has an explicit `message_notifications = 0` (`ALL_MESSAGES`) override.
2. **Opt-In Channel Suppression (`OPT_IN_CHANNELS_ON = 16384`)**:
   - When `(guild_entry.flags & 16384) != 0`, any channel where neither the channel nor its parent category has `OPT_IN_ENABLED` (`(flags & 4096) != 0`) is treated as opt-in suppressed (`opt_in_suppressed = true`).
3. **Total Notification Suppression (`allow_no_messages`)**:
   - `allow_no_messages = effective_muted || resolved_level == 2 (NO_MESSAGES) || opt_in_suppressed`.

---

## 4. Mention Sound Rules, Desktop Banner Urgency, and Badge Matrix

### 4.1 Mention & Sound Decision Matrix (`@9387678`, `@9388508`, `@9225345`)

Given client context (`current_user_id`, `user_dnd`, `streamer_mode`, `app_focused`, `active_channel_id`, `is_platform_embedded`, `desktop_type`, `disable_all_sounds`, `disabled_sounds`, `notify_messages_in_selected_channel`) and channel policy (`effective_muted`, `channel_muted`, `category_muted`, `resolved_level`, `allow_no_messages`, `suppress_everyone`, `suppress_roles`, `resolved_unread`):

1. **Global Early Exits (Pre-Cascade Guards)**:
   - **Self-Author**: `event.author_id == current_user_id` → no sound, no desktop banner, no badge, no unread dot.
   - **Do Not Disturb (`user_dnd == true`) or Streamer Mode (`streamer_mode == true`)**: suppresses all sound and desktop banners across DMs and guilds, while **preserving** `.mention-badge` increments and `.is-unread` state.
   - **Active Focused Channel**: when `app_focused && active_channel_id == Some(event.channel_id)` and `!notify_messages_in_selected_channel`, desktop banner and sound are suppressed.

2. **Guild Channel Mention & Sound Matrix**:

| Event Kind | Condition for Desktop Banner | Condition for Sound (`SoundCue::Mention`) | Condition for `.mention-badge` (`mention_count`) | Condition for Unread Dot (`.is-unread`) | D-Bus Urgency |
|---|---|---|---|---|---|
| **Direct `@user` Mention** | `!user_dnd && !streamer_mode && !active_focused && desktop_enabled && !allow_no_messages` | Banner/Alert active `&& !disable_all_sounds && !disabled_sounds.contains("mention")` | **Always increments** (even when `effective_muted == true` or `resolved_level == 2`) | `!channel_muted && !category_muted` | `Normal` (`1`) |
| **Role `<@&role_id>` Mention** | `!suppress_roles && !user_dnd && !streamer_mode && !active_focused && desktop_enabled && !allow_no_messages` | Alert active `&& !suppress_roles && !disable_all_sounds && !disabled_sounds.contains("mention")` | Increments iff `!suppress_roles` (even when `resolved_level == 2` or `effective_muted`) | `!suppress_roles && !channel_muted && !category_muted` | `Normal` (`1`) |
| **`@everyone` / `@here` Mention** | `!suppress_everyone && !user_dnd && !streamer_mode && !active_focused && desktop_enabled && !allow_no_messages` | Alert active `&& !suppress_everyone && !disable_all_sounds && !disabled_sounds.contains("mention")` | Increments iff `!suppress_everyone` (even when `resolved_level == 2` or `effective_muted`) | `!suppress_everyone && !channel_muted && !category_muted` | `Normal` (`1`) |
| **Regular Message (No Mention)** | `resolved_level == 0 && !user_dnd && !streamer_mode && !active_focused && desktop_enabled && !allow_no_messages` | Alert active `&& resolved_level == 0 && !disable_all_sounds && !disabled_sounds.contains("message1")` | Never (`0`) | `!channel_muted && !category_muted && !opt_in_suppressed && resolved_unread == ALL_MESSAGES` | `Low` (`0`) |
| **Direct Message (DM / Group DM)** | `!user_dnd && !streamer_mode && !active_focused && desktop_enabled && (!channel_muted \|\| mentions_current_user)` | Alert active `&& !disable_all_sounds && !disabled_sounds.contains("message1")` | Always increments on DM | `!channel_muted \|\| mentions_current_user` | `Critical` (`2`) |

---

## 5. Partial Update Merge & REST Persistence Contracts

1. **Dual `channel_overrides` Wire Format (`@9385738`)**:
   - In `READY`, `channel_overrides` is serialized as an array of objects `[{ "channel_id": "...", ... }]`.
   - In `USER_GUILD_SETTINGS_UPDATE` and REST `PATCH` payloads, `channel_overrides` may be an array or a dictionary `{ "<channel_id>": { ... } }`.
2. **Protective Partial Merge Invariant (`USER_GUILD_SETTINGS_UPDATE`, `@9391654`)**:
   - Missing `channel_overrides` key in a partial update preserves the existing override map rather than wiping all 1,328 overrides.
   - Missing `message_notifications` in a newly created guild entry defaults to `guild.default_message_notifications`.
   - Stale version updates (`incoming.version > 0 && incoming.version < existing.version`) are rejected (`last-wins` monotonicity).
3. **Override Visibility Filter (`has_notification_override`)**:
   - A `ChannelOverride` row is shown in the UI overrides table iff `isMuted(override, now_unix) || override.message_notifications ∈ {0, 1, 2} || ((override.flags ?? 0) & (512 | 1024)) != 0`. Rows existing solely for `collapsed: true` (32 in capture) or `OPT_IN_ENABLED: 4096` (1,252 in capture) are hidden from the overrides table.
4. **Non-Destructive Override Reset (`@1622`, `@6230633`)**:
   - Removing an override in the UI sends `{ "message_notifications": 3, "muted": false, "flags": current_flags & !(512 | 1024) }`, preserving `collapsed`, `FAVORITED`, and `OPT_IN_ENABLED`.
