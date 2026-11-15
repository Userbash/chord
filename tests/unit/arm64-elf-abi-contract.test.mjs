// @ts-check
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const root = fileURLToPath(new URL('../../', import.meta.url));
const linux = { skip: process.platform !== 'linux' && 'requires Linux clang/lld/readelf and Python 3' };

/** @param {string} command @param {string[]} args */
function run(command, args) {
  const result = spawnSync(command, args, { encoding: 'utf8', timeout: 20_000 });
  assert.equal(result.error, undefined, `${command}: ${result.error}`);
  return result;
}

/** @param {import('node:test').TestContext} t @param {number} [alignment] */
function linkedElf(t, alignment = 16384) {
  const dir = mkdtempSync(join(tmpdir(), 'arm-elf-contract-'));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  const source = join(dir, 'probe.s');
  const binary = join(dir, 'probe');
  // A linker-produced format fixture, never a Chord executable or release artifact.
  writeFileSync(source, '.global _start\n.text\n_start:\n mov x0, #0\n mov x8, #93\n svc #0\n.section .note.GNU-stack,"",%progbits\n');
  const result = run('clang', ['--target=aarch64-unknown-linux-gnu', '-fuse-ld=lld', '-nostdlib',
    '-pie', '-Wl,--dynamic-linker=/lib/ld-linux-aarch64.so.1', '-Wl,--build-id=sha1',
    '-Wl,-z,relro,-z,now,-z,noexecstack', `-Wl,-z,max-page-size=${alignment}`, source, '-o', binary]);
  assert.equal(result.status, 0, result.stderr);
  return { binary, bytes: readFileSync(binary) };
}

/** @param {Buffer} bytes */
function programHeaders(bytes) {
  // ELF64 layout comes from gABI chapter 7; readelf independently checks the control.
  const offset = Number(bytes.readBigUInt64LE(32));
  const stride = bytes.readUInt16LE(54);
  return Array.from({ length: bytes.readUInt16LE(56) }, (_, index) => offset + stride * index);
}

/** @param {Buffer} bytes @param {number} type */
function segment(bytes, type) {
  const offset = programHeaders(bytes).find(position => bytes.readUInt32LE(position) === type);
  assert.notEqual(offset, undefined, `linker fixture is missing program header ${type}`);
  return /** @type {number} */ (offset);
}

/** @param {string} binary @param {boolean} [gui] */
function validate(binary, gui = true) {
  return run('python3', ['-c',
    'import json,sys; sys.path.insert(0,sys.argv[1]); from verify import validate_elf; ' +
    'print(json.dumps(validate_elf(sys.argv[2],gui=sys.argv[3]=="true")))',
    join(root, 'scripts/arm64'), binary, String(gui)]);
}

/** @param {import('node:test').TestContext} t @param {(bytes: Buffer) => void} mutate @param {RegExp} reason @param {boolean} [gui] */
function rejects(t, mutate, reason, gui = false) {
  const { binary, bytes } = linkedElf(t);
  const control = validate(binary, gui);
  assert.equal(control.status, 0, control.stderr);
  mutate(bytes);
  writeFileSync(binary, bytes);
  const rejected = validate(binary, gui);
  assert.notEqual(rejected.status, 0, 'malformed ELF was accepted');
  assert.match(rejected.stderr, reason);
  assert.doesNotMatch(rejected.stderr, /ModuleNotFoundError|SyntaxError|struct\.error|UnicodeDecodeError/);
}

test('a genuine linked ARM ELF reports segment alignment without inventing a kernel page size', linux, t => {
  const { binary } = linkedElf(t);
  const independent = run('readelf', ['--wide', '--file-header', '--program-headers', binary]);
  assert.equal(independent.status, 0, independent.stderr);
  assert.match(independent.stdout, /AArch64/);
  assert.match(independent.stdout, /LOAD.*0x4000/);
  const result = validate(binary);
  assert.equal(result.status, 0, result.stderr);
  const report = JSON.parse(result.stdout);
  assert.equal(report.load_segment_alignment_bytes, 16384);
  assert.equal(report.kernel_page_size_bytes, null);
  assert.equal('page_size_bytes' in report, false);
  assert.equal(report.full_relro, true);
});

test('runtime ELF validation permits 4 KiB alignment while GUI packaging retains its 16 KiB contract', linux, t => {
  const { binary } = linkedElf(t, 4096);
  assert.equal(validate(binary, false).status, 0);
  const result = validate(binary);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /16 KiB/);
});

test('ELF rejects load bytes larger than the memory image, including the runtime path', linux, t => {
  rejects(t, bytes => {
    // Validate a later LOAD as well; checking only the first header misses this.
    const loads = programHeaders(bytes).filter(position => bytes.readUInt32LE(position) === 1);
    assert.ok(loads.length > 1);
    const load = /** @type {number} */ (loads.at(-1));
    bytes.writeBigUInt64LE(bytes.readBigUInt64LE(load + 32) - 1n, load + 40);
  }, /file size.*memory size/);
});

test('ELF rejects non-power-of-two load alignment even when divisible by 16 KiB', linux, t => {
  rejects(t, bytes => bytes.writeBigUInt64LE(49152n, segment(bytes, 1) + 48), /power of two/);
});

test('ELF rejects incongruent file offsets and virtual addresses', linux, t => {
  rejects(t, bytes => {
    const load = segment(bytes, 1);
    bytes.writeBigUInt64LE(bytes.readBigUInt64LE(load + 16) + 1n, load + 16);
  }, /congruent/);
});

