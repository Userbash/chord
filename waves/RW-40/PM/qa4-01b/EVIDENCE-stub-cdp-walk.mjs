// Ручная проверка (T3) CLI `walk fixture` против локального loopback stub-CDP.
// Evidence, не часть гейта: реального браузера/live Discord нет, сеть — только 127.0.0.1.
// Unit-тесты сокетов не открывают; tests/e2e/** не меняются этим скриптом.
import http from "node:http";
import crypto from "node:crypto";
import { spawn } from "node:child_process";
import { mkdtemp, readFile, readdir } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import assert from "node:assert/strict";

const GUID = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";
const PNG = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 1, 2, 3, 4]);
const ITEMS = ["Мой аккаунт", "Профиль", "Уведомления", "Голос и видео", "Покинуть сервер", "Выйти"];
const PROFILE = {
  nodes: [
    { hitCount: 5, callFrame: { functionName: "onNavClick", url: "file:///fixture/app.js" } },
    { hitCount: 1, callFrame: { functionName: "(idle)", url: "" } },
  ],
};

/** @type {Set<import("node:net").Socket>} */
const sockets = new Set();
const server = http.createServer((req, res) => {
  if (req.url !== "/json/list") {
    res.statusCode = 404;
    res.end();
    return;
  }
  const port = /** @type {import("node:net").AddressInfo} */ (server.address()).port;
  res.setHeader("content-type", "application/json");
  res.end(JSON.stringify([
    { type: "page", url: "file:///fixture/index.html", title: "Fixture", webSocketDebuggerUrl: `ws://127.0.0.1:${port}/devtools/page/1` },
  ]));
});

/** @param {import("node:net").Socket} socket @param {any} message */
function send(socket, message) {
  const data = Buffer.from(JSON.stringify(message));
  const header = data.length < 126
    ? Buffer.from([0x81, data.length])
    : Buffer.from([0x81, 126, data.length >> 8, data.length & 0xff]);
  socket.write(Buffer.concat([header, data]));
}

/** @type {string[]} */
const methods = [];
/** @type {string[]} */
const clickedLabels = [];

server.on("upgrade", (req, socket) => {
  sockets.add(socket);
  socket.on("close", () => sockets.delete(socket));
  const key = String(req.headers["sec-websocket-key"] ?? "");
  const accept = crypto.createHash("sha1").update(key + GUID).digest("base64");
  socket.write(
    "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n"
      + `Sec-WebSocket-Accept: ${accept}\r\n\r\n`,
  );
  let buffer = Buffer.alloc(0);
  socket.on("data", (chunk) => {
    buffer = Buffer.concat([buffer, chunk]);
    for (;;) {
      if (buffer.length < 2) return;
      const opcode = buffer[0] & 0x0f;
      const masked = (buffer[1] & 0x80) !== 0;
      let length = buffer[1] & 0x7f;
      let offset = 2;
      if (length === 126) {
        if (buffer.length < 4) return;
        length = buffer.readUInt16BE(2);
        offset = 4;
      } else if (length === 127) {
        if (buffer.length < 10) return;
        length = Number(buffer.readBigUInt64BE(2));
        offset = 10;
      }
      const maskLength = masked ? 4 : 0;
      if (buffer.length < offset + maskLength + length) return;
      let payload = buffer.subarray(offset + maskLength, offset + maskLength + length);
      if (masked) {
        const mask = buffer.subarray(offset, offset + 4);
        payload = Buffer.from(payload.map((byte, index) => byte ^ mask[index % 4]));
      }
      buffer = buffer.subarray(offset + maskLength + length);
      if (opcode === 8) {
        socket.end();
        return;
      }
      if (opcode !== 1) continue;
      const request = JSON.parse(payload.toString("utf8"));
      methods.push(request.method);
      send(socket, { id: request.id, result: respond(request, socket) });
    }
  });
});

/**
 * @param {{ method: string, params: any }} request
 * @param {import("node:net").Socket} socket
 */
