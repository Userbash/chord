//! Контракт резолвера битовых масок прав доступа Discord API v9 (`permissions_contract`).
//!
//! Оракулы:
//! - `/home/sanya/re-deep/decoded/4d2a64cb5df5fc4e_0`:
//!   - `@11915496` — модуль `136722` (`BigFlagUtils`: `WQ` add, `TF` remove, `pb` filter, `zy` has, `iu` fromString);
//!   - `@9769850..9776000` — модуль `488926` (`PermissionUtils`: `w` расчёт базовых прав гильдии,
//!     `U` трёхуровневый расчёт канальных перезаписей `@everyone` -> `roles` -> `member`,
//!     маски ограничений `y`/`v` `VIEW_CHANNEL | READ_MESSAGE_HISTORY = 66560` для `lurker`/`isPending`/`timedOut`
//!     и `b` `VIEW_CHANNEL | READ_MESSAGE_HISTORY | CHANGE_NICKNAME = 67175424` для AutoMod карантина);
//! - `fixtures/gateway/by-type/002-READY.json` (md5 `a1a8fd5662be88108dd25ac619a377f2`):
//!   реальная гильдия `ITSKIDON` (`1014537851418980372`), роль `@everyone` (`1096185279794753`),
//!   роль `Модератор` (`1057598992449552425`, права `41780636024647`), канал `1275727867497680918`.

mod support;

use chord_core::permissions::{
    ALL_PERMISSIONS, ALL_V9_DEFINED_PERMISSIONS, LURKER_PERMISSIONS_MASK, MemberRestrictions,
    OVERWRITE_TYPE_MEMBER, OVERWRITE_TYPE_ROLE, PermissionCalculator, PermissionOverwrite,
    PermissionParseError, Permissions, QUARANTINED_PERMISSIONS_MASK, ResolvedPermissions,
    RolePermissions, TIMED_OUT_PERMISSIONS_MASK, TwilightChannelType, TwilightPermissions,
    compute_base_permissions, compute_channel_permissions, has_permission,
    resolve_guild_base_permissions, resolve_permissions, resolve_permissions_with_restrictions,
};
use chord_core::ready::Ready;

/// 1. Все 41 битовый флаг Discord v9 (`0..=40`), операции `BigFlagUtils` (`add`, `remove`, `filter`, `has`)
///    и парсинг/форматирование десятичных строк (`from_decimal_str` / `Display`).
#[test]
fn v9_bitfield_constants_bigflag_ops_and_decimal_parsing_contract() {
    assert_eq!(ALL_V9_DEFINED_PERMISSIONS, (1u64 << 41) - 1);
    assert_eq!(
        LURKER_PERMISSIONS_MASK,
        Permissions::VIEW_CHANNEL | Permissions::READ_MESSAGE_HISTORY
    );
    assert_eq!(LURKER_PERMISSIONS_MASK, 66_560);
    assert_eq!(TIMED_OUT_PERMISSIONS_MASK, 66_560);
    assert_eq!(
        QUARANTINED_PERMISSIONS_MASK,
        Permissions::VIEW_CHANNEL
            | Permissions::READ_MESSAGE_HISTORY
            | Permissions::CHANGE_NICKNAME
    );
    assert_eq!(QUARANTINED_PERMISSIONS_MASK, 67_175_424);

    let p = Permissions::EMPTY
        .add(Permissions::VIEW_CHANNEL)
        .add(Permissions::SEND_MESSAGES)
        .add(Permissions::ATTACH_FILES);
    assert_eq!(
        p.bits(),
        Permissions::VIEW_CHANNEL | Permissions::SEND_MESSAGES | Permissions::ATTACH_FILES
    );

    let after_remove = p.remove(Permissions::ATTACH_FILES);
    assert_eq!(
        after_remove.bits(),
        Permissions::VIEW_CHANNEL | Permissions::SEND_MESSAGES
    );
    assert!(!after_remove.contains(Permissions::ATTACH_FILES));

    let filtered = p.filter(LURKER_PERMISSIONS_MASK);
    assert_eq!(filtered.bits(), Permissions::VIEW_CHANNEL);

    // Десятичная сериализация и парсинг (строки вида "1096185279794753" из Discord API v9).
    let parsed = Permissions::from_decimal_str("1096185279794753")
        .expect("valid decimal u64 string must parse");
    assert_eq!(parsed.bits(), 1_096_185_279_794_753);
    assert_eq!(parsed.to_string(), "1096185279794753");

    let zero = Permissions::from_decimal_str("0").expect("zero must parse");
    assert_eq!(zero, Permissions::EMPTY);

    let max = Permissions::from_decimal_str("18446744073709551615").expect("u64::MAX must parse");
    assert_eq!(max, Permissions::ALL);

    // Негативные сценарии парсинга: пустая строка, отрицательное число, нецифровые символы, переполнение u64.
    assert_eq!(
        Permissions::from_decimal_str(""),
        Err(PermissionParseError::Empty)
    );
    assert_eq!(
        Permissions::from_decimal_str("-1"),
        Err(PermissionParseError::InvalidDigit)
    );
    assert_eq!(
        Permissions::from_decimal_str("1024abc"),
        Err(PermissionParseError::InvalidDigit)
    );
    assert_eq!(
        Permissions::from_decimal_str("18446744073709551616"),
        Err(PermissionParseError::Overflow)
    );
}

