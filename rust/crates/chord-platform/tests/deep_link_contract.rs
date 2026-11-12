//! Контракт P6-04: разбор `discord://` deep-link и маршрутизация.
//!
//! Независимый оракул:
//! - `docs/re/main-arch.md` §9.4 — дословная RE-цитата `extractUrlFromArgs`
//!   (`--url` только при наличии `--` после него, иначе argv с индекса 1 с префиксом `discord://`);
//! - строки бандлов `/home/sanya/re-deep/out/bundles/*.js` (только чтение):
//!   `discord://login/one-time?token=…`, `discord://action/oauth2/authorize`,
//!   `discord://app/open#…`, `discord://app/settings/nitro`, `discord:///library/${id}/launch`;
//! - `discord://channels/...` и `discord://-/channels/...` — форма навигации каналов.

use chord_platform::deep_link::{
    BLOCKED_URL_PROTOCOLS, ChannelTarget, CustomProtocolHandler, DISCORD_CLIP_PROTOCOL,
    DISCORD_CLIP_UUID_BYTES, DISCORD_CLIP_UUID_HEX, DeepLink, DeepLinkError, DeepLinkQueue,
    DiscordMp4ErrorReason, MAX_DEEP_LINK_LEN, MAX_INVITE_CODE_LEN, MainWindowPath,
    MainWindowPathError, NavigationDecision, OAuth2AuthorizeError, OAuth2AuthorizeParams,
    ProtocolPrivilegeSpec, Route, before_ready_protocol_registration, check_url_origin_matches,
    classify_webview_navigation, desktop_protocol_handler_entry, extract_deep_link,
    extract_url_from_args, main_window_path, mimeapps_scheme_entries, parse_channel_target,
    parse_deep_link, parse_oauth2_authorize_params, route, serve_disclip_request,
    should_open_external_url, verify_discord_mp4_bytes, verify_discord_mp4_file,
    windows_protocol_registry_commands,
};

fn parse(raw: &str) -> DeepLink<'_> {
    match parse_deep_link(raw) {
        Ok(link) => link,
        Err(err) => panic!("ожидался валидный deep link {raw:?}, получено {err:?}"),
    }
}

fn args(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| (*item).to_owned()).collect()
}

#[test]
fn d01_app_settings_route() {
    let link = parse("discord://app/settings/nitro");
    assert_eq!(link.raw, "discord://app/settings/nitro");
    assert_eq!(link.host, "app");
    assert_eq!(link.path, "/settings/nitro");
    assert_eq!(link.query, None);
    assert_eq!(link.fragment, None);
    assert_eq!(
        route(&link),
        Route::AppSettings {
            section: "nitro".to_owned()
        }
    );
}

#[test]
fn d02_empty_host_library_route() {
    let link = parse("discord:///library/42/launch");
    assert_eq!(link.host, "");
    assert_eq!(link.path, "/library/42/launch");
    assert_eq!(
        route(&link),
        Route::Library {
            id: "42".to_owned()
        }
    );
}

#[test]
fn d03_login_one_time_token_is_redacted_in_debug() {
    let link = parse("discord://login/one-time?token=secret-token");
    assert_eq!(link.host, "login");
    assert_eq!(link.path, "/one-time");
    assert_eq!(link.query, Some("token=secret-token"));

    let parsed_route = route(&link);
    assert_eq!(
        parsed_route,
        Route::LoginOneTime {
            token: Some("secret-token".to_owned())
        }
    );
    let debug = format!("{parsed_route:?}");
    assert!(
        !debug.contains("secret-token"),
        "Debug не должен печатать токен: {debug}"
    );
    assert!(debug.contains("<redacted>"), "ожидалась redaction: {debug}");

    let without_token = route(&parse("discord://login/one-time"));
    assert_eq!(
        without_token,
        Route::LoginOneTime { token: None },
        "отсутствующий токен — валидная форма"
    );
    let none_debug = format!("{without_token:?}");
    assert!(none_debug.contains("None"), "{none_debug}");
    assert!(!none_debug.contains("<redacted>"), "{none_debug}");
}

#[test]
fn d04_oauth2_authorize_route() {
    let link = parse("discord://action/oauth2/authorize?client_id=1&scope=identify&state=x");
    assert_eq!(link.host, "action");
    assert_eq!(link.path, "/oauth2/authorize");
    assert_eq!(link.query, Some("client_id=1&scope=identify&state=x"));
    assert_eq!(route(&link), Route::OAuth2Authorize);
}

#[test]
fn d05_channels_routes() {
    let dashed = parse("discord://-/channels/1/2");
    assert_eq!(dashed.host, "-");
    assert_eq!(dashed.path, "/channels/1/2");
    assert_eq!(
        route(&dashed),
        Route::Channels {
            path: "/channels/1/2".to_owned()
        }
    );

    let direct = parse("discord://channels/@me");
    assert_eq!(direct.host, "channels");
    assert_eq!(direct.path, "/@me");
    assert_eq!(
        route(&direct),
        Route::Channels {
            path: "/channels/@me".to_owned()
        }
    );

    let empty_host = parse("discord:///channels/@me/123");
    assert_eq!(
        route(&empty_host),
        Route::Channels {
            path: "/channels/@me/123".to_owned()
        }
    );
}