test('ELF rejects 4 KiB congruence that violates the declared 16 KiB alignment', linux, t => {
  rejects(t, bytes => {
    const load = segment(bytes, 1);
    bytes.writeBigUInt64LE(bytes.readBigUInt64LE(load + 16) + 4096n, load + 16);
  }, /congruent/);
});

test('ELF rejects virtual memory ranges overflowing an ELF64 address', linux, t => {
  rejects(t, bytes => {
    const load = segment(bytes, 1);
    bytes.writeBigUInt64LE(0xfffffffffffffff0n, load + 16);
    bytes.writeBigUInt64LE(1n, load + 48);
  }, /address.*overflow/);
});

test('ELF rejects a load extent outside the actual file', linux, t => {
  rejects(t, bytes => {
    const load = segment(bytes, 1);
    bytes.writeBigUInt64LE(BigInt(bytes.length + 1), load + 32);
    bytes.writeBigUInt64LE(BigInt(bytes.length + 1), load + 40);
  }, /truncated ELF segment/);
});

test('ELF rejects overlapping file-header and program-header tables', linux, t => {
  rejects(t, bytes => bytes.writeBigUInt64LE(8n, 32), /program headers/);
});

test('ELF rejects an invalid ELF header size', linux, t => {
  rejects(t, bytes => bytes.writeUInt16LE(63, 52), /ELF header/);
});

test('ELF rejects an unsupported object-file version', linux, t => {
  rejects(t, bytes => bytes.writeUInt32LE(0, 20), /ELF header/);
});

test('GUI ELF requires a terminated interpreter path', linux, t => {
  rejects(t, bytes => {
    const interp = segment(bytes, 3);
    const end = Number(bytes.readBigUInt64LE(interp + 8) + bytes.readBigUInt64LE(interp + 32));
    bytes[end - 1] = 0x78;
  }, /ELF interpreter/, true);
});

test('GUI ELF rejects a malformed interpreter with a typed integrity error', linux, t => {
  rejects(t, bytes => {
    const interp = segment(bytes, 3);
    bytes[Number(bytes.readBigUInt64LE(interp + 8))] = 0xff;
  }, /ELF interpreter/, true);
});

test('ELF permits no-alignment load declarations for runtimes but not GUI packages', linux, t => {
  for (const alignment of [0n, 1n]) {
    const { binary, bytes } = linkedElf(t);
    for (const header of programHeaders(bytes).filter(position => bytes.readUInt32LE(position) === 1)) {
      bytes.writeBigUInt64LE(alignment, header + 48);
    }
    writeFileSync(binary, bytes);
    assert.equal(validate(binary, false).status, 0);
    assert.notEqual(validate(binary).status, 0);
  }
});

test('runtime LOAD address and offset remain congruent to the minimum Linux page alignment', linux, t => {
  for (const alignment of [0n, 1n]) {
    rejects(t, bytes => {
      const load = segment(bytes, 1);
      bytes.writeBigUInt64LE(alignment, load + 48);
      bytes.writeBigUInt64LE(bytes.readBigUInt64LE(load + 16) + 1n, load + 16);
    }, /congruent/);
  }
});

test('ELF permits zero-filled BSS beyond the file-backed portion of a LOAD', linux, t => {
  const { binary, bytes } = linkedElf(t);
  const loads = programHeaders(bytes).filter(position => bytes.readUInt32LE(position) === 1);
  const load = /** @type {number} */ (loads.at(-1));
  bytes.writeBigUInt64LE(bytes.readBigUInt64LE(load + 32) + 4096n, load + 40);
  writeFileSync(binary, bytes);
  const result = validate(binary);
  assert.equal(result.status, 0, result.stderr);
});

test('ELF supports a separately linked 64 KiB layout without claiming native page-size qualification', linux, t => {
  const { binary } = linkedElf(t, 65536);
  const result = validate(binary);
  assert.equal(result.status, 0, result.stderr);
  const report = JSON.parse(result.stdout);
  assert.equal(report.load_segment_alignment_bytes, 65536);
  assert.equal(report.kernel_page_size_bytes, null);
});

test('ELF executable requires a load segment even for runtime checks', linux, t => {
  rejects(t, bytes => {
    for (const header of programHeaders(bytes).filter(position => bytes.readUInt32LE(position) === 1)) {
      bytes.writeUInt32LE(0, header);
    }
  }, /requires a PT_LOAD/);
});

test('ELF rejects descending LOAD virtual addresses', linux, t => {
  rejects(t, bytes => {
    const load = segment(bytes, 1);
    bytes.writeBigUInt64LE(0x1000000n, load + 16);
    bytes.writeBigUInt64LE(1n, load + 48);
  }, /ascending order/);
});

test('GUI ELF interpreter must precede the LOAD entries', linux, t => {
  rejects(t, bytes => {
    const interp = segment(bytes, 3);
    const load = segment(bytes, 1);
    const original = Buffer.from(bytes.subarray(interp, interp + 56));
    bytes.copy(bytes, interp, load, load + 56);
    original.copy(bytes, load);
  }, /PT_INTERP must precede/, true);
});

test('unused PT_NULL members are ignored as required by the ELF ABI', linux, t => {
  const { binary, bytes } = linkedElf(t);
  const note = segment(bytes, 4);
  bytes.writeUInt32LE(0, note);
  bytes.writeBigUInt64LE(0xffffffffffffffffn, note + 8);
  writeFileSync(binary, bytes);
  const result = validate(binary, false);
  assert.equal(result.status, 0, result.stderr);
});
