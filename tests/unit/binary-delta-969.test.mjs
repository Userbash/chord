// @ts-check
/**
 * Контрактные и модульные тесты генератора и валидатора бинарных дельт (TASK-969):
 * - SUB-969-1: Формат ENDSLEY/BSDIFF43 + Zstandard (encodeOfft/decodeOfft, сигнатура
 *   Zstd 0x28 0xb5 0x2f 0xfd по /home/sanya/re-deep/gateway-protocol.md §3, заголовок
 *   "ENDSLEY/BSDIFF43" по /home/sanya/re-deep/native-re/raw/updater/src-files.txt:5,
 *   независимая верификация через системный /usr/bin/zstd -d, граничные размеры 0..N).
 * - SUB-969-2: Генерация дельт generateBinaryDelta в режимах bsdiff43-zstd и zstd-patch-from
 *   на базе реального релизного бинарника rust/dist/discord-gpui (оракул rust/dist/SHA256SUMS),
 *   100% побайтовое восстановление SHA-256 и валидация схемы ManifestPatch.
 * - SUB-969-3: Детектирование повреждений патча по RE-контрактам delta_update::verify
 *   ("expected hash '<expected>' for '<path>', actually '<actual>'") и updater_client::install
 *   ("SHA256 mismatch for downloaded package: expected <expected>, got <actual>",
 *   "Content length was <expected> but only transferred <actual> bytes").
 * - SUB-969-4: Атомарный откат applyDeltaWithRollback при повреждении патча или несовпадении
 *   целевого хеша (100% сохранение исходного файла и прав 0o755, очистка staging-файлов,
 *   переключение retryOptions.skip_host_delta / skip_module_delta по /home/sanya/re-deep/main-arch.md §5.2
 *   и fallback на полный пакет).
 * - SUB-969-5: CLI-контракт scripts/generate-delta.mjs (--generate, --verify, --apply, --self-test),
 *   коды выхода 0 / 1 / 2 и маскирование токенов.
 */
import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  chmodSync,
  existsSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, it } from "node:test";
import { fileURLToPath } from "node:url";

const ROOT = fileURLToPath(new URL("../../", import.meta.url));
const SCRIPT_PATH = join(ROOT, "scripts", "generate-delta.mjs");
const DIST_DIR = join(ROOT, "rust", "dist");
const SHA256SUMS_PATH = join(DIST_DIR, "SHA256SUMS");
const GPUI_BIN_PATH = join(DIST_DIR, "discord-gpui");

/**
 * Вычисляет SHA-256 дайджест буфера или файла.
 * @param {Buffer | string} input
 * @returns {string}
 */
function sha256Hex(input) {
  const buf = typeof input === "string" ? readFileSync(input) : input;
  return createHash("sha256").update(buf).digest("hex");
}

/**
 * Создаёт реалистичную пару версий бинарного образа (v1 -> v2) на базе реального среза
 * `rust/dist/discord-gpui` для быстрых и детерминированных проверок.
 * @param {number} [sliceBytes]
 * @returns {{ v1Buf: Buffer, v2Buf: Buffer, fullBinSha256FromOracle: string }}
 */
