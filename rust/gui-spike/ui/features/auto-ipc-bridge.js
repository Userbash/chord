// @ts-check
/**
 * Сгенерировано автоматически скриптом scripts/generate-ipc-bridges.mjs
 * Тонкий мост между Rust Tauri backend и JavaScript frontend.
 * Zero-token deterministic IPC binding generator.
 */

/**
 * Базовый invoke-вызов Tauri
 * @param {string} cmd
 * @param {Record<string, any>} [args]
 * @returns {Promise<any>}
 */
export async function invoke(cmd, args = {}) {
  if (typeof window !== 'undefined' && /** @type {any} */ (window).__TAURI__?.core?.invoke) {
    return /** @type {any} */ (window).__TAURI__.core.invoke(cmd, args);
  }
  console.warn(`[IPC-Stub] Mock call to ${cmd}`, args);
  return null;
}

/** Экспортированные строго типизированные тонкие обвязки команд */
/**
 * Вызов команды process_metrics (источник: main.rs)
 * @returns {Promise<any>}
 */
export async function processMetrics() {
  return invoke("process_metrics", {});
}

/**
 * Вызов команды spike_config (источник: main.rs)
 * @returns {Promise<any>}
 */
export async function spikeConfig() {
  return invoke("spike_config", {});
}

/**
 * Вызов команды report_bench (источник: main.rs)
 * @param {object} [args]
 * @param {any} [args.payload]
 * @returns {Promise<any>}
 */
export async function reportBench({ payload } = {}) {
  return invoke("report_bench", {payload});
}

/**
 * Вызов команды report_ui_snapshot (источник: main.rs)
 * @param {object} [args]
 * @param {any} [args.payload]
 * @returns {Promise<any>}
 */
export async function reportUiSnapshot({ payload } = {}) {
  return invoke("report_ui_snapshot", {payload});
}

/**
 * Вызов команды ready (источник: main.rs)
 * @returns {Promise<any>}
 */
export async function ready() {
  return invoke("ready", {});
}

/**
 * Вызов команды read_state (источник: main.rs)
 * @returns {Promise<any>}
 */
export async function readState() {
  return invoke("read_state", {});
}

/**
 * Вызов команды channels (источник: main.rs)
 * @param {object} [args]
 * @param {any} [args.guild_id]
 * @returns {Promise<any>}
 */
export async function channels({ guild_id } = {}) {
  return invoke("channels", {guild_id});
}

/**
 * Вызов команды messages_page (источник: main.rs)
 * @param {object} [args]
 * @param {any} [args.channel_id]
 * @param {any} [args.cursor]
 * @param {any} [args.before]
 * @param {any} [args.position]
 * @param {any} [args.limit]
 * @returns {Promise<any>}
 */
export async function messagesPage({ channel_id, cursor, before, position, limit } = {}) {
  return invoke("messages_page", {channel_id, cursor, before, position, limit});
}

/**
 * Вызов команды members_page (источник: main.rs)
 * @param {object} [args]
 * @param {any} [args.guild_id]
 * @param {any} [args.cursor]
 * @param {any} [args.limit]
 * @returns {Promise<any>}
 */
export async function membersPage({ guild_id, cursor, limit } = {}) {
  return invoke("members_page", {guild_id, cursor, limit});
}

/**
 * Вызов команды settings (источник: main.rs)
 * @param {object} [args]
 * @param {any} [args.section]
 * @returns {Promise<any>}
 */
export async function settings({ section } = {}) {
  return invoke("settings", {section});
}

/**
 * Вызов команды search_page (источник: main.rs)
 * @param {object} [args]
 * @param {any} [args.query]
 * @param {any} [args.filters]
 * @param {any} [args.cursor]
 * @param {any} [args.limit]
 * @returns {Promise<any>}
 */
export async function searchPage({ query, filters, cursor, limit } = {}) {
  return invoke("search_page", {query, filters, cursor, limit});
}

/**
 * Вызов команды search_remote (источник: main.rs)
 * @param {object} [args]
 * @param {any} [args.guild_id]
 * @param {any} [args.params]
 * @returns {Promise<any>}
 */
