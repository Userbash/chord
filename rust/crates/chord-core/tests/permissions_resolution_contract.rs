//! Контракт движка расчёта прав доступа Discord v9 (TASK-619).
//!
//! Оракулы:
//! - `/home/sanya/re-deep/decoded/4d2a64cb5df5fc4e_0` (`@9770000..9774000`, `BigFlagUtils` `136722`
//!   и `Permissions` `eC = Object.freeze({...})`):
//!   - 41 битовый флаг `Permissions(pub u64)` (`CREATE_INSTANT_INVITE = 1 << 0` .. `MODERATE_MEMBERS = 1 << 40`);
//!   - алгоритм расчёта базовых прав (`compute_base_permissions`) и канальных перезаписей
//!     (`compute_channel_permissions`): `@everyone` → суммарные `role_deny`/`role_allow` → `member`;
//!   - обход ограничений при владении сервером (`user_id == owner_id`) и наличии `ADMINISTRATOR`.
//! - `fixtures/gateway/by-type/002-READY.json` (md5 `a1a8fd5662be88108dd25ac619a377f2`):
//!   реальная гильдия `ITSKIDON` (`1014537851418980372`), роль `@everyone` (`1096185279794753`),
//!   роль `Модератор` (`1057598992449552425`, права `41780636024647`) и канал `1275727867497680918`
//!   с двумя `permission_overwrites` в обратном порядке (сначала роль, затем `@everyone`).

mod support;

use chord_core::permissions::{
    ALL_PERMISSIONS, OVERWRITE_TYPE_MEMBER, OVERWRITE_TYPE_ROLE, PermissionOverwrite, Permissions,
    compute_base_permissions, compute_channel_permissions, has_permission,
};
use chord_core::ready::Ready;

