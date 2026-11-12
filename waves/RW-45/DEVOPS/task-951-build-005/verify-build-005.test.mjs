// @ts-check
/**
 * TASK-951: Контрактный тест централизованного манифеста сборки BUILD-005,
 * слияния рабочих веток в unstable, переноса unstable -> testing и тега build-005-candidate.
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { execSync } from "node:child_process";

const ROOT = "/home/user/orca/project";
const UNSTABLE_WT = "/home/user/orca/workspaces/project/unstable";
const TESTING_WT = "/home/user/orca/workspaces/project/testing";
const MANIFEST_PATH = path.join(ROOT, "waves/BUILD-005.json");
const KANBAN_PATH = path.join(ROOT, "waves/kanban-board.json");

/**
 * @param {string} cmd
 * @param {string} [cwd]
 * @returns {string}
 */
function sh(cmd, cwd = ROOT) {
  return execSync(cmd, { cwd, encoding: "utf8" }).trim();
}

describe("TASK-951: Централизованный манифест BUILD-005 и слияние unstable -> testing", () => {
  it("951.1: рабочая ветка feature/sprints-1-5-build-004 влита в unstable (--no-ff), рабочее дерево unstable чистое", () => {
    const statusLines = sh("git status --porcelain", UNSTABLE_WT)
      .split("\n")
      .map((l) => l.trim())
      .filter((l) => l.length > 0 && !l.endsWith("node_modules"));
    assert.deepEqual(
      statusLines,
      [],
      `Рабочее дерево unstable должно быть чистым, найдено: ${statusLines.join(", ")}`
    );

    const isAncestor = sh(
      "git merge-base --is-ancestor 6f60b11 unstable && echo yes || echo no",
      ROOT
    );
    assert.equal(
      isAncestor,
      "yes",
      "Ветка feature/sprints-1-5-build-004 (6f60b11) должна быть влита в unstable"
    );
  });

  it("951.2: файл waves/BUILD-005.json существует и соответствует JSON-схеме манифеста сборки", () => {
    assert.ok(
      fs.existsSync(MANIFEST_PATH),
      "Файл waves/BUILD-005.json должен существовать"
    );

    const raw = fs.readFileSync(MANIFEST_PATH, "utf8");
    const manifest = JSON.parse(raw);

    assert.equal(manifest.schema_version, "1.0");
    assert.equal(manifest.build_id, "BUILD-005");
    assert.equal(manifest.source_branch, "unstable");
    assert.equal(manifest.target_branch, "testing");
    assert.equal(manifest.candidate_tag, "build-005-candidate");

    assert.match(manifest.unstable_commit, /^[0-9a-f]{40}$/);
    assert.match(manifest.testing_commit, /^[0-9a-f]{40}$/);
    assert.ok(
      !Number.isNaN(Date.parse(manifest.built_at)),
      "built_at должен быть валидной ISO-8601 датой"
    );

    const unstableSha = sh("git rev-parse unstable", ROOT);
    assert.equal(
      manifest.unstable_commit,
      unstableSha,
      "unstable_commit в манифесте должен совпадать с SHA ветки unstable"
    );
  });

  it("951.2: манифест waves/BUILD-005.json содержит полный реестр всех 128 выполненных задач", () => {
    assert.ok(fs.existsSync(MANIFEST_PATH), "waves/BUILD-005.json отсутствует");
    const manifest = JSON.parse(fs.readFileSync(MANIFEST_PATH, "utf8"));
    const kb = JSON.parse(fs.readFileSync(KANBAN_PATH, "utf8"));
    const expectedIds = kb.cards
      .filter((/** @type {{ id: string }} */ c) => !c.id.startsWith("TASK-95"))
      .map((/** @type {{ id: string }} */ c) => c.id);

    assert.equal(manifest.total_tasks, 128, "total_tasks должен быть равен 128");
    assert.ok(Array.isArray(manifest.tasks), "tasks должен быть массивом");
    assert.equal(manifest.tasks.length, 128, "Массив tasks должен содержать ровно 128 задач");

    const seenIds = new Set();
    for (const task of manifest.tasks) {
      assert.match(task.id, /^TASK-\d+$/, `Некорректный ID задачи: ${task.id}`);
      assert.ok(!seenIds.has(task.id), `Дубликат задачи в манифесте: ${task.id}`);
      seenIds.add(task.id);

      assert.equal(typeof task.title, "string");
      assert.ok(task.title.length > 0, `Пустой title у задачи ${task.id}`);
      assert.equal(typeof task.department, "string");
      assert.ok(task.department.length > 0, `Пустой department у задачи ${task.id}`);
      assert.match(task.priority, /^P[0-3]$/, `Некорректный priority у задачи ${task.id}`);
      assert.equal(task.status, "done", `Задача ${task.id} должна иметь статус done`);
      assert.ok(
        typeof task.closed_at === "string" && !Number.isNaN(Date.parse(task.closed_at)),
        `Некорректная дата закрытия closed_at у задачи ${task.id}`
      );
      assert.match(
        task.commit,
        /^[0-9a-f]{7,40}$/,
        `Некорректный хеш коммита у задачи ${task.id}`
      );
    }

    assert.deepEqual(
      [...seenIds].sort(),
      [...expectedIds].sort(),
      "Реестр задач в BUILD-005.json должен 1:1 покрывать все 128 задач доски"
    );
  });

  it("951.3: ветка testing синхронизирована с unstable, легковесный тег build-005-candidate указывает на вершину testing", () => {
    const testingStatus = sh("git status --porcelain", TESTING_WT);
    assert.equal(
      testingStatus,
      "",
      `Рабочее дерево testing должно быть чистым без конфликтов, найдено: ${testingStatus}`
    );

    const isAncestor = sh(
      "git merge-base --is-ancestor unstable testing && echo yes || echo no",
      ROOT
    );
    assert.equal(
      isAncestor,
      "yes",
      "Ветка testing должна содержать все коммиты из unstable"
    );

    const tagRef = sh("git rev-parse --verify refs/tags/build-005-candidate", ROOT);
    const testingHead = sh("git rev-parse refs/heads/testing", ROOT);
    assert.equal(
      tagRef,
      testingHead,
      "Тег build-005-candidate должен указывать строго на вершину ветки testing"
    );

    const tagType = sh("git cat-file -t refs/tags/build-005-candidate", ROOT);
    assert.equal(
      tagType,
      "commit",
      "Тег build-005-candidate должен быть легковесным (lightweight tag -> commit)"
    );
  });
});