/// 2. Базовые права гильдии (`compute_base_permissions` и `resolve_guild_base_permissions`):
///    `@everyone` (id == guild_id) | роли участника, обход владельца (`user_id == owner_id`) и `ADMINISTRATOR`.
#[test]
fn base_permissions_resolver_aggregates_everyone_and_member_roles() {
    let guild_id = 100_000;
    let owner_id = 900_001;
    let member_id = 900_002;
    let role_mod = 200_001;
    let role_helper = 200_002;
    let role_unused = 200_003;

    let guild_roles = [
        RolePermissions {
            id: guild_id,
            permissions: Permissions::VIEW_CHANNEL | Permissions::READ_MESSAGE_HISTORY,
        },
        RolePermissions {
            id: role_mod,
            permissions: Permissions::KICK_MEMBERS | Permissions::MANAGE_MESSAGES,
        },
        RolePermissions {
            id: role_helper,
            permissions: Permissions::SEND_MESSAGES | Permissions::ATTACH_FILES,
        },
        RolePermissions {
            id: role_unused,
            permissions: Permissions::BAN_MEMBERS | Permissions::ADMINISTRATOR,
        },
    ];

    // Участник с ролями role_mod и role_helper (но без role_unused) получает объединение @everyone | role_mod | role_helper.
    let base = resolve_guild_base_permissions(
        guild_id,
        owner_id,
        member_id,
        &guild_roles,
        &[role_mod, role_helper, 999_999],
    );
    let expected = Permissions::VIEW_CHANNEL
        | Permissions::READ_MESSAGE_HISTORY
        | Permissions::KICK_MEMBERS
        | Permissions::MANAGE_MESSAGES
        | Permissions::SEND_MESSAGES
        | Permissions::ATTACH_FILES;
    assert_eq!(base, expected);
    assert!(has_permission(base, Permissions::KICK_MEMBERS));
    assert!(!has_permission(base, Permissions::BAN_MEMBERS));

    // Владелец гильдии всегда получает ALL_PERMISSIONS.
    assert_eq!(
        resolve_guild_base_permissions(guild_id, owner_id, owner_id, &guild_roles, &[]),
        ALL_PERMISSIONS
    );

    // Назначение роли с ADMINISTRATOR (`role_unused`) даёт ALL_PERMISSIONS.
    assert_eq!(
        resolve_guild_base_permissions(guild_id, owner_id, member_id, &guild_roles, &[role_unused]),
        ALL_PERMISSIONS
    );
}

