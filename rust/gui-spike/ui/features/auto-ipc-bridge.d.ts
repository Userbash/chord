/**
 * bindings.d.ts — Автоматически сгенерированные TypeScript типы для Tauri IPC команд.
 * Zero-Token генерация из Rust #[tauri::command] (STAGE-10.BFF.2).
 */

export declare function invoke<T = any>(cmd: string, args?: Record<string, any>): Promise<T>;

/** db_messages (from main.rs) */
export declare function dbMessages(args: { channel_id?: any; before?: any; limit?: any }): Promise<any>;

/** db_query (from main.rs) */
export declare function dbQuery(args: { query?: any; channel_id?: any; author_id?: any; limit?: any; offset?: any }): Promise<any>;

/** process_metrics (from main.rs) */
export declare function processMetrics(args?: Record<string, never>): Promise<any>;

/** spike_config (from main.rs) */
export declare function spikeConfig(args?: Record<string, never>): Promise<any>;

/** report_bench (from main.rs) */
export declare function reportBench(args: { payload?: any }): Promise<any>;

/** report_ui_snapshot (from main.rs) */
export declare function reportUiSnapshot(args: { payload?: any }): Promise<any>;

/** ready (from main.rs) */
export declare function ready(args?: Record<string, never>): Promise<any>;

/** read_state (from main.rs) */
export declare function readState(args?: Record<string, never>): Promise<any>;

/** channels (from main.rs) */
export declare function channels(args: { guild_id?: any }): Promise<any>;

/** messages_page (from main.rs) */
export declare function messagesPage(args: { channel_id?: any; cursor?: any; before?: any; position?: any; limit?: any }): Promise<any>;

/** members_page (from main.rs) */
export declare function membersPage(args: { guild_id?: any; cursor?: any; limit?: any }): Promise<any>;

/** settings (from main.rs) */
export declare function settings(args: { section?: any }): Promise<any>;

/** search_page (from main.rs) */
export declare function searchPage(args: { query?: any; filters?: any; cursor?: any; limit?: any }): Promise<any>;

/** search_remote (from main.rs) */
export declare function searchRemote(args: { guild_id?: any; params?: any }): Promise<any>;

/** send_message (from main.rs) */
export declare function sendMessage(args: { channel_id?: any; content?: any; nonce?: any; attachments?: any }): Promise<any>;

/** edit_message (from main.rs) */
export declare function editMessage(args: { channel_id?: any; message_id?: any; content?: any }): Promise<any>;

/** delete_message (from main.rs) */
export declare function deleteMessage(args: { channel_id?: any; message_id?: any }): Promise<any>;

/** reply_message (from main.rs) */
export declare function replyMessage(args: { channel_id?: any; content?: any; reply_to?: any }): Promise<any>;

/** ack_message (from main.rs) */
export declare function ackMessage(args: { channel_id?: any; message_id?: any; last_viewed?: any; token?: any }): Promise<any>;

/** join_guild (from main.rs) */
export declare function joinGuild(args: { guild_id?: any }): Promise<any>;

/** confirm_member_rules (from main.rs) */
export declare function confirmMemberRules(args: { guild_id?: any }): Promise<any>;

/** create_guild (from main.rs) */
export declare function createGuild(args: { name?: any; icon?: any; template?: any }): Promise<any>;

/** leave_guild (from main.rs) */
export declare function leaveGuild(args: { guild_id?: any; lurking?: any }): Promise<any>;

/** toggle_reaction (from main.rs) */
export declare function toggleReaction(args: { channel_id?: any; message_id?: any; emoji?: any; active?: any }): Promise<any>;

/** add_reaction (from main.rs) */
export declare function addReaction(args: { channel_id?: any; message_id?: any; emoji?: any }): Promise<any>;

/** send_typing (from main.rs) */
export declare function sendTyping(args: { channel_id?: any }): Promise<any>;

/** resolve_invite (from main.rs) */
export declare function resolveInvite(args: { code?: any }): Promise<any>;

/** accept_invite (from main.rs) */
export declare function acceptInvite(args: { code?: any; session_id?: any; invite_instance_id?: any; received_installation_id?: any; captcha_key?: any; captcha_rqtoken?: any; captcha_session_id?: any }): Promise<any>;

/** create_channel_invite (from main.rs) */
export declare function createChannelInvite(args: { channel_id?: any; params?: any }): Promise<any>;

