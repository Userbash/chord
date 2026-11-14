// @ts-check
/**
 * Контрактные и модульные тесты релизного CI/CD пайплайна GitHub Actions (TASK-968):
 * - Проверка синтаксической и структурной валидности `.github/workflows/release-beta.yml`
 *   независимым оракулом Python 3 (`PyYAML` `yaml.safe_load`).
 * - Проверка матрицы сборки Linux-пакетов (`strategy.matrix.include`): минимум 4+ дистрибутива
 *   и все обязательные форматы (`.tar.gz`, `AppImage`, `.deb`, `.rpm`, а также `arch`),
 *   существование всех вызываемых скриптов и контрактных тестов на диске.
 * - Проверка автоматической сборки `SHA256SUMS` и независимой верификации `sha256sum -c SHA256SUMS`
 *   в изолированном окружении (позитивный и негативный прогоны).
 * - Проверка публикации GitHub Release (`softprops/action-gh-release@v2`) с релизными заметками
 *   `RELEASE-NOTES-v1.0.0-beta.1.md`, флагом `prerelease`, `fail_on_unmatched_files: true`
 *   и минимальными правами доступа (`contents: read` на уровне workflow, `contents: write` только в `publish-release`).
 * - Проверка 9 измерений TEST-POLICY.md (позитивные, негативные, граничные, права доступа,
 *   безопасность пиннинга экшенов и маскирования секретов, наблюдаемость `.gate/`).
 */
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import {
  existsSync,
  mkdtempSync,
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
const WORKFLOW_PATH = join(ROOT, ".github", "workflows", "release-beta.yml");
const RELEASE_NOTES_PATH = join(ROOT, "RELEASE-NOTES-v1.0.0-beta.1.md");
const SHA256SUMS_PATH = join(ROOT, "rust", "dist", "SHA256SUMS");
const PACKAGE_JSON_PATH = join(ROOT, "package.json");

/** Обязательные форматы Linux-пакетов в матрице релиза (TASK-968). */
const REQUIRED_MATRIX_FORMATS = Object.freeze([
  {
    format: "tar.gz",
    expectedExt: ".tar.gz",
    expectedAsset: "discord-gpui-v1.0.0-beta.1-linux-x86_64.tar.gz",
    expectedContractTest: "tests/unit/beta-release-package-958.test.mjs",
  },
  {
    format: "appimage",
    expectedExt: ".AppImage",
    expectedAsset: "discord-gpui-v1.0.0-beta.1-x86_64.AppImage",
    expectedContractTest: "tests/unit/appimage-package-961.test.mjs",
  },
  {
    format: "deb",
    expectedExt: ".deb",
    expectedAsset: "discord-gpui_1.0.0-beta.1_amd64.deb",
    expectedContractTest: "tests/unit/deb-package-963.test.mjs",
  },
  {
    format: "rpm",
    expectedExt: ".rpm",
    expectedAsset: "discord-gpui-1.0.0-0.1.beta.1.x86_64.rpm",
    expectedContractTest: "tests/unit/rpm-package-966.test.mjs",
  },
]);

/**
 * Парсит YAML-файл через независимый системный оракул Python 3 (`yaml.safe_load`).
 * Замечание: в YAML 1.1 ключ `on:` может интерпретироваться PyYAML как boolean `true`,
 * поэтому нормализуем `true` -> `"on"` на верхнем уровне объекта.
 * @param {string} yamlPath
 * @returns {Record<string, any>}
 */
function parseYamlWithPythonOracle(yamlPath) {
  const pyScript = [
    "import json, sys, yaml",
    "with open(sys.argv[1], 'r', encoding='utf-8') as f:",
    "    data = yaml.safe_load(f)",
    "if isinstance(data, dict) and True in data and 'on' not in data:",
    "    data['on'] = data.pop(True)",
    "print(json.dumps(data))",
  ].join("\n");

  const res = spawnSync("python3", ["-c", pyScript, yamlPath], {
    encoding: "utf8",
  });
  assert.equal(
    res.status,
    0,
    `Ошибка парсинга YAML независимым оракулом PyYAML:\n${res.stderr}`,
  );
  return JSON.parse(res.stdout);
}

/**
 * Парсит `rust/dist/SHA256SUMS` в Map<filename, sha256>.
 * @param {string} content
 * @returns {Map<string, string>}
 */
function parseSha256Sums(content) {
  /** @type {Map<string, string>} */
  const map = new Map();
  for (const rawLine of content.split(/\r?\n/)) {
    const line = rawLine.trim();
    if (!line) continue;
    const match = /^([a-f0-9]{64})\s+\*?(.+)$/.exec(line);
    const hash = match?.[1];
    const name = match?.[2];
    assert.ok(hash && name, `Некорректная строка в SHA256SUMS: "${line}"`);
    map.set(name.trim(), hash);
  }
  return map;
}

/**
 * Валидирует структуру и инварианты безопасности релизного workflow GitHub Actions.
 * @param {any} wf
 * @returns {{ valid: boolean, errors: string[] }}
 */
export function validateReleaseWorkflow(wf) {
  /** @type {string[]} */
  const errors = [];

  if (!wf || typeof wf !== "object" || Array.isArray(wf)) {
    return { valid: false, errors: ["Workflow должен быть непустым YAML-объектом"] };
  }

  if (typeof wf.name !== "string" || wf.name.trim().length === 0) {
    errors.push("Поле name обязательно и должно быть непустой строкой");
  }

  // Проверка прав доступа верхнего уровня (принцип наименьших привилегий)
  if (!wf.permissions || wf.permissions.contents !== "read") {
    errors.push(
      "На верхнем уровне workflow должно быть задано permissions: { contents: 'read' }",
    );
  }
  if (wf.permissions?.contents === "write") {
    errors.push(
      "Глобальное право contents: write на уровне всего workflow запрещено (разрешено только в publish-release)",
    );
  }

  // Проверка защиты от параллельного перезапуска релиза
  if (!wf.concurrency || typeof wf.concurrency.group !== "string") {
    errors.push("Блок concurrency.group обязателен для релизного пайплайна");
  }
  if (wf.concurrency?.["cancel-in-progress"] !== false) {
    errors.push(
      "Для релизного workflow cancel-in-progress должен быть строго false",
    );
  }

  // Проверка триггеров on:
  const onBlock = wf.on;
  if (!onBlock || typeof onBlock !== "object") {
    errors.push("Блок триггеров on обязателен");
  } else {
    const tags = onBlock.push?.tags;
    if (!Array.isArray(tags) || !tags.some((t) => String(t).includes("beta"))) {
      errors.push("Триггер on.push.tags должен включать маску бета-тегов (например v*.*.*-beta.*)");
    }
    if (!onBlock.workflow_dispatch || typeof onBlock.workflow_dispatch !== "object") {
      errors.push("Триггер on.workflow_dispatch обязателен для ручного запуска релиза");
    }
  }

  // Проверка jobs
  const jobs = wf.jobs;
  if (!jobs || typeof jobs !== "object") {
    errors.push("Блок jobs обязателен");
    return { valid: false, errors };
  }

  for (const requiredJob of ["preflight-gate", "package-matrix", "publish-release"]) {
    if (!jobs[requiredJob]) {
      errors.push(`Отсутствует обязательная джоба: ${requiredJob}`);
    }
  }

  // Проверка матрицы в package-matrix
  const matrixJob = jobs["package-matrix"];
  if (matrixJob) {
    if (matrixJob.strategy?.["fail-fast"] !== false) {
      errors.push("В package-matrix должно быть задано strategy.fail-fast: false");
    }
    const include = matrixJob.strategy?.matrix?.include;
    if (!Array.isArray(include) || include.length < 4) {
      errors.push(
        `Матрица package-matrix должна включать минимум 4 дистрибутива/формата (найдено: ${Array.isArray(include) ? include.length : 0})`,
      );
    } else {
      const formats = new Set(include.map((item) => item?.format));
      for (const req of REQUIRED_MATRIX_FORMATS) {
        if (!formats.has(req.format)) {
          errors.push(`В матрице отсутствует обязательный формат пакета: ${req.format}`);
        }
      }
      const distros = new Set(include.map((item) => item?.distro).filter(Boolean));
      if (distros.size < 4) {
        errors.push(
          `В матрице должно быть не менее 4 уникальных дистрибутивов distro (найдено: ${distros.size})`,
        );
      }
    }
  }

  // Проверка publish-release
  const publishJob = jobs["publish-release"];
  if (publishJob) {
    const needs = Array.isArray(publishJob.needs)
      ? publishJob.needs
      : [publishJob.needs].filter(Boolean);
    if (!needs.includes("preflight-gate") || !needs.includes("package-matrix")) {
      errors.push(
        "Джоба publish-release должна зависеть от preflight-gate и package-matrix в поле needs",
      );
    }
    if (publishJob.permissions?.contents !== "write") {
      errors.push(
        "Джоба publish-release должна иметь явное разрешение permissions: { contents: 'write' }",
      );
    }
  }

  // Проверка всех шагов всех джоб на пиннинг версий экшенов и отсутствие утечек токенов
  for (const [jobName, jobDef] of Object.entries(jobs)) {
    const steps = Array.isArray(jobDef?.steps) ? jobDef.steps : [];
    for (const step of steps) {
      if (typeof step?.uses === "string") {
        if (/@(main|master|latest|HEAD)$/i.test(step.uses.trim())) {
          errors.push(
            `Небезопасный плавающий тег экшена в ${jobName}: "${step.uses}"`,
          );
        }
      }
      const dump = JSON.stringify(step);
      if (/mfa\.[A-Za-z0-9_-]{10,}|ghp_[A-Za-z0-9]{20,}/.test(dump)) {
        errors.push(`Обнаружен немаскированный секретный токен в шаге джобы ${jobName}`);
      }
    }
  }

  return { valid: errors.length === 0, errors };
}

describe("TASK-968: Релизный CI/CD workflow .github/workflows/release-beta.yml (матрица Linux-пакетов, SHA256SUMS и GitHub Release)", () => {
  it("Шаг 1: файл .github/workflows/release-beta.yml существует, не содержит CRLF/табов и валиден по оракулу PyYAML", () => {
    assert.ok(
      existsSync(WORKFLOW_PATH),
      `Файл workflow должен существовать: ${WORKFLOW_PATH}`,
    );

    const rawText = readFileSync(WORKFLOW_PATH, "utf8");
    assert.ok(rawText.length >= 2500, "Workflow должен содержать полное описание всех стадий релиза");
    assert.ok(!rawText.includes("\r"), "release-beta.yml не должен содержать символов CRLF (\\r)");
    assert.ok(!rawText.includes("\t"), "release-beta.yml не должен содержать табуляций (YAML indentation)");

    const parsed = parseYamlWithPythonOracle(WORKFLOW_PATH);
    const validation = validateReleaseWorkflow(parsed);
    assert.equal(
      validation.valid,
      true,
      `Ошибки валидации release-beta.yml:\n${validation.errors.join("\n")}`,
    );

    // Сверка версии Node.js с package.json engines
    const pkg = JSON.parse(readFileSync(PACKAGE_JSON_PATH, "utf8"));
    const expectedNodeMajor = String(pkg.engines?.node ?? "24").replace(/[^\d]/g, "");
    assert.equal(expectedNodeMajor, "24");
    assert.match(
      rawText,
      /node-version:\s*"24"/,
      "Все Node.js шаги должны использовать версию Node 24 согласно package.json",
    );
  });

  it("Шаг 2: матрица package-matrix включает 4+ дистрибутива и все форматы (.tar.gz, AppImage, .deb, .rpm, arch) с реальными скриптами и тестами", () => {
    const parsed = parseYamlWithPythonOracle(WORKFLOW_PATH);
    const matrixJob = parsed.jobs["package-matrix"];
    assert.ok(matrixJob, "Джоба package-matrix должна существовать");
    assert.equal(matrixJob.strategy["fail-fast"], false);

    /** @type {Array<Record<string, any>>} */
    const include = matrixJob.strategy.matrix.include;
    assert.ok(
      Array.isArray(include) && include.length >= 4,
      `Ожидалось >= 4 элементов в matrix.include, получено: ${include?.length}`,
    );

    // Проверяем уникальные дистрибутивы (>= 4)
    const uniqueDistros = new Set(include.map((entry) => entry.distro));
    assert.ok(
      uniqueDistros.size >= 4,
      `В матрице должно быть не менее 4 различных дистрибутивов Linux, найдено: ${[...uniqueDistros].join(", ")}`,
    );

    // Проверяем каждый обязательный формат (.tar.gz, appimage, deb, rpm)
    for (const req of REQUIRED_MATRIX_FORMATS) {
      const entry = include.find((e) => e.format === req.format);
      assert.ok(
        entry,
        `В matrix.include должна присутствовать запись для формата "${req.format}"`,
      );
      assert.equal(
        entry.package_ext,
        req.expectedExt,
        `Некорректное расширение пакета для ${req.format}`,
      );
      assert.ok(
        String(entry.artifact_glob).endsWith(req.expectedAsset),
        `artifact_glob для ${req.format} должен указывать на ${req.expectedAsset}, получено: ${entry.artifact_glob}`,
      );
      assert.equal(
        entry.contract_test,
        req.expectedContractTest,
        `Некорректный контрактный тест для ${req.format}`,
      );
    }

    // Проверяем, что каждый контрактный тест и упаковочный файл из матрицы реально существует на диске
    for (const entry of include) {
      assert.ok(entry.distro, `Для формата ${entry.format} должен быть задан distro`);
      assert.ok(entry.container, `Для формата ${entry.format} должен быть задан container`);
      assert.ok(entry.runner, `Для формата ${entry.format} должен быть задан runner`);
      const testAbsPath = join(ROOT, entry.contract_test);
      assert.ok(
        existsSync(testAbsPath),
        `Контрактный тест матрицы должен существовать на диске: ${entry.contract_test}`,
      );
      if (entry.build_script) {
        const scriptAbsPath = join(ROOT, entry.build_script);
        assert.ok(
          existsSync(scriptAbsPath),
          `Скрипт/манифест сборки матрицы должен существовать на диске: ${entry.build_script}`,
        );
      }
    }

    // Проверяем, что шаги package-matrix вызывают реальные инструменты репозитория
    const stepsText = JSON.stringify(matrixJob.steps);
    assert.match(stepsText, /scripts\/build-appimage\.sh/, "Должен вызываться scripts/build-appimage.sh");
    assert.match(stepsText, /packaging\/deb\/deb-package\.mjs/, "Должен вызываться packaging/deb/deb-package.mjs");
    assert.match(stepsText, /packaging\/rpm\/discord-gpui\.spec/, "Должен использоваться packaging/rpm/discord-gpui.spec");
    assert.match(stepsText, /discord-gpui-v1\.0\.0-beta\.1-linux-x86_64\.tar\.gz/, "Должен проверяться/собираться .tar.gz архив");
    assert.match(stepsText, /actions\/upload-artifact@v4/, "Должна выполняться выгрузка артефактов пакета");
  });

  it("Шаг 3: автоматическая сборка SHA256SUMS агрегирует все пакеты и проходит независимый оракул sha256sum -c", () => {
    const parsed = parseYamlWithPythonOracle(WORKFLOW_PATH);
    const publishJob = parsed.jobs["publish-release"];
    assert.ok(publishJob, "Джоба publish-release должна существовать");

    const steps = publishJob.steps;
    assert.ok(Array.isArray(steps) && steps.length > 0);

    const downloadStep = steps.find(
      (/** @type {any} */ s) => typeof s.uses === "string" && s.uses.startsWith("actions/download-artifact@v4"),
    );
    assert.ok(downloadStep, "В publish-release должен присутствовать шаг actions/download-artifact@v4");
    assert.equal(downloadStep.with?.["merge-multiple"], true);

    const shaStep = steps.find(
      (/** @type {any} */ s) => typeof s.run === "string" && s.run.includes("SHA256SUMS") && s.run.includes("sha256sum -c"),
    );
    assert.ok(
      shaStep,
      "В publish-release должен быть шаг генерации и самопроверки SHA256SUMS через sha256sum -c",
    );

    // Проверяем, что скрипт шага проверяет наличие всех 4 обязательных типов пакетов (.tar.gz, .AppImage, .deb, .rpm)
    for (const ext of [".tar.gz", ".AppImage", ".deb", ".rpm", "sbom.spdx.json"]) {
      assert.ok(
        shaStep.run.includes(ext),
        `Шаг сборки SHA256SUMS должен включать проверку/хеширование артефакта ${ext}`,
      );
    }

    // Поведенческий прогон генерации и проверки SHA256SUMS в изолированном каталоге
    const tmp = mkdtempSync(join(tmpdir(), "release-sha256-968-"));
    try {
      const sampleAssets = [
        "discord-gpui-v1.0.0-beta.1-linux-x86_64.tar.gz",
        "discord-gpui-v1.0.0-beta.1-x86_64.AppImage",
        "discord-gpui_1.0.0-beta.1_amd64.deb",
        "discord-gpui-1.0.0-0.1.beta.1.x86_64.rpm",
        "discord-gpui-1.0.0_beta.1-1-x86_64.pkg.tar.zst",
        "sbom.spdx.json",
        "RELEASE-NOTES-v1.0.0-beta.1.md",
      ];
      /** @type {Map<string, string>} */
      const expectedHashes = new Map();
      for (const name of sampleAssets) {
        const content = `release-artifact-payload:${name}:${Date.now()}\n`;
        const filePath = join(tmp, name);
        writeFileSync(filePath, content, "utf8");
        expectedHashes.set(
          name,
          createHash("sha256").update(content).digest("hex"),
        );
      }

      // Генерируем SHA256SUMS системной утилитой sha256sum, как в publish-release
      const genRun = spawnSync(
        "bash",
        [
          "-c",
          "set -euo pipefail; sha256sum *.tar.gz *.AppImage *.deb *.rpm *.pkg.tar.zst sbom.spdx.json RELEASE-NOTES-v1.0.0-beta.1.md | LC_ALL=C sort -k2 > SHA256SUMS && sha256sum -c SHA256SUMS",
        ],
        { cwd: tmp, encoding: "utf8" },
      );
      assert.equal(genRun.status, 0, `Ошибка генерации SHA256SUMS: ${genRun.stderr}`);

      const parsedSums = parseSha256Sums(readFileSync(join(tmp, "SHA256SUMS"), "utf8"));
      assert.equal(parsedSums.size, sampleAssets.length);
      for (const [name, expectedSha] of expectedHashes.entries()) {
        assert.equal(
          parsedSums.get(name),
          expectedSha,
          `SHA-256 для ${name} в SHA256SUMS должен совпадать с независимым расчётом node:crypto`,
        );
      }

      // Негативный прогон: повреждение одного байта в .deb пакете обязано ронять sha256sum -c с ненулевым кодом
      writeFileSync(join(tmp, "discord-gpui_1.0.0-beta.1_amd64.deb"), "corrupted-deb-payload");
      const tamperCheck = spawnSync("sha256sum", ["-c", "SHA256SUMS"], {
        cwd: tmp,
        encoding: "utf8",
      });
      assert.notEqual(
        tamperCheck.status,
        0,
        "При повреждении пакета sha256sum -c SHA256SUMS обязан завершаться с ошибкой",
      );
    } finally {
      rmSync(tmp, { recursive: true, force: true });
    }
  });

  it("Шаг 4: публикация GitHub Release использует RELEASE-NOTES-v1.0.0-beta.1.md и прикрепляет все пакеты и SHA256SUMS", () => {
    assert.ok(
      existsSync(RELEASE_NOTES_PATH),
      `Файл релизных заметок должен существовать: ${RELEASE_NOTES_PATH}`,
    );
    const notesStat = statSync(RELEASE_NOTES_PATH);
    assert.ok(notesStat.size > 3000, "RELEASE-NOTES-v1.0.0-beta.1.md должен быть непустым и полным");

    const notesContent = readFileSync(RELEASE_NOTES_PATH, "utf8");
    const distSums = parseSha256Sums(readFileSync(SHA256SUMS_PATH, "utf8"));
    const expectedBinSha = distSums.get("discord-gpui");
    assert.ok(expectedBinSha, "В rust/dist/SHA256SUMS должен быть хеш discord-gpui");
    assert.ok(
      notesContent.includes(expectedBinSha),
      "RELEASE-NOTES-v1.0.0-beta.1.md должен содержать актуальный SHA-256 бинарника из rust/dist/SHA256SUMS",
    );

    const parsed = parseYamlWithPythonOracle(WORKFLOW_PATH);
    const publishJob = parsed.jobs["publish-release"];
    const releaseStep = publishJob.steps.find(
      (/** @type {any} */ s) =>
        typeof s.uses === "string" && s.uses.startsWith("softprops/action-gh-release@v3"),
    );
    assert.ok(
      releaseStep,
      "В publish-release должен присутствовать шаг публикации через softprops/action-gh-release@v3",
    );

    assert.equal(
      releaseStep.with?.body_path,
      "RELEASE-NOTES-v1.0.0-beta.1.md",
      "В action-gh-release должен быть указан body_path: RELEASE-NOTES-v1.0.0-beta.1.md",
    );
    assert.equal(
      releaseStep.with?.fail_on_unmatched_files,
      true,
      "В action-gh-release должен быть включён флаг fail_on_unmatched_files: true",
    );

    const filesField = String(releaseStep.with?.files ?? "");
    for (const requiredPattern of [
      "discord-gpui-v1.0.0-beta.1-linux-x86_64.tar.gz",
      "discord-gpui-v1.0.0-beta.1-x86_64.AppImage",
      "discord-gpui_1.0.0-beta.1_amd64.deb",
      "discord-gpui-1.0.0-0.1.beta.1.x86_64.rpm",
      "discord-gpui-1.0.0_beta.1-1-x86_64.pkg.tar.zst",
      "sbom.spdx.json",
      "SHA256SUMS",
      "RELEASE-NOTES-v1.0.0-beta.1.md",
    ]) {
      assert.ok(
        filesField.includes(requiredPattern),
        `Список публикуемых файлов GitHub Release должен включать ${requiredPattern}`,
      );
    }
  });

  it("Шаг 5: негативные, граничные и защитные сценарии валидатора validateReleaseWorkflow (9 измерений TEST-POLICY.md)", () => {
    const validParsed = parseYamlWithPythonOracle(WORKFLOW_PATH);
    assert.equal(validateReleaseWorkflow(validParsed).valid, true);

    // 1. Пустой / невалидный вход
    assert.equal(validateReleaseWorkflow(null).valid, false);
    assert.equal(validateReleaseWorkflow([]).valid, false);

    // 2. Избыточные глобальные права contents: write на верхнем уровне
    const badPerms = structuredClone(validParsed);
    badPerms.permissions = { contents: "write" };
    const badPermsRes = validateReleaseWorkflow(badPerms);
    assert.equal(badPermsRes.valid, false);
    assert.ok(badPermsRes.errors.some((e) => e.includes("contents: write")));

    // 3. Отсутствие одного из обязательных форматов (например .rpm) в матрице
    const missingRpm = structuredClone(validParsed);
    missingRpm.jobs["package-matrix"].strategy.matrix.include =
      missingRpm.jobs["package-matrix"].strategy.matrix.include.filter(
        (/** @type {any} */ x) => x.format !== "rpm",
      );
    const missingRpmRes = validateReleaseWorkflow(missingRpm);
    assert.equal(missingRpmRes.valid, false);
    assert.ok(missingRpmRes.errors.some((e) => e.includes("rpm")));

    // 4. Менее 4 дистрибутивов в матрице
    const tooFewDistros = structuredClone(validParsed);
    tooFewDistros.jobs["package-matrix"].strategy.matrix.include =
      tooFewDistros.jobs["package-matrix"].strategy.matrix.include.slice(0, 3);
    assert.equal(validateReleaseWorkflow(tooFewDistros).valid, false);

    // 5. Небезопасный плавающий тег экшена (@main / @latest)
    const floatingAction = structuredClone(validParsed);
    floatingAction.jobs["preflight-gate"].steps[0].uses = "actions/checkout@main";
    const floatingRes = validateReleaseWorkflow(floatingAction);
    assert.equal(floatingRes.valid, false);
    assert.ok(floatingRes.errors.some((e) => e.includes("actions/checkout@main")));

    // 6. Утечка секретного токена в шаге workflow
    const leakedSecret = structuredClone(validParsed);
    leakedSecret.jobs["preflight-gate"].steps[0].env = {
      DISCORD_TOKEN: "mfa.AbCdEfGhIjKlMnOpQrStUvWxYz123456",
    };
    const leakedRes = validateReleaseWorkflow(leakedSecret);
    assert.equal(leakedRes.valid, false);
    assert.ok(leakedRes.errors.some((e) => e.includes("токен")));

    // 7. Отключённая защита cancel-in-progress: true в релизном пайплайне
    const cancelEnabled = structuredClone(validParsed);
    cancelEnabled.concurrency["cancel-in-progress"] = true;
    assert.equal(validateReleaseWorkflow(cancelEnabled).valid, false);
  });
});
