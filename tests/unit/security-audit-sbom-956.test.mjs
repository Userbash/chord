// @ts-check
/**
 * Контрактные и модульные тесты комплексного аудита безопасности, очистки секретов
 * и генерации SBOM-манифеста по стандарту SPDX 2.3 (TASK-956):
 * - SUB-956-1: Статический поиск секретов по 20+ регулярным выражениям (токены Discord MFA,
 *   Bot, User JWT, Webhook, отсутствие приватных ключей и захардкоженных секретов DAVE MLS
 *   в исходных кодах и релизном бинарнике rust/dist/discord-gpui).
 * - SUB-956-2: Аудит маскирования в логах паник и logcat (маски mfa.*** и Bot ***),
 *   проверка Debug-редактирования (<redacted> / <скрыто>) и zeroize-очистки секретов в ОЗУ
 *   перед деаллокацией (Drop для DaveMlsEpochSecrets и DaveHashRatchet).
 * - SUB-956-3: Генерация и верификация финального SBOM-манифеста rust/dist/sbom.spdx.json
 *   по стандарту SPDX 2.3 и проверка соответствия лицензий всех 85+ зависимостей (98 пакетов)
 *   политике открытого ПО (MIT / Apache-2.0 / BSD-3-Clause и др.).
 */
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, it } from "node:test";
import { fileURLToPath } from "node:url";

const ROOT = fileURLToPath(new URL("../../", import.meta.url));
const SECRET_SCAN_SCRIPT = join(ROOT, "scripts", "secret-scan.mjs");
const SBOM_SCRIPT = join(ROOT, "scripts", "sbom.mjs");
const SBOM_PATH = join(ROOT, "rust", "dist", "sbom.spdx.json");
const DIST_BIN_PATH = join(ROOT, "rust", "dist", "discord-gpui");
const REPORT_RW44_PATH = join(ROOT, "waves", "RW-44", "QUALITY", "SECURITY-AUDIT-REPORT.md");
const REPORT_RW45_PATH = join(ROOT, "waves", "RW-45", "QUALITY", "SECURITY-AUDIT-REPORT.md");

