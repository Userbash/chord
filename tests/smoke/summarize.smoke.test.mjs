// @ts-check
// RED (M4): реализация — CLI summarize. Контракт: рядом с логом появляется
// <лог>.by-function.csv, в stdout — сводка (паритет scripts/summarize-calls.sh).
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { readFile, writeFile } from "node:fs/promises";
import { basename, join } from "node:path";
import { fileURLToPath } from "node:url";
import { runCli } from "../helpers/cli.mjs";
import { makeTmpDir } from "../helpers/tmp.mjs";

const callsFixture = fileURLToPath(new URL("../fixtures/logs/calls.sample.jsonl", import.meta.url));
const realCallsLog = fileURLToPath(new URL("../../logs/calls-voice-28170.jsonl", import.meta.url));

describe("discord-lab summarize", () => {
  it("пишет <лог>.by-function.csv и печатает сводку", async (t) => {
    const dir = await makeTmpDir(t);
    const logPath = join(dir, "calls-123.jsonl");
    await writeFile(logPath, await readFile(callsFixture));
    const result = await runCli(["summarize", logPath]);
    assert.equal(result.code, 0, result.stderr);
    assert.match(result.stdout, /вызовов 6/);
    assert.match(result.stdout, /уникальных функций 3/);
    assert.ok(result.stdout.includes(`${basename(logPath)}.by-function.csv`));

    const csv = await readFile(`${logPath}.by-function.csv`, "utf8");
    assert.equal(
      csv,
      [
        "calls,module,name,address",
        "2,discord_utils.node,PollMetrics,0x2a10",
        "3,discord_utils.node,Task::Run,0x1f00",
        "1,discord_voice.node,RtpTransport::SendRtp,0x3300",
        "",
      ].join("\n"),
    );
  });

  it("без аргументов и без logs/calls-*.jsonl (cwd) → код 1 с подсказкой", async (t) => {
    const dir = await makeTmpDir(t);
    const result = await runCli(["summarize"], { cwd: dir });
    assert.equal(result.code, 1);
    assert.match(result.stderr, /Логов не найдено/);
  });

  it("полный реальный voice-лог: stdout и CSV совпадают с независимым оракулом", async (t) => {
    // Провенанс: logs/calls-voice-28170.jsonl, md5=f4e09451d03f27f3fa12f65d34c4f76f.
    // Оракулы: wc -l => 4297; jq | sort | uniq -c | sed | awk => 6 уникальных функций,
    // CSV md5=69a2ad5637f26f0491697246fa163b50.
    let source;
    try {
      source = await readFile(realCallsLog, "utf8");
    } catch (error) {
      if (/** @type {NodeJS.ErrnoException} */ (error).code === "ENOENT") {
        t.skip("отсутствует logs/calls-voice-28170.jsonl");
        return;
      }
      throw error;
    }
    const dir = await makeTmpDir(t);
    const logPath = join(dir, "calls-voice-28170.jsonl");
    await writeFile(logPath, source);
    const result = await runCli(["summarize", logPath]);
    assert.equal(result.code, 0, result.stderr);
    assert.match(result.stdout, /вызовов 4297/);
    assert.match(result.stdout, /уникальных функций 6/);
    const csv = await readFile(`${logPath}.by-function.csv`, "utf8");
    assert.equal(csv, [
      "calls,module,name,address",
      "262,discord_voice.node,WebRtcOpus_Decode,0x7df88e2bc110",
      "228,discord_voice.node,_ZN4absl22internal_any_invocable12LocalInvokerILb0EvOZN6webrtc18ModuleRtpRtcpImpl239ScheduleMaybeSendRtcpAtOrAfterTimestampENS2_9TimestampENS2_9TimeDeltaEE3$_0JEEET0_PNS0_15TypeErasedStateEDpNS0_18ForwardedParameterIT2_E4typeE,0x7df88e3ac3e0",
      "262,discord_voice.node,_ZN6webrtc9NetEqImpl12InsertPacketERKNS_9RTPHeaderENS_9ArrayViewIKhLln4711EEERKNS_13RtpPacketInfoE,0x7df88e405530",
      "210,discord_voice.node,_ZN7discord5media19SocketClientAdapter8SendRtcpEN6webrtc9ArrayViewIKhLln4711EEERKNS2_13PacketOptionsE,0x7df88dffcc50",
      "2880,discord_voice.node,_ZNK6webrtc9NetEqImpl22FilteredCurrentDelayMsEv,0x7df88e407410",
      "455,discord_voice.node,_ZZN6webrtc9NetEqImpl12InsertPacketERKNS_9RTPHeaderENS_9ArrayViewIKhLln4711EEERKNS_13RtpPacketInfoEENK3$_2clERNS_12AudioDecoder11ParseResultE,0x7df88e406220",
      "",
    ].join("\n"));
  });
});