/// 1. Проверка всех 41 битовых флагов Discord API v9 (`0..=40`) и обёрточного типа `Permissions(pub u64)`.
#[test]
fn v9_permission_bitfield_flags_match_official_bit_positions() {
    let expected_flags: [(u64, u32, &str); 41] = [
        (
            Permissions::CREATE_INSTANT_INVITE,
            0,
            "CREATE_INSTANT_INVITE",
        ),
        (Permissions::KICK_MEMBERS, 1, "KICK_MEMBERS"),
        (Permissions::BAN_MEMBERS, 2, "BAN_MEMBERS"),
        (Permissions::ADMINISTRATOR, 3, "ADMINISTRATOR"),
        (Permissions::MANAGE_CHANNELS, 4, "MANAGE_CHANNELS"),
        (Permissions::MANAGE_GUILD, 5, "MANAGE_GUILD"),
        (Permissions::ADD_REACTIONS, 6, "ADD_REACTIONS"),
        (Permissions::VIEW_AUDIT_LOG, 7, "VIEW_AUDIT_LOG"),
        (Permissions::PRIORITY_SPEAKER, 8, "PRIORITY_SPEAKER"),
        (Permissions::STREAM, 9, "STREAM"),
        (Permissions::VIEW_CHANNEL, 10, "VIEW_CHANNEL"),
        (Permissions::SEND_MESSAGES, 11, "SEND_MESSAGES"),
        (Permissions::SEND_TTS_MESSAGES, 12, "SEND_TTS_MESSAGES"),
        (Permissions::MANAGE_MESSAGES, 13, "MANAGE_MESSAGES"),
        (Permissions::EMBED_LINKS, 14, "EMBED_LINKS"),
        (Permissions::ATTACH_FILES, 15, "ATTACH_FILES"),
        (
            Permissions::READ_MESSAGE_HISTORY,
            16,
            "READ_MESSAGE_HISTORY",
        ),
        (Permissions::MENTION_EVERYONE, 17, "MENTION_EVERYONE"),
        (Permissions::USE_EXTERNAL_EMOJIS, 18, "USE_EXTERNAL_EMOJIS"),
        (Permissions::VIEW_GUILD_INSIGHTS, 19, "VIEW_GUILD_INSIGHTS"),
        (Permissions::CONNECT, 20, "CONNECT"),
        (Permissions::SPEAK, 21, "SPEAK"),
        (Permissions::MUTE_MEMBERS, 22, "MUTE_MEMBERS"),
        (Permissions::DEAFEN_MEMBERS, 23, "DEAFEN_MEMBERS"),
        (Permissions::MOVE_MEMBERS, 24, "MOVE_MEMBERS"),
        (Permissions::USE_VAD, 25, "USE_VAD"),
        (Permissions::CHANGE_NICKNAME, 26, "CHANGE_NICKNAME"),
        (Permissions::MANAGE_NICKNAMES, 27, "MANAGE_NICKNAMES"),
        (Permissions::MANAGE_ROLES, 28, "MANAGE_ROLES"),
        (Permissions::MANAGE_WEBHOOKS, 29, "MANAGE_WEBHOOKS"),
        (
            Permissions::MANAGE_GUILD_EXPRESSIONS,
            30,
            "MANAGE_GUILD_EXPRESSIONS",
        ),
        (
            Permissions::USE_APPLICATION_COMMANDS,
            31,
            "USE_APPLICATION_COMMANDS",
        ),
        (Permissions::REQUEST_TO_SPEAK, 32, "REQUEST_TO_SPEAK"),
        (Permissions::MANAGE_EVENTS, 33, "MANAGE_EVENTS"),
        (Permissions::MANAGE_THREADS, 34, "MANAGE_THREADS"),
        (
            Permissions::CREATE_PUBLIC_THREADS,
            35,
            "CREATE_PUBLIC_THREADS",
        ),
        (
            Permissions::CREATE_PRIVATE_THREADS,
            36,
            "CREATE_PRIVATE_THREADS",
        ),
        (
            Permissions::USE_EXTERNAL_STICKERS,
            37,
            "USE_EXTERNAL_STICKERS",
        ),
        (
            Permissions::SEND_MESSAGES_IN_THREADS,
            38,
            "SEND_MESSAGES_IN_THREADS",
        ),
        (
            Permissions::USE_EMBEDDED_ACTIVITIES,
            39,
            "USE_EMBEDDED_ACTIVITIES",
        ),
        (Permissions::MODERATE_MEMBERS, 40, "MODERATE_MEMBERS"),
    ];

    let mut combined = 0u64;
    for (flag, shift, name) in expected_flags {
        let expected = 1u64 << shift;
        assert_eq!(
            flag, expected,
            "флаг {name} должен быть равен 1 << {shift} ({expected}), получено {flag}"
        );
        assert_eq!(
            flag.count_ones(),
            1,
            "флаг {name} должен содержать ровно 1 бит"
        );
        assert_eq!(
            combined & flag,
            0,
            "флаг {name} не должен пересекаться с предыдущими битами"
        );
        combined |= flag;
    }

    assert_eq!(combined, (1u64 << 41) - 1);

    let wrapped = Permissions(Permissions::VIEW_CHANNEL | Permissions::SEND_MESSAGES);
    assert_eq!(
        wrapped.0,
        Permissions::VIEW_CHANNEL | Permissions::SEND_MESSAGES
    );
    assert!(wrapped.has(Permissions::VIEW_CHANNEL));
    assert!(wrapped.has(Permissions::SEND_MESSAGES));
    assert!(!wrapped.has(Permissions::MANAGE_MESSAGES));
}

/// 2. Шаги 1–2: базовые права (`compute_base_permissions`) — объединение `@everyone` и ролей,
///    обход для владельца гильдии и роли с `ADMINISTRATOR`.
#[test]
fn base_permissions_combines_roles_and_honors_owner_and_admin_bypass() {
    let owner_id = 100_001;
    let member_id = 200_002;
    let everyone = Permissions::VIEW_CHANNEL | Permissions::READ_MESSAGE_HISTORY;
    let role_a = Permissions::SEND_MESSAGES | Permissions::ADD_REACTIONS;
    let role_b = Permissions::CONNECT | Permissions::SPEAK | Permissions::MODERATE_MEMBERS;

    // Обычный участник без дополнительных ролей получает только права @everyone.
    let base_everyone_only = compute_base_permissions(owner_id, member_id, everyone, &[]);
    assert_eq!(base_everyone_only, everyone);

    // Обычный участник с двумя ролями получает побитовое ИЛИ всех своих ролей и @everyone.
    let base_combined = compute_base_permissions(owner_id, member_id, everyone, &[role_a, role_b]);
    assert_eq!(base_combined, everyone | role_a | role_b);
    assert!(has_permission(base_combined, Permissions::MODERATE_MEMBERS));
    assert!(!has_permission(base_combined, Permissions::BAN_MEMBERS));

    // Владелец сервера (`user_id == owner_id`) получает полные права даже при пустых ролях (0).
    let owner_perms = compute_base_permissions(owner_id, owner_id, 0, &[]);
    assert_eq!(owner_perms, ALL_PERMISSIONS);
    assert!(has_permission(owner_perms, Permissions::ADMINISTRATOR));
    assert!(has_permission(owner_perms, Permissions::MANAGE_GUILD));
    assert!(has_permission(owner_perms, Permissions::MODERATE_MEMBERS));

    // Участник с ролью `ADMINISTRATOR` получает `ALL_PERMISSIONS`.
    let admin_role_perms = compute_base_permissions(
        owner_id,
        member_id,
        everyone,
        &[role_a, Permissions::ADMINISTRATOR],
    );
    assert_eq!(admin_role_perms, ALL_PERMISSIONS);

    // Если сам `@everyone` содержит `ADMINISTRATOR`, также возвращается `ALL_PERMISSIONS`.
    let everyone_admin_perms =
        compute_base_permissions(owner_id, member_id, Permissions::ADMINISTRATOR, &[]);
    assert_eq!(everyone_admin_perms, ALL_PERMISSIONS);
}

