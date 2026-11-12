#!/usr/bin/env python3
"""Мутатор RW-07: применяет ровно одну мутацию (или red-заглушку) к файлам репозитория.

Использование:
  python3 mutate.py --stub
  python3 mutate.py <имя-мутации>
Каждая замена обязана встретиться ровно один раз (кроме явно указанных count).
"""
import sys
from pathlib import Path

ROOT = Path("/home/user/orca/project/rust/crates/discord-core")
DISPATCH = ROOT / "src/dispatch.rs"
WS = ROOT / "src/ws.rs"

S_FROM_NAME = """    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|kind| kind.as_str() == name)
    }"""
S_ALL_RESUMED = "        DispatchKind::Resumed,"
S_AS_STR_READY = '            Self::Ready => "READY",'
S_OP_GUARD = """    if value.get("op").and_then(EtfValue::as_int) != Some(0) {
        return None;
    }
"""
S_SEQ = """    let seq = value
        .get("s")
        .and_then(EtfValue::as_int)
        .and_then(|raw| u64::try_from(raw).ok());"""
S_DATA = '    let data = value.get("d").cloned().unwrap_or(EtfValue::Nil);'
S_KIND = "    let kind = DispatchKind::from_name(&name);"
S_NAME_ARM = """        EtfValue::Atom(name) | EtfValue::Str(name) => name.clone(),"""
S_EVENT_TAIL = """    Some(DispatchEvent {
        kind,
        name,"""
S_PUSH_BLOCK = """                    // Роутер диспатчей (RW-07): op0 → типизированное событие; неизвестный тип —
                    // raw-событие (`kind: None`) и накопительный счётчик, не молчание.
                    if let Some(event) = route_dispatch(&value) {
                        if event.kind.is_none() {
                            self.unknown_dispatches = self.unknown_dispatches.saturating_add(1);
                        }
                        self.events.push(event);
                    }
"""
S_TAKE = """    pub fn take_events(&mut self) -> Vec<DispatchEvent> {
        std::mem::take(&mut self.events)
    }"""
S_ALL_BODY = """    pub const ALL: [DispatchKind; 21] = [
        DispatchKind::ChannelInfo,
        DispatchKind::GuildMembersChunk,
        DispatchKind::GuildMemberListUpdate,
        DispatchKind::MessageAck,
        DispatchKind::MessageCreate,
        DispatchKind::MessagePollVoteAdd,
        DispatchKind::MessageReactionAdd,
        DispatchKind::MessageUpdate,
        DispatchKind::PresenceUpdate,
        DispatchKind::Ready,
        DispatchKind::ReadySupplemental,
        DispatchKind::Resumed,
        DispatchKind::SessionsReplace,
        DispatchKind::SoundboardSounds,
        DispatchKind::ThreadListSync,
        DispatchKind::TypingStart,
        DispatchKind::UserSettingsProtoUpdate,
        DispatchKind::VoiceChannelStartTimeUpdate,
        DispatchKind::VoiceChannelStatusUpdate,
        DispatchKind::VoiceServerUpdate,
        DispatchKind::VoiceStateUpdate,
    ];"""
S_AS_STR_BODY = """    pub fn as_str(self) -> &'static str {
        match self {
            Self::ChannelInfo => "CHANNEL_INFO",
            Self::GuildMembersChunk => "GUILD_MEMBERS_CHUNK",
            Self::GuildMemberListUpdate => "GUILD_MEMBER_LIST_UPDATE",
            Self::MessageAck => "MESSAGE_ACK",
            Self::MessageCreate => "MESSAGE_CREATE",
            Self::MessagePollVoteAdd => "MESSAGE_POLL_VOTE_ADD",
            Self::MessageReactionAdd => "MESSAGE_REACTION_ADD",
            Self::MessageUpdate => "MESSAGE_UPDATE",
            Self::PresenceUpdate => "PRESENCE_UPDATE",
            Self::Ready => "READY",
            Self::ReadySupplemental => "READY_SUPPLEMENTAL",
            Self::Resumed => "RESUMED",
            Self::SessionsReplace => "SESSIONS_REPLACE",
            Self::SoundboardSounds => "SOUNDBOARD_SOUNDS",
            Self::ThreadListSync => "THREAD_LIST_SYNC",
            Self::TypingStart => "TYPING_START",
            Self::UserSettingsProtoUpdate => "USER_SETTINGS_PROTO_UPDATE",
            Self::VoiceChannelStartTimeUpdate => "VOICE_CHANNEL_START_TIME_UPDATE",
            Self::VoiceChannelStatusUpdate => "VOICE_CHANNEL_STATUS_UPDATE",
            Self::VoiceServerUpdate => "VOICE_SERVER_UPDATE",
            Self::VoiceStateUpdate => "VOICE_STATE_UPDATE",
        }
    }"""
