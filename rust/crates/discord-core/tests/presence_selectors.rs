//! W18c / P2-09c: селекторы presence и member-list activities.
//!
//! Оракул — `010`, `018`, `003`, `008` и официальный presence-store
//! (`waves/RW-18/RESEARCH-P2-09.md` §2). Тесты падают до реализации селекторов —
//! compile-red.

mod support;

use discord_core::codec::etf::EtfValue;
use discord_core::state::{
    ActivityType, Event, EventPayload, MemberListEntry, MemberListOp, MemberPresence,
    PresenceScope, PresenceStatus, State, apply_frame,
};
use support::state_fixture::{frame, int, list, load, map, minimal_ready_frame, text};

fn minimal_state() -> State {
    let event = Event::from_frame(&minimal_ready_frame()).expect("minimal READY");
    let EventPayload::Ready(ready) = event.payload else {
        panic!("ожидался READY");
    };
    State::from_ready(&ready)
}

fn apply_frames(state: State, frames: &[EtfValue]) -> State {
    frames
        .iter()
        .fold(state, |state, frame| apply_frame(state, frame).0)
}

fn presence_frame(
    seq: u64,
    user_id: &str,
    guild_id: Option<&str>,
    status: &str,
    processed: i64,
    activities: Vec<EtfValue>,
) -> EtfValue {
    let mut pairs = vec![
        (
            "user",
            map(vec![("id", text(user_id)), ("username", text("user"))]),
        ),
        ("status", text(status)),
        ("processed_at_timestamp", int(processed)),
        ("client_status", map(vec![("desktop", text(status))])),
        ("activities", list(activities)),
    ];
    if let Some(guild_id) = guild_id {
        pairs.push(("guild_id", text(guild_id)));
    }
    frame("PRESENCE_UPDATE", seq, map(pairs))
}

fn activity(name: &str, activity_type: i64, created_at: i64) -> EtfValue {
    map(vec![
        ("name", text(name)),
        ("type", int(activity_type)),
        ("created_at", int(created_at)),
    ])
}

fn activity_with_details(
    name: &str,
    activity_type: i64,
    created_at: i64,
    details: &str,
) -> EtfValue {
    map(vec![
        ("name", text(name)),
        ("type", int(activity_type)),
        ("created_at", int(created_at)),
        ("details", text(details)),
    ])
}

fn presence_frame_clients(
    seq: u64,
    user_id: &str,
    guild_id: Option<&str>,
    status: &str,
    clients: Vec<(&str, &str)>,
    activities: Vec<EtfValue>,
) -> EtfValue {
    let client_status = clients
        .into_iter()
        .map(|(client, client_status)| (client, text(client_status)))
        .collect::<Vec<_>>();
    let mut pairs = vec![
        (
            "user",
            map(vec![("id", text(user_id)), ("username", text("user"))]),
        ),
        ("status", text(status)),
        ("processed_at_timestamp", int(1)),
        ("client_status", map(client_status)),
        ("activities", list(activities)),
    ];
    if let Some(guild_id) = guild_id {
        pairs.push(("guild_id", text(guild_id)));
    }
    frame("PRESENCE_UPDATE", seq, map(pairs))
}

fn presence_frame_with_hidden(
    seq: u64,
    user_id: &str,
    guild_id: Option<&str>,
    status: &str,
    processed: i64,
    activities: Vec<EtfValue>,
    hidden_activities: Vec<EtfValue>,
) -> EtfValue {
    let mut pairs = vec![
        (
            "user",
            map(vec![("id", text(user_id)), ("username", text("user"))]),
        ),
        ("status", text(status)),
        ("processed_at_timestamp", int(processed)),
        ("client_status", map(vec![("desktop", text(status))])),
        ("activities", list(activities)),
        ("hidden_activities", list(hidden_activities)),
    ];
    if let Some(guild_id) = guild_id {
        pairs.push(("guild_id", text(guild_id)));
    }
    frame("PRESENCE_UPDATE", seq, map(pairs))
}