/// 3. Шаг 3: иерархия канальных перезаписей `@everyone` -> `roles` (OR allow / OR deny) -> `member`.
#[test]
fn channel_overwrites_follow_everyone_then_roles_then_member_hierarchy() {
    let guild_id = 10_000;
    let user_id = 20_000;
    let role_muted = 30_001;
    let role_writer = 30_002;
    let role_foreign = 30_999;

    let base = Permissions::VIEW_CHANNEL
        | Permissions::SEND_MESSAGES
        | Permissions::ATTACH_FILES
        | Permissions::EMBED_LINKS
        | Permissions::ADD_REACTIONS;

    // 3a) @everyone запрещает SEND_MESSAGES и ATTACH_FILES, но разрешает USE_EXTERNAL_EMOJIS.
    // 3b) role_muted запрещает ADD_REACTIONS и SEND_MESSAGES, а role_writer разрешает SEND_MESSAGES и ATTACH_FILES.
    //     Так как `role_allow` накладывается после `& !role_deny`, SEND_MESSAGES и ATTACH_FILES восстанавливаются,
    //     а ADD_REACTIONS остаётся запрещённым.
    // 3c) member overwrite запрещает ATTACH_FILES и разрешает MANAGE_MESSAGES.
    // Перезаписи специально переданы в обратном порядке (member -> role -> @everyone),
    // чтобы доказать независимость от порядка элементов в срезе.
    let overwrites = [
        PermissionOverwrite {
            id: user_id,
            overwrite_type: OVERWRITE_TYPE_MEMBER,
            allow: Permissions::MANAGE_MESSAGES,
            deny: Permissions::ATTACH_FILES,
        },
        PermissionOverwrite {
            id: role_foreign,
            overwrite_type: OVERWRITE_TYPE_ROLE,
            allow: Permissions::BAN_MEMBERS,
            deny: Permissions::VIEW_CHANNEL,
        },
        PermissionOverwrite {
            id: role_writer,
            overwrite_type: OVERWRITE_TYPE_ROLE,
            allow: Permissions::SEND_MESSAGES | Permissions::ATTACH_FILES,
            deny: 0,
        },
        PermissionOverwrite {
            id: role_muted,
            overwrite_type: OVERWRITE_TYPE_ROLE,
            allow: 0,
            deny: Permissions::SEND_MESSAGES | Permissions::ADD_REACTIONS,
        },
        PermissionOverwrite {
            id: guild_id,
            overwrite_type: OVERWRITE_TYPE_ROLE,
            allow: Permissions::USE_EXTERNAL_EMOJIS,
            deny: Permissions::SEND_MESSAGES | Permissions::ATTACH_FILES,
        },
    ];

    let computed = compute_channel_permissions(
        base,
        guild_id,
        user_id,
        &[role_muted, role_writer],
        &overwrites,
    );

    let expected = Permissions::VIEW_CHANNEL
        | Permissions::EMBED_LINKS
        | Permissions::USE_EXTERNAL_EMOJIS
        | Permissions::SEND_MESSAGES
        | Permissions::MANAGE_MESSAGES;

    assert_eq!(computed, expected);
    assert!(has_permission(computed, Permissions::SEND_MESSAGES));
    assert!(has_permission(computed, Permissions::MANAGE_MESSAGES));
    assert!(has_permission(computed, Permissions::USE_EXTERNAL_EMOJIS));
    assert!(!has_permission(computed, Permissions::ATTACH_FILES));
    assert!(!has_permission(computed, Permissions::ADD_REACTIONS));
    assert!(!has_permission(computed, Permissions::BAN_MEMBERS));
}