S_ROUTE = """pub fn route_dispatch(value: &EtfValue) -> Option<DispatchEvent> {
    if value.get("op").and_then(EtfValue::as_int) != Some(0) {
        return None;
    }
    let name = match value.get("t")? {
        EtfValue::Atom(name) | EtfValue::Str(name) => name.clone(),
        _ => return None,
    };
    let kind = DispatchKind::from_name(&name);
    let seq = value
        .get("s")
        .and_then(EtfValue::as_int)
        .and_then(|raw| u64::try_from(raw).ok());
    let data = value.get("d").cloned().unwrap_or(EtfValue::Nil);
    Some(DispatchEvent {
        kind,
        name,
        seq,
        data,
    })
}"""

MUTATIONS = {
    # (a) from_name регистронезависимый
    "a_from_name_ci": (DISPATCH, [(S_FROM_NAME, """    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|kind| kind.as_str().eq_ignore_ascii_case(name))
    }""")]),
    # (b) в ALL дубль вместо варианта
    "b_all_dup": (DISPATCH, [(S_ALL_RESUMED, "        DispatchKind::Ready,", 1)]),
    # (c) опечатка в имени в as_str
    "c_as_str_typo": (DISPATCH, [(S_AS_STR_READY, '            Self::Ready => "REDY",')]),
    # (d) снята проверка op == 0
    "d_no_op_guard": (DISPATCH, [(S_OP_GUARD, "")]),
    # (e) снят u64::try_from для отрицательного s
    "e_no_try_from": (DISPATCH, [
        (S_SEQ, S_SEQ.replace(".and_then(|raw| u64::try_from(raw).ok());", ".map(|raw| raw as u64);")),
    ]),
    # (f) data всегда Nil
    "f_data_nil": (DISPATCH, [(S_DATA, "    let data = EtfValue::Nil;")]),
    # (g) неизвестный t -> None вместо raw
    "g_unknown_none": (DISPATCH, [
        (S_KIND, """    let kind = match DispatchKind::from_name(&name) {
        Some(kind) => kind,
        None => return None,
    };"""),
        (S_EVENT_TAIL, """    Some(DispatchEvent {
        kind: Some(kind),
        name,"""),
    ]),
    # (h) t принимает только Atom, не Str
    "h_atom_only": (DISPATCH, [(S_NAME_ARM, "        EtfValue::Atom(name) => name.clone(),")]),
    # (i) в пайпе не растёт unknown_dispatches
    "i_no_counter": (WS, [
        ("""                    if let Some(event) = route_dispatch(&value) {
                        if event.kind.is_none() {
                            self.unknown_dispatches = self.unknown_dispatches.saturating_add(1);
                        }
                        self.events.push(event);
                    }
""", """                    if let Some(event) = route_dispatch(&value) {
                        self.events.push(event);
                    }
"""),
    ]),
    # (j) raw-событие не пушится в events
    "j_raw_not_pushed": (WS, [
        ("""                    if let Some(event) = route_dispatch(&value) {
                        if event.kind.is_none() {
                            self.unknown_dispatches = self.unknown_dispatches.saturating_add(1);
                        }
                        self.events.push(event);
                    }
""", """                    if let Some(event) = route_dispatch(&value) {
                        if event.kind.is_none() {
                            self.unknown_dispatches = self.unknown_dispatches.saturating_add(1);
                        } else {
                            self.events.push(event);
                        }
                    }
"""),
    ]),
    # (k) take_events не дренирует (клонирует)
    "k_take_clone": (WS, [(S_TAKE, """    pub fn take_events(&mut self) -> Vec<DispatchEvent> {
        self.events.clone()
    }""")]),
    # (l) seq отсутствует -> Some(0)
    "l_seq_zero": (DISPATCH, [
        (S_SEQ, S_SEQ.replace("u64::try_from(raw).ok());", "u64::try_from(raw).ok())\n        .or(Some(0));")),
    ]),
    # --- собственные мутации ревьюера ---
    # (m) известное имя в нижнем регистре роутится (канонизация внутри route_dispatch)
    "m_lower_known_routes": (DISPATCH, [(S_KIND, "    let kind = DispatchKind::from_name(&name.to_uppercase());")]),
    # (n) отсутствие d -> None вместо Nil
    "n_data_required": (DISPATCH, [(S_DATA, "    let data = value.get(\"d\").cloned()?;")]),
    # (o) счётчик считает все события, не только raw
    "o_counter_all": (WS, [
        ("""                    if let Some(event) = route_dispatch(&value) {
                        if event.kind.is_none() {
                            self.unknown_dispatches = self.unknown_dispatches.saturating_add(1);
                        }
                        self.events.push(event);
                    }
""", """                    if let Some(event) = route_dispatch(&value) {
                        self.unknown_dispatches = self.unknown_dispatches.saturating_add(1);
                        self.events.push(event);
                    }
"""),
    ]),
    # (p) take_events сбрасывает накопительный счётчик
    "p_take_resets_counter": (WS, [(S_TAKE, """    pub fn take_events(&mut self) -> Vec<DispatchEvent> {
        self.unknown_dispatches = 0;
        std::mem::take(&mut self.events)
    }""")]),
    # (q) take_events возвращает события в обратном порядке
    "q_take_reversed": (WS, [(S_TAKE, """    pub fn take_events(&mut self) -> Vec<DispatchEvent> {
        let mut events = std::mem::take(&mut self.events);
        events.reverse();
        events
    }""")]),
    # (r) имя raw-события теряется (пустая строка)
    "r_raw_name_dropped": (DISPATCH, [
        (S_EVENT_TAIL, """    Some(DispatchEvent {
        kind,
        name: kind.map_or_else(String::new, |kind| kind.as_str().to_string()),"""),
    ]),
}