#[test]
fn d06_rejects_empty_scheme_length_and_controls() {
    assert_eq!(parse_deep_link(""), Err(DeepLinkError::Empty));
    assert_eq!(
        parse_deep_link("https://discord.com/app"),
        Err(DeepLinkError::WrongScheme)
    );
    assert_eq!(
        parse_deep_link("discord:app/open"),
        Err(DeepLinkError::Malformed),
        "discord: без // — Malformed (ТЗ)"
    );
    assert_eq!(parse_deep_link("no-scheme"), Err(DeepLinkError::Malformed));
    assert_eq!(
        parse_deep_link("discord://ap p/open"),
        Err(DeepLinkError::Malformed),
        "пробел в host запрещён"
    );
    assert_eq!(
        parse_deep_link("discord ://app/open"),
        Err(DeepLinkError::Malformed),
        "пробел в scheme запрещён"
    );

    for raw in [
        "discord://app/open\n",
        "discord://app/open\r",
        "discord://app/open\u{1f}",
        "discord://app/open\u{7f}",
        "discord://app/open?x=\u{0}",
    ] {
        assert_eq!(
            parse_deep_link(raw),
            Err(DeepLinkError::Malformed),
            "control chars в raw: {raw:?}"
        );
    }

    // `%`-encoded control-последовательность — обычный текст, не control char.
    assert!(parse_deep_link("discord://app/open?x=%0A").is_ok());

    // Граница лимита: 4096 включительно, 4097 — TooLong.
    let at_limit = format!("discord://app/open#{}", "a".repeat(MAX_DEEP_LINK_LEN - 19));
    assert_eq!(at_limit.len(), MAX_DEEP_LINK_LEN);
    assert!(parse_deep_link(&at_limit).is_ok(), "4096 байт допустимы");
    let over_limit = format!("{at_limit}a");
    assert_eq!(parse_deep_link(&over_limit), Err(DeepLinkError::TooLong));
    assert_eq!(
        parse_deep_link(&format!(
            "https://discord.com/{}",
            "x".repeat(MAX_DEEP_LINK_LEN)
        )),
        Err(DeepLinkError::TooLong),
        "лимит проверяется до схемы"
    );
}

#[test]
fn d07_extract_deep_link_matches_re() {
    // RE: for (let i = 1; i < args.length; i++) if (args[i].toLowerCase().startsWith("discord://"))
    assert_eq!(
        extract_deep_link(&args(&["discord-lab", "--flag", "DISCORD://app/open"])),
        Some("DISCORD://app/open")
    );
    assert_eq!(
        extract_deep_link(&args(&["discord://app/open"])),
        None,
        "argv[0] не сканируется"
    );
    assert_eq!(
        extract_deep_link(&args(&["discord-lab", "--flag", "https://example.com"])),
        None
    );

    // RE: --url без `--` → null.
    assert_eq!(
        extract_deep_link(&args(&["discord-lab", "--url", "discord://app/open"])),
        None
    );
    // RE: `--` раньше `--url` → null (сравнение индексов).
    assert_eq!(
        extract_deep_link(&args(&["discord-lab", "--", "--url", "discord://app/open"])),
        None
    );
    // RE: `--` после `--url` → аргумент сразу после `--`.
    assert_eq!(
        extract_deep_link(&args(&[
            "discord-lab",
            "--url",
            "ignored",
            "--",
            "discord://login/one-time?token=x"
        ])),
        Some("discord://login/one-time?token=x")
    );
    // RE не валидирует содержимое аргумента после `--`.
    assert_eq!(
        extract_deep_link(&args(&[
            "discord-lab",
            "--url",
            "ignored",
            "--",
            "https://example.com"
        ])),
        Some("https://example.com")
    );
    // RE: args[passThroughArgsIndex + 1] === undefined → null.
    assert_eq!(
        extract_deep_link(&args(&["discord-lab", "--url", "x", "--"])),
        None
    );
    assert_eq!(extract_deep_link(&[]), None);
}

#[test]
fn d08_query_and_fragment_are_split_before_path() {
    let link = parse("discord://app/open?x=1#frag");
    assert_eq!(link.path, "/open");
    assert_eq!(link.query, Some("x=1"));
    assert_eq!(link.fragment, Some("frag"));

    // `#` раньше `?`: остаток после `#` — fragment целиком.
    let fragment_first = parse("discord://app/open#frag?x=1");
    assert_eq!(fragment_first.path, "/open");
    assert_eq!(fragment_first.query, None);
    assert_eq!(fragment_first.fragment, Some("frag?x=1"));

    for (raw, query, fragment) in [
        ("discord://app/open?", None, None),
        ("discord://app/open#", None, None),
        ("discord://app/open?#", None, None),
        ("discord://app/open?x=1#", Some("x=1"), None),
        ("discord://app/open#frag", None, Some("frag")),
    ] {
        let parsed = parse(raw);
        assert_eq!(parsed.query, query, "{raw}");
        assert_eq!(parsed.fragment, fragment, "{raw}");
        assert_eq!(parsed.path, "/open", "{raw}");
    }
}

#[test]
fn d09_scheme_and_host_case_insensitive_path_case_sensitive() {
    let link = parse("DISCORD://APP/open");
    assert_eq!(link.host, "APP");
    assert_eq!(route(&link), Route::AppOpen);
    assert_eq!(
        route(&parse("discord://app/OPEN")),
        Route::Unknown,
        "путь чувствителен к регистру"
    );
}

#[test]
fn d10_unconfirmed_routes_are_unknown() {
    for raw in [
        "discord://app/not-open",
        "discord://library/42",
        "discord://app/settings",
        "discord://app/settings/",
        "discord://app/settings/nitro/extra",
        "discord://login/one-time/extra",
        "discord://action/oauth2",
        "discord://channels",
        "discord://-/channels",
        "discord://foo/bar",
        "discord://app/open/extra",
        "discord://-/library/42/launch",
        "discord://",
    ] {
        assert_eq!(route(&parse(raw)), Route::Unknown, "{raw}");
    }
}