/// s01: статусы и default offline.
#[test]
fn s01_status_and_defaults() {
    let state = apply_frames(
        minimal_state(),
        &[presence_frame(
            2,
            "100",
            Some("10"),
            "online",
            1,
            Vec::new(),
        )],
    );
    assert_eq!(state.status(100), PresenceStatus::Online);
    assert_eq!(state.status_in_guild(100, 10), PresenceStatus::Online);
    assert_eq!(state.status_in_guild(100, 20), PresenceStatus::Offline);
    assert_eq!(state.status(999), PresenceStatus::Offline);
    assert!(state.is_online(100));
    assert!(!state.is_offline(100));
    assert!(state.is_offline(999));
}

/// s02: агрегат выбирает больший processed timestamp; guild-селектор точен.
#[test]
fn s02_aggregate_uses_latest_processed() {
    let state = apply_frames(
        minimal_state(),
        &[
            presence_frame(2, "200", Some("10"), "online", 100, Vec::new()),
            presence_frame(3, "200", Some("20"), "idle", 200, Vec::new()),
        ],
    );
    let selected = state.presence(200).expect("aggregate");
    assert_eq!(selected.guild_id, Some(20));
    assert_eq!(state.status(200), PresenceStatus::Idle);
    assert_eq!(state.status_in_guild(200, 10), PresenceStatus::Online);
    assert_eq!(state.status_in_guild(200, 20), PresenceStatus::Idle);
}

/// s03: при равном processed выбирает scope с большим числом activities.
#[test]
fn s03_tie_breaks_by_activity_count() {
    let state = apply_frames(
        minimal_state(),
        &[
            presence_frame(
                2,
                "300",
                Some("10"),
                "online",
                100,
                vec![activity("one", 2, 1)],
            ),
            presence_frame(
                3,
                "300",
                Some("20"),
                "online",
                100,
                vec![activity("two-a", 2, 2), activity("two-b", 3, 3)],
            ),
        ],
    );
    assert_eq!(state.presence(300).expect("aggregate").guild_id, Some(20));
}

/// s04: offline scope не выбирается и не понижает online.
#[test]
fn s04_offline_scope_is_not_selected() {
    let state = apply_frames(
        minimal_state(),
        &[
            presence_frame(2, "400", Some("10"), "online", 100, Vec::new()),
            presence_frame(3, "400", Some("20"), "offline", 200, Vec::new()),
        ],
    );
    assert_eq!(state.presence(400).expect("aggregate").guild_id, Some(10));
    assert_eq!(state.status(400), PresenceStatus::Online);
    assert_eq!(
        state
            .presence_in_guild(400, 20)
            .expect("offline scope")
            .status,
        PresenceStatus::Offline
    );
}

/// s15: offline+hidden не понижает живой online-агрегат (oracle).
#[test]
fn s15_offline_hidden_does_not_downgrade_online() {
    let state = apply_frames(
        minimal_state(),
        &[
            presence_frame_with_hidden(
                2,
                "405",
                Some("10"),
                "online",
                100,
                vec![activity("live", 2, 1)],
                Vec::new(),
            ),
            presence_frame_with_hidden(
                3,
                "405",
                Some("20"),
                "offline",
                200,
                Vec::new(),
                vec![activity("hidden", 0, 1)],
            ),
        ],
    );
    let aggregate = state.presence(405).expect("aggregate");
    assert_eq!(aggregate.guild_id, Some(10));
    assert_eq!(state.status(405), PresenceStatus::Online);
    assert!(state.is_online(405));
    assert!(!state.is_offline(405));
    assert_eq!(state.activities(405)[0].name, "live");
    assert_eq!(
        state
            .primary_activity(405)
            .map(|activity| activity.name.as_str()),
        Some("live")
    );
    assert_eq!(state.online_user_ids(), vec![405]);
    assert_eq!(state.guild_online_count(10), 1);
    assert_eq!(state.guild_online_count(20), 0);
    let hidden = state
        .presence_in_guild(405, 20)
        .expect("offline hidden scope");
    assert_eq!(hidden.status, PresenceStatus::Offline);
    assert_eq!(hidden.hidden_activities.len(), 1);
}