STUB_DISPATCH = (DISPATCH, [
    (S_ALL_BODY, "    pub const ALL: [DispatchKind; 0] = [];"),
    (S_FROM_NAME.replace("        Self::ALL.iter().copied().find(|kind| kind.as_str() == name)", "        let _ = name;\n        None"), None),  # no-op marker
    (S_FROM_NAME, """    pub fn from_name(name: &str) -> Option<Self> {
        let _ = name;
        None
    }"""),
    (S_AS_STR_BODY, """    pub fn as_str(self) -> &'static str {
        let _ = self;
        ""
    }"""),
    (S_ROUTE, """pub fn route_dispatch(value: &EtfValue) -> Option<DispatchEvent> {
    let _ = value;
    None
}"""),
])
STUB_WS = (WS, [(S_PUSH_BLOCK, "")])


def apply(pairs, path):
    text = path.read_text()
    for item in pairs:
        if item[1] is None:
            continue  # маркер-заглушка
        old, new = item[0], item[1]
        count = item[2] if len(item) > 2 else 1
        found = text.count(old)
        if found != count:
            raise SystemExit(f"FAIL: {path.name}: фрагмент найден {found} раз (ожидалось {count}):\n{old[:120]}")
        text = text.replace(old, new, count)
    path.write_text(text)


def main():
    if len(sys.argv) != 2:
        raise SystemExit("usage: mutate.py --stub | <mutation-name>")
    name = sys.argv[1]
    if name == "--stub":
        apply(STUB_DISPATCH[1], STUB_DISPATCH[0])
        apply(STUB_WS[1], STUB_WS[0])
        print("stub applied")
        return
    if name not in MUTATIONS:
        raise SystemExit("unknown mutation: " + name)
    path, pairs = MUTATIONS[name]
    apply(pairs, path)
    print("mutation applied:", name)


if __name__ == "__main__":
    main()