#[test]
fn d11_authority_only_and_trailing_slash_paths() {
    let link = parse("discord://?x=1");
    assert_eq!(link.host, "");
    assert_eq!(link.path, "");
    assert_eq!(link.query, Some("x=1"));
    assert_eq!(link.fragment, None);
    assert_eq!(route(&link), Route::Unknown);

    let bare = parse("discord://");
    assert_eq!(bare.host, "");
    assert_eq!(bare.path, "");
    assert_eq!((bare.query, bare.fragment), (None, None));

    let trailing = parse("discord://app/");
    assert_eq!(trailing.host, "app");
    assert_eq!(trailing.path, "/");
    assert_eq!(route(&trailing), Route::Unknown);
}

#[test]
fn d12_token_extraction_edges() {
    for (raw, expected) in [
        ("discord://login/one-time?token=", None),
        ("discord://login/one-time?other=1", None),
        ("discord://login/one-time?a=1&token=t1&token=t2", Some("t1")),
        ("discord://login/one-time?TOKEN=upper", None),
        ("discord://login/one-time?token=a%20b", Some("a%20b")),
    ] {
        assert_eq!(
            route(&parse(raw)),
            Route::LoginOneTime {
                token: expected.map(str::to_owned)
            },
            "{raw}"
        );
    }
}

#[test]
fn d13_deep_link_debug_redacts_query_and_fragment() {
    let link = parse("discord://login/one-time?token=secret-token#frag-secret");
    let debug = format!("{link:?}");
    assert!(!debug.contains("secret"), "Debug утёк секрет: {debug}");
    assert!(!debug.contains("raw"), "Debug печатает raw: {debug}");
    assert!(debug.contains("login"), "host потерян: {debug}");
    assert!(debug.contains("<redacted>"), "нет redaction: {debug}");

    let plain = parse("discord://app/open");
    let plain_debug = format!("{plain:?}");
    assert!(plain_debug.contains("query: None"), "{plain_debug}");
    assert!(plain_debug.contains("fragment: None"), "{plain_debug}");
}

#[test]
fn d14_disclip_protocol_parsing() {
    assert_eq!(DISCORD_CLIP_PROTOCOL, "disclip");

    let link1 = parse("disclip://my-cool-clip");
    assert_eq!(link1.scheme, "disclip");
    assert_eq!(link1.host, "my-cool-clip");
    assert_eq!(
        route(&link1),
        Route::Clip {
            id: "my-cool-clip".to_owned()
        }
    );

    let link2 = parse("disclip:///clip-123");
    assert_eq!(link2.scheme, "disclip");
    assert_eq!(link2.host, "");
    assert_eq!(link2.path, "/clip-123");
    assert_eq!(
        route(&link2),
        Route::Clip {
            id: "clip-123".to_owned()
        }
    );

    let link3 = parse("disclip:clip-456");
    assert_eq!(link3.scheme, "disclip");
    assert_eq!(
        route(&link3),
        Route::Clip {
            id: "clip-456".to_owned()
        }
    );

    let link4 = parse("disclip://clips/clip-789");
    assert_eq!(
        route(&link4),
        Route::Clip {
            id: "clip-789".to_owned()
        }
    );

    let link5 = parse("discord://clips/clip-789");
    assert_eq!(
        route(&link5),
        Route::Clip {
            id: "clip-789".to_owned()
        }
    );

    let link6 = parse("discord://-/clips/clip-789");
    assert_eq!(
        route(&link6),
        Route::Clip {
            id: "clip-789".to_owned()
        }
    );
}

#[test]
fn d15_extract_url_from_args_oracle() {
    // 1. Direct deep link in argv[1..]
    assert_eq!(
        extract_url_from_args(&args(&["discord-lab", "discord://channels/123/456"])),
        Some("discord://channels/123/456".to_owned())
    );
    assert_eq!(
        extract_url_from_args(&args(&["discord-lab", "--flag", "DISCLIP://clip-123"])),
        Some("DISCLIP://clip-123".to_owned())
    );
    assert_eq!(
        extract_url_from_args(&args(&["discord-lab", "disclip:clip-456"])),
        Some("disclip:clip-456".to_owned())
    );

    // 2. Electron oracle: --url before --
    assert_eq!(
        extract_url_from_args(&args(&[
            "discord-lab",
            "--url",
            "ignored",
            "--",
            "discord://login/one-time?token=x"
        ])),
        Some("discord://login/one-time?token=x".to_owned())
    );

    // 3. --url after --
    assert_eq!(
        extract_url_from_args(&args(&[
            "discord-lab",
            "--",
            "--url",
            "discord://channels/1/2"
        ])),
        Some("discord://channels/1/2".to_owned())
    );

    // 4. --url=<val> after --
    assert_eq!(
        extract_url_from_args(&args(&["discord-lab", "--", "--url=disclip://clip-99"])),
        Some("disclip://clip-99".to_owned())
    );

    // 5. Standalone --url=<val>
    assert_eq!(
        extract_url_from_args(&args(&["discord-lab", "--url=discord://app/open"])),
        Some("discord://app/open".to_owned())
    );

    // 6. Standalone --url <val>
    assert_eq!(
        extract_url_from_args(&args(&["discord-lab", "--url", "discord://channels/1/2"])),
        Some("discord://channels/1/2".to_owned())
    );

    // 7. Direct link after --
    assert_eq!(
        extract_url_from_args(&args(&["discord-lab", "--", "discord://channels/1/2"])),
        Some("discord://channels/1/2".to_owned())
    );

    // 8. Empty and non-matches
    assert_eq!(extract_url_from_args(&[]), None);
    assert_eq!(extract_url_from_args(&args(&["discord://app/open"])), None);
    assert_eq!(
        extract_url_from_args(&args(&["discord-lab", "--flag"])),
        None
    );
    assert_eq!(
        extract_url_from_args(&args(&["discord-lab", "--url"])),
        None
    );
    assert_eq!(
        extract_url_from_args(&args(&["discord-lab", "--url="])),
        None
    );
}

