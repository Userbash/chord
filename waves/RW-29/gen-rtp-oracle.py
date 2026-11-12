#!/usr/bin/env python3
"""RW-29 / P4-04 — извлечение RTP-оракула из golden-pcap (только чтение).

Источник: /home/user/clean-room-oracle/voice/voice-udp4.pcap (LINKTYPE_LINUX_SLL2, snaplen 96).
Выход: fixtures/voice/rtp-pt120-oracle.json — байтовые пакеты PT120 клиента.
Скрипт детерминирован: порядок по времени захвата, JSON с сортированными ключами.
"""
import json
import pathlib
import struct

SOURCE = pathlib.Path("/home/user/clean-room-oracle/voice/voice-udp4.pcap")
OUT = pathlib.Path("/home/user/orca/project/fixtures/voice/rtp-pt120-oracle.json")
SRC_FLOW = (60047, 19329)


def udp_payloads(data: bytes):
    off = 24
    linktype = struct.unpack("<I", data[20:24])[0]
    while off + 16 <= len(data):
        ts_s, ts_us, cap, _orig = struct.unpack("<IIII", data[off : off + 16])
        off += 16
        frame = data[off : off + cap]
        off += cap
        if linktype == 276:  # SLL2
            proto = frame[:2]
            ip = frame[20:]
        elif linktype == 113:  # SLL
            proto = frame[14:16]
            ip = frame[16:]
        else:  # Ethernet
            proto = frame[12:14]
            ip = frame[14:]
        if proto != b"\x08\x00":
            continue
        ihl = (ip[0] & 0x0F) * 4
        total = struct.unpack(">H", ip[2:4])[0]
        if ip[9] != 17:
            continue
        udp = ip[ihl:total]
        sport, dport = struct.unpack(">HH", udp[:4])
        yield ts_s, ts_us, sport, dport, udp[8:]


def parse_rtp(payload: bytes) -> dict:
    version = payload[0] >> 6
    marker = bool((payload[1] >> 7) & 1)
    pt = payload[1] & 0x7F
    seq, timestamp, ssrc = struct.unpack(">HII", payload[2:12])
    index = 12
    ext = None
    if (payload[0] >> 4) & 1:
        profile, words = struct.unpack(">HH", payload[index : index + 4])
        data = payload[index + 4 : index + 4 + 4 * words]
        index += 4 + 4 * words
        ext = {
            "profile": f"0x{profile:04x}",
            "words": words,
            "data_hex": data.hex(),
            "valid_0xbede": profile == 0xBEDE and words == 2 and len(data) == 8,
        }
    remainder = payload[index:]
    counter = None
    if ext and ext["valid_0xbede"] and len(remainder) >= 4:
        counter = struct.unpack("<I", remainder[-4:])[0]
    return {
        "version": version,
        "marker": marker,
        "payload_type": pt,
        "sequence": seq,
        "timestamp": timestamp,
        "ssrc": ssrc,
        "extension": ext,
        "remainder_len": len(remainder),
        "remainder_hex": remainder.hex(),
        "counter": counter,
        "raw_hex": payload.hex(),
    }


def main() -> int:
    data = SOURCE.read_bytes()
    packets = []
    for ts_s, ts_us, sport, dport, payload in udp_payloads(data):
        if (sport, dport) != SRC_FLOW or len(payload) < 12:
            continue
        if payload[1] & 0x7F != 120:
            continue
        record = parse_rtp(payload)
        record["captured_at"] = f"{ts_s}.{ts_us:06d}"
        record["length"] = len(payload)
        anomaly = record["extension"] is None or not record["extension"]["valid_0xbede"]
        record["anomaly"] = (
            "no-valid-extension" if anomaly else ("counter-tail" if record["counter"] is None else None)
        )
        packets.append(record)
    result = {
        "source": str(SOURCE),
        "source_note": "golden RE dump, read-only; LINKTYPE_LINUX_SLL2; snaplen 96",
        "flow": f"{SRC_FLOW[0]} -> {SRC_FLOW[1]}",
        "packets": packets,
    }
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(json.dumps(result, indent=1, sort_keys=True) + "\n", encoding="utf-8")
    valid = [p for p in packets if not p["anomaly"]]
    deltas = [b["timestamp"] - a["timestamp"] for a, b in zip(valid, valid[1:])]
    print(f"packets={len(packets)} valid={len(valid)} anomalies={len(packets) - len(valid)}")
    print(f"seq={[p['sequence'] for p in packets]}")
    print(f"ts_deltas={deltas}")
    print(f"markers={[p['marker'] for p in packets]}")
    print(f"counters={[p['counter'] for p in valid]}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