/// 3. Трёхуровневый конвейер перезаписей (`base -> @everyone -> role overwrites -> member overwrites`)
///    и точные флаги возможностей (`can_send`, `can_attach`, `can_delete_others`, `can_kick`).
#[test]
fn full_resolver_pipeline_base_to_role_to_member_overwrites_and_capability_flags() {
    let guild_id = 55_000;
    let owner_id = 55_001;
    let user_id = 55_002;
    let role_a = 66_001;
    let role_b = 66_002;

    let base_perms = compute_base_permissions(
        owner_id,
        user_id,
        Permissions::VIEW_CHANNEL
            | Permissions::READ_MESSAGE_HISTORY
            | Permissions::SEND_MESSAGES
            | Permissions::ATTACH_FILES,
        &[Permissions::KICK_MEMBERS],
    );

    // 1) @everyone запрещает SEND_MESSAGES и ATTACH_FILES;
    // 2) role_a запрещает SEND_MESSAGES, но role_b разрешает SEND_MESSAGES и ATTACH_FILES
    //    (на уровне ролей allow побеждает deny: `(perms & !role_deny) | role_allow`);
    // 3) member overwrite запрещает ATTACH_FILES, и одновременно выставляет allow и deny для MANAGE_MESSAGES
    //    (на уровне участника allow побеждает deny: `(perms & !member_deny) | member_allow`).
    let overwrites = [
        PermissionOverwrite {
            id: user_id,
            overwrite_type: OVERWRITE_TYPE_MEMBER,
            allow: Permissions::MANAGE_MESSAGES,
            deny: Permissions::ATTACH_FILES | Permissions::MANAGE_MESSAGES,
        },
        PermissionOverwrite {
            id: role_a,
            overwrite_type: OVERWRITE_TYPE_ROLE,
            allow: 0,
            deny: Permissions::SEND_MESSAGES,
        },
        PermissionOverwrite {
            id: role_b,
            overwrite_type: OVERWRITE_TYPE_ROLE,
            allow: Permissions::SEND_MESSAGES | Permissions::ATTACH_FILES,
            deny: 0,
        },
        PermissionOverwrite {
            id: guild_id,
            overwrite_type: OVERWRITE_TYPE_ROLE,
            allow: 0,
            deny: Permissions::SEND_MESSAGES | Permissions::ATTACH_FILES,
        },
    ];

    let resolved: ResolvedPermissions = resolve_permissions(
        base_perms,
        guild_id,
        user_id,
        &[role_a, role_b],
        &overwrites,
    );

    assert!(resolved.can_view);
    assert!(resolved.can_send);
    assert!(!resolved.can_attach);
    assert!(resolved.can_delete_others);
    assert!(resolved.can_kick);
    assert!(!resolved.can_ban);
    assert!(!resolved.is_admin);
    assert_eq!(
        resolved.channel_bits,
        Permissions::VIEW_CHANNEL
            | Permissions::READ_MESSAGE_HISTORY
            | Permissions::KICK_MEMBERS
            | Permissions::SEND_MESSAGES
            | Permissions::MANAGE_MESSAGES
    );
    assert_eq!(resolved.effective_bits, resolved.channel_bits);
}