export async function searchRemote({ guild_id, params } = {}) {
  return invoke("search_remote", {guild_id, params});
}

/**
 * Вызов команды send_message (источник: main.rs)
 * @param {object} [args]
 * @param {any} [args.channel_id]
 * @param {any} [args.content]
 * @param {any} [args.nonce]
 * @param {any} [args.attachments]
 * @returns {Promise<any>}
 */
export async function sendMessage({ channel_id, content, nonce, attachments } = {}) {
  return invoke("send_message", {channel_id, content, nonce, attachments});
}

/**
 * Вызов команды edit_message (источник: main.rs)
 * @param {object} [args]
 * @param {any} [args.channel_id]
 * @param {any} [args.message_id]
 * @param {any} [args.content]
 * @returns {Promise<any>}
 */
export async function editMessage({ channel_id, message_id, content } = {}) {
  return invoke("edit_message", {channel_id, message_id, content});
}

/**
 * Вызов команды delete_message (источник: main.rs)
 * @param {object} [args]
 * @param {any} [args.channel_id]
 * @param {any} [args.message_id]
 * @returns {Promise<any>}
 */
export async function deleteMessage({ channel_id, message_id } = {}) {
  return invoke("delete_message", {channel_id, message_id});
}

/**
 * Вызов команды reply_message (источник: main.rs)
 * @param {object} [args]
 * @param {any} [args.channel_id]
 * @param {any} [args.content]
 * @param {any} [args.reply_to]
 * @returns {Promise<any>}
 */
export async function replyMessage({ channel_id, content, reply_to } = {}) {
  return invoke("reply_message", {channel_id, content, reply_to});
}

/**
 * Вызов команды ack_message (источник: main.rs)
 * @param {object} [args]
 * @param {any} [args.channel_id]
 * @param {any} [args.message_id]
 * @param {any} [args.last_viewed]
 * @param {any} [args.token]
 * @returns {Promise<any>}
 */
export async function ackMessage({ channel_id, message_id, last_viewed, token } = {}) {
  return invoke("ack_message", {channel_id, message_id, last_viewed, token});
}

/**
 * Вызов команды join_guild (источник: main.rs)
 * @param {object} [args]
 * @param {any} [args.guild_id]
 * @returns {Promise<any>}
 */
export async function joinGuild({ guild_id } = {}) {
  return invoke("join_guild", {guild_id});
}

/**
 * Вызов команды confirm_member_rules (источник: main.rs)
 * @param {object} [args]
 * @param {any} [args.guild_id]
 * @returns {Promise<any>}
 */
export async function confirmMemberRules({ guild_id } = {}) {
  return invoke("confirm_member_rules", {guild_id});
}

/**
 * Вызов команды create_guild (источник: main.rs)
 * @param {object} [args]
 * @param {any} [args.name]
 * @param {any} [args.icon]
 * @param {any} [args.template]
 * @returns {Promise<any>}
 */
export async function createGuild({ name, icon, template } = {}) {
  return invoke("create_guild", {name, icon, template});
}

/**
 * Вызов команды leave_guild (источник: main.rs)
 * @param {object} [args]
 * @param {any} [args.guild_id]
 * @param {any} [args.lurking]
 * @returns {Promise<any>}
 */
export async function leaveGuild({ guild_id, lurking } = {}) {
  return invoke("leave_guild", {guild_id, lurking});
}

/**
 * Вызов команды toggle_reaction (источник: main.rs)
 * @param {object} [args]
 * @param {any} [args.channel_id]
 * @param {any} [args.message_id]
 * @param {any} [args.emoji]
 * @param {any} [args.active]
 * @returns {Promise<any>}
 */
export async function toggleReaction({ channel_id, message_id, emoji, active } = {}) {
  return invoke("toggle_reaction", {channel_id, message_id, emoji, active});
}

/**
 * Вызов команды add_reaction (источник: main.rs)
 * @param {object} [args]
 * @param {any} [args.channel_id]
 * @param {any} [args.message_id]
 * @param {any} [args.emoji]
 * @returns {Promise<any>}
 */
