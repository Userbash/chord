// Ревью RW-03: независимый мини-парсер ETF для HELLO-терма (свой код, не кодек проекта).
// Поддерживаем только теги, встречающиеся в HELLO-кадре: ATOM_EXT(0x64), MAP_EXT(0x74),
// INTEGER_EXT(0x62), SMALL_INTEGER_EXT(0x61), BINARY_EXT(0x6d), LIST_EXT(0x6c), NIL_EXT(0x6a),
// STRING_EXT(0x6b), NEW_FLOAT_EXT(0x46), SMALL_TUPLE_EXT(0x68), BIG_INT_EXT(0x6e).
import fs from "node:fs";

const HELLO_HEX =
  "8374000000046400016474000000026400065f74726163656c000000016d000000325b22676174657761792d7072642d61726d2d75732d65617374312d642d30346d35222c7b226d6963726f73223a302e307d5d6a6400126865617274626561745f696e74657276616c620000a1226400026f70610a640001736400036e696c640001746400036e696c";

function fromHex(s) {
  return Buffer.from(s, "hex");
}

class P {
  constructor(buf) {
    this.b = buf;
    this.i = 0;
  }
  u8() {
    return this.b[this.i++];
  }
  u16() {
    const v = this.b.readUInt16BE(this.i);
    this.i += 2;
    return v;
  }
  u32() {
    const v = this.b.readUInt32BE(this.i);
    this.i += 4;
    return v;
  }
  i32() {
    const v = this.b.readInt32BE(this.i);
    this.i += 4;
    return v;
  }
  bytes(n) {
    const v = this.b.subarray(this.i, this.i + n);
    this.i += n;
    return v;
  }
}

function parse(p) {
  const tag = p.u8();
  switch (tag) {
    case 0x61: // SMALL_INTEGER_EXT
      return p.u8();
    case 0x62: // INTEGER_EXT
      return p.i32();
    case 0x64: {
      // ATOM_EXT (latin1)
      const len = p.u16();
      return p.bytes(len).toString("latin1");
    }
    case 0x6a: // NIL_EXT
      return null;
    case 0x6b: {
      // STRING_EXT
      const len = p.u16();
      return p.bytes(len).toString("latin1");
    }
    case 0x6d: {
      // BINARY_EXT
      const len = p.u32();
      return p.bytes(len).toString("utf8");
    }
    case 0x6c: {
      // LIST_EXT
      const n = p.u32();
      const arr = [];
      for (let k = 0; k < n; k++) arr.push(parse(p));
      if (p.u8() !== 0x6a) throw new Error("список без хвоста NIL_EXT");
      return arr;
    }
    case 0x74: {
      // MAP_EXT
      const n = p.u32();
      const m = new Map();
      for (let k = 0; k < n; k++) {
        const key = parse(p);
        m.set(key, parse(p));
      }
      return m;
    }
    case 0x68: {
      // SMALL_TUPLE_EXT
      const n = p.u8();
      const arr = [];
      for (let k = 0; k < n; k++) arr.push(parse(p));
      return arr;
    }
    case 0x46: {
      const v = p.b.readDoubleBE(p.i);
      p.i += 8;
      return v;
    }
    case 0x6e: {
      // SMALL_BIG_EXT: len, sign, little-endian digits (в этом кадре не встречается)
      const n = p.u8();
      const sign = p.u8();
      let v = 0n;
      for (let k = n - 1; k >= 0; k--) v = (v << 8n) | BigInt(p.bytes(1)[0]);
      return sign ? -v : v;
    }
    default:
      throw new Error(`неизвестный тег 0x${tag.toString(16)} на смещении ${p.i - 1}`);
  }
}

const term = fromHex(HELLO_HEX);
console.log("длина терма:", term.length, "Б (учитывая ведущий 0x83 =", term[0].toString(16) + ")");
const p = new P(term);
if (tagVersion(p) !== 0x83) throw new Error("нет версии 0x83");
function tagVersion(pp) {
  return pp.u8();
}
const value = parse(p);
console.log("потреблено:", p.i, "из", term.length, "| хвост:", term.length - p.i, "Б");
console.log("op =", value.get("op"));
const d = value.get("d");
console.log("d.heartbeat_interval =", d.get("heartbeat_interval"), "(0x" + d.get("heartbeat_interval").toString(16) + ")");
console.log("s =", value.get("s"), "| t =", value.get("t"));
console.log("ключи:", [...value.keys()].join(","));
console.log("ключи d:", [...d.keys()].join(","));

// Проверка ожиданий из ТЗ/тестов.
const ok = value.get("op") === 10 && d.get("heartbeat_interval") === 41250 && term.length === 138;
console.log("ИТОГ:", ok ? "op10 / interval=41250 / 138 Б — СОГЛАСНО ТЗ" : "РАСХОЖДЕНИЕ");
process.exit(ok ? 0 : 1);