/// 4. Неявные зависимости канальных прав Discord v9:
///    - без `VIEW_CHANNEL` все канальные права обнуляются (`effective_bits == 0`, `can_send = false`, `can_attach = false`, `can_delete_others = false`);
///    - без `SEND_MESSAGES` сбрасываются `ATTACH_FILES`, `EMBED_LINKS`, `MENTION_EVERYONE`, `SEND_TTS_MESSAGES` (`can_attach = false`);
///    - без `CONNECT` сбрасываются голосовые права `SPEAK`, `STREAM`, `USE_VAD`, `MUTE_MEMBERS`, `DEAFEN_MEMBERS`, `MOVE_MEMBERS`, `PRIORITY_SPEAKER`.
#[test]
fn implicit_channel_permission_dependencies_strip_dependent_capabilities() {
    let guild_id = 77_000;
    let user_id = 77_001;
    let role_id = 77_002;

    let rich_base = Permissions::VIEW_CHANNEL
        | Permissions::SEND_MESSAGES
        | Permissions::SEND_TTS_MESSAGES
        | Permissions::ATTACH_FILES
        | Permissions::EMBED_LINKS
        | Permissions::MENTION_EVERYONE
        | Permissions::MANAGE_MESSAGES
        | Permissions::KICK_MEMBERS
        | Permissions::CONNECT
        | Permissions::SPEAK
        | Permissions::STREAM
        | Permissions::USE_VAD;

    // Сценарий A: канал закрыт (`deny: VIEW_CHANNEL`), даже если разрешены SEND_MESSAGES и ATTACH_FILES.
    let denied_view = resolve_permissions(
        rich_base,
        guild_id,
        user_id,
        &[role_id],
        &[PermissionOverwrite {
            id: guild_id,
            overwrite_type: OVERWRITE_TYPE_ROLE,
            allow: Permissions::SEND_MESSAGES | Permissions::ATTACH_FILES,
            deny: Permissions::VIEW_CHANNEL,
        }],
    );
    assert!(!denied_view.can_view);
    assert!(!denied_view.can_send);
    assert!(!denied_view.can_attach);
    assert!(!denied_view.can_delete_others);
    assert!(!denied_view.can_connect);
    assert!(!denied_view.can_speak);
    assert!(!denied_view.can_stream);
    assert_eq!(denied_view.effective_bits, 0);
    // При этом серверное право кика (`base_bits`) сохранено:
    assert!(denied_view.can_kick);

    // Сценарий B: `VIEW_CHANNEL` есть, но `SEND_MESSAGES` запрещён -> `can_attach` и `ATTACH_FILES`/`EMBED_LINKS` сброшены,
    // однако `MANAGE_MESSAGES` (`can_delete_others`) продолжает работать.
    let denied_send = resolve_permissions(
        rich_base,
        guild_id,
        user_id,
        &[role_id],
        &[PermissionOverwrite {
            id: user_id,
            overwrite_type: OVERWRITE_TYPE_MEMBER,
            allow: Permissions::ATTACH_FILES | Permissions::EMBED_LINKS,
            deny: Permissions::SEND_MESSAGES,
        }],
    );
    assert!(denied_send.can_view);
    assert!(!denied_send.can_send);
    assert!(!denied_send.can_attach);
    assert!(denied_send.can_delete_others);
    assert_eq!(
        denied_send.effective_bits
            & (Permissions::ATTACH_FILES
                | Permissions::EMBED_LINKS
                | Permissions::SEND_TTS_MESSAGES
                | Permissions::MENTION_EVERYONE),
        0
    );

    // Сценарий C: `CONNECT` запрещён в голосовом канале -> `can_speak` и `can_stream` равны `false`.
    let denied_connect = resolve_permissions(
        rich_base,
        guild_id,
        user_id,
        &[role_id],
        &[PermissionOverwrite {
            id: role_id,
            overwrite_type: OVERWRITE_TYPE_ROLE,
            allow: Permissions::SPEAK | Permissions::STREAM,
            deny: Permissions::CONNECT,
        }],
    );
    assert!(denied_connect.can_view);
    assert!(!denied_connect.can_connect);
    assert!(!denied_connect.can_speak);
    assert!(!denied_connect.can_stream);
    assert_eq!(
        denied_connect.effective_bits
            & (Permissions::SPEAK | Permissions::STREAM | Permissions::USE_VAD),
        0
    );
}

