// @ts-check
// RED (M1): реализация — src/core/report/domains.mjs. Контракт: классификация
// по доменам (паритет extract-domains.sh + domain-report.sh).
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { classifyName, domainCsv, formatDomainSummary } from "../../src/core/report/domains.mjs";

const domainIds = ["webrtc", "rtp-rtcp", "ice", "stun", "turn", "sdp", "dtls", "rtt", "jitter", "latency", "audio", "video", "camera", "network", "voice", "krisp-noise"];
/** @param {string} name */
const fixture = (name) => readFileSync(fileURLToPath(new URL(`../fixtures/domains/${name}.rows.csv`, import.meta.url)), "utf8");

/** @param {string} line */
function csvFields(line) {
  const fields = [];
  let field = "";
  let quoted = false;
  for (let i = 0; i < line.length; i++) {
    const char = line[i];
    if (char === '"' && quoted && line[i + 1] === '"') { field += '"'; i++; }
    else if (char === '"') quoted = !quoted;
    else if (char === "," && !quoted) { fields.push(field); field = ""; }
    else field += char;
  }
  fields.push(field);
  return fields;
}

describe("classifyName", () => {
  it("реальные строки каждого непустого домена классифицируются в него", () => {
    // Источники functions/domains/<domain>.csv; md5: audio 3b0c3f5c4a5eff334a6149e6470cf979,
    // camera a23341eafb82e4a1cfdc301e84b41be6, jitter 093dccb913e17ce8d716d373b7e05daf,
    // krisp-noise f1e29ffb46edad6389d10030d7bc6227, latency e7e97291b2318a8237ca78400c308ceb,
    // network 8bdf523fd1f45ffc7b8ace85bcf471ae, rtp-rtcp 72a97685ec1cf02893e6f995d138ced0,
    // rtt 8fceed4eb18f79122b940237683b4a65, sdp 43e1a63c1699cc8c99f1f559c4ce1366,
    // turn c54d4511894af7ba6ec2e8971e6a9466, video 9a99b693077bcc238af6019b1009c5b1,
    // voice fed70d4fbe4cbdfba0544bcefcff0c9b, webrtc 4e5d246173f4e9179fe935ebb19f504f.
    // Выдержки: awk -F, 'NR<=9' (заголовок + первые 8 реальных строк); jq/wc подтвердили,
    // что dtls.csv, ice.csv и stun.csv содержат только заголовок.
    for (const domain of domainIds) {
      const lines = fixture(domain).trimEnd().split("\n");
      if (lines.length === 1) continue;
      for (const line of lines.slice(1)) {
        const fields = csvFields(line);
        assert.ok(classifyName(fields[2] ?? "").includes(domain), `${domain}: ${fields[2]}`);
      }
    }
  });

  it("реальное имя getenv из таблицы символов не классифицируется", () => {
    // Источник functions/static/discord_krisp.nm.txt (md5 506fc99599552eb5a1f4a34a20bdae62),
    // c++filt ossl_safe_getenv -> ossl_safe_getenv; grep доменных таблиц не находит имя.
    assert.deepEqual(classifyName("ossl_safe_getenv"), []);
  });

  it("реальное имя из двух таблиц сохраняет документированный порядок", () => {
    // _ZN12_GLOBAL__N_113RtcpTransport7SendRtpEN6webrtc9ArrayViewIKhLln4711EEERKNS1_13PacketOptionsE
    // присутствует в webrtc.csv и rtp-rtcp.csv; деманглинг подтверждается c++filt.
    assert.deepEqual(classifyName("(anonymous namespace)::RtcpTransport::SendRtp(webrtc::ArrayView<unsigned char const, -4711l>, webrtc::PacketOptions const&)"), ["webrtc", "rtp-rtcp", "network"]);
  });
  it("аудиомодуль webrtc → webrtc + audio", () => {
    assert.deepEqual(classifyName("webrtc::AudioDeviceModule::Record"), ["webrtc", "audio"]);
  });

  it("RTP/RTT/transport попадают сразу в несколько доменов; порядок доменов фиксирован", () => {
    assert.deepEqual(classifyName("RtpTransportControllerSend::OnRttUpdate"), ["rtp-rtcp", "rtt", "network"]);
  });

  it("кодеки и камеры", () => {
    assert.deepEqual(classifyName("OpusDecInst"), ["audio"]);
    assert.deepEqual(classifyName("ScreenCastPortal::OpenPipeWire"), ["camera"]);
  });

  it("krisp-шумодав", () => {
    assert.deepEqual(classifyName("KrispModel::Init"), ["krisp-noise"]);
  });

  it("обычное имя не попадает ни в один домен", () => {
    assert.deepEqual(classifyName("HelloWorld"), []);
  });
});

describe("domainCsv", () => {
  it("заголовок; demangled в кавычках с удвоением внутренних кавычек", () => {
    /** @type {import("../../src/core/types.js").DomainRow[]} */
    const rows = [
      { module: "discord_voice", name: "_ZN1a", demangled: 'Foo::Bar<T> "x"', address: "0x10", size: "4" },
    ];
    assert.equal(
      domainCsv(rows),
      ['module,name,demangled,address,size', 'discord_voice,_ZN1a,"Foo::Bar<T> ""x""",0x10,4'].join("\n"),
    );
  });

  it("пустой список → только заголовок", () => {
    assert.equal(domainCsv([]), "module,name,demangled,address,size");
  });
});

describe("formatDomainSummary", () => {
  it("id выровнен до 12, число — вправо до 8 (%-12s %8d)", () => {
    const text = formatDomainSummary([
      ["webrtc", 11049],
      ["ice", 0],
    ]);
    const lines = text.split("\n");
    assert.equal(lines[0], `webrtc${" ".repeat(10)}11049`);
    assert.equal(lines[1], `ice${" ".repeat(17)}0`);
    assert.equal(lines[0]?.length, 21);
  });

  it("совпадает с bash printf для агрегатов REPORT.md", () => {
    // REPORT.md md5 3380d4b5ec7bce49c01df85f262d71cc; эталонный формат: printf '%-12s %8d\\n'.
    /** @type {Array<readonly [string, number]>} */
    const reportCounts = [["audio", 4176], ["camera", 161], ["dtls", 0], ["ice", 0], ["jitter", 29], ["krisp-noise", 551], ["latency", 71], ["network", 3743], ["rtp-rtcp", 1742], ["rtt", 58], ["sdp", 125], ["stun", 0], ["turn", 99], ["video", 5045], ["voice", 604], ["webrtc", 11049]];
    const expected = "audio            4176\ncamera            161\ndtls                0\nice                 0\njitter             29\nkrisp-noise       551\nlatency            71\nnetwork          3743\nrtp-rtcp         1742\nrtt                58\nsdp               125\nstun                0\nturn               99\nvideo            5045\nvoice             604\nwebrtc          11049";
    assert.equal(formatDomainSummary(reportCounts), expected);
  });
});