/// s05: scopes, users, ids.
#[test]
fn s05_scope_and_user_selectors() {
    let state = apply_frames(
        minimal_state(),
        &[
            presence_frame(2, "500", Some("10"), "online", 100, Vec::new()),
            presence_frame(3, "500", None, "idle", 50, Vec::new()),
            presence_frame(4, "501", None, "online", 80, Vec::new()),
        ],
    );
    assert_eq!(state.presence_scope_count(500), 2);
    assert_eq!(state.presence_user_count(), 2);
    assert_eq!(state.presence_user_ids(), vec![500, 501]);
    assert!(state.has_presence(500));
    assert!(state.has_presence(501));
    assert!(!state.has_presence(999));
    let scopes = state
        .presence_scopes(500)
        .map(|presence| presence.scope())
        .collect::<Vec<_>>();
    assert!(scopes.contains(&PresenceScope::Guild(10)));
    assert!(scopes.contains(&PresenceScope::Private));
    assert!(
        state
            .presence_scope(500, PresenceScope::Guild(10))
            .is_some()
    );
    assert!(state.presence_scope(500, PresenceScope::Private).is_some());
    assert!(state.client_status_in_guild(500, 10).is_some());
    assert!(state.client_status_in_guild(500, 20).is_none());
}

/// s06: официальная сортировка activities.
#[test]
fn s06_activity_order() {
    let state = apply_frames(
        minimal_state(),
        &[presence_frame(
            2,
            "600",
            Some("10"),
            "online",
            100,
            vec![
                activity("listen-old", 2, 10),
                activity("play", 0, 10),
                activity("custom", 4, 50),
                activity("compete", 5, 10),
                activity("listen-new", 2, 20),
                activity("stream", 1, 10),
            ],
        )],
    );
    let names = state
        .activities(600)
        .iter()
        .map(|activity| activity.name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        names,
        vec![
            "custom",
            "compete",
            "stream",
            "play",
            "listen-new",
            "listen-old"
        ]
    );
}