function buildRealisticBinaryPair(sliceBytes = 262_144) {
  const sumsContent = readFileSync(SHA256SUMS_PATH, "utf8");
  const match = /^([a-f0-9]{64})\s+\*?discord-gpui$/m.exec(sumsContent);
  assert.ok(match?.[1], "В rust/dist/SHA256SUMS должна присутствовать запись для discord-gpui");

  const fdBuf = readFileSync(GPUI_BIN_PATH);
  assert.equal(
    sha256Hex(fdBuf),
    match[1],
    "Фактический SHA-256 бинарника rust/dist/discord-gpui должен совпадать с оракулом SHA256SUMS",
  );

  const v1Buf = Buffer.from(fdBuf.subarray(0, Math.min(sliceBytes, fdBuf.byteLength)));
  const v2Buf = Buffer.concat([
    v1Buf.subarray(0, 4096),
    Buffer.from("DISCORD_GPUI_V1_0_0_BETA_2_BUILD_006_DELTA_PATCH_SECTION", "utf8"),
    v1Buf.subarray(4096, 65536),
    v1Buf.subarray(65600, 180000),
    Buffer.from([0xde, 0xad, 0xbe, 0xef, 0xca, 0xfe, 0xba, 0xbe]),
    v1Buf.subarray(180000),
    Buffer.from("TAIL_SIGNATURE_V1_0_0_BETA_2", "utf8"),
  ]);
  // Дополнительно модифицируем несколько байт внутри сдвигаемой секции (имитация релокаций ELF)
  for (let offset = 10_000; offset < 10_256 && offset + 4 < v2Buf.length; offset += 32) {
    v2Buf[offset] = ((v2Buf[offset] ?? 0) + 7) & 0xff;
    v2Buf[offset + 1] = ((v2Buf[offset + 1] ?? 0) ^ 0x5a) & 0xff;
  }

  return {
    v1Buf,
    v2Buf,
    fullBinSha256FromOracle: match[1],
  };
}

