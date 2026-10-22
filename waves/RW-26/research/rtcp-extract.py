#!/usr/bin/env python3
"""RTCP/RTP oracle extractor for B07/RW-26 (P4-07 statistics).

Read-only over /home/sanya/re-deep/voice/*.pcap (classic pcap, DLT_LINUX_SLL2=276).
No third-party deps: stdlib only.

Usage:
  python3 rtcp-extract.py <pcap> [out.json] [--fixture N]

Outputs a JSON document with every RTCP compound packet, every report block,
per-SSRC RTP reception stats, and RTT estimates derived from LSR/DLSR against
locally sent Sender Reports (full NTP timestamp -> pcap arrival time map).
"""
import json
import struct
import sys


def u32(buf, off, endian):
    return struct.unpack_from(endian + "I", buf, off)[0]


def read_pcap(path):
    buf = open(path, "rb").read()
    magic = buf[:4]
    if magic == b"\xd4\xc3\xb2\xa1":
        endian, nano = "<", False
    elif magic == b"\xa1\xb2\xc3\xd4":
        endian, nano = ">", False
    elif magic == b"\x4d\x3c\xb2\xa1":
        endian, nano = "<", True
    elif magic == b"\xa1\xb2\x3c\x4d":
        endian, nano = ">", True
    else:
        raise SystemExit("not a classic pcap: %r" % magic)
    linktype = u32(buf, 20, endian)
    off = 24
    while off + 16 <= len(buf):
        ts_sec = u32(buf, off, endian)
        ts_frac = u32(buf, off + 4, endian)
        incl = u32(buf, off + 8, endian)
        off += 16
        if off + incl > len(buf):
            break
        pkt = buf[off:off + incl]
        off += incl
        ts_ms = ts_sec * 1000 + (ts_frac / 1e6 if nano else ts_frac / 1e3)
        yield ts_ms, pkt, linktype


def udp_payload(pkt, linktype):
    l3 = {1: 14, 113: 16, 276: 20}.get(linktype, 4)
    if len(pkt) < l3 + 20 or (pkt[l3] >> 4) != 4 or pkt[l3 + 9] != 17:
        return None
    ihl = (pkt[l3] & 0x0F) * 4
    ip = pkt[l3:l3 + ihl]
    src = ".".join(str(b) for b in ip[12:16])
    dst = ".".join(str(b) for b in ip[16:20])
    u = l3 + ihl
    sport, dport = struct.unpack_from(">HH", pkt, u)
    ulen = struct.unpack_from(">H", pkt, u + 4)[0]
    return {
        "src": src, "dst": dst, "sport": sport, "dport": dport,
        "payload": pkt[u + 8:u + min(ulen, len(pkt) - u)],
    }


def parse_blocks(data, count):
    blocks = []
    for i in range(count):
        o = i * 24
        if o + 24 > len(data):
            raise ValueError("truncated report block")
        cum = int.from_bytes(data[o + 5:o + 8], "big", signed=True)
        blocks.append({
            "ssrc": struct.unpack_from(">I", data, o)[0],
            "fraction_lost_raw": data[o + 4],
            "fraction_lost": data[o + 4] / 256.0,
            "cumulative_lost": cum,
            "extended_highest_seq": struct.unpack_from(">I", data, o + 8)[0],
            "jitter": struct.unpack_from(">I", data, o + 12)[0],
            "lsr": struct.unpack_from(">I", data, o + 16)[0],
            "dlsr": struct.unpack_from(">I", data, o + 20)[0],
        })
    return blocks


