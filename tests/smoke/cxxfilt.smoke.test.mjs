// @ts-check
// RED (M4): реализация — src/adapters/cxxfilt.mjs.
// Нужен бинарь c++filt; его нет — тест пропускается.
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { chmod, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { demangle } from "../../src/adapters/cxxfilt.mjs";
import { findInPath } from "../helpers/browser.mjs";

const REAL_MANGLED = [
  "_Z10ConsoleLogNSt4__Cr12basic_stringIcNS_11char_traitsIcEENS_9allocatorIcEEEES5_",
  "_Z10InitializeN7discord8node_api6ObjectE",
  "_Z10SaveClipExN7discord8node_api6ObjectENS0_8CallbackES2_",
  "_Z10SetAecDumpb",
  "_Z11SetLoopbackbN7discord8node_api6ObjectE",
  "_Z12SetInputModeRN7discord18ConnectionSettingsENS_8OptionalIiEENS2_INS_8node_api6ObjectEEE",
  "_Z12SetKrispPathNSt4__Cr12basic_stringIcNS_11char_traitsIcEENS_9allocatorIcEEEE",
];

describe("demangle (c++filt)", () => {
  it("деманглит имена, сохраняя порядок; неманагленные — как есть", async (t) => {
    if (!findInPath("c++filt")) return t.skip("c++filt не найден в PATH");
    const out = await demangle([
      "_ZN4base4Task3RunEv",
      "plain_symbol",
      "_ZN7discord5media10Connection5StartEv",
    ]);
    assert.deepEqual(out, ["base::Task::Run()", "plain_symbol", "discord::media::Connection::Start()"]);
  });

  it("обрабатывает реальные discord_voice-символы и сохраняет неманагленные имена", async (t) => {
    if (!findInPath("c++filt")) return t.skip("c++filt не найден в PATH");
    // Источник functions/static/discord_voice.nm.txt, md5 bd7bd35ab5ed4b5648b0881c808d1473.
    // Первые семь имён с _Z; ожидаемые значения получены независимо `c++filt < имена`.
    const out = await demangle([REAL_MANGLED[0] ?? "", "plain_voice_symbol", REAL_MANGLED[1] ?? "", REAL_MANGLED[2] ?? "", "cc_encoder_add_frame_i420", ...REAL_MANGLED.slice(3)]);
    assert.deepEqual(out, [
      "ConsoleLog(std::__Cr::basic_string<char, std::__Cr::char_traits<char>, std::__Cr::allocator<char> >, std::__Cr::basic_string<char, std::__Cr::char_traits<char>, std::__Cr::allocator<char> >)",
      "plain_voice_symbol",
      "Initialize(discord::node_api::Object)",
      "SaveClipEx(discord::node_api::Object, discord::node_api::Callback, discord::node_api::Callback)",
      "cc_encoder_add_frame_i420",
      "SetAecDump(bool)",
      "SetLoopback(bool, discord::node_api::Object)",
      "SetInputMode(discord::ConnectionSettings&, discord::Optional<int>, discord::Optional<discord::node_api::Object>)",
      "SetKrispPath(std::__Cr::basic_string<char, std::__Cr::char_traits<char>, std::__Cr::allocator<char> >)",
    ]);
  });

  it("пустой список → пустой список без запуска процесса", async (t) => {
    if (!findInPath("c++filt")) return t.skip("c++filt не найден в PATH");
    assert.deepEqual(await demangle([]), []);
  });

  it("зависший c++filt → таймаут с kill", async () => {
    const dir = await mkdtemp(join(tmpdir(), "cxxfilt-timeout-"));
    const previousPath = process.env.PATH;
    try {
      const binary = join(dir, "c++filt");
      await writeFile(binary, "#!/bin/sh\nexec /bin/sleep 30\n");
      await chmod(binary, 0o755);
      process.env.PATH = dir;
      const started = Date.now();
      await assert.rejects(() => demangle(["x"], { timeoutMs: 200 }), /c\+\+filt: таймаут 200 мс/);
      assert.ok(Date.now() - started < 5000);
    } finally {
      if (previousPath === undefined) delete process.env.PATH;
      else process.env.PATH = previousPath;
      await rm(dir, { recursive: true, force: true });
    }
  });

  it("ранний выход c++filt → EPIPE не роняет процесс", async () => {
    const dir = await mkdtemp(join(tmpdir(), "cxxfilt-epipe-"));
    const previousPath = process.env.PATH;
    try {
      const binary = join(dir, "c++filt");
      await writeFile(binary, "#!/bin/sh\nexit 0\n");
      await chmod(binary, 0o755);
      process.env.PATH = dir;
      await assert.rejects(() => demangle(Array.from({ length: 20000 }, (_, i) => `_Z${i}v`)), /c\+\+filt: stdin/);
    } finally {
      if (previousPath === undefined) delete process.env.PATH;
      else process.env.PATH = previousPath;
      await rm(dir, { recursive: true, force: true });
    }
  });

  it('ENOENT (PATH="") → reject без краха', async () => {
    const previousPath = process.env.PATH;
    try {
      process.env.PATH = "";
      await assert.rejects(() => demangle(["x"]), /spawn|ENOENT|c\+\+filt/i);
    } finally {
      if (previousPath === undefined) delete process.env.PATH;
      else process.env.PATH = previousPath;
    }
  });
});