/** request_upload_slots (from main.rs) */
export declare function requestUploadSlots(args: { channel_id?: any; files?: any }): Promise<any>;

/** attachments_upload_put (from main.rs) */
export declare function attachmentsUploadPut(args: { payload?: any; slot_id?: any; id?: any; channel_id?: any; upload_url?: any; content_type?: any; data?: any; bytes?: any }): Promise<any>;

/** list_channel_pins (from main.rs) */
export declare function listChannelPins(args: { channel_id?: any }): Promise<any>;

/** pin_channel_message (from main.rs) */
export declare function pinChannelMessage(args: { channel_id?: any; message_id?: any }): Promise<any>;

/** unpin_channel_message (from main.rs) */
export declare function unpinChannelMessage(args: { channel_id?: any; message_id?: any }): Promise<any>;

/** list_channel_threads (from main.rs) */
export declare function listChannelThreads(args: { channel_id?: any; archived?: any }): Promise<any>;

/** create_invite (from main.rs) */
export declare function createInvite(args: { channel_id?: any; params?: any; max_age?: any; max_uses?: any; temporary?: any; flags?: any }): Promise<any>;

/** attachments_upload_slots (from main.rs) */
export declare function attachmentsUploadSlots(args: { channel_id?: any; files?: any }): Promise<any>;

/** pins_list (from main.rs) */
export declare function pinsList(args: { channel_id?: any }): Promise<any>;

/** threads_list (from main.rs) */
export declare function threadsList(args: { channel_id?: any; archived?: any }): Promise<any>;

/** update_guild_notification_settings (from main.rs) */
export declare function updateGuildNotificationSettings(args: { guild_id?: any; patch?: any; settings?: any; payload?: any }): Promise<any>;

/** update_channel_notification_override (from main.rs) */
export declare function updateChannelNotificationOverride(args: { guild_id?: any; channel_id?: any; override_field?: any; override_patch?: any; patch?: any; payload?: any }): Promise<any>;

/** net_connect (from main.rs) */
export declare function netConnect(args?: Record<string, never>): Promise<any>;

/** net_status (from main.rs) */
export declare function netStatus(args?: Record<string, never>): Promise<any>;

/** auth_status (from main.rs) */
export declare function authStatus(args?: Record<string, never>): Promise<any>;

/** auth_login_token (from main.rs) */
export declare function authLoginToken(args: { token?: any }): Promise<any>;

/** auth_restore (from main.rs) */
export declare function authRestore(args?: Record<string, never>): Promise<any>;

/** auth_logout (from main.rs) */
export declare function authLogout(args?: Record<string, never>): Promise<any>;

/** auth_login_password (from main.rs) */
export declare function authLoginPassword(args: { login?: any; password?: any; captcha_key?: any; captcha_rqtoken?: any; captcha_session_id?: any; captcha_rqdata?: any }): Promise<any>;

/** auth_mfa (from main.rs) */
export declare function authMfa(args: { method?: any; code?: any; ticket?: any; login_instance_id?: any }): Promise<any>;

/** auth_sms (from main.rs) */
export declare function authSms(args: { ticket?: any }): Promise<any>;

/** net_dispatch_stats (from main.rs) */
export declare function netDispatchStats(args?: Record<string, never>): Promise<any>;

/** net_disconnect (from main.rs) */
export declare function netDisconnect(args?: Record<string, never>): Promise<any>;

/** settings_load (from main.rs) */
export declare function settingsLoad(args?: Record<string, never>): Promise<any>;

/** settings_save (from main.rs) */
export declare function settingsSave(args: { settings?: any }): Promise<any>;

/** invoke_bff_domain (from main.rs) */
export declare function invokeBffDomain(args: { action?: any }): Promise<any>;

/** voice_connect (from main.rs) */
export declare function voiceConnect(args: { guild_id?: any; channel_id?: any; self_mute?: any; self_deaf?: any; self_video?: any; self_stream?: any }): Promise<any>;

/** voice_disconnect (from main.rs) */
export declare function voiceDisconnect(args: { guild_id?: any }): Promise<any>;

/** voice_mute (from main.rs) */
export declare function voiceMute(args: { self_mute?: any }): Promise<any>;

/** voice_deaf (from main.rs) */
export declare function voiceDeaf(args: { self_deaf?: any }): Promise<any>;

/** voice_rtc_status (from main.rs) */
export declare function voiceRtcStatus(args?: Record<string, never>): Promise<any>;