/// 5. Маски ограничений участника из RE-дампа (`lurker`, `is_pending`, `is_timed_out`, `is_quarantined`)
///    и иммунитет `ADMINISTRATOR` к тайм-ауту и карантину (`4d2a64cb5df5fc4e_0@9771450`).
#[test]
fn member_restrictions_lurker_pending_timeout_and_quarantine_masks_match_re_oracle() {
    let guild_id = 88_000;
    let user_id = 88_001;
    let base = Permissions::VIEW_CHANNEL
        | Permissions::READ_MESSAGE_HISTORY
        | Permissions::SEND_MESSAGES
        | Permissions::ATTACH_FILES
        | Permissions::CHANGE_NICKNAME
        | Permissions::KICK_MEMBERS
        | Permissions::CONNECT
        | Permissions::SPEAK;

    // Timed-out участник без ADMINISTRATOR урезается до VIEW_CHANNEL | READ_MESSAGE_HISTORY.
    let timed_out = resolve_permissions_with_restrictions(
        base,
        guild_id,
        user_id,
        &[],
        &[],
        MemberRestrictions {
            is_timed_out: true,
            ..MemberRestrictions::default()
        },
    );
    assert!(timed_out.can_view);
    assert!(timed_out.can_read_history);
    assert!(!timed_out.can_send);
    assert!(!timed_out.can_attach);
    assert!(!timed_out.can_kick);
    assert_eq!(
        timed_out.effective_bits,
        Permissions::VIEW_CHANNEL | Permissions::READ_MESSAGE_HISTORY
    );

    // AutoMod карантин сохраняет CHANGE_NICKNAME вместе с VIEW_CHANNEL | READ_MESSAGE_HISTORY.
    let quarantined = resolve_permissions_with_restrictions(
        base,
        guild_id,
        user_id,
        &[],
        &[],
        MemberRestrictions {
            is_quarantined: true,
            ..MemberRestrictions::default()
        },
    );
    assert_eq!(
        quarantined.effective_bits,
        Permissions::VIEW_CHANNEL
            | Permissions::READ_MESSAGE_HISTORY
            | Permissions::CHANGE_NICKNAME
    );
    assert!(!quarantined.can_send);

    // ADMINISTRATOR обходит timed_out и quarantined (в точности как `!l` в `U()` в `4d2a64cb5df5fc4e_0`).
    let admin_timed_out = resolve_permissions_with_restrictions(
        base | Permissions::ADMINISTRATOR,
        guild_id,
        user_id,
        &[],
        &[],
        MemberRestrictions {
            is_timed_out: true,
            is_quarantined: true,
            ..MemberRestrictions::default()
        },
    );
    assert!(admin_timed_out.is_admin);
    assert!(admin_timed_out.can_send);
    assert!(admin_timed_out.can_attach);
    assert!(admin_timed_out.can_delete_others);
    assert!(admin_timed_out.can_kick);
    assert_eq!(admin_timed_out.effective_bits, ALL_PERMISSIONS);

    // Lurker / Pending урезается до LURKER_PERMISSIONS_MASK (66560).
    let pending = resolve_permissions_with_restrictions(
        base,
        guild_id,
        user_id,
        &[],
        &[],
        MemberRestrictions {
            is_pending: true,
            ..MemberRestrictions::default()
        },
    );
    assert_eq!(pending.effective_bits, LURKER_PERMISSIONS_MASK);
    assert!(!pending.can_send);
}

