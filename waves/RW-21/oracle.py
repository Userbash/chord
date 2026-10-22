#!/usr/bin/env python3
"""Independent P2-08 settings-proto oracle (stdlib only).

Reads the sanitized READY snapshot and the USER_SETTINGS_PROTO_UPDATE capture from the
read-only RE dump, decodes protobuf wire data, and prints the facts used by the Rust tests.
It does not import or execute the Rust implementation.

Usage:
  python3 waves/RW-21/oracle.py [READY.json] [msgs.jsonl]
"""
from __future__ import annotations

import base64
import glob
import hashlib
import json
import sys
from pathlib import Path

RE_DEEP = Path("/home/sanya/re-deep")
DEFAULT_READY = RE_DEEP / "gateway/by-type/002-READY.json"
DEFAULT_MSGS = RE_DEEP / "gw-proto/msgs-run2-95974_13072.jsonl"


def varint(data: bytes, offset: int) -> tuple[int, int]:
    value = 0
    shift = 0
    while True:
        if offset >= len(data):
            raise ValueError("truncated varint")
        byte = data[offset]
        offset += 1
        value |= (byte & 0x7F) << shift
        if not byte & 0x80:
            return value, offset
        shift += 7
        if shift > 70:
            raise ValueError("varint overflow")


def signed64(value: int) -> int:
    return value - (1 << 64) if value >= (1 << 63) else value


def fields(data: bytes) -> list[tuple[int, int, int | bytes]]:
    out: list[tuple[int, int, int | bytes]] = []
    offset = 0
    while offset < len(data):
        tag, offset = varint(data, offset)
        number, wire = tag >> 3, tag & 7
        if number == 0:
            raise ValueError("field 0")
        if wire == 0:
            value, offset = varint(data, offset)
        elif wire == 1:
            value = data[offset : offset + 8]
            offset += 8
        elif wire == 2:
            length, offset = varint(data, offset)
            value = data[offset : offset + length]
            offset += length
        elif wire == 5:
            value = data[offset : offset + 4]
            offset += 4
        else:
            raise ValueError(f"unsupported wire type {wire}")
        if offset > len(data):
            raise ValueError("truncated field")
        out.append((number, wire, value))
    return out


def wrapper(data: bytes, kind: str):
    fs = fields(data)
    if kind == "string":
        for number, wire, item in fs:
            if number == 1:
                if wire != 2:
                    raise ValueError("StringValue field 1 is not bytes")
                return item.decode("utf-8")
        return ""
    value = 0
    for number, wire, item in fs:
        if number == 1:
            if wire != 0:
                raise ValueError("wrapper field 1 is not varint")
            value = item
    if kind == "bool":
        return bool(value)
    if kind == "int32":
        return value - (1 << 32) if value >= (1 << 31) else value
    if kind == "int64":
        return signed64(value)
    if kind == "uint64":
        return value
    raise ValueError(kind)


def scalar(data: bytes | None, number: int, kind: str = "int32"):
    if data is None:
        return None
    for n, wire, value in fields(data):
        if n != number:
            continue
        if wire == 1 and kind in ("uint64", "fixed64"):
            return int.from_bytes(value, "little")
        if wire != 0:
            raise ValueError(f"field {number} is not varint/fixed64")
        if kind == "int32":
            return value - (1 << 32) if value >= (1 << 31) else value
        if kind in ("uint64", "fixed64"):
            return value
        return value
    return None


def message_field(data: bytes, number: int) -> bytes | None:
    for n, wire, value in fields(data):
        if n == number:
            if wire != 2:
                raise ValueError(f"field {number} is not length-delimited")
            return value
    return None


