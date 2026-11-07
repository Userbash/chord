#!/usr/bin/env python3
"""Independent oracle check for task-291 (TR-198A).

Reimplements the reference JS behaviour taken verbatim from
/home/sanya/re-deep/decoded/3eefa7dae1e2c234_0 (module 613681) and recomputes
the expectations asserted by the Rust tests, so the tests are compared against
the shipped bundle, not against the Rust implementation.
"""
import math

PING_FINE, PING_AVG, LOSS_FINE, LOSS_AVG = 250, 500, 5.0, 10.0
MAX_LEN, AVG_WINDOW, TIMEOUT_MS = 199, 20, 4000

def js_quality(connected, last_ping, loss):
    # get quality(){ let e=this.getLastPing(); return state!==RTC_CONNECTED||void 0===e
    #   ? UNKNOWN : e>500||(loss!=null&&loss>10) ? BAD : e>250||(loss!=null&&loss>5) ? AVERAGE : FINE }
    if not connected or last_ping is None:
        return "unknown"
    if last_ping > 500 or (loss is not None and loss > 10):
        return "bad"
    if last_ping > 250 or (loss is not None and loss > 5):
        return "average"
    return "fine"

def js_avg_ping(values):
    # getAveragePing(){ let e=this._pings.slice(0,Math.min(this._pings.length,20));
    #   return 0===e.length||null==this._socket?0:e.reduce((e,t)=>e+t.value,0)/e.length }
    if not values:
        return 0
    head = values[:min(len(values), AVG_WINDOW)]
    return sum(head) / len(head)

def js_push_history(values, new):
    # _handlePing(e){ this._pings.push({time:Date.now(),value:e}); while(this._pings.length>=200) this._pings.shift(); e>500&&this._pingBadCount++ }
    out = values + [new]
    while len(out) >= 200:
        out.pop(0)
    return out, new > 500

def js_timeout_fires(seqs, seq):
    # _handlePingTimeout(e,t){ this._pingTimeouts.push(e); let n=...; n>=3 && s[n-1]===s[n-2]+1 && s[n-2]===s[n-3]+1 && this._handlePing(t) }
    s = seqs + [seq]
    n = len(s)
    return s, (n >= 3 and s[n-1] == s[n-2] + 1 and s[n-2] == s[n-3] + 1)

def js_loss_rate(prev_sent, prev_lost, curr_sent, curr_lost):
    # mod 206959: deltaLost/(deltaSent+deltaLost)*100, 0 if deltaSent==0, clamp 0..1
    d_sent = curr_sent - prev_sent
    d_lost = curr_lost - prev_lost
    if d_sent == 0:
        return 0.0
    return min(max(d_lost / (d_sent + d_lost), 0.0), 1.0) * 100.0

def js_short_hostname(host):
    # getShortHostname: e=>null==e?"":e.split(".")[0]
    return "" if host is None else host.split(".")[0]

ok = True
def check(name, got, want):
    global ok
    good = got == want or (isinstance(got, float) and isinstance(want, float) and abs(got - want) < 1e-9)
    ok = ok and good
    print(f"[{'OK ' if good else 'BAD'}] {name}: oracle={got!r} test/impl={want!r}")

print("== ConnectionQuality::evaluate vs get quality (mod 613681 @ 2469479) ==")
cases = [
    ((False, 10, 0.0), "unknown"), ((True, None, 0.0), "unknown"),
    ((True, 45, 0.0), "fine"), ((True, 250, 5.0), "fine"), ((True, 45, None), "fine"),
    ((True, 251, 0.0), "average"), ((True, 500, 0.0), "average"),
    ((True, 80, 5.0001), "average"), ((True, 80, 10.0), "average"),
    ((True, 501, 0.0), "bad"), ((True, 40, 10.0001), "bad"),
    ((True, 100, 100.0), "bad"),
]
for (args, want) in cases:
    check(f"evaluate{args}", js_quality(*args), want)

print("\n== PingHistory::push_ping vs _handlePing (@ 2504200) ==")
buf = []
for v in range(1, 201):
    buf, bad = js_push_history(buf, v)
check("len after 200 pushes", len(buf), MAX_LEN)
check("oldest retained", buf[0], 2)
check("newest retained", buf[-1], 200)
check("ping_bad_count 250", js_push_history([], 250)[1], False)
check("ping_bad_count 500", js_push_history([], 500)[1], False)
check("ping_bad_count 501", js_push_history([], 501)[1], True)

print("\n== PingHistory::average_ping_ms vs getAveragePing (@ 2474810) ==")
check("empty", js_avg_ping([]), 0)
check("single 123", js_avg_ping([123]), 123)
check("first 20 of 1..=25 (raw)", js_avg_ping(list(range(1, 26))), 10.5)
print("[NOTE] reference returns the raw mean 10.5; the contract types it as u64 and rounds half away "
      "from zero (Rust f64::round / JS Math.round) -> 11, which is what the reference UI shows via "
      "toFixed(0) and what analytics sends via Math.round(getAveragePing())")

print("\n== PingHistory::note_ping_timeout vs _handlePingTimeout (@ 2504320) ==")
seqs = []
res = []
for s in (1, 2, 4, 5, 6):
    seqs, fired = js_timeout_fires(seqs, s)
    res.append((s, fired))
check("firing points for 1,2,4,5,6", [s for s, f in res if f], [6])
seqs = []
res = []
for s in (1, 2, 4):
    seqs, fired = js_timeout_fires(seqs, s)
    res.append(fired)
check("non-consecutive 1,2,4 never fires", any(res), False)
check("timeout sample value", TIMEOUT_MS, 4000)

print("\n== compute_outbound_loss_rate_pct vs handleStats (mod 206959 @ 10812800) ==")
check("(1000,0)->(1090,10)", js_loss_rate(1000, 0, 1090, 10), 10.0)
check("(0,0)->(99,1)", js_loss_rate(0, 0, 99, 1), 1.0)
check("deltaSent==0", js_loss_rate(1000, 0, 1000, 0), 0.0)

print("\n== short_hostname vs getShortHostname (@ 7459580) ==")
check("finland10192.discord.media:443", js_short_hostname("finland10192.discord.media:443"), "finland10192")
check("plainhost", js_short_hostname("plainhost"), "plainhost")
check("empty", js_short_hostname(""), "")
print("[DIFF] oracle is hostname.split('.')[0] with no trim and no ':' handling: "
      "'host:443' -> 'host:443', '  a.b  ' -> '  a'. Contract (TASK.md L46) requires trim + '.'/':' cut, "
      "so the implementation follows the contract; the divergence is spec-level and only reachable with "
      "inputs the reference never passes (it cuts the port away in endpoint parsing).")
check("host:443 (oracle)", js_short_hostname("host:443"), "host:443")

print("\nRESULT:", "ALL CHECKS MATCH" if ok else "MISMATCH")
raise SystemExit(0 if ok else 1)