/// 6. Сквозная сверка резолвера на реальном захвате `fixtures/gateway/by-type/002-READY.json` (гильдия `ITSKIDON`).
#[test]
fn ready_capture_guild_roles_and_channel_overwrites_resolve_exact_capabilities() {
    let value = support::ready_fixture::load_ready().expect("002-READY.json fixture must load");
    let ready = Ready::from_dispatch(&value).expect("valid READY payload");

    let guild_id = 1_014_537_851_418_980_372u64;
    let moderator_role_id = 1_057_598_992_449_552_425u64;
    let special_role_id = 1_275_727_508_259_606_598u64;
    let target_channel_id = 1_275_727_867_497_680_918u64;

    let guild = ready
        .guilds
        .iter()
        .find(|g| g.id == guild_id)
        .expect("ITSKIDON guild must exist in 002-READY.json");
    let channel = guild
        .channels
        .iter()
        .find(|c| c.id == target_channel_id)
        .expect("target channel must exist in ITSKIDON");

    let role_defs: Vec<RolePermissions> = guild.roles.iter().map(RolePermissions::from).collect();
    let overwrites: Vec<PermissionOverwrite> = channel
        .permission_overwrites
        .iter()
        .map(PermissionOverwrite::from)
        .collect();

    let base = resolve_guild_base_permissions(
        guild_id,
        999_999_999,
        ready.user.id,
        &role_defs,
        &[moderator_role_id],
    );
    assert_eq!(base, 1_097_311_798_689_607);

    let without_special = resolve_permissions(
        base,
        guild_id,
        ready.user.id,
        &[moderator_role_id],
        &overwrites,
    );
    assert_eq!(without_special.channel_bits, 463_046_060_607_303);
    assert!(without_special.can_view);
    assert!(without_special.can_read_history);
    assert!(!without_special.can_send);
    assert!(!without_special.can_attach);
    assert!(!without_special.can_delete_others);
    assert!(without_special.can_kick);

    let with_special = resolve_permissions(
        base,
        guild_id,
        ready.user.id,
        &[moderator_role_id, special_role_id],
        &overwrites,
    );
    assert_eq!(with_special.channel_bits, 463_320_938_565_447);
    assert_eq!(
        compute_channel_permissions(
            base,
            guild_id,
            ready.user.id,
            &[moderator_role_id, special_role_id],
            &overwrites
        ),
        with_special.channel_bits
    );
    assert!(with_special.can_view);
    assert!(with_special.can_read_history);
    assert!(with_special.can_send);
    assert!(with_special.can_attach);
    assert!(!with_special.can_delete_others);
    assert!(with_special.can_kick);

    // Без канальных перезаписей базовые права модератора в ITSKIDON дают can_delete_others = true.
    let default_channel =
        resolve_permissions(base, guild_id, ready.user.id, &[moderator_role_id], &[]);
    assert!(default_channel.can_delete_others);
    assert!(default_channel.can_send);
    assert!(default_channel.can_attach);
    assert!(default_channel.can_kick);
}

/// 7. `twilight-util::permission_calculator::PermissionCalculator` Administrator bypass:
///    Administrator role or base bit bypasses all channel denial overwrites.
#[test]
fn twilight_permission_calculator_administrator_bypass_ignores_channel_denials() {
    let guild_id = 10_001u64;
    let user_id = 20_002u64;
    let admin_role_id = 30_003u64;

    let deny_all = [
        PermissionOverwrite {
            id: guild_id,
            overwrite_type: OVERWRITE_TYPE_ROLE,
            allow: 0,
            deny: Permissions::VIEW_CHANNEL | Permissions::SEND_MESSAGES | Permissions::CONNECT,
        },
        PermissionOverwrite {
            id: admin_role_id,
            overwrite_type: OVERWRITE_TYPE_ROLE,
            allow: 0,
            deny: Permissions::VIEW_CHANNEL | Permissions::SEND_MESSAGES,
        },
        PermissionOverwrite {
            id: user_id,
            overwrite_type: OVERWRITE_TYPE_MEMBER,
            allow: 0,
            deny: Permissions::VIEW_CHANNEL | Permissions::SEND_MESSAGES,
        },
    ];

    let channel_perms = compute_channel_permissions(
        Permissions::VIEW_CHANNEL | Permissions::ADMINISTRATOR,
        guild_id,
        user_id,
        &[admin_role_id],
        &deny_all,
    );
    assert_eq!(channel_perms, ALL_PERMISSIONS);

    let granted_admin_ow = [PermissionOverwrite {
        id: user_id,
        overwrite_type: OVERWRITE_TYPE_MEMBER,
        allow: Permissions::ADMINISTRATOR,
        deny: Permissions::VIEW_CHANNEL | Permissions::SEND_MESSAGES,
    }];
    let channel_admin = compute_channel_permissions(
        Permissions::VIEW_CHANNEL,
        guild_id,
        user_id,
        &[],
        &granted_admin_ow,
    );
    assert_eq!(channel_admin, ALL_PERMISSIONS);
}