/// s07: несколько playing сворачиваются в один самый свежий.
#[test]
fn s07_multiple_playing_collapse() {
    let state = apply_frames(
        minimal_state(),
        &[presence_frame(
            2,
            "700",
            Some("10"),
            "online",
            100,
            vec![
                activity("old-play", 0, 10),
                activity("new-play", 0, 20),
                activity("custom", 4, 30),
            ],
        )],
    );
    let names = state
        .activities(700)
        .iter()
        .map(|activity| activity.name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(names, vec!["custom", "new-play"]);
}

/// s08: primary/custom/by-type.
#[test]
fn s08_activity_selectors() {
    let state = apply_frames(
        minimal_state(),
        &[presence_frame(
            2,
            "800",
            Some("10"),
            "online",
            100,
            vec![
                activity("custom", 4, 30),
                activity("listen", 2, 10),
                activity("play", 0, 20),
            ],
        )],
    );
    assert_eq!(state.custom_status(800).expect("custom").name, "custom");
    // После custom официальный порядок даёт playing (priority 1), затем listening (0).
    assert_eq!(state.primary_activity(800).expect("primary").name, "play");
    assert_eq!(
        state
            .activity_by_type(800, ActivityType::Listening)
            .expect("listening")
            .name,
        "listen"
    );
    assert!(
        state
            .activity_by_type(800, ActivityType::Competing)
            .is_none()
    );
}

/// s09: client status, mobile/vr.
#[test]
fn s09_client_status_and_mobile_vr() {
    let state = apply_frames(minimal_state(), &[load("018-PRESENCE_UPDATE.json")]);
    let user = 702_520_807_447_855_184;
    let clients = state.client_status(user).expect("client status");
    assert_eq!(clients.get("mobile"), Some(&PresenceStatus::Online));
    assert_eq!(clients.get("vr"), Some(&PresenceStatus::Idle));
    assert!(state.is_mobile_online(user));
    assert!(!state.is_vr_online(user));
    assert!(state.client_status(999).is_none());
    assert!(!state.is_mobile_online(999));
    assert!(!state.is_vr_online(999));
}

/// s16: положительный VR и конфликт mobile/desktop.
#[test]
fn s16_vr_and_mobile_client_guards() {
    let state = apply_frames(
        minimal_state(),
        &[
            presence_frame_clients(
                2,
                "950",
                Some("10"),
                "online",
                vec![("vr", "online")],
                Vec::new(),
            ),
            presence_frame_clients(
                3,
                "951",
                Some("10"),
                "online",
                vec![("mobile", "online"), ("desktop", "online")],
                Vec::new(),
            ),
            presence_frame_clients(
                4,
                "952",
                Some("10"),
                "online",
                vec![("desktop", "online")],
                Vec::new(),
            ),
        ],
    );
    assert!(state.is_vr_online(950));
    assert!(!state.is_mobile_online(950));
    assert!(!state.is_mobile_online(951));
    assert!(!state.is_mobile_online(952));
    assert!(!state.is_vr_online(951));
    assert_eq!(
        state
            .client_status_in_guild(950, 10)
            .and_then(|statuses| statuses.get("vr")),
        Some(&PresenceStatus::Online)
    );
}

/// s17: displayable tie-break при равном приоритете и created_at.
#[test]
fn s17_displayable_tie_break() {
    let state = apply_frames(
        minimal_state(),
        &[presence_frame(
            2,
            "960",
            Some("10"),
            "online",
            1,
            vec![
                activity("plain", 2, 10),
                activity_with_details("rich", 2, 10, "details"),
            ],
        )],
    );
    let names = state
        .activities(960)
        .iter()
        .map(|activity| activity.name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(names, vec!["rich", "plain"]);
}

/// s18: is_offline — отсутствие или статус Offline, не «любой не-online».
#[test]
fn s18_offline_selector_status_semantics() {
    let state = apply_frames(
        minimal_state(),
        &[
            presence_frame(2, "970", Some("10"), "invisible", 1, Vec::new()),
            presence_frame(3, "971", Some("10"), "away", 1, Vec::new()),
        ],
    );
    assert_eq!(state.status(970), PresenceStatus::Invisible);
    assert!(!state.is_online(970));
    assert!(!state.is_offline(970));
    assert_eq!(
        state.status(971),
        PresenceStatus::Unknown("away".to_string())
    );
    assert!(!state.is_online(971));
    assert!(!state.is_offline(971));
    assert!(state.is_offline(999));
}

/// s10: online ids и guild count.
#[test]
fn s10_online_selectors() {
    let state = apply_frames(
        minimal_state(),
        &[
            presence_frame(2, "900", Some("10"), "online", 1, Vec::new()),
            presence_frame(3, "901", Some("10"), "idle", 1, Vec::new()),
            presence_frame(4, "902", Some("10"), "offline", 1, Vec::new()),
            presence_frame(5, "903", Some("20"), "dnd", 1, Vec::new()),
        ],
    );
    assert_eq!(state.online_user_ids(), vec![900, 901, 903]);
    assert_eq!(state.guild_online_count(10), 2);
    assert_eq!(state.guild_online_count(20), 1);
    assert_eq!(state.guild_online_count(30), 0);
    assert!(state.is_online(901));
    assert!(state.is_offline(902));
}

/// s11: activities по guild и агрегату.
#[test]
fn s11_activities_by_guild() {
    let state = apply_frames(
        minimal_state(),
        &[
            presence_frame(
                2,
                "1000",
                Some("10"),
                "online",
                100,
                vec![activity("guild-a", 2, 1)],
            ),
            presence_frame(
                3,
                "1000",
                Some("20"),
                "online",
                200,
                vec![activity("guild-b", 3, 2)],
            ),
        ],
    );
    assert_eq!(state.activities(1000)[0].name, "guild-b");
    assert_eq!(state.activities_in_guild(1000, 10)[0].name, "guild-a");
    assert!(state.activities_in_guild(1000, 30).is_empty());
    assert!(state.activities(999).is_empty());
}

/// s12: hidden activities и видимость offline+hidden.
#[test]
fn s12_hidden_activities() {
    let frame = frame(
        "PRESENCE_UPDATE",
        2,
        map(vec![
            (
                "user",
                map(vec![("id", text("1100")), ("username", text("user"))]),
            ),
            ("guild_id", text("10")),
            ("status", text("offline")),
            ("activities", list(vec![activity("visible", 0, 1)])),
            ("hidden_activities", list(vec![activity("hidden", 0, 2)])),
        ]),
    );
    let state = apply_frames(minimal_state(), &[frame]);
    let presence = state.presence(1100).expect("visible hidden");
    assert!(presence.is_visible());
    assert_eq!(presence.hidden_activities.len(), 1);
    assert_eq!(state.hidden_activities(1100)[0].name, "hidden");
    assert_eq!(state.status(1100), PresenceStatus::Offline);
    assert!(!state.is_online(1100));
}

/// s13: пустое состояние — без паник.
#[test]
fn s13_empty_state_selectors() {
    let state = minimal_state();
    assert!(state.presence(1).is_none());
    assert_eq!(state.status(1), PresenceStatus::Offline);
    assert!(state.activities(1).is_empty());
    assert!(state.hidden_activities(1).is_empty());
    assert!(state.online_user_ids().is_empty());
    assert_eq!(state.guild_online_count(10), 0);
    assert_eq!(state.presence_user_count(), 0);
    assert_eq!(state.presence_scope_count(1), 0);
    assert!(state.client_status(1).is_none());
    assert!(state.primary_activity(1).is_none());
    assert!(state.custom_status(1).is_none());
    assert_eq!(state.status_in_guild(1, 10), PresenceStatus::Offline);
    assert!(state.presence_scopes(1).next().is_none());
    assert!(!state.is_online(1));
    assert!(state.is_offline(1));
    assert!(!state.is_mobile_online(1));
    assert!(!state.is_vr_online(1));
}

/// s14: member-list `008` — activities и timestamps разобраны.
#[test]
fn s14_member_presence_activities() {
    let event = Event::from_frame(&load("008-GUILD_MEMBER_LIST_UPDATE.json")).expect("list");
    let EventPayload::GuildMemberListUpdate(update) = event.payload else {
        panic!("ожидался member list");
    };
    let mut found = None;
    for op in &update.ops {
        let entries: &[MemberListEntry] = match op {
            MemberListOp::Sync { items, .. } => items,
            MemberListOp::Insert { item, .. } | MemberListOp::Update { item, .. } => {
                std::slice::from_ref(item)
            }
            MemberListOp::Delete { .. } => &[],
        };
        for entry in entries {
            if let MemberListEntry::Member { user, presence, .. } = entry
                && user.id == 95_101_520_944_570_368
            {
                found = Some(presence);
            }
        }
    }
    let presence = found.expect("member presence");
    assert_eq!(presence.status.as_deref(), Some("online"));
    assert_eq!(
        presence.client_status.get("desktop").map(String::as_str),
        Some("online")
    );
    assert_eq!(presence.activities.len(), 1);
    assert_eq!(presence.activities[0].name, "Wife's Bedroom");
    assert_eq!(
        presence.activities[0]
            .timestamps
            .as_ref()
            .and_then(|timestamps| timestamps.start),
        Some(1_792_390_727_877)
    );
    assert_eq!(presence.processed_at_timestamp, Some(1_792_390_727_986));
    assert!(MemberPresence::default().activities.is_empty());
}