/** voice_state (from main.rs) */
export declare function voiceState(args: { guild_id?: any; channel_id?: any; self_mute?: any; self_deaf?: any; self_video?: any; self_stream?: any; peer_user_ids?: any }): Promise<any>;

/** voice_e2ee_status (from main.rs) */
export declare function voiceE2eeStatus(args: { channel_id?: any }): Promise<any>;

/** audio_devices (from main.rs) */
export declare function audioDevices(args?: Record<string, never>): Promise<any>;

/** audio_set_volume (from main.rs) */
export declare function audioSetVolume(args: { volume_pct?: any; kind?: any }): Promise<any>;

/** audio_mute (from main.rs) */
export declare function audioMute(args: { muted?: any }): Promise<any>;

/** voice_audio_devices (from main.rs) */
export declare function voiceAudioDevices(args?: Record<string, never>): Promise<any>;

/** voice_set_audio_device (from main.rs) */
export declare function voiceSetAudioDevice(args: { kind?: any; device_id?: any; volume_pct?: any; volume_percent?: any; muted?: any }): Promise<any>;

/** voice_desktop_sources (from main.rs) */
export declare function voiceDesktopSources(args?: Record<string, never>): Promise<any>;

/** voice_stream_start (from main.rs) */
export declare function voiceStreamStart(args: { spec?: any }): Promise<any>;

/** voice_stream_stop (from main.rs) */
export declare function voiceStreamStop(args: { stream_key?: any }): Promise<any>;

/** voice_stream_watch (from main.rs) */
export declare function voiceStreamWatch(args: { stream_key?: any }): Promise<any>;

/** voice_stream_set_paused (from main.rs) */
export declare function voiceStreamSetPaused(args: { stream_key?: any }): Promise<any>;

/** resolve_gift_code (from main.rs) */
export declare function resolveGiftCode(args: { code?: any; with_application?: any; with_subscription_plan?: any }): Promise<any>;

/** create_gift_code (from main.rs) */
export declare function createGiftCode(args: { sku_id?: any; subscription_plan_id?: any; gift_style?: any; custom_message?: any }): Promise<any>;

/** fetch_user_gift_codes (from main.rs) */
export declare function fetchUserGiftCodes(args: { sku_id?: any; subscription_plan_id?: any }): Promise<any>;

/** revoke_gift_code (from main.rs) */
export declare function revokeGiftCode(args: { code?: any }): Promise<any>;

/** redeem_gift_code (from main.rs) */
export declare function redeemGiftCode(args: { code?: any; channel_id?: any; payment_source_id?: any; gateway_checkout_context?: any }): Promise<any>;

/** fetch_user_entitlements (from main.rs) */
export declare function fetchUserEntitlements(args: { with_sku?: any; with_application?: any; entitlement_type?: any; exclude_ended?: any }): Promise<any>;

/** fetch_application_entitlements (from main.rs) */
export declare function fetchApplicationEntitlements(args: { application_id?: any; exclude_consumed?: any }): Promise<any>;

/** fetch_giftable_entitlements (from main.rs) */
export declare function fetchGiftableEntitlements(args: { country_code?: any }): Promise<any>;

/** list_nitro_gift_plans (from main.rs) */
export declare function listNitroGiftPlans(args?: Record<string, never>): Promise<any>;

/** remote_auth_start_session (from main.rs) */
export declare function remoteAuthStartSession(args: { endpoint?: any }): Promise<any>;

/** remote_auth_process_frame (from main.rs) */
export declare function remoteAuthProcessFrame(args: { frame_json?: any }): Promise<any>;

/** remote_auth_heartbeat_tick (from main.rs) */
export declare function remoteAuthHeartbeatTick(args?: Record<string, never>): Promise<any>;

/** remote_auth_initialize (from main.rs) */
export declare function remoteAuthInitialize(args: { fingerprint?: any }): Promise<any>;

/** remote_auth_finish (from main.rs) */
export declare function remoteAuthFinish(args: { handshake_token?: any; temporary_token?: any }): Promise<any>;

/** remote_auth_cancel (from main.rs) */
export declare function remoteAuthCancel(args: { handshake_token?: any }): Promise<any>;

/** rest_request (from main.rs) */
export declare function restRequest(args: { method?: any; endpoint?: any; body?: any }): Promise<any>;

/** remote_auth_cancel_session (from main.rs) */
export declare function remoteAuthCancelSession(args?: Record<string, never>): Promise<any>;
