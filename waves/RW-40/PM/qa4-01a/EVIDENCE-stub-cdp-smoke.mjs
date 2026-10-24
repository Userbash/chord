// Ручная проверка CLI cdp (eval/list/clicktext/shot) против локального stub-CDP.
// Не коммитится: unit-тесты не открывают сокетов, e2e/** read-only, браузера в среде нет.
import http from "node:http";
import crypto from "node:crypto";
import { spawn } from "node:child_process";
import { mkdtemp, readFile } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import assert from "node:assert/strict";

const GUID = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";
const PNG = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 1, 2, 3]);

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
      send(socket, { id: request.id, result: respond(request) });
    }
  });
});

/** @param {{ method: string, params: any }} request */
function respond(request) {
  if (request.method === "Page.captureScreenshot") return { data: PNG.toString("base64") };
  const expression = String(request.params?.expression ?? "");
  if (expression.includes("const needle")) {
    return { result: { value: expression.includes(JSON.stringify("мой аккаунт")) ? { clicked: true, tag: "BUTTON", text: "Мой аккаунт" } : { clicked: false } } };
  }
  if (expression.includes("querySelectorAll") && expression.includes("getBoundingClientRect")) {
    return { result: { value: Array.from({ length: 6 }, (_, index) => ({ tag: "BUTTON", role: null, aria: null, text: `b${index}`, visible: true, rect: [0, index, 10, 10] })) } };
  }
  if (expression === "document.title") return { result: { value: "Discord Lab Fixture" } };
  if (expression === "1+1") return { result: { value: 2 } };
  return { exceptionDetails: { text: "stub: неизвестное выражение" } };
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

const results = [];
try {
  const targets = await cli(["cdp", "targets"]);
  assert.equal(targets.code, 0, targets.stderr);
  assert.ok(targets.stdout.includes("index.html"));
  results.push(["targets", targets.code]);

  const title = await cli(["cdp", "eval", "document.title"]);
  assert.equal(title.code, 0, title.stderr);
  assert.equal(JSON.parse(title.stdout), "Discord Lab Fixture");
  results.push(["eval title", title.code]);

  const sum = await cli(["cdp", "eval", "1+1"]);
  assert.equal(JSON.parse(sum.stdout), 2);
  results.push(["eval 1+1", sum.code]);

  const broken = await cli(["cdp", "eval", "boom()"]);
  assert.equal(broken.code, 2, broken.stderr);
  assert.match(broken.stderr, /stub: неизвестное выражение/);
  results.push(["eval JS-ошибка → 2", broken.code]);

  const click = await cli(["cdp", "clicktext", "Мой аккаунт"]);
  assert.equal(click.code, 0, click.stderr);
  assert.equal(JSON.parse(click.stdout).clicked, true);
  results.push(["clicktext", click.code]);

  const missing = await cli(["cdp", "clicktext", "Нет такой"]);
  assert.equal(missing.code, 2, missing.stderr);
  assert.match(missing.stderr, /элемент не найден: «Нет такой»/);
  results.push(["clicktext не найден → 2", missing.code]);

  const list = await cli(["cdp", "list", "button"]);
  assert.equal(list.code, 0, list.stderr);
  assert.equal(JSON.parse(list.stdout).length, 6);
  assert.ok(JSON.parse(list.stdout).every((entry) => entry.tag === "BUTTON"));
  results.push(["list", list.code]);

  const dir = await mkdtemp(join(tmpdir(), "qa4-01a-shot-"));
  const shotPath = join(dir, "nested", "shot.png");
  const shot = await cli(["cdp", "shot", shotPath]);
  assert.equal(shot.code, 0, shot.stderr);
  assert.deepEqual([...await readFile(shotPath)], [...PNG]);
  results.push(["shot", shot.code]);

  const badShot = await cli(["cdp", "shot", "/dev/full"]);
  assert.equal(badShot.code, 2, badShot.stderr);
  assert.match(badShot.stderr, /Не удалось записать скриншот/);
  results.push(["shot ошибка записи → 2", badShot.code]);

  console.log(JSON.stringify(results));
} finally {
  for (const socket of sockets) socket.destroy();
  server.closeAllConnections();
  server.close();
}
process.exit(0);