#[test]
fn d16_deep_link_queue_lifecycle() {
    let mut queue = DeepLinkQueue::with_capacity(3);
    assert!(!queue.is_window_ready());
    assert_eq!(queue.len(), 0);
    assert!(queue.is_empty());

    // Pre-ready: links are queued
    let res1 = queue.open_or_queue_url("discord://-/channels/1/2");
    assert_eq!(res1, Ok(None));
    assert_eq!(queue.len(), 1);

    let res2 = queue.open_or_queue_url("discord://app/settings/nitro");
    assert_eq!(res2, Ok(None));
    assert_eq!(queue.len(), 2);

    let res3 = queue.open_or_queue_url("disclip://clip-01");
    assert_eq!(res3, Ok(None));
    assert_eq!(queue.len(), 3);

    // Capped FIFO: pushing a 4th item drops the oldest (channels/1/2)
    let res4 = queue.open_or_queue_url("disclip://clip-02");
    assert_eq!(res4, Ok(None));
    assert_eq!(queue.len(), 3);

    // Drain queued before or at ready
    let drained = queue.drain_queued();
    assert_eq!(drained.len(), 3);
    assert_eq!(drained[0].path(), "/settings/nitro");
    assert_eq!(drained[1].path(), "/clips/clip-01");
    assert_eq!(drained[2].path(), "/clips/clip-02");
    assert!(queue.is_empty());

    // Mark window ready
    queue.mark_window_ready();
    assert!(queue.is_window_ready());

    // Post-ready: links are returned immediately, not queued
    let post_res = queue.open_or_queue_url("discord://guilds/500/600");
    assert_eq!(
        post_res,
        Ok(Some(MainWindowPath {
            path: "/channels/500/600".to_owned(),
            query: None,
        }))
    );
    assert!(queue.is_empty());
}

#[test]
fn d17_main_window_path_routing_and_errors() {
    // Channel routing
    assert_eq!(
        main_window_path("discord://-/channels/1/2"),
        Ok(MainWindowPath {
            path: "/channels/1/2".to_owned(),
            query: None,
        })
    );
    assert_eq!(
        main_window_path("discord://channels/@me"),
        Ok(MainWindowPath {
            path: "/channels/@me".to_owned(),
            query: None,
        })
    );

    // Guild routing
    assert_eq!(
        main_window_path("discord://guilds/100/200"),
        Ok(MainWindowPath {
            path: "/channels/100/200".to_owned(),
            query: None,
        })
    );
    assert_eq!(
        main_window_path("discord://guilds/100"),
        Ok(MainWindowPath {
            path: "/channels/100".to_owned(),
            query: None,
        })
    );

    // Settings routing
    assert_eq!(
        main_window_path("discord://app/settings/nitro"),
        Ok(MainWindowPath {
            path: "/settings/nitro".to_owned(),
            query: None,
        })
    );
    assert_eq!(
        main_window_path("discord://settings/voice"),
        Ok(MainWindowPath {
            path: "/settings/voice".to_owned(),
            query: None,
        })
    );

    // Clip routing
    assert_eq!(
        main_window_path("disclip://clip-42"),
        Ok(MainWindowPath {
            path: "/clips/clip-42".to_owned(),
            query: None,
        })
    );
    assert_eq!(
        main_window_path("disclip:clip-42"),
        Ok(MainWindowPath {
            path: "/clips/clip-42".to_owned(),
            query: None,
        })
    );
    assert_eq!(
        main_window_path("discord://clips/clip-42"),
        Ok(MainWindowPath {
            path: "/clips/clip-42".to_owned(),
            query: None,
        })
    );

    // Query preservation
    assert_eq!(
        main_window_path("discord://-/channels/1/2?search=test"),
        Ok(MainWindowPath {
            path: "/channels/1/2".to_owned(),
            query: Some("search=test".to_owned()),
        })
    );

    // Unknown route error (typed, no panic)
    assert!(matches!(
        main_window_path("discord://random/unknown"),
        Err(MainWindowPathError::UnknownRoute(_))
    ));

    // Invalid URL error (typed, no panic)
    assert_eq!(
        main_window_path("https://example.com"),
        Err(MainWindowPathError::InvalidUrl(DeepLinkError::WrongScheme))
    );
    assert_eq!(
        main_window_path(""),
        Err(MainWindowPathError::InvalidUrl(DeepLinkError::Empty))
    );
}

