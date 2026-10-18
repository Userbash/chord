// @ts-check
// RED (M1): реализация — src/core/calls.mjs. Контракт: записи вызовов и сводка
// (паритет scripts/logger.mjs hook + scripts/summarize-calls.sh).
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { callLine, isCallPayload, parseCallLog, summarizeCallLines } from "../../src/core/calls.mjs";

/** @typedef {import("../../src/core/types.js").CallRecord} CallRecord */

const callsFixture = fileURLToPath(new URL("../fixtures/logs/calls.sample.jsonl", import.meta.url));
const voiceFixture = fileURLToPath(new URL("../fixtures/logs/calls-voice.sample.jsonl", import.meta.url));

/**
 * @param {{ module: string, name: string, address: string } & Partial<CallRecord>} partial
 * @returns {CallRecord}
 */
const call = (partial) => ({ type: "call", threadId: 1, at: 0, args: [], ...partial });

describe("isCallPayload", () => {
  it("истина только для объекта с type === 'call'", () => {
    assert.equal(isCallPayload({ type: "call", module: "m", name: "n", address: "a" }), true);
    assert.equal(isCallPayload({ type: "error", message: "x" }), false);
    assert.equal(isCallPayload(null), false);
    assert.equal(isCallPayload("call"), false);
  });
});

describe("callLine", () => {
  it("JSON + перевод строки для вызова", () => {
    const payload = call({ module: "m", name: "n", address: "a" });
    assert.equal(callLine(payload), `${JSON.stringify(payload)}\n`);
  });

  it("не-вызов → null (строка не пишется)", () => {
    assert.equal(callLine({ type: "error" }), null);
  });
});

describe("parseCallLog", () => {
  it("разбирает фикстуру; пустые, битые строки и не-вызовы пропускаются", async () => {
    const text = `${await readFile(callsFixture, "utf8")}\nне json\n\n{"type":"error"}\n`;
    const records = parseCallLog(text);
    assert.equal(records.length, 6);
    assert.equal(records[0]?.name, "Task::Run");
    assert.equal(records[5]?.module, "discord_voice.node");
  });

  it("golden: разбирает выдержку voice-лога по независимому оракулу", async () => {
    // Провенанс: tests/fixtures/logs/calls-voice.sample.jsonl, md5=b22e970909d7bdb2785ea107ddd70144.
    // Оракул: wc -l и jq -r '[.module,.name,.address] | @tsv' | sort | uniq | wc -l => 30 записей, 6 троек.
    const records = parseCallLog(await readFile(voiceFixture, "utf8"));
    assert.equal(records.length, 30);
    assert.equal(new Set(records.map(({ module, name, address }) => `${module}\t${name}\t${address}`)).size, 6);
  });
});