function respond(request, socket) {
  if (request.method === "Page.captureScreenshot") return { data: PNG.toString("base64") };
  if (request.method === "Profiler.stop") return { profile: PROFILE };
  const expression = String(request.params?.expression ?? "");
  if (expression.includes("scrollIntoView")) {
    const label = ITEMS.find((text) => expression.includes(JSON.stringify(text))) ?? "";
    clickedLabels.push(label);
    if (label === "Уведомления") {
      // Событие консоли до ответа на evaluate: порядок как у реального Runtime.
      send(socket, {
        method: "Runtime.consoleAPICalled",
        params: { type: "error", args: [{ value: "fixture: раздел уведомлений сломан" }] },
      });
    }
    return { result: { value: { clicked: true, text: label } } };
  }
  if (expression.includes("querySelectorAll")) {
    return { result: { value: ITEMS.map((text) => ({ text })) } };
  }
  return { result: { value: null } };
}

await new Promise((done) => server.listen(0, "127.0.0.1", done));
const endpoint = `http://127.0.0.1:${/** @type {import("node:net").AddressInfo} */ (server.address()).port}`;
const env = { ...process.env, CDP_ENDPOINT: endpoint, CDP_TARGET: "index.html" };

/** @param {string[]} args */
function cli(args) {
  return new Promise((resolve, reject) => {
    const child = spawn(process.execPath, ["bin/discord-lab.mjs", ...args], { cwd: process.cwd(), env });
    let stdout = "";
    let stderr = "";
    child.stdout.setEncoding("utf8").on("data", (chunk) => { stdout += chunk; });
    child.stderr.setEncoding("utf8").on("data", (chunk) => { stderr += chunk; });
    child.once("error", reject);
    child.once("close", (code) => resolve({ code, stdout, stderr }));
  });
}

const dir = await mkdtemp(join(tmpdir(), "discord-lab-stub-walk-"));
const out = join(dir, "ui-walk-fixture.jsonl");
const screens = join(dir, "screens");
try {
  const result = await cli(["walk", "fixture", "--url", "index.html", "--out", out, "--screens", screens, "--settle-ms", "0"]);
  assert.equal(result.code, 0, result.stderr);
  assert.match(result.stdout, /ПРОПУСК \(запрещено\): Покинуть сервер/u);
  assert.match(result.stdout, /ПРОПУСК \(запрещено\): Выйти/u);
  assert.match(result.stdout, /Готово: 4 шагов/u);

  const records = (await readFile(out, "utf8")).split("\n").filter((line) => line.trim()).map((line) => JSON.parse(line));
  assert.deepEqual(records.map((record) => record.label), ITEMS.slice(0, 4));
  assert.deepEqual(records.map((record) => record.step), [1, 2, 3, 4]);
  for (const record of records) {
    assert.equal(record.clicked, true, record.label);
    const bytes = await readFile(record.screenshot);
    assert.deepEqual([...bytes.subarray(0, 4)], [0x89, 0x50, 0x4e, 0x47]);
  }
  assert.equal(records.find((record) => record.label === "Уведомления")?.consoleErrors, 1);
  assert.equal(records.find((record) => record.label === "Профиль")?.consoleErrors, 0);
  assert.equal(records.find((record) => record.label === "Профиль")?.topFunctions[0]?.fn, "onNavClick");
  assert.equal((await readdir(screens)).filter((name) => name.endsWith(".png")).length, 4);
  assert.deepEqual(clickedLabels, ITEMS.slice(0, 4));
  for (const forbidden of ["Page.navigate", "Target.attachToTarget", "Input.dispatchMouseEvent", "Network.enable"]) {
    assert.equal(methods.includes(forbidden), false, `запрещённый вызов ${forbidden}`);
  }
  console.log("OK stub-CDP walk: 4 шага, 4 PNG, consoleErrors 1, запрещённые вызовы отсутствуют");
  console.log(`artifacts: ${out}`);
} finally {
  for (const socket of sockets) socket.destroy();
  server.close();
}
