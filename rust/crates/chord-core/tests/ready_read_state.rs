//! RW-12 / P2-01c — `read_state` и `user_guild_settings` (кейсы C1–C11).
//!
//! Тесты идут только через публичный `Ready::parse`/`Ready::from_dispatch`: парсеры
//! `pub(crate)` и напрямую недоступны. Оракул фикстурных чисел — jq по
//! `fixtures/gateway/by-type/002-READY.json` (команды воспроизведены в отчёте
//! `waves/RW-12/reports/w12c.md`); синтетика сверяется с контрактом A1 §2.3
//! (`waves/RW-12/DESIGN-P2-01.md`).

mod support;

use chord_core::ready::{ChannelOverride, ReadState, ReadStateEntry, Ready, UserGuildSettings};
use support::ready_fixture::{load_ready, parse_json};

/// Синтетический payload. Якоря (`guilds:[]`, `user.id`, `session_id`) обязательны для
/// композиции; C-кейсам нужен только `extra`, который начинается с запятой.
fn parse_payload(extra: &str) -> Ready {
    let text = format!(r#"{{"guilds":[],"user":{{"id":"1"}},"session_id":"s"{extra}}}"#);
    let value = parse_json(&text).expect("синтетический payload парсится");
    Ready::parse(&value).expect("синтетический payload разбирается без Err")
}

/// Реальная фикстура как op0-кадр READY.
fn fixture_ready() -> Ready {
    let frame = load_ready().expect("002-READY.json читается адаптером");
    Ready::from_dispatch(&frame).expect("READY-фикстура разбирается по контракту A1")
}

/// Диагностики C-части: фильтр не зависит от наполнения W12a/W12b.
fn c_diagnostics(ready: &Ready) -> Vec<String> {
    let diagnostics = &ready.diagnostics;
    diagnostics
        .unknown_top_level
        .iter()
        .chain(&diagnostics.malformed)
        .chain(&diagnostics.duplicates)
        .chain(&diagnostics.ignored)
        .filter(|entry| entry.contains("read_state") || entry.contains("user_guild_settings"))
        .cloned()
        .collect()
}

fn find_read_state(ready: &Ready, id: u64) -> &ReadStateEntry {
    ready
        .read_state
        .entries
        .iter()
        .find(|entry| entry.id == id)
        .unwrap_or_else(|| panic!("нет read_state-записи с id {id}"))
}

fn find_override(ready: &Ready, channel_id: u64) -> &ChannelOverride {
    ready
        .user_guild_settings
        .entries
        .iter()
        .flat_map(|entry| entry.channel_overrides.iter())
        .find(|over| over.channel_id == channel_id)
        .unwrap_or_else(|| panic!("нет channel_override с channel_id {channel_id}"))
}

/// C1: `read_state` фикстуры — счётчики и первая запись.
#[test]
fn c1_read_state_fixture_matches_oracle() {
    let ready = fixture_ready();
    let state = &ready.read_state;

    assert_eq!(state.entries.len(), 157, "jq: .d.read_state.entries|length");
    assert!(!state.partial, "jq: .d.read_state.partial == \"false\"");
    assert_eq!(state.version, 128_949, "jq: .d.read_state.version");

    assert_eq!(
        state
            .entries
            .iter()
            .filter(|entry| entry.flags.is_none())
            .count(),
        2,
        "без flags 2/157"
    );
    assert_eq!(
        state
            .entries
            .iter()
            .filter(|entry| entry.last_viewed.is_none())
            .count(),
        14,
        "без last_viewed 14/157"
    );
    assert_eq!(
        state
            .entries
            .iter()
            .filter(|entry| entry.badge_count.is_some())
            .count(),
        2,
        "badge_count 2/157"
    );
    assert_eq!(
        state
            .entries
            .iter()
            .filter(|entry| entry.last_acked_id.is_some())
            .count(),
        2,
        "last_acked_id 2/157"
    );
    assert_eq!(
        state
            .entries
            .iter()
            .filter(|entry| entry.read_state_type.is_some())
            .count(),
        2,
        "read_state_type 2/157"
    );

    // Порядок провода сохранён; первая запись захвата разобрана полностью.
    let first = &state.entries[0];
    assert_eq!(first.id, 274_093_052_673_392_641);
    assert_eq!(first.flags, Some(0));
    assert_eq!(first.last_message_id, Some(1_547_947_251_035_406_357));
    assert_eq!(
        first.last_pin_timestamp.as_deref(),
        Some("1970-01-01T00:00:00+00:00")
    );
    assert_eq!(first.mention_count, Some(0));
    assert_eq!(first.last_viewed, None);
    assert_eq!(first.badge_count, None);

    let diagnostics = c_diagnostics(&ready);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

/// C2: записи без optional-полей (флагов/последнего сообщения/пина/упоминаний).
#[test]
fn c2_entries_without_optional_fields_are_none() {
    let ready = fixture_ready();

    for id in [384_299_970_632_876_032, 1_014_537_851_418_980_372] {
        let entry = find_read_state(&ready, id);
        assert_eq!(entry.flags, None, "flags отсутствует у {id}");
        assert_eq!(
            entry.last_message_id, None,
            "last_message_id отсутствует у {id}"
        );
        assert_eq!(
            entry.last_pin_timestamp, None,
            "last_pin_timestamp отсутствует у {id}"
        );
        assert_eq!(
            entry.mention_count, None,
            "mention_count отсутствует у {id}"
        );
    }

    let user_like = find_read_state(&ready, 384_299_970_632_876_032);
    assert_eq!(user_like.badge_count, Some(16));
    assert_eq!(user_like.last_acked_id, Some(1_294_268_550_826_950_656));
    assert_eq!(user_like.read_state_type, Some(2));
    assert_eq!(user_like.last_viewed, None);

    let guild_like = find_read_state(&ready, 1_014_537_851_418_980_372);
    assert_eq!(guild_like.badge_count, Some(0));
    assert_eq!(guild_like.last_acked_id, Some(1_237_379_809_395_605_509));
    assert_eq!(guild_like.read_state_type, Some(4));

    let diagnostics = c_diagnostics(&ready);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

/// C3: дубликат id в read_state — last wins, позиция первого, запись в `duplicates`.
#[test]
fn c3_read_state_duplicate_id_last_wins() {
    let ready = parse_payload(
        r#","read_state":{"partial":"false","version":5,"entries":[
            {"id":"7","flags":1,"last_viewed":10},
            {"id":"7","flags":2,"mention_count":3}
        ]}"#,
    );

    let state = &ready.read_state;
    assert_eq!(state.entries.len(), 1, "дубликат схлопнут в одну запись");
    assert_eq!(state.entries[0].id, 7);
    // Last wins: значение последнего вхождения; позиция первого — индекс 0.
    assert_eq!(state.entries[0].flags, Some(2));
    assert_eq!(state.entries[0].mention_count, Some(3));
    assert_eq!(state.entries[0].last_viewed, None);
    assert_eq!(ready.diagnostics.duplicates, ["read_state.entries:7"]);

    let diagnostics = c_diagnostics(&ready);
    assert_eq!(diagnostics, ["read_state.entries:7"]);
}

/// C4: `user_guild_settings` фикстуры — счётчики и DM-запись с `guild_id: "nil"`.
#[test]
fn c4_user_guild_settings_fixture_matches_oracle() {
    let ready = fixture_ready();
    let settings = &ready.user_guild_settings;

    assert_eq!(
        settings.entries.len(),
        58,
        "jq: .d.user_guild_settings.entries|length"
    );
    assert!(
        !settings.partial,
        "jq: .d.user_guild_settings.partial == \"false\""
    );
    assert_eq!(settings.version, 734, "jq: .d.user_guild_settings.version");

    let overrides: usize = settings
        .entries
        .iter()
        .map(|entry| entry.channel_overrides.len())
        .sum();
    assert_eq!(overrides, 1328, "jq: сумма channel_overrides//[]");

    assert_eq!(
        settings
            .entries
            .iter()
            .filter(|entry| entry.guild_id.is_none())
            .count(),
        1,
        "guild_id \"nil\" 1/58"
    );
    assert_eq!(
        settings
            .entries
            .iter()
            .filter(|entry| entry.channel_overrides.is_empty())
            .count(),
        6,
        "channel_overrides null 6/58"
    );
    assert_eq!(
        settings
            .entries
            .iter()
            .filter(|entry| entry.mute_config.is_some())
            .count(),
        21,
        "mute_config-объект 21/58"
    );

    // DM-настройки: guild_id "nil" и mute_config "nil" — отсутствие, не ошибка.
    let dm = settings
        .entries
        .iter()
        .find(|entry| entry.guild_id.is_none())
        .expect("DM-запись с guild_id nil");
    assert_eq!(dm.version, 666);
    assert_eq!(dm.flags, 0);
    assert_eq!(dm.channel_overrides.len(), 3);
    assert_eq!(dm.mute_config, None);

    let diagnostics = c_diagnostics(&ready);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

/// C5: optional-поля ChannelOverride и MuteConfig на всей фикстуре.
#[test]
fn c5_channel_override_optionals_match_oracle() {
    let ready = fixture_ready();
    let overrides = ready
        .user_guild_settings
        .entries
        .iter()
        .flat_map(|entry| entry.channel_overrides.iter())
        .collect::<Vec<_>>();

    assert_eq!(overrides.len(), 1328);
    assert_eq!(
        overrides.iter().filter(|over| over.flags.is_none()).count(),
        75,
        "75/1328 без flags"
    );

    let mute_configs = overrides
        .iter()
        .filter_map(|over| over.mute_config.as_ref())
        .collect::<Vec<_>>();
    assert_eq!(mute_configs.len(), 38, "mute_config-объект 38/1328");
    assert_eq!(
        mute_configs
            .iter()
            .filter(|config| config.selected_time_window == Some(-1))
            .count(),
        35
    );
    assert_eq!(
        mute_configs
            .iter()
            .filter(|config| config.selected_time_window == Some(3_600))
            .count(),
        2
    );
    assert_eq!(
        mute_configs
            .iter()
            .filter(|config| config.selected_time_window == Some(86_400))
            .count(),
        1
    );
    // end_time: 3 ISO-строки, 35 атомов "nil" → None.
    assert_eq!(
        mute_configs
            .iter()
            .filter(|config| config.end_time.is_some())
            .count(),
        3
    );
    assert_eq!(
        mute_configs
            .iter()
            .filter(|config| config.end_time.is_none())
            .count(),
        35
    );

    let full = find_override(&ready, 815_478_490_379_452_467);
    assert_eq!(full.flags, None);
    assert!(!full.collapsed);
    assert_eq!(full.message_notifications, 3);
    assert!(full.muted);
    let mute_config = full.mute_config.as_ref().expect("mute_config");
    assert_eq!(
        mute_config.end_time.as_deref(),
        Some("2021-03-01T10:34:12.429000+00:00")
    );
    assert_eq!(mute_config.selected_time_window, Some(86_400));

    let nil_end = find_override(&ready, 925_220_180_173_393_961);
    let mute_config = nil_end.mute_config.as_ref().expect("mute_config");
    assert_eq!(mute_config.end_time, None, "\"nil\" → None");
    assert_eq!(mute_config.selected_time_window, Some(-1));

    let hour = find_override(&ready, 495_657_709_715_324_948);
    let mute_config = hour.mute_config.as_ref().expect("mute_config");
    assert_eq!(
        mute_config.end_time.as_deref(),
        Some("2021-02-27T17:14:08.418000+00:00")
    );
    assert_eq!(mute_config.selected_time_window, Some(3_600));
}

/// C6: дубликат UGS `guild_id`, включая `None` (DM) — last wins + `duplicates`.
#[test]
fn c6_user_guild_settings_duplicate_guild_id_last_wins() {
    let ready = parse_payload(
        r#","user_guild_settings":{"version":9,"entries":[
            {"guild_id":"1","version":1},
            {"guild_id":"nil","version":2},
            {"guild_id":"1","version":3},
            {"guild_id":"nil","version":4}
        ]}"#,
    );

    let settings = &ready.user_guild_settings;
    assert_eq!(settings.entries.len(), 2);
    assert_eq!(settings.entries[0].guild_id, Some(1));
    assert_eq!(settings.entries[0].version, 3);
    assert_eq!(settings.entries[1].guild_id, None);
    assert_eq!(settings.entries[1].version, 4);
    assert_eq!(settings.version, 9);
    assert_eq!(
        ready.diagnostics.duplicates,
        [
            "user_guild_settings.entries:1",
            "user_guild_settings.entries:nil"
        ]
    );
}