#[test]
fn d18_extended_re_routes_invite_gift_user_quest_handoff_connect() {
    assert_eq!(MAX_INVITE_CODE_LEN, 64);

    // 1. Invite routes (`BVt.INVITE` = `/invite/${code}`)
    for raw in [
        "discord://invite/rust-lang",
        "discord://-/invite/rust-lang",
        "discord:///invite/rust-lang",
        "discord://invites/rust-lang",
        "discord://-/invites/rust-lang/",
    ] {
        let link = parse(raw);
        assert_eq!(
            route(&link),
            Route::Invite {
                code: "rust-lang".to_owned()
            },
            "{raw}"
        );
        assert_eq!(
            main_window_path(raw),
            Ok(MainWindowPath {
                path: "/invite/rust-lang".to_owned(),
                query: None,
            }),
            "{raw}"
        );
    }

    // Boundary: 64-char invite code is valid, 65-char is Unknown
    let code_64 = "a".repeat(MAX_INVITE_CODE_LEN);
    let url_64 = format!("discord://invite/{code_64}");
    assert_eq!(route(&parse(&url_64)), Route::Invite { code: code_64 });
    let url_65 = format!("discord://invite/{}", "a".repeat(MAX_INVITE_CODE_LEN + 1));
    assert_eq!(route(&parse(&url_65)), Route::Unknown);

    // 2. Gift code routes (`BVt.GIFT_CODE` = `/gifts/${code}`, `BVt.APP_WITH_GIFT_CODE` = `/app/gifts/${code}`)
    for raw in [
        "discord://gifts/nitro-2026_abc",
        "discord://-/gifts/nitro-2026_abc",
        "discord:///gifts/nitro-2026_abc",
        "discord://app/gifts/nitro-2026_abc",
    ] {
        let link = parse(raw);
        assert_eq!(
            route(&link),
            Route::GiftCode {
                code: "nitro-2026_abc".to_owned()
            },
            "{raw}"
        );
        assert_eq!(
            main_window_path(raw),
            Ok(MainWindowPath {
                path: "/gifts/nitro-2026_abc".to_owned(),
                query: None,
            }),
            "{raw}"
        );
    }

    // 3. User profile routes (`BVt.USERS` = `/users/${id}`)
    let max_snowflake = u64::MAX.to_string();
    let user_url = format!("discord://users/{max_snowflake}");
    assert_eq!(
        route(&parse(&user_url)),
        Route::UserProfile {
            user_id: max_snowflake.clone()
        }
    );
    assert_eq!(
        main_window_path(&user_url),
        Ok(MainWindowPath {
            path: format!("/users/{max_snowflake}"),
            query: None,
        })
    );
    assert_eq!(
        route(&parse("discord://-/users/@me")),
        Route::UserProfile {
            user_id: "@me".to_owned()
        }
    );
    assert_eq!(
        route(&parse("discord://users/0")),
        Route::Unknown,
        "Snowflake 0 недопустим для профиля пользователя"
    );
    assert_eq!(
        route(&parse("discord://users/not-a-snowflake")),
        Route::Unknown
    );

    // 4. Quests (`BVt.QUESTS` = `/quests/${id}`)
    assert_eq!(
        route(&parse("discord://quests/quest_99")),
        Route::Quest {
            quest_id: "quest_99".to_owned()
        }
    );
    assert_eq!(
        main_window_path("discord://-/quests/quest_99"),
        Ok(MainWindowPath {
            path: "/quests/quest_99".to_owned(),
            query: None,
        })
    );

    // 5. Login handoff (`BVt.LOGIN_HANDOFF` = `/login/handoff`) & Connect authorize (`/connect/authorize`)
    assert_eq!(
        route(&parse("discord://login/handoff?handoff_token=secret123")),
        Route::LoginHandoff
    );
    assert_eq!(
        main_window_path("discord://login/handoff?handoff_token=secret123"),
        Ok(MainWindowPath {
            path: "/login/handoff".to_owned(),
            query: Some("handoff_token=secret123".to_owned()),
        })
    );
    assert_eq!(
        route(&parse("discord://connect/authorize?client_id=1")),
        Route::ConnectAuthorize
    );
    assert_eq!(
        main_window_path("discord://-/connect/authorize?client_id=1"),
        Ok(MainWindowPath {
            path: "/connect/authorize".to_owned(),
            query: Some("client_id=1".to_owned()),
        })
    );
}

#[test]
fn d19_structured_channel_targets_and_snowflake_bounds() {
    assert_eq!(
        parse_channel_target("/channels/@me"),
        Some(ChannelTarget::DirectMessagesHome)
    );
    assert_eq!(
        parse_channel_target("/channels/@me/123456789012345678"),
        Some(ChannelTarget::DirectMessage {
            channel_id: 123_456_789_012_345_678,
            message_id: None,
        })
    );
    assert_eq!(
        parse_channel_target("/channels/@me/123456789012345678/987654321098765432"),
        Some(ChannelTarget::DirectMessage {
            channel_id: 123_456_789_012_345_678,
            message_id: Some(987_654_321_098_765_432),
        })
    );
    assert_eq!(
        parse_channel_target("/channels/1"),
        Some(ChannelTarget::GuildChannel {
            guild_id: 1,
            channel_id: None,
            message_id: None,
        })
    );
    assert_eq!(
        parse_channel_target(&format!("/channels/{}/{}", u64::MAX, u64::MAX - 1)),
        Some(ChannelTarget::GuildChannel {
            guild_id: u64::MAX,
            channel_id: Some(u64::MAX - 1),
            message_id: None,
        })
    );
    assert_eq!(
        parse_channel_target("/channels/100/200/300"),
        Some(ChannelTarget::GuildChannel {
            guild_id: 100,
            channel_id: Some(200),
            message_id: Some(300),
        })
    );
    assert_eq!(
        parse_channel_target("/channels/100/200/threads/300/400"),
        Some(ChannelTarget::GuildThread {
            guild_id: 100,
            channel_id: 200,
            thread_id: 300,
            message_id: Some(400),
        })
    );

    // Invalid / boundary Snowflakes (0, overflow > u64::MAX, leading plus, extra segments)
    for invalid in [
        "/channels/0",
        "/channels/100/0",
        "/channels/100/200/0",
        "/channels/@me/0",
        "/channels/18446744073709551616",
        "/channels/+100/200",
        "/channels/100/200/300/400",
        "/channels/100/200/threads/0",
        "/other/100/200",
    ] {
        assert_eq!(parse_channel_target(invalid), None, "{invalid}");
    }
}