def parse_rtcp(payload):
    """Parse the RTCP part of a packet and stop at the SRTCP trailer.

    Discord voice captures show ``[clear RTCP][auth tag 16][counter 4 optional]``:
    the clear prefix is self-describing through the RTCP length fields, so walk
    packets while the next 4 bytes still look like RTCP (version 2) and stop at
    the first non-RTCP byte. Returns ``(packets, consumed)``.
    """
    out = []
    o = 0
    while o + 4 <= len(payload) and (payload[o] >> 6) == 2:
        b0, pt, length = payload[o], payload[o + 1], struct.unpack_from(">H", payload, o + 2)[0]
        total = (length + 1) * 4
        if o + total > len(payload):
            raise ValueError("rtcp length %d exceeds payload" % total)
        body = payload[o + 4:o + total]
        count = b0 & 0x1F
        rec = {
            "offset": o, "version": b0 >> 6, "padding": (b0 >> 5) & 1,
            "report_count": count, "payload_type": pt, "length_words": length,
            "raw_len": total,
        }
        if pt == 200 and len(body) >= 24:
            rec.update({
                "kind": "SR",
                "sender_ssrc": struct.unpack_from(">I", body, 0)[0],
                "ntp_msw": struct.unpack_from(">I", body, 4)[0],
                "ntp_lsw": struct.unpack_from(">I", body, 8)[0],
                "ntp_timestamp": (struct.unpack_from(">I", body, 4)[0] << 32)
                | struct.unpack_from(">I", body, 8)[0],
                "rtp_timestamp": struct.unpack_from(">I", body, 12)[0],
                "packet_count": struct.unpack_from(">I", body, 16)[0],
                "octet_count": struct.unpack_from(">I", body, 20)[0],
                "reports": parse_blocks(body[24:], count),
            })
        elif pt == 201:
            rec.update({
                "kind": "RR",
                "sender_ssrc": struct.unpack_from(">I", body, 0)[0],
                "reports": parse_blocks(body[4:], count),
            })
        elif pt == 202:
            rec.update({"kind": "SDES", "sender_ssrc": struct.unpack_from(">I", body, 0)[0] if len(body) >= 4 else 0})
        elif pt == 203:
            rec.update({"kind": "BYE", "ssrcs": [
                struct.unpack_from(">I", body, i)[0] for i in range(0, min(len(body), count * 4), 4)
            ]})
        elif pt == 204:
            rec.update({
                "kind": "APP",
                "sender_ssrc": struct.unpack_from(">I", body, 0)[0] if len(body) >= 4 else 0,
                "name": body[4:8].decode("latin-1", "replace") if len(body) >= 8 else "",
            })
        else:
            rec.update({"kind": "UNKNOWN"})
        out.append(rec)
        o += total
    return out, o


def rtp_header(payload):
    if len(payload) < 12 or (payload[0] >> 6) != 2:
        return None
    cc = payload[0] & 0x0F
    ext = (payload[0] >> 4) & 1
    hdr = 12 + cc * 4
    if ext and len(payload) >= hdr + 4:
        hdr += 4 + struct.unpack_from(">H", payload, hdr + 2)[0] * 4
    if len(payload) < hdr:
        return None
    return {
        "pt": payload[1] & 0x7F,
        "marker": payload[1] >> 7,
        "seq": struct.unpack_from(">H", payload, 2)[0],
        "ts": struct.unpack_from(">I", payload, 4)[0],
        "ssrc": struct.unpack_from(">I", payload, 8)[0],
        "hdr_len": hdr,
        "payload_len": len(payload) - hdr,
    }