export async function addReaction({ channel_id, message_id, emoji } = {}) {
  return invoke("add_reaction", {channel_id, message_id, emoji});
}

/**
 * Вызов команды send_typing (источник: main.rs)
 * @param {object} [args]
 * @param {any} [args.channel_id]
 * @returns {Promise<any>}
 */
export async function sendTyping({ channel_id } = {}) {
  return invoke("send_typing", {channel_id});
}

/**
 * Вызов команды resolve_invite (источник: main.rs)
 * @param {object} [args]
 * @param {any} [args.code]
 * @returns {Promise<any>}
 */
export async function resolveInvite({ code } = {}) {
  return invoke("resolve_invite", {code});
}

/**
 * Вызов команды accept_invite (источник: main.rs)
 * @param {object} [args]
 * @param {any} [args.code]
 * @param {any} [args.session_id]
 * @param {any} [args.invite_instance_id]
 * @param {any} [args.received_installation_id]
 * @param {any} [args.captcha_key]
 * @param {any} [args.captcha_rqtoken]
 * @param {any} [args.captcha_session_id]
 * @returns {Promise<any>}
 */
export async function acceptInvite({ code, session_id, invite_instance_id, received_installation_id, captcha_key, captcha_rqtoken, captcha_session_id } = {}) {
  return invoke("accept_invite", {code, session_id, invite_instance_id, received_installation_id, captcha_key, captcha_rqtoken, captcha_session_id});
}

/**
 * Вызов команды create_channel_invite (источник: main.rs)
 * @param {object} [args]
 * @param {any} [args.channel_id]
 * @param {any} [args.params]
 * @returns {Promise<any>}
 */
export async function createChannelInvite({ channel_id, params } = {}) {
  return invoke("create_channel_invite", {channel_id, params});
}

/**
 * Вызов команды request_upload_slots (источник: main.rs)
 * @param {object} [args]
 * @param {any} [args.channel_id]
 * @param {any} [args.files]
 * @returns {Promise<any>}
 */
export async function requestUploadSlots({ channel_id, files } = {}) {
  return invoke("request_upload_slots", {channel_id, files});
}

/**
 * Вызов команды attachments_upload_put (источник: main.rs)
 * @param {object} [args]
 * @param {any} [args.payload]
 * @param {any} [args.slot_id]
 * @param {any} [args.id]
 * @param {any} [args.channel_id]
 * @param {any} [args.upload_url]
 * @param {any} [args.content_type]
 * @param {any} [args.data]
 * @param {any} [args.bytes]
 * @returns {Promise<any>}
 */
export async function attachmentsUploadPut({ payload, slot_id, id, channel_id, upload_url, content_type, data, bytes } = {}) {
  return invoke("attachments_upload_put", {payload, slot_id, id, channel_id, upload_url, content_type, data, bytes});
}

/**
 * Вызов команды list_channel_pins (источник: main.rs)
 * @param {object} [args]
 * @param {any} [args.channel_id]
 * @returns {Promise<any>}
 */
export async function listChannelPins({ channel_id } = {}) {
  return invoke("list_channel_pins", {channel_id});
}

/**
 * Вызов команды pin_channel_message (источник: main.rs)
 * @param {object} [args]
 * @param {any} [args.channel_id]
 * @param {any} [args.message_id]
 * @returns {Promise<any>}
 */
export async function pinChannelMessage({ channel_id, message_id } = {}) {
  return invoke("pin_channel_message", {channel_id, message_id});
}

/**
 * Вызов команды unpin_channel_message (источник: main.rs)
 * @param {object} [args]
 * @param {any} [args.channel_id]
 * @param {any} [args.message_id]
 * @returns {Promise<any>}
 */
export async function unpinChannelMessage({ channel_id, message_id } = {}) {
  return invoke("unpin_channel_message", {channel_id, message_id});
}

/**
 * Вызов команды list_channel_threads (источник: main.rs)
 * @param {object} [args]
 * @param {any} [args.channel_id]
 * @param {any} [args.archived]
 * @returns {Promise<any>}
 */