/// 8. `twilight-util::permission_calculator::PermissionCalculator` Guild Owner bypass:
///    Guild owner always receives full permissions regardless of empty roles or channel denials.
#[test]
fn twilight_permission_calculator_guild_owner_bypass_grants_full_access() {
    let guild_id = 50_000u64;
    let owner_id = 60_000u64;

    let base = compute_base_permissions(owner_id, owner_id, 0, &[]);
    assert_eq!(base, ALL_PERMISSIONS);

    let deny_overwrites = [PermissionOverwrite {
        id: owner_id,
        overwrite_type: OVERWRITE_TYPE_MEMBER,
        allow: 0,
        deny: Permissions::VIEW_CHANNEL | Permissions::SEND_MESSAGES,
    }];
    let resolved = resolve_permissions(base, guild_id, owner_id, &[], &deny_overwrites);
    assert!(resolved.is_admin);
    assert!(resolved.can_view);
    assert!(resolved.can_send);
    assert_eq!(resolved.effective_bits, ALL_PERMISSIONS);
}

/// 9. `twilight-util::permission_calculator::PermissionCalculator` Denial overwrites:
///    `@everyone`, role, and member denial overwrites strip permissions according to hierarchy.
#[test]
fn twilight_permission_calculator_denial_overwrites_cascade_everyone_role_member() {
    use twilight_model::id::Id;

    let guild_id = Id::new(100);
    let user_id = Id::new(200);
    let role_id = Id::new(300);

    let everyone = TwilightPermissions::VIEW_CHANNEL
        | TwilightPermissions::SEND_MESSAGES
        | TwilightPermissions::EMBED_LINKS
        | TwilightPermissions::ATTACH_FILES
        | TwilightPermissions::ADD_REACTIONS;
    let member_roles = [(role_id, TwilightPermissions::MANAGE_MESSAGES)];

    let overwrites = [PermissionOverwrite {
        id: 300,
        overwrite_type: OVERWRITE_TYPE_ROLE,
        allow: Permissions::ATTACH_FILES,
        deny: Permissions::SEND_MESSAGES,
    }
    .to_twilight_overwrite(1)];

    let calc = PermissionCalculator::new(guild_id, user_id, everyone, &member_roles);
    let result = calc.in_channel(TwilightChannelType::GuildText, &overwrites);

    assert!(result.contains(TwilightPermissions::VIEW_CHANNEL));
    assert!(result.contains(TwilightPermissions::MANAGE_MESSAGES));
    assert!(result.contains(TwilightPermissions::ADD_REACTIONS));
    assert!(!result.contains(TwilightPermissions::SEND_MESSAGES));
    assert!(!result.contains(TwilightPermissions::ATTACH_FILES));
    assert!(!result.contains(TwilightPermissions::EMBED_LINKS));
}

/// 10. Multi-role overwrite union: role `allow` on one role overrides role `deny` on another role,
///     while member `deny` overrides role `allow`.
#[test]
fn twilight_permission_calculator_multi_role_allow_overrides_other_role_deny() {
    let guild_id = 1_000u64;
    let user_id = 2_000u64;
    let role_deny = 3_001u64;
    let role_allow = 3_002u64;

    let base = Permissions::VIEW_CHANNEL | Permissions::CONNECT | Permissions::SPEAK;
    let overwrites = [
        PermissionOverwrite {
            id: role_deny,
            overwrite_type: OVERWRITE_TYPE_ROLE,
            allow: 0,
            deny: Permissions::CONNECT | Permissions::SPEAK | Permissions::STREAM,
        },
        PermissionOverwrite {
            id: role_allow,
            overwrite_type: OVERWRITE_TYPE_ROLE,
            allow: Permissions::CONNECT | Permissions::SPEAK | Permissions::STREAM,
            deny: 0,
        },
        PermissionOverwrite {
            id: user_id,
            overwrite_type: OVERWRITE_TYPE_MEMBER,
            allow: 0,
            deny: Permissions::STREAM,
        },
    ];

    let perms = compute_channel_permissions(
        base,
        guild_id,
        user_id,
        &[role_deny, role_allow],
        &overwrites,
    );
    assert!(has_permission(perms, Permissions::VIEW_CHANNEL));
    assert!(has_permission(perms, Permissions::CONNECT));
    assert!(has_permission(perms, Permissions::SPEAK));
    assert!(!has_permission(perms, Permissions::STREAM));
}