/// C7: битые типы контейнеров/версии — дефолт + `ignored`, без `Err`.
#[test]
fn c7_broken_types_fall_back_with_ignored() {
    let ready =
        parse_payload(r#","read_state":{"entries":5,"version":"x"},"user_guild_settings":5"#);

    assert!(ready.read_state.entries.is_empty());
    assert_eq!(ready.read_state.version, 0);
    assert!(!ready.read_state.partial);
    assert_eq!(ready.user_guild_settings, UserGuildSettings::default());

    let diagnostics = c_diagnostics(&ready);
    assert_eq!(
        diagnostics,
        [
            "d.read_state.entries",
            "d.read_state.version",
            "d.user_guild_settings"
        ],
        "неверный тип необязательного поля → дефолт + ignored"
    );
}

/// C8: `partial` — атомы/строки "true"/"false" и Int 0/1; прочее → false + `ignored`.
#[test]
fn c8_partial_accepts_bool_forms_and_flags_unknown() {
    let cases: [(&str, bool, bool); 6] = [
        (r#""true""#, true, false),
        ("1", true, false),
        (r#""false""#, false, false),
        ("0", false, false),
        (r#""yes""#, false, true),
        ("2", false, true),
    ];

    for (raw, expected, flagged) in cases {
        let ready = parse_payload(&format!(
            r#","read_state":{{"partial":{raw}}},"user_guild_settings":{{"partial":{raw}}}"#
        ));
        assert_eq!(
            ready.read_state.partial, expected,
            "read_state.partial для {raw}"
        );
        assert_eq!(
            ready.user_guild_settings.partial, expected,
            "user_guild_settings.partial для {raw}"
        );

        let diagnostics = c_diagnostics(&ready);
        if flagged {
            assert_eq!(
                diagnostics,
                ["d.read_state.partial", "d.user_guild_settings.partial"],
                "неизвестная форма partial ({raw}) → false + ignored"
            );
        } else {
            assert!(
                diagnostics.is_empty(),
                "лишняя диагностика для {raw}: {diagnostics:?}"
            );
        }
    }
}

/// C9: отсутствие `read_state`/UGS, пустые `entries` и `nil` → `Default`.
#[test]
fn c9_missing_or_empty_read_state_is_default() {
    let bare = parse_payload("");
    assert_eq!(bare.read_state, ReadState::default());
    assert_eq!(bare.user_guild_settings, UserGuildSettings::default());
    assert!(c_diagnostics(&bare).is_empty());

    let empty =
        parse_payload(r#","read_state":{"entries":[]},"user_guild_settings":{"entries":[]}"#);
    assert!(empty.read_state.entries.is_empty());
    assert_eq!(empty.read_state.version, 0);
    assert!(!empty.read_state.partial);
    assert!(empty.user_guild_settings.entries.is_empty());
    assert_eq!(empty.user_guild_settings.version, 0);
    assert!(!empty.user_guild_settings.partial);
    assert!(c_diagnostics(&empty).is_empty());

    // `null`/атом "nil" — та же семантика отсутствия, без диагностики.
    let nil = parse_payload(r#","read_state":null,"user_guild_settings":"nil""#);
    assert_eq!(nil.read_state, ReadState::default());
    assert_eq!(nil.user_guild_settings, UserGuildSettings::default());
    assert!(c_diagnostics(&nil).is_empty());
}

/// C10: битый id элемента (read_state и channel_override) — skip + `malformed`.
#[test]
fn c10_broken_ids_skip_elements_with_malformed() {
    let ready = parse_payload(
        r#","read_state":{"version":1,"entries":[
            {"id":"1"},{"id":"abc"},{"id":"-5"},{"id":"2"}
        ]},"user_guild_settings":{"entries":[
            {"guild_id":"1","channel_overrides":[
                {"channel_id":"5"},{"channel_id":"abc"},{"channel_id":-1}
            ]}
        ]}"#,
    );

    let state = &ready.read_state;
    assert_eq!(
        state
            .entries
            .iter()
            .map(|entry| entry.id)
            .collect::<Vec<_>>(),
        [1_u64, 2]
    );

    let settings = &ready.user_guild_settings;
    assert_eq!(settings.entries.len(), 1);
    assert_eq!(
        settings.entries[0]
            .channel_overrides
            .iter()
            .map(|over| over.channel_id)
            .collect::<Vec<_>>(),
        [5_u64]
    );

    let diagnostics = c_diagnostics(&ready);
    assert_eq!(
        diagnostics,
        [
            "read_state.entries[1]",
            "read_state.entries[2]",
            "user_guild_settings.entries[0].channel_overrides[1]",
            "user_guild_settings.entries[0].channel_overrides[2]"
        ]
    );
}

/// C11: id записи read_state может быть пользователем или гильдией — принимается как есть.
#[test]
fn c11_user_and_guild_snowflakes_are_accepted_as_read_state_ids() {
    let ready = fixture_ready();

    // Оракул jq: эти id совпадают с `d.user.id` и id гильдии ITSKIDON; P2-01 не сверяет
    // `read_state` с каналами (интерпретация — P2-07).
    let user_id = 384_299_970_632_876_032;
    let guild_id = 1_014_537_851_418_980_372;

    let user_entry = find_read_state(&ready, user_id);
    assert_eq!(user_entry.id, user_id);
    assert_eq!(user_entry.read_state_type, Some(2));

    let guild_entry = find_read_state(&ready, guild_id);
    assert_eq!(guild_entry.id, guild_id);
    assert_eq!(guild_entry.read_state_type, Some(2 + 2));
    assert_eq!(guild_entry.badge_count, Some(0));
}

/// TASK-751: всеядная десериализация last_acked_id, last_message_id, mention_count (строки, числа, null/nil/"").
#[test]
fn c12_omnivorous_last_acked_id_last_message_id_and_mention_count() {
    let ready = parse_payload(
        r#","read_state":{"version":10,"partial":false,"entries":[
            {"id":"101","last_acked_id":"5001","last_message_id":"5002","mention_count":"4","read_state_type":2,"badge_count":"3"},
            {"id":102,"last_acked_id":6001,"last_message_id":6002,"mention_count":8,"read_state_type":"2","badge_count":5},
            {"id":"103","last_acked_id":null,"last_message_id":null,"mention_count":null},
            {"id":"104","last_acked_id":"nil","last_message_id":"","mention_count":"nil"},
            {"id":"105","last_acked_id":0,"last_message_id":0,"mention_count":"0"}
        ]}"#,
    );

    let entries = &ready.read_state.entries;
    assert_eq!(entries.len(), 5);

    assert_eq!(entries[0].id, 101);
    assert_eq!(entries[0].last_acked_id, Some(5001));
    assert_eq!(entries[0].last_message_id, Some(5002));
    assert_eq!(entries[0].mention_count, Some(4));
    assert_eq!(entries[0].read_state_type, Some(2));
    assert_eq!(entries[0].badge_count, Some(3));

    assert_eq!(entries[1].id, 102);
    assert_eq!(entries[1].last_acked_id, Some(6001));
    assert_eq!(entries[1].last_message_id, Some(6002));
    assert_eq!(entries[1].mention_count, Some(8));
    assert_eq!(entries[1].read_state_type, Some(2));
    assert_eq!(entries[1].badge_count, Some(5));

    assert_eq!(entries[2].id, 103);
    assert_eq!(entries[2].last_acked_id, None);
    assert_eq!(entries[2].last_message_id, None);
    assert_eq!(entries[2].mention_count, None);

    assert_eq!(entries[3].id, 104);
    assert_eq!(entries[3].last_acked_id, None);
    assert_eq!(entries[3].last_message_id, None);
    assert_eq!(entries[3].mention_count, None);

    assert_eq!(entries[4].id, 105);
    assert_eq!(entries[4].last_acked_id, Some(0));
    assert_eq!(entries[4].last_message_id, Some(0));
    assert_eq!(entries[4].mention_count, Some(0));
    assert!(
        ready.diagnostics.ignored.is_empty(),
        "ожидалось отсутствие диагностик игнорирования, получено: {:?}",
        ready.diagnostics.ignored
    );
}