export async function listChannelThreads({ channel_id, archived } = {}) {
  return invoke("list_channel_threads", {channel_id, archived});
}

/**
 * Вызов команды create_invite (источник: main.rs)
 * @param {object} [args]
 * @param {any} [args.channel_id]
 * @param {any} [args.params]
 * @param {any} [args.max_age]
 * @param {any} [args.max_uses]
 * @param {any} [args.temporary]
 * @param {any} [args.flags]
 * @returns {Promise<any>}
 */
export async function createInvite({ channel_id, params, max_age, max_uses, temporary, flags } = {}) {
  return invoke("create_invite", {channel_id, params, max_age, max_uses, temporary, flags});
}

/**
 * Вызов команды attachments_upload_slots (источник: main.rs)
 * @param {object} [args]
 * @param {any} [args.channel_id]
 * @param {any} [args.files]
 * @returns {Promise<any>}
 */
export async function attachmentsUploadSlots({ channel_id, files } = {}) {
  return invoke("attachments_upload_slots", {channel_id, files});
}

/**
 * Вызов команды pins_list (источник: main.rs)
 * @param {object} [args]
 * @param {any} [args.channel_id]
 * @returns {Promise<any>}
 */
export async function pinsList({ channel_id } = {}) {
  return invoke("pins_list", {channel_id});
}

/**
 * Вызов команды threads_list (источник: main.rs)
 * @param {object} [args]
 * @param {any} [args.channel_id]
 * @param {any} [args.archived]
 * @returns {Promise<any>}
 */
export async function threadsList({ channel_id, archived } = {}) {
  return invoke("threads_list", {channel_id, archived});
}

/**
 * Вызов команды update_guild_notification_settings (источник: main.rs)
 * @param {object} [args]
 * @param {any} [args.guild_id]
 * @param {any} [args.patch]
 * @param {any} [args.settings]
 * @param {any} [args.payload]
 * @returns {Promise<any>}
 */
export async function updateGuildNotificationSettings({ guild_id, patch, settings, payload } = {}) {
  return invoke("update_guild_notification_settings", {guild_id, patch, settings, payload});
}

/**
 * Вызов команды update_channel_notification_override (источник: main.rs)
 * @param {object} [args]
 * @param {any} [args.guild_id]
 * @param {any} [args.channel_id]
 * @param {any} [args.override_field]
 * @param {any} [args.override_patch]
 * @param {any} [args.patch]
 * @param {any} [args.payload]
 * @returns {Promise<any>}
 */
export async function updateChannelNotificationOverride({ guild_id, channel_id, override_field, override_patch, patch, payload } = {}) {
  return invoke("update_channel_notification_override", {guild_id, channel_id, override_field, override_patch, patch, payload});
}

/**
 * Вызов команды net_connect (источник: main.rs)
 * @returns {Promise<any>}
 */
export async function netConnect() {
  return invoke("net_connect", {});
}

/**
 * Вызов команды net_status (источник: main.rs)
 * @returns {Promise<any>}
 */
export async function netStatus() {
  return invoke("net_status", {});
}

/**
 * Вызов команды auth_status (источник: main.rs)
 * @returns {Promise<any>}
 */
export async function authStatus() {
  return invoke("auth_status", {});
}

/**
 * Вызов команды auth_login_token (источник: main.rs)
 * @param {object} [args]
 * @param {any} [args.token]
 * @returns {Promise<any>}
 */
export async function authLoginToken({ token } = {}) {
  return invoke("auth_login_token", {token});
}

/**
 * Вызов команды auth_restore (источник: main.rs)
 * @returns {Promise<any>}
 */
export async function authRestore() {
  return invoke("auth_restore", {});
}

/**
 * Вызов команды auth_logout (источник: main.rs)
 * @returns {Promise<any>}
 */
export async function authLogout() {
  return invoke("auth_logout", {});
}