/// 4. Шаг 4 и негативные сценарии: `ADMINISTRATOR` не урезается канальными `deny`,
///    а чужие/некорректные `overwrite_type` игнорируются.
#[test]
fn administrator_bypasses_channel_denials_and_invalid_overwrite_types_are_ignored() {
    let guild_id = 500;
    let user_id = 600;
    let role_id = 700;

    let strict_deny_overwrites = [
        PermissionOverwrite {
            id: guild_id,
            overwrite_type: OVERWRITE_TYPE_ROLE,
            allow: 0,
            deny: Permissions::VIEW_CHANNEL | Permissions::SEND_MESSAGES,
        },
        PermissionOverwrite {
            id: role_id,
            overwrite_type: OVERWRITE_TYPE_ROLE,
            allow: 0,
            deny: Permissions::VIEW_CHANNEL | Permissions::CONNECT,
        },
        PermissionOverwrite {
            id: user_id,
            overwrite_type: OVERWRITE_TYPE_MEMBER,
            allow: 0,
            deny: Permissions::VIEW_CHANNEL | Permissions::SEND_MESSAGES,
        },
    ];

    // Базовый ADMINISTRATOR не теряет доступ даже при тотальном deny на всех уровнях канала.
    let from_base_admin = compute_channel_permissions(
        Permissions::VIEW_CHANNEL | Permissions::ADMINISTRATOR,
        guild_id,
        user_id,
        &[role_id],
        &strict_deny_overwrites,
    );
    assert_eq!(from_base_admin, ALL_PERMISSIONS);

    // Если перезапись роли/участника выдаёт ADMINISTRATOR в канале (Шаг 4), возвращаются все права.
    let granted_admin_in_channel = compute_channel_permissions(
        Permissions::VIEW_CHANNEL,
        guild_id,
        user_id,
        &[role_id],
        &[
            PermissionOverwrite {
                id: guild_id,
                overwrite_type: OVERWRITE_TYPE_ROLE,
                allow: 0,
                deny: Permissions::SEND_MESSAGES,
            },
            PermissionOverwrite {
                id: user_id,
                overwrite_type: OVERWRITE_TYPE_MEMBER,
                allow: Permissions::ADMINISTRATOR,
                deny: Permissions::VIEW_CHANNEL,
            },
        ],
    );
    assert_eq!(granted_admin_in_channel, ALL_PERMISSIONS);

    // Негативный тест типов перезаписей:
    // - overwrite с id == user_id, но overwrite_type == 0 (ROLE), когда у участника нет такой роли;
    // - overwrite с id == role_id, но overwrite_type == 1 (MEMBER), когда user_id != role_id;
    // - overwrite с неизвестным overwrite_type == 2.
    let mismatched_overwrites = [
        PermissionOverwrite {
            id: user_id,
            overwrite_type: OVERWRITE_TYPE_ROLE,
            allow: Permissions::MANAGE_GUILD,
            deny: Permissions::VIEW_CHANNEL,
        },
        PermissionOverwrite {
            id: role_id,
            overwrite_type: OVERWRITE_TYPE_MEMBER,
            allow: Permissions::BAN_MEMBERS,
            deny: Permissions::VIEW_CHANNEL,
        },
        PermissionOverwrite {
            id: user_id,
            overwrite_type: 2,
            allow: Permissions::KICK_MEMBERS,
            deny: Permissions::VIEW_CHANNEL,
        },
    ];
    let unchanged = compute_channel_permissions(
        Permissions::VIEW_CHANNEL,
        guild_id,
        user_id,
        &[role_id],
        &mismatched_overwrites,
    );
    assert_eq!(unchanged, Permissions::VIEW_CHANNEL);
}