describe("TASK-956: Комплексный аудит безопасности, очистка секретов и генерация SBOM-манифеста SPDX 2.3", () => {
  it("SUB-956-1 (Шаг 1.1 & 1.2): сканирует кодовую базу и бинарник по 20+ паттернам (Discord MFA/Bot/User токены, ключи DAVE MLS) с 0 находок", async () => {
    const {
      SECRET_PATTERNS,
      scanTextForSecrets,
      scanRepositoryForSecrets,
      scanBinaryArtifactForSecrets,
      verifyNoDaveMlsPrivateKeys,
    } = await import("../../scripts/secret-scan.mjs");

    // 1. Проверка полноты набора правил: не менее 20 паттернов регулярных выражений
    assert.ok(
      Array.isArray(SECRET_PATTERNS) && SECRET_PATTERNS.length >= 20,
      `Ожидалось >= 20 паттернов поиска секретов, получено: ${SECRET_PATTERNS?.length}`,
    );

    const ruleIds = new Set(SECRET_PATTERNS.map((p) => p.id));
    for (const requiredRule of [
      "discord_mfa_token",
      "discord_user_or_bot_token",
      "discord_bot_auth_header",
      "discord_webhook_url",
      "dave_mls_private_key_pem",
      "dave_mls_hardcoded_secret",
    ]) {
      assert.ok(ruleIds.has(requiredRule), `Должно присутствовать правило ${requiredRule}`);
    }

    // 2. Позитивный сценарий: сканирование реальной кодовой базы показывает 0 находок
    const repoScan = scanRepositoryForSecrets(ROOT);
    assert.equal(
      repoScan.passed,
      true,
      `Обнаружены утечки в репозитории: ${JSON.stringify(repoScan.findings)}`,
    );
    assert.equal(repoScan.findings.length, 0, "Сканирование кодовой базы должно показать 0 находок");
    assert.ok(repoScan.filesScanned >= 150, `Ожидалось >= 150 просканированных файлов, получено ${repoScan.filesScanned}`);

    // 3. Шаг 1.2: Проверка отсутствия приватных ключей DAVE MLS в исходных кодах и бинарнике
    const daveCheck = verifyNoDaveMlsPrivateKeys(ROOT);
    assert.equal(daveCheck.passed, true, `Нарушение DAVE MLS: ${daveCheck.reasons.join("; ")}`);
    assert.equal(daveCheck.leakedKeys.length, 0);

    const binScan = scanBinaryArtifactForSecrets(DIST_BIN_PATH);
    assert.equal(binScan.passed, true, `В бинарнике найдены секреты: ${JSON.stringify(binScan.findings)}`);
    assert.equal(binScan.findings.length, 0);

    // 4. Негативные и граничные сценарии детектора секретов (независимый оракул синтетических токенов)
    // Собираем тестовые строки динамически, чтобы сам файл теста не содержал литералов секретов
    const fakeMfa84 = "mfa." + "A9_b-7xQ".repeat(10) + "Z1y2"; // ровно 84 символа после mfa.
    const fakeMfa83Boundary = "mfa." + "A9_b-7xQ".repeat(10) + "Z1y"; // 83 символа (ниже границы 84)
    const fakeBotHeader = "Authorization: Bot " + "Nzk5" + "MDEyMzQ1Njc4OTAxMjM0NTY" + ".X9kLmN." + "qR7vW2pL9xK4mN8bV1cZ5jH3gF6dS0aP2oI8uY";
    const fakeUserJwt = "OTg3" + "NjU0MzIxMDk4NzY1NDMyMQ" + ".G7pQrS." + "mK9vX2bN4lP8qW1zR5tY7uI3oP6aS9dF2gH5jK";
    const fakeDavePem = ["-----BEGIN DAVE MLS PRIVATE KEY-----", "MIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQg", "-----END DAVE MLS PRIVATE KEY-----"].join("\n");
    const fakeDaveHardcoded = 'const joiner_secret = "' + "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08" + '";';

    const boundarySample = `const shortToken = "${fakeMfa83Boundary}";`;
    const boundaryRes = scanTextForSecrets(boundarySample, "src/boundary.rs");
    assert.equal(boundaryRes.length, 0, "Токен MFA длиной 83 символа не должен давать ложного срабатывания");

    const leakSample = [
      `let t1 = "${fakeMfa84}";`,
      `let h1 = "${fakeBotHeader}";`,
      `let u1 = "${fakeUserJwt}";`,
      fakeDavePem,
      fakeDaveHardcoded,
    ].join("\n");

    const detected = scanTextForSecrets(leakSample, "src/leaked_sample.rs");
    const detectedIds = new Set(detected.map((d) => d.ruleId));
    assert.ok(detectedIds.has("discord_mfa_token"), "Должен детектироваться MFA-токен (84 символа)");
    assert.ok(detectedIds.has("discord_bot_auth_header"), "Должен детектироваться Bot токен");
    assert.ok(detectedIds.has("discord_user_or_bot_token"), "Должен детектироваться User/Bot JWT токен");
    assert.ok(detectedIds.has("dave_mls_private_key_pem"), "Должен детектироваться PEM-ключ DAVE MLS");
    assert.ok(detectedIds.has("dave_mls_hardcoded_secret"), "Должен детектироваться захардкоженный секрет DAVE MLS");

    // Убеждаемся, что превью в отчёте о находке маскирует сам секрет (не утекает в лог сканера)
    for (const item of detected) {
      assert.ok(!item.redactedPreview.includes(fakeMfa84), "Превью находки обязано маскировать секрет");
    }
  });

  it("SUB-956-2 (Шаг 2.1 & 2.2): маскирует токены в выводе panic/logcat масками mfa.*** и Bot *** и проверяет zeroize очистку памяти", async () => {
    const {
      maskSensitiveLogOutput,
      zeroizeBuffer,
      auditRustRedactionAndZeroize,
    } = await import("../../scripts/secret-scan.mjs");

    // 1. Шаг 2.1: Проверка маскирования токенов в выводе panic и logcat
    const rawMfa = "mfa." + "kL9_mN2-pQ5xR8vW1zB4cV7nM0qW3eR6tY9uI2oP5aS8dF1gH4jK7lZ0xC3vB6nM9qW2eR5tY8uI1oP4aS7d";
    const rawBotSecret = "Nzk5" + "MDEyMzQ1Njc4OTAxMjM0NTY" + ".X9kLmN." + "qR7vW2pL9xK4mN8bV1cZ5jH3gF6dS0aP2oI8uY";
    const rawUserToken = "OTg3" + "NjU0MzIxMDk4NzY1NDMyMQ" + ".G7pQrS." + "mK9vX2bN4lP8qW1zR5tY7uI3oP6aS9dF2gH5jK";

    const panicLogSample = [
      `thread 'tokio-runtime-worker' panicked at 'REST 401 Unauthorized: header Authorization="Bot ${rawBotSecret}", mfa_token="${rawMfa}", user_token="${rawUserToken}"', src/net/mod.rs:412:9`,
      `2026-11-11T15:40:00Z ERROR logcat::auth: login failed for token=${rawMfa} and Bot ${rawBotSecret}`,
    ].join("\n");

    const maskedLog = maskSensitiveLogOutput(panicLogSample);
    assert.ok(maskedLog.includes("mfa.***"), `В замаскированном выводе должна быть маска mfa.***: ${maskedLog}`);
    assert.ok(maskedLog.includes("Bot ***"), `В замаскированном выводе должна быть маска Bot ***: ${maskedLog}`);
    assert.ok(!maskedLog.includes(rawMfa), "Сырой MFA-токен должен быть полностью удалён из вывода panic/logcat");
    assert.ok(!maskedLog.includes(rawBotSecret), "Сырой Bot-токен должен быть полностью удалён из вывода panic/logcat");
    assert.ok(!maskedLog.includes(rawUserToken), "Сырой User-токен должен быть полностью удалён из вывода panic/logcat");

    // Граничный случай: пустая строка и лог без секретов не искажаются
    assert.equal(maskSensitiveLogOutput(""), "");
    const safeLine = "INFO gpui::window: frame rendered in 412us (120 FPS budget ok)";
    assert.equal(maskSensitiveLogOutput(safeLine), safeLine);

    // 2. Шаг 2.2: Проверка zeroize очистки буфера секретов в оперативной памяти перед деаллокацией
    const secretBytes = new Uint8Array([
      0xde, 0xad, 0xbe, 0xef, 0xca, 0xfe, 0xba, 0xbe,
      0x13, 0x37, 0x42, 0x99, 0xaa, 0x55, 0xff, 0x01,
    ]);
    const zeroResult = zeroizeBuffer(secretBytes);
    assert.equal(zeroResult.zeroizedBytes, 16);
    assert.equal(zeroResult.allZero, true);
    assert.ok(
      secretBytes.every((b) => b === 0),
      "Каждый байт буфера секрета должен быть перезаписан нулём (0x00)",
    );

    // 3. Статический и контрактный аудит Rust-исходников на zeroize_slice, Drop и Debug <redacted>
    const rustAudit = auditRustRedactionAndZeroize(ROOT);
    assert.equal(rustAudit.passed, true, `Сбой аудита zeroize/redaction: ${rustAudit.reasons.join("; ")}`);
    assert.equal(rustAudit.hasBlackBoxZeroize, true, "Должен использоваться black_box(0) в zeroize_slice");
    assert.equal(rustAudit.hasEpochSecretsDrop, true, "DaveMlsEpochSecrets должен очищать 8 секретов в Drop");
    assert.equal(rustAudit.hasHashRatchetDrop, true, "DaveHashRatchet должен очищать next_secret и кэш ключей в Drop");
    assert.ok(
      rustAudit.redactedDebugCount >= 25,
      `Ожидалось >= 25 полей с маскировкой <redacted>/<скрыто> в Debug, найдено: ${rustAudit.redactedDebugCount}`,
    );
  });

  it("SUB-956-3 (Шаг 3.1 & 3.2): генерирует валидный SBOM SPDX 2.3 (rust/dist/sbom.spdx.json) и проверяет лицензии всех 85+ зависимостей", async () => {
    const { verifySbomSpdx23Manifest } = await import("../../scripts/secret-scan.mjs");

    // 1. Шаг 3.1: Запуск node scripts/sbom.mjs --check --output rust/dist/sbom.spdx.json
    const sbomRun = spawnSync(
      process.execPath,
      [SBOM_SCRIPT, "--check", "--output", SBOM_PATH],
      { cwd: ROOT, encoding: "utf8" },
    );
    assert.equal(sbomRun.status, 0, `Ошибка генерации SBOM: ${sbomRun.stderr}`);
    assert.ok(existsSync(SBOM_PATH), "Манифест rust/dist/sbom.spdx.json должен существовать");

    // 2. Шаг 3.2: Валидация структуры SPDX 2.3 и лицензий всех 85+ зависимостей (фактически 98 пакетов)
    const sbomAudit = verifySbomSpdx23Manifest(SBOM_PATH);
    assert.equal(sbomAudit.passed, true, `Невалидный SBOM: ${sbomAudit.reasons.join("; ")}`);
    assert.equal(sbomAudit.spdxVersion, "SPDX-2.3");
    assert.equal(sbomAudit.dataLicense, "CC0-1.0");
    assert.equal(sbomAudit.spdxId, "SPDXRef-DOCUMENT");
    assert.ok(
      sbomAudit.packageCount >= 85,
      `Ожидалось >= 85 пакетов в SBOM, получено: ${sbomAudit.packageCount}`,
    );
    assert.equal(sbomAudit.rejectedPackages.length, 0, "Все пакеты должны иметь разрешённые открытые лицензии");
    assert.equal(sbomAudit.noAssertionPackages.length, 0, "Ни один пакет не должен иметь NOASSERTION в licenseDeclared");
    assert.ok(sbomAudit.describesCount >= 10, "SBOM должен содержать DESCRIBES-связи для workspace-крейтов");
    assert.ok(sbomAudit.dependsOnCount >= 85, "SBOM должен содержать граф DEPENDS_ON");

    // 3. Негативный тест валидатора SBOM: пакет с запрещённой лицензией GPL-3.0-only или неверной версией SPDX
    const tmp = mkdtempSync(join(tmpdir(), "sbom-neg-956-"));
    try {
      const badSbomPath = join(tmp, "bad-sbom.spdx.json");
      const validDoc = JSON.parse(readFileSync(SBOM_PATH, "utf8"));
      const mutatedDoc = {
        ...validDoc,
        packages: [
          ...validDoc.packages,
          {
            name: "copyleft-injected-crate",
            SPDXID: "SPDXRef-Package-copyleft-injected-crate-1.0.0",
            versionInfo: "1.0.0",
            downloadLocation: "NOASSERTION",
            filesAnalyzed: false,
            licenseDeclared: "GPL-3.0-only",
            licenseConcluded: "NOASSERTION",
            copyrightText: "NOASSERTION",
          },
        ],
      };
      writeFileSync(badSbomPath, JSON.stringify(mutatedDoc, null, 2));
      const badAudit = verifySbomSpdx23Manifest(badSbomPath);
      assert.equal(badAudit.passed, false);
      assert.ok(badAudit.rejectedPackages.includes("copyleft-injected-crate@1.0.0: GPL-3.0-only"));
    } finally {
      rmSync(tmp, { recursive: true, force: true });
    }
  });

  it("CLI и отчётность: запускает scripts/secret-scan.mjs и проверяет отчёт SECURITY-AUDIT-REPORT.md", () => {
    const tmp = mkdtempSync(join(tmpdir(), "sec-audit-956-"));
    try {
      const outJson = join(tmp, "security-audit.json");
      const outMd = join(tmp, "SECURITY-AUDIT-REPORT.md");

      const run = spawnSync(
        process.execPath,
        [SECRET_SCAN_SCRIPT, "--check", "--out-json", outJson, "--out-md", outMd],
        { cwd: ROOT, encoding: "utf8" },
      );
      assert.equal(run.status, 0, `stderr: ${run.stderr}`);
      assert.ok(existsSync(outJson), "JSON-сводка аудита безопасности должна быть создана");
      assert.ok(existsSync(outMd), "Markdown-отчёт аудита безопасности должен быть создан");

      const summary = JSON.parse(readFileSync(outJson, "utf8"));
      assert.equal(summary.taskId, "TASK-956");
      assert.equal(summary.status, "passed");
      assert.equal(summary.secretScan.findingsCount, 0);
      assert.ok(summary.secretScan.patternCount >= 20);
      assert.equal(summary.daveMlsAudit.passed, true);
      assert.equal(summary.maskingAndZeroize.passed, true);
      assert.equal(summary.sbom.passed, true);
      assert.equal(summary.sbom.spdxVersion, "SPDX-2.3");
      assert.ok(summary.sbom.packageCount >= 85);

      // Проверка --dry-run
      const dryRun = spawnSync(process.execPath, [SECRET_SCAN_SCRIPT, "--dry-run"], {
        cwd: ROOT,
        encoding: "utf8",
      });
      assert.equal(dryRun.status, 0);
      assert.match(dryRun.stdout, /SPDX-2\.3/);

      // Коды выхода CLI: 1 при неизвестном флаге, 2 при отсутствии SBOM-файла
      const badFlag = spawnSync(process.execPath, [SECRET_SCAN_SCRIPT, "--unknown-flag"], {
        cwd: ROOT,
        encoding: "utf8",
      });
      assert.equal(badFlag.status, 1);

      const missingFile = spawnSync(
        process.execPath,
        [SECRET_SCAN_SCRIPT, "--sbom", "/nonexistent/sbom-missing.spdx.json"],
        { cwd: ROOT, encoding: "utf8" },
      );
      assert.equal(missingFile.status, 2);

      // Проверка опубликованных отчётов SECURITY-AUDIT-REPORT.md
      for (const reportPath of [REPORT_RW44_PATH, REPORT_RW45_PATH]) {
        assert.ok(existsSync(reportPath), `Отчёт ${reportPath} должен существовать`);
        const md = readFileSync(reportPath, "utf8");
        assert.match(md, /TASK-956/);
        assert.match(md, /SPDX-2\.3/);
        assert.match(md, /mfa\.\*\*\*/);
        assert.match(md, /Bot \*\*\*/);
        assert.match(md, /zeroize_slice/);
        assert.match(md, /sbom\.spdx\.json/);
        assert.match(md, /0\s*(находок|утечек|leaks)/i);
      }
    } finally {
      rmSync(tmp, { recursive: true, force: true });
    }
  });
});