/**
 * Вызов команды auth_login_password (источник: main.rs)
 * @param {object} [args]
 * @param {any} [args.login]
 * @param {any} [args.password]
 * @param {any} [args.captcha_key]
 * @param {any} [args.captcha_rqtoken]
 * @param {any} [args.captcha_session_id]
 * @param {any} [args.captcha_rqdata]
 * @returns {Promise<any>}
 */
export async function authLoginPassword({ login, password, captcha_key, captcha_rqtoken, captcha_session_id, captcha_rqdata } = {}) {
  return invoke("auth_login_password", {login, password, captcha_key, captcha_rqtoken, captcha_session_id, captcha_rqdata});
}

/**
 * Вызов команды auth_mfa (источник: main.rs)
 * @param {object} [args]
 * @param {any} [args.method]
 * @param {any} [args.code]
 * @param {any} [args.ticket]
 * @param {any} [args.login_instance_id]
 * @returns {Promise<any>}
 */
export async function authMfa({ method, code, ticket, login_instance_id } = {}) {
  return invoke("auth_mfa", {method, code, ticket, login_instance_id});
}

/**
 * Вызов команды auth_sms (источник: main.rs)
 * @param {object} [args]
 * @param {any} [args.ticket]
 * @returns {Promise<any>}
 */
export async function authSms({ ticket } = {}) {
  return invoke("auth_sms", {ticket});
}

/**
 * Вызов команды net_dispatch_stats (источник: main.rs)
 * @returns {Promise<any>}
 */
export async function netDispatchStats() {
  return invoke("net_dispatch_stats", {});
}

/**
 * Вызов команды net_disconnect (источник: main.rs)
 * @returns {Promise<any>}
 */
export async function netDisconnect() {
  return invoke("net_disconnect", {});
}

/**
 * Вызов команды settings_load (источник: main.rs)
 * @returns {Promise<any>}
 */
export async function settingsLoad() {
  return invoke("settings_load", {});
}

/**
 * Вызов команды settings_save (источник: main.rs)
 * @param {object} [args]
 * @param {any} [args.settings]
 * @returns {Promise<any>}
 */
export async function settingsSave({ settings } = {}) {
  return invoke("settings_save", {settings});
}

/**
 * Вызов команды invoke_bff_domain (источник: main.rs)
 * @param {object} [args]
 * @param {any} [args.action]
 * @returns {Promise<any>}
 */
export async function invokeBffDomain({ action } = {}) {
  return invoke("invoke_bff_domain", {action});
}

/**
 * Вызов команды voice_connect (источник: main.rs)
 * @param {object} [args]
 * @param {any} [args.guild_id]
 * @param {any} [args.channel_id]
 * @param {any} [args.self_mute]
 * @param {any} [args.self_deaf]
 * @param {any} [args.self_video]
 * @param {any} [args.self_stream]
 * @returns {Promise<any>}
 */
export async function voiceConnect({ guild_id, channel_id, self_mute, self_deaf, self_video, self_stream } = {}) {
  return invoke("voice_connect", {guild_id, channel_id, self_mute, self_deaf, self_video, self_stream});
}

/**
 * Вызов команды voice_disconnect (источник: main.rs)
 * @param {object} [args]
 * @param {any} [args.guild_id]
 * @returns {Promise<any>}
 */
export async function voiceDisconnect({ guild_id } = {}) {
  return invoke("voice_disconnect", {guild_id});
}

/**
 * Вызов команды voice_mute (источник: main.rs)
 * @param {object} [args]
 * @param {any} [args.self_mute]
 * @returns {Promise<any>}
 */
export async function voiceMute({ self_mute } = {}) {
  return invoke("voice_mute", {self_mute});
}

/**
 * Вызов команды voice_deaf (источник: main.rs)
 * @param {object} [args]
 * @param {any} [args.self_deaf]
 * @returns {Promise<any>}
 */
export async function voiceDeaf({ self_deaf } = {}) {
  return invoke("voice_deaf", {self_deaf});
}

/**
 * Вызов команды voice_rtc_status (источник: main.rs)
 * @returns {Promise<any>}
 */