describe("summarizeCallLines", () => {
  it("группирует по (module,name,address), сортирует и экранирует как summarize-calls.sh", async () => {
    const records = parseCallLog(await readFile(callsFixture, "utf8"));
    const summary = summarizeCallLines(records);
    assert.equal(summary.total, 6);
    assert.equal(summary.unique, 3);
    assert.equal(
      summary.csv,
      [
        "calls,module,name,address",
        "2,discord_utils.node,PollMetrics,0x2a10",
        "3,discord_utils.node,Task::Run,0x1f00",
        "1,discord_voice.node,RtpTransport::SendRtp,0x3300",
      ].join("\n"),
    );
  });

  it("удваивает кавычки в полях; внешних кавычек нет", () => {
    const records = [
      call({ module: "m", name: 'He said "hi"', address: "0x1" }),
      call({ module: "m", name: 'He said "hi"', address: "0x1" }),
    ];
    const summary = summarizeCallLines(records);
    assert.equal(summary.total, 2);
    assert.equal(summary.unique, 1);
    assert.equal(summary.csv, 'calls,module,name,address\n2,m,He said ""hi"",0x1');
  });

  it("пустой список → только заголовок", () => {
    assert.deepEqual(summarizeCallLines([]), {
      total: 0,
      unique: 0,
      csv: "calls,module,name,address",
    });
  });

  it("golden: voice-выдержка даёт полный CSV в байтовом порядке shell-оракула", async () => {
    // Провенанс: tests/fixtures/logs/calls-voice.sample.jsonl, md5=b22e970909d7bdb2785ea107ddd70144.
    // Оракул: jq -r '[.module,.name,.address] | @tsv' | sort | uniq -c | sed -E 's/^ *([0-9]+) /\\1\\t/' | awk CSV => total=30, unique=6.
    const records = parseCallLog(await readFile(voiceFixture, "utf8"));
    const summary = summarizeCallLines(records);
    assert.equal(summary.total, 30);
    assert.equal(summary.unique, 6);
    assert.equal(summary.csv, [
      "calls,module,name,address",
      "5,discord_voice.node,WebRtcOpus_Decode,0x7df88e2bc110",
      "5,discord_voice.node,_ZN4absl22internal_any_invocable12LocalInvokerILb0EvOZN6webrtc18ModuleRtpRtcpImpl239ScheduleMaybeSendRtcpAtOrAfterTimestampENS2_9TimestampENS2_9TimeDeltaEE3$_0JEEET0_PNS0_15TypeErasedStateEDpNS0_18ForwardedParameterIT2_E4typeE,0x7df88e3ac3e0",
      "5,discord_voice.node,_ZN6webrtc9NetEqImpl12InsertPacketERKNS_9RTPHeaderENS_9ArrayViewIKhLln4711EEERKNS_13RtpPacketInfoE,0x7df88e405530",
      "5,discord_voice.node,_ZN7discord5media19SocketClientAdapter8SendRtcpEN6webrtc9ArrayViewIKhLln4711EEERKNS2_13PacketOptionsE,0x7df88dffcc50",
      "5,discord_voice.node,_ZNK6webrtc9NetEqImpl22FilteredCurrentDelayMsEv,0x7df88e407410",
      "5,discord_voice.node,_ZZN6webrtc9NetEqImpl12InsertPacketERKNS_9RTPHeaderENS_9ArrayViewIKhLln4711EEERKNS_13RtpPacketInfoEENK3$_2clERNS_12AudioDecoder11ParseResultE,0x7df88e406220",
    ].join("\n"));
  });

  it("стресс: 50 000 записей одной функции и 5 000 нескольких функций", () => {
    // Провенанс: объём 50 000/5 000 выбран по требованию W1a; независимый оракул — Array.from + Set.
    const one = Array.from({ length: 50_000 }, () => call({ module: "stress.node", name: "Hot::Call", address: "0x1" }));
    const oneSummary = summarizeCallLines(one);
    assert.equal(oneSummary.total, 50_000);
    assert.equal(oneSummary.unique, 1);
    assert.equal(oneSummary.csv, "calls,module,name,address\n50000,stress.node,Hot::Call,0x1");

    const many = Array.from({ length: 5_000 }, (_, i) => call({
      module: "stress.node",
      name: `Fn${String(i % 17).padStart(2, "0")}`,
      address: `0x${(i % 17).toString(16)}`,
    }));
    const manySummary = summarizeCallLines(many);
    assert.equal(manySummary.total, 5_000);
    assert.equal(manySummary.unique, 17);
    assert.match(manySummary.csv, /^calls,module,name,address\n/);
    assert.equal(manySummary.csv.split("\n").length, 18);
    assert.ok(manySummary.csv.indexOf("Fn00") < manySummary.csv.indexOf("Fn16"));
  });

  it("экранирует имя в стиле реального лога и удваивает две кавычки", () => {
    // Провенанс: имя _ZN4absl...$_0JEE... из voice-лога (с $, скобками); ожидается отсутствие внешних кавычек.
    const realStyle = "_ZN4absl::LocalInvoker<$ (x) <y>>";
    const quoted = 'name "a" and "b"';
    const summary = summarizeCallLines([
      call({ module: "m", name: realStyle, address: "0x2" }),
      call({ module: "m", name: quoted, address: "0x1" }),
    ]);
    assert.match(summary.csv, /m,_ZN4absl::LocalInvoker<\$ \(x\) <y>>,0x2/);
    assert.match(summary.csv, /m,name ""a"" and ""b"",0x1/);
    assert.doesNotMatch(summary.csv, /,\"/);
  });
});