#[test]
fn d20_oauth2_authorize_query_params_and_redirect_uri_security() {
    // Matches `Kk["/oauth2/authorize"]` in `/home/sanya/re-deep/decoded/4d2a64cb5df5fc4e_0`
    let link = parse(
        "discord://action/oauth2/authorize?client_id=123456789&scope=identify%20guilds+connections&redirect_uri=https%3A%2F%2Fexample.com%2Fcb&response_type=code&state=secret-state-99&nonce=secret-nonce-88&code_challenge=xyz_challenge&code_challenge_method=S256",
    );
    let params = parse_oauth2_authorize_params(link.query).expect("valid oauth2 params");
    assert_eq!(
        params,
        OAuth2AuthorizeParams {
            client_id: 123_456_789,
            scopes: vec![
                "identify".to_owned(),
                "guilds".to_owned(),
                "connections".to_owned()
            ],
            redirect_uri: "https://example.com/cb".to_owned(),
            response_type: "code".to_owned(),
            state: Some("secret-state-99".to_owned()),
            nonce: Some("secret-nonce-88".to_owned()),
            code_challenge: Some("xyz_challenge".to_owned()),
            code_challenge_method: Some("S256".to_owned()),
        }
    );

    // Debug must redact `state` and `nonce`
    let dbg = format!("{params:?}");
    assert!(!dbg.contains("secret-state-99"), "state leaked: {dbg}");
    assert!(!dbg.contains("secret-nonce-88"), "nonce leaked: {dbg}");
    assert!(dbg.contains("<redacted>"), "missing redaction: {dbg}");

    // Dangerous redirect_uri schemes and missing required params must fail
    assert_eq!(
        parse_oauth2_authorize_params(Some(
            "client_id=123&scope=identify&redirect_uri=javascript%3Aalert(1)&response_type=code"
        )),
        Err(OAuth2AuthorizeError::UnsafeRedirectUri)
    );
    assert_eq!(
        parse_oauth2_authorize_params(Some(
            "client_id=123&scope=identify&redirect_uri=http%3A%2F%2Fevil.example.com%2Fcb&response_type=code"
        )),
        Err(OAuth2AuthorizeError::UnsafeRedirectUri)
    );
    assert_eq!(
        parse_oauth2_authorize_params(Some(
            "client_id=0&scope=identify&redirect_uri=https%3A%2F%2Fexample.com%2Fcb&response_type=code"
        )),
        Err(OAuth2AuthorizeError::InvalidClientId)
    );
    assert_eq!(
        parse_oauth2_authorize_params(Some(
            "client_id=123&scope=&redirect_uri=https%3A%2F%2Fexample.com%2Fcb&response_type=code"
        )),
        Err(OAuth2AuthorizeError::MissingScope)
    );
    assert_eq!(
        parse_oauth2_authorize_params(None),
        Err(OAuth2AuthorizeError::MissingQuery)
    );
}

#[test]
fn d21_path_traversal_and_percent_encoded_injection_blocked() {
    for malicious in [
        "discord://-/channels/1/../../evil",
        "discord://channels/../../etc/passwd",
        "discord://-/channels/1/%2e%2e/evil",
        "discord://-/channels/1/%2E%2E/evil",
        "discord://-/channels/1/..%2fevil",
        "discord://-/channels/1\\..\\evil",
        "discord://app/settings/..",
        "discord://app/settings/.",
        "discord://app/settings/%2e%2e",
        "discord://settings/..",
        "discord:///library/../launch",
        "discord:///library/%2e%2e/launch",
        "discord://guilds/../123",
        "discord://guilds/123/..",
        "disclip://..",
        "disclip://.",
        "disclip://%2e%2e",
        "disclip:///../secret.mp4",
        "discord://clips/..",
        "discord://clips/%2e%2e",
        "discord://clips/clip%00.mp4",
        "discord://invite/..",
        "discord://gifts/..",
    ] {
        let link = parse(malicious);
        assert_eq!(
            route(&link),
            Route::Unknown,
            "path traversal должен блокироваться в route: {malicious}"
        );
        assert!(
            matches!(
                main_window_path(malicious),
                Err(MainWindowPathError::UnknownRoute(_))
            ),
            "main_window_path должен отвергать traversal: {malicious}"
        );
        assert_eq!(
            classify_webview_navigation(malicious),
            NavigationDecision::Block,
            "classify_webview_navigation должен блокировать traversal: {malicious}"
        );
    }
}