export async function voiceRtcStatus() {
  return invoke("voice_rtc_status", {});
}

/**
 * Вызов команды voice_audio_devices (источник: main.rs)
 * @returns {Promise<any>}
 */
export async function voiceAudioDevices() {
  return invoke("voice_audio_devices", {});
}

/**
 * Вызов команды voice_set_audio_device (источник: main.rs)
 * @param {object} [args]
 * @param {any} [args.kind]
 * @param {any} [args.device_id]
 * @param {any} [args.volume_pct]
 * @param {any} [args.volume_percent]
 * @param {any} [args.muted]
 * @returns {Promise<any>}
 */
export async function voiceSetAudioDevice({ kind, device_id, volume_pct, volume_percent, muted } = {}) {
  return invoke("voice_set_audio_device", {kind, device_id, volume_pct, volume_percent, muted});
}

/**
 * Вызов команды voice_desktop_sources (источник: main.rs)
 * @returns {Promise<any>}
 */
export async function voiceDesktopSources() {
  return invoke("voice_desktop_sources", {});
}

/**
 * Вызов команды voice_stream_start (источник: main.rs)
 * @param {object} [args]
 * @param {any} [args.spec]
 * @returns {Promise<any>}
 */
export async function voiceStreamStart({ spec } = {}) {
  return invoke("voice_stream_start", {spec});
}

/**
 * Вызов команды voice_stream_stop (источник: main.rs)
 * @param {object} [args]
 * @param {any} [args.stream_key]
 * @returns {Promise<any>}
 */
export async function voiceStreamStop({ stream_key } = {}) {
  return invoke("voice_stream_stop", {stream_key});
}

/**
 * Вызов команды voice_stream_watch (источник: main.rs)
 * @param {object} [args]
 * @param {any} [args.stream_key]
 * @returns {Promise<any>}
 */
export async function voiceStreamWatch({ stream_key } = {}) {
  return invoke("voice_stream_watch", {stream_key});
}

/**
 * Вызов команды voice_stream_set_paused (источник: main.rs)
 * @param {object} [args]
 * @param {any} [args.stream_key]
 * @returns {Promise<any>}
 */
export async function voiceStreamSetPaused({ stream_key } = {}) {
  return invoke("voice_stream_set_paused", {stream_key});
}

/**
 * Вызов команды resolve_gift_code (источник: main.rs)
 * @param {object} [args]
 * @param {any} [args.code]
 * @param {any} [args.with_application]
 * @param {any} [args.with_subscription_plan]
 * @returns {Promise<any>}
 */
export async function resolveGiftCode({ code, with_application, with_subscription_plan } = {}) {
  return invoke("resolve_gift_code", {code, with_application, with_subscription_plan});
}

/**
 * Вызов команды create_gift_code (источник: main.rs)
 * @param {object} [args]
 * @param {any} [args.sku_id]
 * @param {any} [args.subscription_plan_id]
 * @param {any} [args.gift_style]
 * @param {any} [args.custom_message]
 * @returns {Promise<any>}
 */
export async function createGiftCode({ sku_id, subscription_plan_id, gift_style, custom_message } = {}) {
  return invoke("create_gift_code", {sku_id, subscription_plan_id, gift_style, custom_message});
}

/**
 * Вызов команды fetch_user_gift_codes (источник: main.rs)
 * @param {object} [args]
 * @param {any} [args.sku_id]
 * @param {any} [args.subscription_plan_id]
 * @returns {Promise<any>}
 */
export async function fetchUserGiftCodes({ sku_id, subscription_plan_id } = {}) {
  return invoke("fetch_user_gift_codes", {sku_id, subscription_plan_id});
}

/**
 * Вызов команды revoke_gift_code (источник: main.rs)
 * @param {object} [args]
 * @param {any} [args.code]
 * @returns {Promise<any>}
 */
export async function revokeGiftCode({ code } = {}) {
  return invoke("revoke_gift_code", {code});
}