/// 11. Boundary snowflakes (`0`, `u64::MAX`), high bitmasks (`> 1 << 46`), and unknown overwrite types.
#[test]
fn twilight_permission_calculator_boundary_snowflakes_and_unknown_overwrite_types() {
    let guild_id = 0u64;
    let user_id = u64::MAX;
    let role_id = u64::MAX - 1;
    let high_bit = 1u64 << 50;

    let base = Permissions::VIEW_CHANNEL | Permissions::SEND_MESSAGES | high_bit;
    let overwrites = [
        PermissionOverwrite {
            id: guild_id,
            overwrite_type: OVERWRITE_TYPE_ROLE,
            allow: 0,
            deny: high_bit,
        },
        PermissionOverwrite {
            id: role_id,
            overwrite_type: OVERWRITE_TYPE_ROLE,
            allow: high_bit,
            deny: Permissions::SEND_MESSAGES,
        },
        PermissionOverwrite {
            id: user_id,
            overwrite_type: 99,
            allow: Permissions::ADMINISTRATOR,
            deny: Permissions::VIEW_CHANNEL,
        },
    ];

    let perms = compute_channel_permissions(base, guild_id, user_id, &[role_id], &overwrites);
    assert!(has_permission(perms, Permissions::VIEW_CHANNEL));
    assert!(has_permission(perms, high_bit));
    assert!(!has_permission(perms, Permissions::SEND_MESSAGES));
    assert!(!has_permission(perms, Permissions::ADMINISTRATOR));
}

/// 12. Channel-type specific permission filtering via `PermissionCalculator::in_channel`.
#[test]
fn twilight_permission_calculator_channel_type_omissions_text_voice_stage() {
    use twilight_model::id::Id;

    let guild_id = Id::new(777);
    let user_id = Id::new(888);
    let everyone = TwilightPermissions::VIEW_CHANNEL
        | TwilightPermissions::SEND_MESSAGES
        | TwilightPermissions::CONNECT
        | TwilightPermissions::SPEAK
        | TwilightPermissions::REQUEST_TO_SPEAK;

    let text_perms = PermissionCalculator::new(guild_id, user_id, everyone, &[])
        .in_channel(TwilightChannelType::GuildText, &[]);
    assert!(text_perms.contains(TwilightPermissions::VIEW_CHANNEL));
    assert!(text_perms.contains(TwilightPermissions::SEND_MESSAGES));
    assert!(!text_perms.contains(TwilightPermissions::CONNECT));
    assert!(!text_perms.contains(TwilightPermissions::SPEAK));

    let voice_perms = PermissionCalculator::new(guild_id, user_id, everyone, &[])
        .in_channel(TwilightChannelType::GuildVoice, &[]);
    assert!(voice_perms.contains(TwilightPermissions::VIEW_CHANNEL));
    assert!(voice_perms.contains(TwilightPermissions::CONNECT));
    assert!(voice_perms.contains(TwilightPermissions::SPEAK));
    assert!(!voice_perms.contains(TwilightPermissions::SEND_MESSAGES));

    let stage_perms = PermissionCalculator::new(guild_id, user_id, everyone, &[])
        .in_channel(TwilightChannelType::GuildStageVoice, &[]);
    assert!(stage_perms.contains(TwilightPermissions::VIEW_CHANNEL));
    assert!(stage_perms.contains(TwilightPermissions::CONNECT));
    assert!(stage_perms.contains(TwilightPermissions::REQUEST_TO_SPEAK));
    assert!(!stage_perms.contains(TwilightPermissions::SPEAK));
}