def main():
    path = sys.argv[1]
    out_path = sys.argv[2] if len(sys.argv) > 2 else path.rsplit(".", 1)[0] + ".rtcp.json"
    fixture_n = 20
    if "--fixture" in sys.argv:
        fixture_n = int(sys.argv[sys.argv.index("--fixture") + 1])

    flows = {}
    sent_sr = []          # (ntp_middle32, arrival_ms, full_ntp, flow)
    rtp = {}              # (src,sport,dst,dport,ssrc) -> seq/time list
    for ts_ms, pkt, linktype in read_pcap(path):
        u = udp_payload(pkt, linktype)
        if not u:
            continue
        p = u["payload"]
        key = "%s:%d -> %s:%d" % (u["src"], u["sport"], u["dst"], u["dport"])
        flow = flows.setdefault(key, {"flow": key, "rtcp": [], "rtp_packets": 0})
        if len(p) >= 8 and (p[0] >> 6) == 2 and 200 <= p[1] <= 206:
            try:
                packets, consumed = parse_rtcp(p)
                if consumed == 0:
                    raise ValueError("no RTCP packet in payload")
            except ValueError as exc:
                flow.setdefault("errors", []).append({"at_ms": ts_ms, "error": str(exc)})
                continue
            flow["rtcp"].append({
                "at_ms": ts_ms, "len": len(p), "consumed": consumed,
                "trailer_len": len(p) - consumed, "packets": packets,
            })
            for rec in packets:
                if rec.get("kind") == "SR":
                    middle = rec["ntp_msw"]
                    sent_sr.append((middle, ts_ms, rec["ntp_timestamp"], key))
                    flow.setdefault("sent_sr", []).append({
                        "at_ms": ts_ms, "ntp_timestamp": rec["ntp_timestamp"],
                        "middle32": middle, "rtp_timestamp": rec.get("rtp_timestamp"),
                        "packet_count": rec.get("packet_count"),
                        "octet_count": rec.get("octet_count"),
                    })
        else:
            h = rtp_header(p)
            if h:
                flow["rtp_packets"] += 1
                rtp.setdefault((key, h["ssrc"]), []).append((ts_ms, h["seq"], h["ts"], h["payload_len"]))

    # RTT from RR blocks: use locally sent SR with matching LSR.
    sr_by_middle = {}
    for middle, at_ms, ntp, key in sent_sr:
        sr_by_middle.setdefault(middle, (at_ms, ntp, key))

    for key, flow in flows.items():
        for compound in flow["rtcp"]:
            for rec in compound["packets"]:
                if rec.get("kind") not in ("SR", "RR"):
                    continue
                for block in rec.get("reports", []):
                    lsr, dlsr = block["lsr"], block["dlsr"]
                    if lsr and lsr in sr_by_middle:
                        sr_ms, _, sr_flow = sr_by_middle[lsr]
                        # LSR/DLSR are in 1/65536 s; arrival and sr_ms are ms.
                        rtt_ms = (compound["at_ms"] - sr_ms) - dlsr / 65.536
                        block["rtt_ms"] = round(rtt_ms, 3)
                        block["lsr_matched_flow"] = sr_flow

    # Independent RTP reception stats per stream.
    streams = []
    for (flow_key, ssrc), rows in sorted(rtp.items()):
        seqs = [r[1] for r in rows]
        expected = len(seqs)
        highest = max(seqs)
        lowest = min(seqs)
        span = (highest - lowest) & 0xFFFF
        received = len(set(seqs))
        lost = max(0, span + 1 - received) if expected else 0
        arrivals = [r[0] for r in rows]
        gaps = [b - a for a, b in zip(arrivals, arrivals[1:])]
        # RFC 3550 jitter uses RTP timestamps and a 48 kHz clock for Opus.
        jitter_units = 0.0
        prev_ts = prev_arrival = None
        for ts_ms, seq, ts, _plen in rows:
            if prev_ts is not None:
                d = (ts - prev_ts) - (ts_ms - prev_arrival) * 48.0
                jitter_units += (abs(d) - jitter_units) / 16.0
            prev_ts, prev_arrival = ts, ts_ms
        streams.append({
            "flow": flow_key, "ssrc": ssrc, "packets": expected,
            "first_seq": seqs[0], "last_seq": seqs[-1], "seq_span": span,
            "unique_seqs": received, "inferred_lost": lost,
            "first_at_ms": round(arrivals[0], 3), "last_at_ms": round(arrivals[-1], 3),
            "gap_ms_p50": round(sorted(gaps)[len(gaps) // 2], 3) if gaps else None,
            "gap_ms_max": round(max(gaps), 3) if gaps else None,
            "jitter_rfc3550_units": round(jitter_units, 3),
            "jitter_ms": round(jitter_units / 48.0, 3),
        })

    doc = {
        "pcap": path,
        "flows": [flows[k] for k in sorted(flows)],
        "streams": streams,
        "sent_sr": [{"middle32": m, "at_ms": a, "ntp_timestamp": n, "flow": k} for m, a, n, k in sent_sr],
    }
    with open(out_path, "w") as fh:
        json.dump(doc, fh, indent=1, sort_keys=True)
    print("wrote", out_path, "flows", len(doc["flows"]), "streams", len(streams), "sent_sr", len(sent_sr))

    # Fixture: first N server->client RTCP payloads that carry report blocks,
    # stored with a 4-byte big-endian length prefix so a Rust test can split them.
    if "--fixture" in sys.argv:
        server_flow = None
        for key in sorted(flows):
            flow = flows[key]
            if any(
                r.get("reports")
                for c in flow["rtcp"]
                for r in c["packets"]
            ) and flows[key]["rtcp"]:
                server_flow = flow
                break
        if not server_flow:
            raise SystemExit("no RTCP flow with report blocks")
        picked = [
            c for c in server_flow["rtcp"]
            if any(r.get("reports") for r in c["packets"])
        ][:fixture_n]
        base = path.rsplit("/", 1)[-1].split(".")[0]
        out_dir = out_path.rsplit("/", 1)[0] if "/" in out_path else "."
        fixture_dir = out_dir + "/fixtures"
        os.makedirs(fixture_dir, exist_ok=True)
        raw_path = fixture_dir + "/rtcp-" + base + "-server.bin"
        expected_path = fixture_dir + "/rtcp-" + base + "-server.json"
        wanted = {c["at_ms"] for c in picked}
        with open(raw_path, "wb") as fh:
            for ts_ms, pkt, linktype in read_pcap(path):
                u = udp_payload(pkt, linktype)
                if not u:
                    continue
                key = "%s:%d -> %s:%d" % (u["src"], u["sport"], u["dst"], u["dport"])
                p = u["payload"]
                if key != server_flow["flow"] or ts_ms not in wanted:
                    continue
                if len(p) >= 8 and (p[0] >> 6) == 2 and 200 <= p[1] <= 206:
                    fh.write(struct.pack(">I", len(p)) + p)
        expected = {
            "pcap": path,
            "flow": server_flow["flow"],
            "packets": [
                {"at_ms": round(c["at_ms"], 3), "len": c["len"], "packets": c["packets"]}
                for c in picked
            ],
        }
        with open(expected_path, "w") as fh:
            json.dump(expected, fh, indent=1, sort_keys=True)
        print("wrote", raw_path, expected_path, "packets", len(picked))


if __name__ == "__main__":
    import os
    main()