/// 5. Граничные значения (`0`, `u64::MAX`, конфликт `allow` и `deny` на одном уровне) и `has_permission`.
#[test]
fn boundary_snowflakes_same_level_allow_precedence_and_has_permission_semantics() {
    // На одном уровне (`@everyone`, роль или участник) одновременный `allow` и `deny` одного и того же бита
    // решается в пользу `allow`: `perms = (perms & !deny) | allow`.
    let guild_id = u64::MAX;
    let user_id = 0;
    let role_id = u64::MAX - 1;

    let same_bit_conflict = [
        PermissionOverwrite {
            id: guild_id,
            overwrite_type: OVERWRITE_TYPE_ROLE,
            allow: Permissions::SEND_MESSAGES,
            deny: Permissions::SEND_MESSAGES,
        },
        PermissionOverwrite {
            id: role_id,
            overwrite_type: OVERWRITE_TYPE_ROLE,
            allow: Permissions::ATTACH_FILES,
            deny: Permissions::ATTACH_FILES,
        },
        PermissionOverwrite {
            id: user_id,
            overwrite_type: OVERWRITE_TYPE_MEMBER,
            allow: Permissions::ADD_REACTIONS,
            deny: Permissions::ADD_REACTIONS,
        },
    ];

    let perms = compute_channel_permissions(
        Permissions::VIEW_CHANNEL,
        guild_id,
        user_id,
        &[role_id],
        &same_bit_conflict,
    );
    assert!(has_permission(perms, Permissions::VIEW_CHANNEL));
    assert!(has_permission(perms, Permissions::SEND_MESSAGES));
    assert!(has_permission(perms, Permissions::ATTACH_FILES));
    assert!(has_permission(perms, Permissions::ADD_REACTIONS));

    // Составная проверка `has_permission`: требует наличия всех битов в `required_perm`,
    // если нет `ADMINISTRATOR`.
    let composite = Permissions::VIEW_CHANNEL | Permissions::SEND_MESSAGES;
    assert!(has_permission(perms, composite));
    assert!(!has_permission(Permissions::VIEW_CHANNEL, composite));
    assert!(has_permission(
        Permissions::ADMINISTRATOR,
        composite | Permissions::MANAGE_GUILD
    ));
}

/// 6. Сквозная сверка с эталонным захватом `fixtures/gateway/by-type/002-READY.json` (гильдия `ITSKIDON`).
#[test]
fn ready_fixture_guild_and_channel_overwrites_match_independent_oracle() {
    let value = support::ready_fixture::load_ready().expect("002-READY.json fixture must load");
    let ready = Ready::from_dispatch(&value).expect("valid READY payload");

    let guild_id = 1_014_537_851_418_980_372u64;
    let moderator_role_id = 1_057_598_992_449_552_425u64;
    let channel_override_role_id = 1_275_727_508_259_606_598u64;
    let target_channel_id = 1_275_727_867_497_680_918u64;

    let guild = ready
        .guilds
        .iter()
        .find(|g| g.id == guild_id)
        .expect("ITSKIDON guild in 002-READY.json");
    let everyone_role = guild
        .roles
        .iter()
        .find(|r| r.id == guild_id)
        .expect("@everyone role in ITSKIDON");
    let moderator_role = guild
        .roles
        .iter()
        .find(|r| r.id == moderator_role_id)
        .expect("moderator role in ITSKIDON");
    let channel = guild
        .channels
        .iter()
        .find(|c| c.id == target_channel_id)
        .expect("target channel in ITSKIDON");

    assert_eq!(everyone_role.permissions, 1_096_185_279_794_753);
    assert_eq!(moderator_role.permissions, 41_780_636_024_647);

    let base = compute_base_permissions(
        999_999_999,
        ready.user.id,
        everyone_role.permissions,
        &[moderator_role.permissions],
    );
    assert_eq!(base, 1_097_311_798_689_607);

    let overwrites: Vec<PermissionOverwrite> = channel
        .permission_overwrites
        .iter()
        .map(PermissionOverwrite::from)
        .collect();
    assert_eq!(overwrites.len(), 2);

    // В фикстуре 002-READY.json сначала идёт role overwrite (1275727508259606598), а вторым — @everyone (1014537851418980372).
    let perms_without_special_role = compute_channel_permissions(
        base,
        guild_id,
        ready.user.id,
        &[moderator_role_id],
        &overwrites,
    );
    assert_eq!(perms_without_special_role, 463_046_060_607_303);

    let perms_with_special_role = compute_channel_permissions(
        base,
        guild_id,
        ready.user.id,
        &[moderator_role_id, channel_override_role_id],
        &overwrites,
    );
    assert_eq!(perms_with_special_role, 463_320_938_565_447);
    assert!(!has_permission(
        perms_with_special_role,
        Permissions::MANAGE_CHANNELS
    ));
}