#[test]
fn d22_webview_navigation_and_external_url_security_guard() {
    // Verify BLOCKED_URL_PROTOCOLS oracle from `asar-extract/core/bundle.js` #1331
    for proto in [
        "file:",
        "javascript:",
        "vbscript:",
        "data:",
        "about:",
        "chrome:",
        "ms-cxh:",
        "ms-cxh-full:",
        "ms-word:",
        "shell:",
        "ms-msdt:",
        "search-ms:",
        "ms-officecmd:",
        "ms-appinstaller:",
    ] {
        assert!(
            BLOCKED_URL_PROTOCOLS.contains(&proto),
            "отсутствует в BLOCKED_URL_PROTOCOLS: {proto}"
        );
        let test_url = format!("{proto}//test");
        assert!(
            !should_open_external_url(&test_url),
            "should_open_external_url должен блокировать {test_url}"
        );
    }

    // 1-letter schemes (e.g. Windows drive paths `C:\Windows\System32\cmd.exe`) must be blocked
    assert!(!should_open_external_url("c:/windows/system32/cmd.exe"));
    assert!(!should_open_external_url("C:\\Windows\\System32\\cmd.exe"));
    assert!(!should_open_external_url("https://user:pass@example.com/"));
    assert!(should_open_external_url(
        "https://discord.com/acknowledgements"
    ));
    assert!(should_open_external_url("mailto:support@discord.com"));

    // Origin matching (`securityUtils.checkUrlOriginMatches`)
    assert!(check_url_origin_matches(
        "https://discord.com/channels/@me",
        "https://DISCORD.COM/app"
    ));
    assert!(check_url_origin_matches(
        "https://discord.com:443/app",
        "https://discord.com/login"
    ));
    assert!(!check_url_origin_matches(
        "http://discord.com/app",
        "https://discord.com/app"
    ));
    assert!(!check_url_origin_matches(
        "https://ptb.discord.com/app",
        "https://discord.com/app"
    ));

    // WebView navigation classification
    assert!(classify_webview_navigation("discord://-/channels/1/2").is_internal());
    assert!(classify_webview_navigation("https://discord.com/app").is_internal());
    assert!(classify_webview_navigation("https://example.com/docs").should_open_external());
    assert_eq!(
        classify_webview_navigation("http://discord.com/app"),
        NavigationDecision::Block
    );
}

#[test]
fn d23_protocol_registration_privileges_desktop_and_windows_registry() {
    // 1. `beforeReadyProtocolRegistration` (`m-app-46312.txt`)
    let schemes = before_ready_protocol_registration();
    assert_eq!(
        schemes,
        vec![ProtocolPrivilegeSpec {
            scheme: "disclip",
            standard: true,
            secure: true,
            cors_enabled: true,
            support_fetch_api: true,
            stream: true,
        }]
    );

    // 2. XDG `.desktop` protocol handler & `mimeapps.list`
    let desktop =
        desktop_protocol_handler_entry("Discord", "/usr/bin/discord-gui-spike").expect("desktop");
    assert!(desktop.contains("[Desktop Entry]\n"));
    assert!(desktop.contains("Exec=/usr/bin/discord-gui-spike --url -- %u\n"));
    assert!(desktop.contains("MimeType=x-scheme-handler/discord;x-scheme-handler/disclip;\n"));
    assert!(desktop.contains("NoDisplay=true\n"));
    assert!(desktop.contains("Terminal=false\n"));

    let mimeapps = mimeapps_scheme_entries("discord-lab.desktop").expect("mimeapps");
    assert_eq!(
        mimeapps,
        vec![
            (
                "x-scheme-handler/discord".to_owned(),
                "discord-lab.desktop".to_owned()
            ),
            (
                "x-scheme-handler/disclip".to_owned(),
                "discord-lab.desktop".to_owned()
            ),
        ]
    );

    // 3. Windows Squirrel registry commands (`m-app-27251.txt` `installProtocol`)
    let reg_cmds = windows_protocol_registry_commands(
        "Discord",
        r"C:\Users\testuser\AppData\Local\Discord\app-1.0.162\Discord.exe",
    )
    .expect("windows registry commands");
    assert_eq!(reg_cmds.len(), 4);
    assert_eq!(
        reg_cmds[0],
        vec![
            r"HKCU\Software\Classes\Discord".to_owned(),
            "/ve".to_owned(),
            "/d".to_owned(),
            "URL:Discord Protocol".to_owned(),
        ]
    );
    assert_eq!(
        reg_cmds[3],
        vec![
            r"HKCU\Software\Classes\Discord\shell\open\command".to_owned(),
            "/ve".to_owned(),
            "/d".to_owned(),
            r#""C:\Users\testuser\AppData\Local\Discord\app-1.0.162\Discord.exe" --url -- "%1""#
                .to_owned(),
        ]
    );

    // Reject control characters or quotes injection in protocol/executable path
    assert!(desktop_protocol_handler_entry("Discord\nEvil=1", "/usr/bin/discord").is_err());
    assert!(windows_protocol_registry_commands("Discord", "C:\\bad\"path\\Discord.exe").is_err());
    assert!(windows_protocol_registry_commands("bad/proto", "C:\\Discord.exe").is_err());
}