describe("TASK-969: Генератор бинарных дельта-патчей zstd/bsdiff и проверка отката", () => {
  it("SUB-969-1: кодирует ENDSLEY/BSDIFF43 + Zstandard (0x28 0xb5 0x2f 0xfd) и проходит сверку с независимым /usr/bin/zstd", async () => {
    const {
      ZSTD_MAGIC_BYTES,
      ENDSLEY_BSDIFF43_MAGIC,
      encodeOfft,
      decodeOfft,
      createBsdiff43Patch,
      applyBsdiff43Patch,
    } = await import("../../scripts/generate-delta.mjs");

    // 1. Проверка констант по RE-оракулу (/home/sanya/re-deep/gateway-protocol.md §3 и src-files.txt:5)
    assert.deepEqual(
      Buffer.from(ZSTD_MAGIC_BYTES),
      Buffer.from([0x28, 0xb5, 0x2f, 0xfd]),
      "Магические байты Zstandard должны быть 28 b5 2f fd",
    );
    assert.equal(ENDSLEY_BSDIFF43_MAGIC, "ENDSLEY/BSDIFF43");

    // 2. Позитивные и граничные случаи 64-битного знакового представления bsdiff (offtout/offtin)
    const boundaryOffsets = [
      0n,
      1n,
      -1n,
      255n,
      -255n,
      65535n,
      -65535n,
      1_048_576n,
      -1_048_576n,
      BigInt(Number.MAX_SAFE_INTEGER),
      -BigInt(Number.MAX_SAFE_INTEGER),
      0x7fff_ffff_ffff_ffffn,
      -0x7fff_ffff_ffff_ffffn,
    ];
    for (const val of boundaryOffsets) {
      const encoded = encodeOfft(val);
      assert.equal(encoded.byteLength, 8, "offtout всегда должен занимать ровно 8 байт");
      const decoded = decodeOfft(encoded, 0);
      assert.equal(decoded, val, `Несовпадение decodeOfft(encodeOfft(${val}))`);
    }

    // 3. Проверка генерации патча и сверка через независимый системный оракул `/usr/bin/zstd -d`
    const { v1Buf, v2Buf } = buildRealisticBinaryPair(131_072);
    const patchBuf = createBsdiff43Patch(v1Buf, v2Buf);

    assert.deepEqual(
      patchBuf.subarray(0, 4),
      Buffer.from([0x28, 0xb5, 0x2f, 0xfd]),
      "Сжатый патч должен начинаться с магических байт кадра Zstandard (28 b5 2f fd)",
    );
    assert.ok(
      patchBuf.byteLength < v2Buf.byteLength * 0.1,
      `Дельта-патч (${patchBuf.byteLength} Б) должен быть существенно компактнее целевого образа (${v2Buf.byteLength} Б)`,
    );

    const tmp = mkdtempSync(join(tmpdir(), "bsdiff43-oracle-969-"));
    try {
      const patchFile = join(tmp, "sample.patch.zst");
      const rawBsdiffFile = join(tmp, "sample.bsdiff43");
      writeFileSync(patchFile, patchBuf);

      execFileSync("zstd", ["-d", "-q", "-f", patchFile, "-o", rawBsdiffFile]);
      const rawBsdiff = readFileSync(rawBsdiffFile);

      assert.equal(
        rawBsdiff.subarray(0, 16).toString("ascii"),
        "ENDSLEY/BSDIFF43",
        "Независимая распаковка через /usr/bin/zstd должна выявлять заголовок ENDSLEY/BSDIFF43",
      );
      assert.equal(
        Number(decodeOfft(rawBsdiff, 16)),
        v2Buf.byteLength,
        "Поле new_size в заголовке ENDSLEY/BSDIFF43 должно точно равняться размеру целевого файла",
      );
    } finally {
      rmSync(tmp, { recursive: true, force: true });
    }

    const restored = applyBsdiff43Patch(v1Buf, patchBuf);
    assert.equal(sha256Hex(restored), sha256Hex(v2Buf), "SHA-256 после applyBsdiff43Patch должен совпадать на 100%");
    assert.equal(Buffer.compare(restored, v2Buf), 0, "Побайтовое содержимое должно совпадать на 100%");

    // 4. Граничные случаи: пустой старый буфер (0 -> N), пустой новый буфер (N -> 0), идентичные буферы (N -> N)
    const emptyBuf = Buffer.alloc(0);
    const smallBuf = Buffer.from("discord-gpui-boundary-payload-0123456789", "utf8");

    const patchFromEmpty = createBsdiff43Patch(emptyBuf, smallBuf);
    assert.deepEqual(applyBsdiff43Patch(emptyBuf, patchFromEmpty), smallBuf);

    const patchToEmpty = createBsdiff43Patch(smallBuf, emptyBuf);
    assert.deepEqual(applyBsdiff43Patch(smallBuf, patchToEmpty), emptyBuf);

    const patchIdentical = createBsdiff43Patch(v1Buf, v1Buf);
    assert.ok(patchIdentical.byteLength < 128, "Дельта идентичных файлов должна сжиматься в < 128 байт");
    assert.deepEqual(applyBsdiff43Patch(v1Buf, patchIdentical), v1Buf);
  });

  it("SUB-969-2: генерирует дельты в режимах bsdiff43-zstd и zstd-patch-from со 100% восстановлением SHA-256 и валидирует ManifestPatch", async () => {
    const {
      generateBinaryDelta,
      validateDeltaManifest,
      applyDeltaWithRollback,
    } = await import("../../scripts/generate-delta.mjs");

    const { v1Buf, v2Buf } = buildRealisticBinaryPair(262_144);
    const expectedSourceSha = sha256Hex(v1Buf);
    const expectedTargetSha = sha256Hex(v2Buf);

    const tmp = mkdtempSync(join(tmpdir(), "delta-gen-969-"));
    try {
      const oldPath = join(tmp, "discord-gpui-v1.bin");
      const newPath = join(tmp, "discord-gpui-v2.bin");
      writeFileSync(oldPath, v1Buf, { mode: 0o755 });
      writeFileSync(newPath, v2Buf, { mode: 0o755 });

      for (const engine of /** @type {const} */ (["bsdiff43-zstd", "zstd-patch-from"])) {
        const patchPath = join(tmp, `discord-gpui.${engine}.delta.zst`);
        const manifestPath = join(tmp, `discord-gpui.${engine}.manifest.json`);

        const genResult = generateBinaryDelta({
          oldFile: oldPath,
          newFile: newPath,
          patchFile: patchPath,
          manifestFile: manifestPath,
          engine,
          moduleName: "host",
          fromVersion: "1.0.0-beta.1",
          toVersion: "1.0.0-beta.2",
        });

        assert.equal(genResult.ok, true, `Генерация дельты (${engine}) должна завершиться успешно`);
        assert.equal(genResult.manifest.source.sha256, expectedSourceSha);
        assert.equal(genResult.manifest.target.sha256, expectedTargetSha);
        assert.equal(genResult.manifest.source.size, v1Buf.byteLength);
        assert.equal(genResult.manifest.target.size, v2Buf.byteLength);
        assert.equal(genResult.manifest.patch.zstdMagicHex, "28b52ffd");
        assert.equal(genResult.selfVerified, true, "Генератор обязан выполнять самопроверку дельты при сборке");

        const validation = validateDeltaManifest(genResult.manifest);
        assert.equal(
          validation.valid,
          true,
          `Манифест (${engine}) должен проходить валидацию: ${validation.errors.join("; ")}`,
        );

        // Применение патча на рабочую копию v1 -> получаем точный v2
        const targetCopy = join(tmp, `target-${engine}.bin`);
        writeFileSync(targetCopy, v1Buf, { mode: 0o755 });
        chmodSync(targetCopy, 0o755);

        const applyRes = applyDeltaWithRollback({
          targetFile: targetCopy,
          patchFile: patchPath,
          manifest: genResult.manifest,
        });

        assert.equal(applyRes.ok, true, `Применение дельты (${engine}) должно быть успешным`);
        assert.equal(applyRes.status, "applied");
        assert.equal(applyRes.verifyResult, "Ok");
        assert.equal(applyRes.rolledBack, false);
        assert.equal(sha256Hex(targetCopy), expectedTargetSha, `SHA-256 после ${engine} должен совпадать на 100%`);
        assert.equal(Buffer.compare(readFileSync(targetCopy), v2Buf), 0);
        assert.equal(statSync(targetCopy).mode & 0o777, 0o755, "Права 0755 бинарника должны сохраняться");
      }

      // Негативные сценарии validateDeltaManifest
      assert.equal(validateDeltaManifest(null).valid, false);
      assert.equal(
        validateDeltaManifest({
          schemaVersion: 1,
          engine: "bsdiff43-zstd",
          moduleName: "host",
          fromVersion: "1.0.0-beta.1",
          toVersion: "1.0.0-beta.2",
          source: { sha256: "not-a-sha256", size: 10 },
          target: { sha256: expectedTargetSha, size: 20 },
          patch: { sha256: expectedSourceSha, size: 5, zstdMagicHex: "28b52ffd" },
        }).valid,
        false,
        "Невалидный SHA-256 источника должен отклоняться валидатором",
      );
      assert.equal(
        validateDeltaManifest({
          schemaVersion: 1,
          engine: "bsdiff43-zstd",
          moduleName: "host",
          fromVersion: "1.0.0-beta.1",
          toVersion: "1.0.0-beta.2",
          source: { sha256: expectedSourceSha, size: 10 },
          target: { sha256: expectedTargetSha, size: 20 },
          patch: { sha256: expectedSourceSha, size: 5, zstdMagicHex: "00000000" },
        }).valid,
        false,
        "Неверная сигнатура Zstd в манифесте должна отклоняться валидатором",
      );
    } finally {
      rmSync(tmp, { recursive: true, force: true });
    }
  });

  it("SUB-969-3: формирует точные диагностические контракты RE-дампа delta_update::verify и updater_client::install при искажении хешей и размера", async () => {
    const {
      VERIFY_RESULTS,
      UPDATER_ERROR_CODES,
      verifyFileHash,
      verifyDownloadedPatchPackage,
    } = await import("../../scripts/generate-delta.mjs");

    const tmp = mkdtempSync(join(tmpdir(), "verify-re-969-"));
    try {
      const sampleFile = join(tmp, "discord_voice.node");
      const content = Buffer.from("discord-voice-native-module-payload", "utf8");
      writeFileSync(sampleFile, content);
      const actualSha = sha256Hex(content);
      const wrongSha = "a".repeat(64);

      // 1. Позитивная проверка verifyFileHash
      const okCheck = verifyFileHash(sampleFile, actualSha, content.byteLength);
      assert.equal(okCheck.result, VERIFY_RESULTS.OK);
      assert.equal(okCheck.ok, true);

      // 2. Несовпадение хеша файла -> точный формат из delta_update/src/verify.rs:8873
      //    "expected hash '<expected>' for '<path>', actually '<actual>'"
      const badHashCheck = verifyFileHash(sampleFile, wrongSha, content.byteLength);
      assert.equal(badHashCheck.ok, false);
      assert.equal(badHashCheck.result, VERIFY_RESULTS.INVALID_HASH);
      assert.ok(
        badHashCheck.message.includes(`expected hash '${wrongSha}' for '${sampleFile}', actually '${actualSha}'`),
        `Ожидался RE-формат verify.rs, получено: ${badHashCheck.message}`,
      );

      // 3. Отсутствующий файл -> VerifyResult::MissingFile
      const missingCheck = verifyFileHash(join(tmp, "nonexistent.bin"), actualSha);
      assert.equal(missingCheck.ok, false);
      assert.equal(missingCheck.result, VERIFY_RESULTS.MISSING_FILE);

      // 4. Несовпадение размера пакета патча -> точный формат из updater_client/src/install.rs:9824
      //    "Content length was <expected> but only transferred <actual> bytes"
      const badLenPkg = verifyDownloadedPatchPackage(sampleFile, {
        sha256: actualSha,
        size: content.byteLength + 128,
        zstdMagicHex: "28b52ffd",
      });
      assert.equal(badLenPkg.ok, false);
      assert.equal(badLenPkg.errorCode, UPDATER_ERROR_CODES.CONTENT_LENGTH_MISMATCH);
      assert.ok(
        badLenPkg.message.includes(
          `Content length was ${content.byteLength + 128} but only transferred ${content.byteLength} bytes`,
        ),
        `Ожидался RE-формат install.rs (Content length), получено: ${badLenPkg.message}`,
      );

      // 5. Несовпадение SHA-256 пакета патча -> точный формат из updater_client/src/install.rs:9822
      //    "SHA256 mismatch for downloaded package: expected <expected>, got <actual>"
      const badShaPkg = verifyDownloadedPatchPackage(sampleFile, {
        sha256: wrongSha,
        size: content.byteLength,
        zstdMagicHex: "28b52ffd",
      });
      assert.equal(badShaPkg.ok, false);
      assert.equal(badShaPkg.errorCode, UPDATER_ERROR_CODES.SHA256_MISMATCH);
      assert.ok(
        badShaPkg.message.includes(`SHA256 mismatch for downloaded package: expected ${wrongSha}, got ${actualSha}`),
        `Ожидался RE-формат install.rs (SHA256 mismatch), получено: ${badShaPkg.message}`,
      );
    } finally {
      rmSync(tmp, { recursive: true, force: true });
    }
  });

  it("SUB-969-4: выполняет атомарный откат при повреждении патча/целевого хеша и переключает skip_host_delta / skip_module_delta", async () => {
    const {
      ZSTD_MAGIC_BYTES,
      ENDSLEY_BSDIFF43_MAGIC,
      encodeOfft,
      generateBinaryDelta,
      applyDeltaWithRollback,
      createDefaultRetryOptions,
    } = await import("../../scripts/generate-delta.mjs");
    const { zstdCompressSync } = await import("node:zlib");

    const { v1Buf, v2Buf } = buildRealisticBinaryPair(131_072);
    const v1Sha = sha256Hex(v1Buf);
    const v2Sha = sha256Hex(v2Buf);

    const tmp = mkdtempSync(join(tmpdir(), "rollback-969-"));
    try {
      const oldPath = join(tmp, "old.bin");
      const newPath = join(tmp, "new.bin");
      const validPatchPath = join(tmp, "valid.delta.zst");
      const manifestPath = join(tmp, "manifest.json");

      writeFileSync(oldPath, v1Buf, { mode: 0o755 });
      writeFileSync(newPath, v2Buf, { mode: 0o755 });

      const { manifest } = generateBinaryDelta({
        oldFile: oldPath,
        newFile: newPath,
        patchFile: validPatchPath,
        manifestFile: manifestPath,
        engine: "bsdiff43-zstd",
        moduleName: "host",
        fromVersion: "1.0.0-beta.1",
        toVersion: "1.0.0-beta.2",
      });

      const validPatchBytes = readFileSync(validPatchPath);

      // Сценарий А: Повреждение 1 байта в теле Zstd-патча (bit-flip) -> откат + skip_host_delta = true
      const targetA = join(tmp, "target-a.bin");
      writeFileSync(targetA, v1Buf, { mode: 0o755 });
      chmodSync(targetA, 0o755);

      const flippedPatchPath = join(tmp, "flipped.delta.zst");
      const flippedBytes = Buffer.from(validPatchBytes);
      const midIdx = Math.floor(flippedBytes.byteLength / 2);
      flippedBytes[midIdx] = (flippedBytes[midIdx] ?? 0) ^ 0xff;
      writeFileSync(flippedPatchPath, flippedBytes);

      const retryA = createDefaultRetryOptions();
      const resA = applyDeltaWithRollback({
        targetFile: targetA,
        patchFile: flippedPatchPath,
        manifest,
        taskType: "HostInstall",
        retryOptions: retryA,
      });

      assert.equal(resA.ok, false, "Повреждённый патч должен быть отклонён");
      assert.equal(resA.rolledBack, true, "Должен быть выполнен автоматический откат");
      assert.equal(sha256Hex(targetA), v1Sha, "Целевой файл после отката должен на 100% совпадать с исходным v1");
      assert.equal(statSync(targetA).mode & 0o777, 0o755, "Права доступа 0755 должны сохраниться после отката");
      assert.equal(
        resA.nextRetryOptions.skip_host_delta,
        true,
        "При сбое HostInstall должен выставляться флаг skip_host_delta = true (по main-arch.md §5.2)",
      );

      // Сценарий Б: Усечённый патч (обрыв загрузки) для модуля discord_voice -> откат + skip_module_delta["discord_voice"] = true
      const targetB = join(tmp, "target-b.bin");
      writeFileSync(targetB, v1Buf, { mode: 0o755 });
      const truncatedPatchPath = join(tmp, "truncated.delta.zst");
      writeFileSync(truncatedPatchPath, validPatchBytes.subarray(0, Math.floor(validPatchBytes.byteLength / 3)));

      const resB = applyDeltaWithRollback({
        targetFile: targetB,
        patchFile: truncatedPatchPath,
        manifest: { ...manifest, moduleName: "discord_voice" },
        taskType: "ModuleInstall",
        retryOptions: createDefaultRetryOptions(),
      });

      assert.equal(resB.ok, false);
      assert.equal(resB.rolledBack, true);
      assert.equal(sha256Hex(targetB), v1Sha);
      assert.equal(
        resB.nextRetryOptions.skip_module_delta["discord_voice"],
        true,
        "При сбое ModuleInstall должен выставляться skip_module_delta[moduleName] = true",
      );

      // Сценарий В: Валидный по Zstd и своему SHA-256 патч, но внутри повреждённый управляющий блок ENDSLEY/BSDIFF43
      // (выход за границы буфера old_pos / new_pos) -> ошибка применения + атомарный откат
      const targetC = join(tmp, "target-c.bin");
      writeFileSync(targetC, v1Buf, { mode: 0o755 });
      const malformedBsdiffRaw = Buffer.concat([
        Buffer.from(ENDSLEY_BSDIFF43_MAGIC, "ascii"),
        encodeOfft(BigInt(v2Buf.byteLength)),
        // Управляющий триплет: add_len = 99999999 (больше new_size), copy_len = 0, seek = 0
        encodeOfft(99_999_999n),
        encodeOfft(0n),
        encodeOfft(0n),
      ]);
      const malformedPatchCompressed = zstdCompressSync(malformedBsdiffRaw);
      assert.deepEqual(malformedPatchCompressed.subarray(0, 4), ZSTD_MAGIC_BYTES);
      const malformedPatchPath = join(tmp, "malformed-triples.delta.zst");
      writeFileSync(malformedPatchPath, malformedPatchCompressed);

      const resC = applyDeltaWithRollback({
        targetFile: targetC,
        patchFile: malformedPatchPath,
        manifest: {
          ...manifest,
          patch: {
            sha256: sha256Hex(malformedPatchCompressed),
            size: malformedPatchCompressed.byteLength,
            zstdMagicHex: "28b52ffd",
          },
        },
      });

      assert.equal(resC.ok, false);
      assert.equal(resC.rolledBack, true);
      assert.equal(resC.errorCode, "PatchFailed");
      assert.equal(sha256Hex(targetC), v1Sha, "После падения парсера BSDIFF43 целевой файл обязан откатиться к v1");

      // Сценарий Г: Патч успешно применился, но итоговый SHA-256 не совпадает с manifest.target.sha256
      // + проверка автоматического фолбэка на полный образ (fallbackFullFile)
      const targetD = join(tmp, "target-d.bin");
      writeFileSync(targetD, v1Buf, { mode: 0o755 });
      const wrongTargetManifest = {
        ...manifest,
        target: {
          sha256: "f".repeat(64),
          size: v2Buf.byteLength,
        },
      };

      const resD = applyDeltaWithRollback({
        targetFile: targetD,
        patchFile: validPatchPath,
        manifest: wrongTargetManifest,
      });

      assert.equal(resD.ok, false);
      assert.equal(resD.rolledBack, true);
      assert.equal(resD.verifyResult, "InvalidHash");
      assert.equal(sha256Hex(targetD), v1Sha, "При несовпадении целевого SHA-256 обязан сработать откат к v1");

      // Сценарий Д: Откат сбойной дельты + автоматический переход на полный образ (fallbackFullFile)
      const targetE = join(tmp, "target-e.bin");
      writeFileSync(targetE, v1Buf, { mode: 0o755 });
      const resE = applyDeltaWithRollback({
        targetFile: targetE,
        patchFile: flippedPatchPath,
        manifest,
        fallbackFullFile: newPath,
      });

      assert.equal(resE.ok, true, "При наличии валидного fallbackFullFile обновление завершается через полный пакет");
      assert.equal(resE.status, "fallback_full_applied");
      assert.equal(resE.rolledBack, true, "Флаг rolledBack фиксирует, что дельта была откачена перед фолбэком");
      assert.equal(resE.fallbackUsed, true);
      assert.equal(sha256Hex(targetE), v2Sha, "После fallbackFullFile целевой файл имеет 100% хеш новой версии v2");

      // Проверяем отсутствие забытых временных файлов .staging или .rollback.bak
      const leftoverTemp = readdirSync(tmp).filter(
        (name) => name.includes(".staging") || name.includes(".rollback.bak"),
      );
      assert.deepEqual(leftoverTemp, [], "Временные staging и backup файлы должны быть полностью удалены");
    } finally {
      rmSync(tmp, { recursive: true, force: true });
    }
  });

  it("SUB-969-5: проверяет CLI scripts/generate-delta.mjs (--generate, --verify, --apply, --self-test), коды 0/1/2 и маскирование токенов", async () => {
    const { maskSensitiveTokens } = await import("../../scripts/generate-delta.mjs");

    // 1. Маскирование секретных токенов в сообщениях об ошибках
    const rawMfa = "mfa." + "aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789_secretTokenData";
    const rawBot = "Bot " + "mock_bot_token_delta_test_123456";
    const rawGhp = "ghp_" + "1234567890abcdefghijklmnopqrstuvwxyzABCD";
    const sanitized = maskSensitiveTokens(`Ошибка обновления: ${rawMfa} | ${rawBot} | ${rawGhp}`);
    assert.ok(sanitized.includes("mfa.***"));
    assert.ok(sanitized.includes("Bot ***"));
    assert.ok(sanitized.includes("ghp_***"));
    assert.ok(!sanitized.includes(rawMfa));
    assert.ok(!sanitized.includes(rawGhp));

    const tmp = mkdtempSync(join(tmpdir(), "cli-delta-969-"));
    try {
      // 2. CLI --self-test --out-json
      const selfReportPath = join(tmp, "self-test-report.json");
      const selfRun = spawnSync(
        process.execPath,
        [SCRIPT_PATH, "--self-test", "--out-json", selfReportPath],
        { cwd: ROOT, encoding: "utf8" },
      );
      assert.equal(selfRun.status, 0, `CLI --self-test завершился с ошибкой: ${selfRun.stderr}`);
      assert.ok(existsSync(selfReportPath), "CLI --self-test должен создавать JSON-отчёт");
      const report = JSON.parse(readFileSync(selfReportPath, "utf8"));
      assert.equal(report.taskId, "TASK-969");
      assert.equal(report.status, "passed");
      assert.equal(report.hashRestored100Percent, true);
      assert.equal(report.rollbackVerified, true);

      // 3. Полный цикл CLI: --generate -> --verify -> --apply
      const { v1Buf, v2Buf } = buildRealisticBinaryPair(65_536);
      const oldCliFile = join(tmp, "cli-old.bin");
      const newCliFile = join(tmp, "cli-new.bin");
      const patchCliFile = join(tmp, "cli.delta.zst");
      const manifestCliFile = join(tmp, "cli.manifest.json");
      const targetCliFile = join(tmp, "cli-target.bin");

      writeFileSync(oldCliFile, v1Buf, { mode: 0o755 });
      writeFileSync(newCliFile, v2Buf, { mode: 0o755 });
      writeFileSync(targetCliFile, v1Buf, { mode: 0o755 });

      const genCli = spawnSync(
        process.execPath,
        [
          SCRIPT_PATH,
          "--generate",
          "--old",
          oldCliFile,
          "--new",
          newCliFile,
          "--patch",
          patchCliFile,
          "--manifest",
          manifestCliFile,
        ],
        { cwd: ROOT, encoding: "utf8" },
      );
      assert.equal(genCli.status, 0, `CLI --generate упал: ${genCli.stderr}`);

      const verifyCli = spawnSync(
        process.execPath,
        [SCRIPT_PATH, "--verify", "--patch", patchCliFile, "--manifest", manifestCliFile],
        { cwd: ROOT, encoding: "utf8" },
      );
      assert.equal(verifyCli.status, 0, `CLI --verify упал: ${verifyCli.stderr}`);

      const applyCli = spawnSync(
        process.execPath,
        [
          SCRIPT_PATH,
          "--apply",
          "--target",
          targetCliFile,
          "--patch",
          patchCliFile,
          "--manifest",
          manifestCliFile,
        ],
        { cwd: ROOT, encoding: "utf8" },
      );
      assert.equal(applyCli.status, 0, `CLI --apply упал: ${applyCli.stderr}`);
      assert.equal(sha256Hex(targetCliFile), sha256Hex(v2Buf));

      // 4. Негативный вызов CLI --apply с повреждённым патчем -> код выхода 2 и автоматический откат
      writeFileSync(targetCliFile, v1Buf, { mode: 0o755 });
      const badPatchCliFile = join(tmp, "cli-corrupt.delta.zst");
      writeFileSync(badPatchCliFile, Buffer.from("CORRUPT_ZSTD_DELTA_PAYLOAD", "utf8"));

      const corruptApplyCli = spawnSync(
        process.execPath,
        [
          SCRIPT_PATH,
          "--apply",
          "--target",
          targetCliFile,
          "--patch",
          badPatchCliFile,
          "--manifest",
          manifestCliFile,
        ],
        { cwd: ROOT, encoding: "utf8" },
      );
      assert.equal(corruptApplyCli.status, 2, "При повреждённом патче CLI должен завершаться с кодом 2");
      assert.equal(
        sha256Hex(targetCliFile),
        sha256Hex(v1Buf),
        "Целевой файл после ошибки CLI --apply обязан сохранить исходный SHA-256",
      );

      // 5. Некорректные аргументы CLI -> код выхода 1
      const badArgsCli = spawnSync(process.execPath, [SCRIPT_PATH, "--invalid-option"], {
        cwd: ROOT,
        encoding: "utf8",
      });
      assert.equal(badArgsCli.status, 1, "При неверных аргументах CLI должен возвращать код 1");
    } finally {
      rmSync(tmp, { recursive: true, force: true });
    }
  });
});