/**
 * Вызов команды redeem_gift_code (источник: main.rs)
 * @param {object} [args]
 * @param {any} [args.code]
 * @param {any} [args.channel_id]
 * @param {any} [args.payment_source_id]
 * @param {any} [args.gateway_checkout_context]
 * @returns {Promise<any>}
 */
export async function redeemGiftCode({ code, channel_id, payment_source_id, gateway_checkout_context } = {}) {
  return invoke("redeem_gift_code", {code, channel_id, payment_source_id, gateway_checkout_context});
}

/**
 * Вызов команды fetch_user_entitlements (источник: main.rs)
 * @param {object} [args]
 * @param {any} [args.with_sku]
 * @param {any} [args.with_application]
 * @param {any} [args.entitlement_type]
 * @param {any} [args.exclude_ended]
 * @returns {Promise<any>}
 */
export async function fetchUserEntitlements({ with_sku, with_application, entitlement_type, exclude_ended } = {}) {
  return invoke("fetch_user_entitlements", {with_sku, with_application, entitlement_type, exclude_ended});
}

/**
 * Вызов команды fetch_application_entitlements (источник: main.rs)
 * @param {object} [args]
 * @param {any} [args.application_id]
 * @param {any} [args.exclude_consumed]
 * @returns {Promise<any>}
 */
export async function fetchApplicationEntitlements({ application_id, exclude_consumed } = {}) {
  return invoke("fetch_application_entitlements", {application_id, exclude_consumed});
}

/**
 * Вызов команды fetch_giftable_entitlements (источник: main.rs)
 * @param {object} [args]
 * @param {any} [args.country_code]
 * @returns {Promise<any>}
 */
export async function fetchGiftableEntitlements({ country_code } = {}) {
  return invoke("fetch_giftable_entitlements", {country_code});
}

/**
 * Вызов команды list_nitro_gift_plans (источник: main.rs)
 * @returns {Promise<any>}
 */
export async function listNitroGiftPlans() {
  return invoke("list_nitro_gift_plans", {});
}

/**
 * Вызов команды remote_auth_start_session (источник: main.rs)
 * @param {object} [args]
 * @param {any} [args.endpoint]
 * @returns {Promise<any>}
 */
export async function remoteAuthStartSession({ endpoint } = {}) {
  return invoke("remote_auth_start_session", {endpoint});
}

/**
 * Вызов команды remote_auth_process_frame (источник: main.rs)
 * @param {object} [args]
 * @param {any} [args.frame_json]
 * @returns {Promise<any>}
 */
export async function remoteAuthProcessFrame({ frame_json } = {}) {
  return invoke("remote_auth_process_frame", {frame_json});
}

/**
 * Вызов команды remote_auth_heartbeat_tick (источник: main.rs)
 * @returns {Promise<any>}
 */
export async function remoteAuthHeartbeatTick() {
  return invoke("remote_auth_heartbeat_tick", {});
}

/**
 * Вызов команды remote_auth_initialize (источник: main.rs)
 * @param {object} [args]
 * @param {any} [args.fingerprint]
 * @returns {Promise<any>}
 */
export async function remoteAuthInitialize({ fingerprint } = {}) {
  return invoke("remote_auth_initialize", {fingerprint});
}

/**
 * Вызов команды remote_auth_finish (источник: main.rs)
 * @param {object} [args]
 * @param {any} [args.handshake_token]
 * @param {any} [args.temporary_token]
 * @returns {Promise<any>}
 */
export async function remoteAuthFinish({ handshake_token, temporary_token } = {}) {
  return invoke("remote_auth_finish", {handshake_token, temporary_token});
}

/**
 * Вызов команды remote_auth_cancel (источник: main.rs)
 * @param {object} [args]
 * @param {any} [args.handshake_token]
 * @returns {Promise<any>}
 */
export async function remoteAuthCancel({ handshake_token } = {}) {
  return invoke("remote_auth_cancel", {handshake_token});
}

/**
 * Вызов команды remote_auth_cancel_session (источник: main.rs)
 * @returns {Promise<any>}
 */
export async function remoteAuthCancelSession() {
  return invoke("remote_auth_cancel_session", {});
}