#[test]
fn d24_disclip_mp4_verification_and_range_serving_contract() {
    // Oracle: `m-core-7635.txt` (`DISCORD_UUID = "a1c8529933464db888f083f57a75a5ef"`)
    assert_eq!(DISCORD_CLIP_UUID_HEX, "a1c8529933464db888f083f57a75a5ef");
    assert_eq!(
        DISCORD_CLIP_UUID_BYTES,
        [
            0xa1, 0xc8, 0x52, 0x99, 0x33, 0x46, 0x4d, 0xb8, 0x88, 0xf0, 0x83, 0xf5, 0x7a, 0x75,
            0xa5, 0xef
        ]
    );

    // Build a valid minimal Discord MP4 buffer:
    // Box 1: [size=16 (0x00000010), b"ftyp", b"isom", 0, 0, 0, 0]
    // Box 2: [size=28 (0x0000001c), b"uuid", DISCORD_CLIP_UUID_BYTES (16B), b"META"]
    let mut valid_mp4 = Vec::new();
    valid_mp4.extend_from_slice(&16u32.to_be_bytes());
    valid_mp4.extend_from_slice(b"ftypisom\x00\x00\x00\x00");
    valid_mp4.extend_from_slice(&28u32.to_be_bytes());
    valid_mp4.extend_from_slice(b"uuid");
    valid_mp4.extend_from_slice(&DISCORD_CLIP_UUID_BYTES);
    valid_mp4.extend_from_slice(b"META");

    assert_eq!(verify_discord_mp4_bytes("clip.mp4", &valid_mp4), Ok(()));
    assert_eq!(
        verify_discord_mp4_bytes("clip.mkv", &valid_mp4),
        Err(DiscordMp4ErrorReason::NotMp4Extension)
    );
    assert_eq!(
        DiscordMp4ErrorReason::NotMp4Extension.as_re_str(),
        "not-mp4-extension"
    );

    // Corrupt signature
    let mut bad_sig = valid_mp4.clone();
    bad_sig[4..12].copy_from_slice(b"ftypmp42");
    assert_eq!(
        verify_discord_mp4_bytes("clip.mp4", &bad_sig),
        Err(DiscordMp4ErrorReason::NotMp4Signature)
    );
    assert_eq!(
        DiscordMp4ErrorReason::NotMp4Signature.as_re_str(),
        "not-mp4-signature"
    );

    // Missing Discord UUID box
    let no_uuid = valid_mp4[..16].to_vec();
    assert_eq!(
        verify_discord_mp4_bytes("clip.mp4", &no_uuid),
        Err(DiscordMp4ErrorReason::NoDiscordUuidBox)
    );
    assert_eq!(
        DiscordMp4ErrorReason::NoDiscordUuidBox.as_re_str(),
        "no-discord-uuid-box"
    );

    // Malformed box size (size = 4 < 8)
    let mut bad_box = valid_mp4[..16].to_vec();
    bad_box.extend_from_slice(&4u32.to_be_bytes());
    bad_box.extend_from_slice(b"uuid");
    assert_eq!(
        verify_discord_mp4_bytes("clip.mp4", &bad_box),
        Err(DiscordMp4ErrorReason::MalformedBox)
    );
    assert_eq!(
        DiscordMp4ErrorReason::MalformedBox.as_re_str(),
        "malformed-box"
    );

    // File + HTTP Range serving contract (`200`, `206`, `416`, `404` on traversal)
    let temp_dir = std::env::temp_dir().join(format!("dlab-clips-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).expect("create clips temp dir");
    let clip_path = temp_dir.join("highlight.mp4");
    std::fs::write(&clip_path, &valid_mp4).expect("write valid mp4");

    assert!(verify_discord_mp4_file(&temp_dir, &clip_path).is_ok());

    // 200 OK full response
    let resp_200 = serve_disclip_request(&temp_dir, "disclip://clips/highlight.mp4", None);
    assert_eq!(resp_200.status, 200);
    assert_eq!(resp_200.content_type, "video/mp4");
    assert_eq!(resp_200.accept_ranges.as_deref(), Some("bytes"));
    assert_eq!(resp_200.content_length, Some(valid_mp4.len()));
    assert_eq!(resp_200.body, valid_mp4);

    // 206 Partial Content (`Range: bytes=4-11` -> b"ftypisom")
    let resp_206 = serve_disclip_request(
        &temp_dir,
        "disclip://clips/highlight.mp4",
        Some("bytes=4-11"),
    );
    assert_eq!(resp_206.status, 206);
    assert_eq!(
        resp_206.content_range.as_deref(),
        Some(format!("bytes 4-11/{}", valid_mp4.len()).as_str())
    );
    assert_eq!(resp_206.content_length, Some(8));
    assert_eq!(resp_206.body, b"ftypisom");

    // 416 Range Not Satisfiable
    let resp_416 = serve_disclip_request(
        &temp_dir,
        "disclip://clips/highlight.mp4",
        Some("bytes=999-1000"),
    );
    assert_eq!(resp_416.status, 416);
    assert_eq!(
        resp_416.content_range.as_deref(),
        Some(format!("bytes */{}", valid_mp4.len()).as_str())
    );
    assert!(resp_416.body.is_empty());

    // 404 on path traversal attempt
    let resp_traversal =
        serve_disclip_request(&temp_dir, "disclip://clips/..%2fhighlight.mp4", None);
    assert_eq!(resp_traversal.status, 404);

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn d25_custom_protocol_handler_state_machine_and_metrics() {
    let mut handler = CustomProtocolHandler::with_capacity(4);
    assert!(!handler.is_window_ready());
    assert_eq!(handler.accepted_count(), 0);
    assert_eq!(handler.queued_count(), 0);
    assert_eq!(handler.blocked_count(), 0);

    // 1. Pre-ready valid deep links are queued
    assert_eq!(
        handler.handle_open_url("discord://invite/rust-lang"),
        Ok(None)
    );
    assert_eq!(
        handler.handle_open_url("discord://-/channels/100/200"),
        Ok(None)
    );
    assert_eq!(handler.queued_count(), 2);
    assert_eq!(handler.accepted_count(), 0);

    // 2. Malicious or invalid URL increments blocked_count
    assert!(handler.handle_open_url("javascript:alert(1)").is_err());
    assert!(
        handler
            .handle_open_url("discord://-/channels/1/../../evil")
            .is_err()
    );
    assert_eq!(handler.blocked_count(), 2);

    // 3. Mark window ready and drain queued paths
    let drained = handler.mark_window_ready_and_drain();
    assert!(handler.is_window_ready());
    assert_eq!(drained.len(), 2);
    assert_eq!(drained[0].path(), "/invite/rust-lang");
    assert_eq!(drained[1].path(), "/channels/100/200");
    assert_eq!(handler.accepted_count(), 2);

    // 4. Post-ready valid deep link is dispatched immediately
    let immediate = handler
        .handle_open_url("discord://gifts/nitro-boost")
        .expect("valid gift deep link");
    assert_eq!(
        immediate,
        Some(MainWindowPath {
            path: "/gifts/nitro-boost".to_owned(),
            query: None,
        })
    );
    assert_eq!(handler.accepted_count(), 3);
}