def md5(data: bytes) -> str:
    return hashlib.md5(data).hexdigest()


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def decode_ready(path: Path) -> dict:
    obj = json.loads(path.read_text())
    raw = base64.b64decode(obj["d"]["user_settings_proto"])
    top = fields(raw)

    def section(number: int) -> bytes | None:
        return message_field(raw, number)

    versions_raw = section(1)
    versions = {
        "client_version": scalar(versions_raw, 1) if versions_raw is not None else None,
        "server_version": scalar(versions_raw, 2) if versions_raw is not None else None,
        "data_version": scalar(versions_raw, 3) if versions_raw is not None else None,
    }

    appearance_raw = section(13)
    appearance = {
        "theme": scalar(appearance_raw, 1),
        "developer_mode": scalar(appearance_raw, 2),
        "timestamp_hour_cycle": scalar(appearance_raw, 9),
        "ui_density": scalar(appearance_raw, 12),
    }

    voice_raw = section(5)
    voice = {
        "always_preview_video": wrapper(message_field(voice_raw, 5), "bool")
        if message_field(voice_raw, 5) is not None
        else None,
        "afk_timeout": wrapper(message_field(voice_raw, 6), "int32")
        if message_field(voice_raw, 6) is not None
        else None,
        "stream_notifications_enabled": wrapper(
            message_field(voice_raw, 7), "bool"
        )
        if message_field(voice_raw, 7) is not None
        else None,
        "native_phone_integration_enabled": wrapper(
            message_field(voice_raw, 8), "bool"
        )
        if message_field(voice_raw, 8) is not None
        else None,
        "disable_stream_previews": wrapper(
            message_field(voice_raw, 10), "bool"
        )
        if message_field(voice_raw, 10) is not None
        else None,
    }

    text_raw = section(6)
    text_bool = {
        7: "view_image_descriptions",
        9: "inline_attachment_media",
        10: "inline_embed_media",
        11: "gif_auto_play",
        12: "render_embeds",
        13: "render_reactions",
        14: "animate_emoji",
        16: "enable_tts_command",
        17: "message_display_compact",
        20: "view_nsfw_guilds",
        21: "convert_emoticons",
        22: "expression_suggestions_enabled",
        23: "view_nsfw_commands",
    }
    text: dict[str, object] = {}
    for number, name in text_bool.items():
        value = message_field(text_raw, number)
        text[name] = wrapper(value, "bool") if value is not None else None
    text["animate_stickers"] = wrapper(message_field(text_raw, 15), "int32")
    text["explicit_content_filter"] = wrapper(message_field(text_raw, 19), "int32")
    text["dm_spam_filter_v2"] = scalar(text_raw, 27)

    notifications_raw = section(7)
    notify = message_field(notifications_raw, 2)
    notifications = {
        "notify_friends_on_go_live": wrapper(notify, "bool")
        if notify is not None
        else None,
        "notification_center_acked_before_id": scalar(notifications_raw, 3, "uint64"),
    }

    status_raw = section(11)
    status = {
        "status": wrapper(message_field(status_raw, 1), "string")
        if message_field(status_raw, 1) is not None
        else None,
        "show_current_game": wrapper(message_field(status_raw, 3), "bool")
        if message_field(status_raw, 3) is not None
        else None,
        "status_created_at_ms": wrapper(message_field(status_raw, 5), "uint64")
        if message_field(status_raw, 5) is not None
        else None,
    }

    localization_raw = section(12)
    localization = {
        "locale": wrapper(message_field(localization_raw, 1), "string")
        if message_field(localization_raw, 1) is not None
        else None,
        "timezone_offset": wrapper(message_field(localization_raw, 2), "int64")
        if message_field(localization_raw, 2) is not None
        else None,
    }

    return {
        "source": str(path),
        "base64_len": len(obj["d"]["user_settings_proto"]),
        "raw_len": len(raw),
        "sha256": sha256(raw),
        "md5": md5(raw),
        "top_level": [number for number, _, _ in top],
        "versions": versions,
        "inbox_current_tab": scalar(section(2), 1),
        "appearance": appearance,
        "voice_and_video": voice,
        "text_and_images": text,
        "notifications": notifications,
        "status": status,
        "localization": localization,
    }


def decode_update(path: Path) -> dict:
    base = None
    for line in path.read_text().splitlines():
        if "USER_SETTINGS_PROTO_UPDATE" not in line:
            continue
        obj = json.loads(line)
        settings = obj["term"]["d"]["settings"]
        base = settings["proto"]
        payload = obj["term"]["d"]
        break
    if base is None:
        raise SystemExit("USER_SETTINGS_PROTO_UPDATE not found")
    raw = base64.b64decode(base)
    top = fields(raw)
    versions_raw = message_field(raw, 1)
    return {
        "source": str(path),
        "base64_len": len(base),
        "raw_len": len(raw),
        "sha256": sha256(raw),
        "md5": md5(raw),
        "type": payload["settings"]["type"],
        "partial": payload["partial"],
        "top_level": [number for number, _, _ in top],
        "lengths": {
            number: len(value) for number, wire, value in top if wire == 2
        },
        "versions": {
            "client_version": scalar(versions_raw, 1),
            "server_version": scalar(versions_raw, 2),
            "data_version": scalar(versions_raw, 3),
        },
    }


def main() -> None:
    ready_path = Path(sys.argv[1]) if len(sys.argv) > 1 else DEFAULT_READY
    msgs_path = Path(sys.argv[2]) if len(sys.argv) > 2 else DEFAULT_MSGS
    print(json.dumps(decode_ready(ready_path), ensure_ascii=False, indent=2, sort_keys=True))
    print(
        json.dumps(
            decode_update(msgs_path), ensure_ascii=False, indent=2, sort_keys=True
        )
    )


if __name__ == "__main__":
    g = glob  # keep import for ad-hoc branching compatibility
    main()
