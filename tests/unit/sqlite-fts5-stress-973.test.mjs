// @ts-check
/**
 * Контрактные и модульные тесты стресс-теста SQLite WAL + FTS5 (TASK-973):
 * - SUB-973-1: Инициализация SQLite в режиме PRAGMA journal_mode = WAL, таблицы messages, channels,
 *   виртуальные таблицы FTS5 (messages_fts, quick_switcher_fts) с токенизатором unicode61.
 * - SUB-973-2: Параллельная запись 100 000 сообщений в 10 потоков (worker_threads) с конкурентными
 *   транзакциями BEGIN IMMEDIATE и одновременным полнотекстовым поиском FTS5 MATCH без дедлоков.
 * - SUB-973-3: Проверка целостности базы (PRAGMA integrity_check = ok, PRAGMA quick_check = ok,
 *   FTS5 integrity-check, совпадение числа строк 100 000 в messages и messages_fts).
 * - SUB-973-4: 9 измерений тестирования (позитивные, негативные, граничные Snowflake 0/u64::MAX,
 *   длина 2000 символов, offset=9975, ошибки SQLITE_BUSY и откат транзакций, пустые запросы,
 *   HTTP 202 indexing state, права доступа VIEW_CHANNEL/AGE_GATED_SPACES, восстановление rebuild +
 *   wal_checkpoint, маскирование токенов и CLI коды выхода 0/1/2).
 */
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { DatabaseSync } from "node:sqlite";
import { describe, it } from "node:test";
import { fileURLToPath } from "node:url";

const ROOT = fileURLToPath(new URL("../../", import.meta.url));
const SCRIPT_PATH = join(ROOT, "scripts", "stress-fts5.mjs");

describe("TASK-973: Стресс-тест SQLite FTS5 при конкурентных транзакциях WAL (scripts/stress-fts5.mjs)", () => {
  it("SUB-973-1: инициализирует файловую БД SQLite в режиме WAL с таблицами messages, messages_fts и quick_switcher_fts", async () => {
    const {
      FTS5_STRESS_DEFAULT_CONFIG,
      DISCORD_SEARCH_IPC_CHANNELS,
      initFts5WalDatabase,
      verifyDatabaseIntegrity,
    } = await import("../../scripts/stress-fts5.mjs");

    assert.equal(FTS5_STRESS_DEFAULT_CONFIG.totalMessages, 100_000);
    assert.equal(FTS5_STRESS_DEFAULT_CONFIG.parallelThreads, 10);
    assert.equal(FTS5_STRESS_DEFAULT_CONFIG.messagesPerThread, 10_000);
    assert.equal(FTS5_STRESS_DEFAULT_CONFIG.maxMessageLength, 2_000);
    assert.equal(FTS5_STRESS_DEFAULT_CONFIG.maxSearchOffset, 9_975);
    assert.equal(FTS5_STRESS_DEFAULT_CONFIG.defaultSearchLimit, 25);
    assert.equal(FTS5_STRESS_DEFAULT_CONFIG.maxSnowflake, 18446744073709551615n);

    assert.deepEqual(DISCORD_SEARCH_IPC_CHANNELS, [
      "DISCORD_SEARCH_INDEX_DOMAINS",
      "DISCORD_SEARCH_CLEAR_INDEX",
      "DISCORD_SEARCH_DELETE_DOMAINS",
      "DISCORD_SEARCH_DELETE_ITEMS",
    ]);

    const tmp = mkdtempSync(join(tmpdir(), "fts5-init-973-"));
    try {
      const dbPath = join(tmp, "wal-init.db");
      const db = initFts5WalDatabase(dbPath);
      try {
        const check = verifyDatabaseIntegrity(db, { expectedMessages: 0 });
        assert.equal(check.passed, true);
        assert.equal(check.journalMode, "wal");
        assert.equal(check.integrityCheck, "ok");
        assert.equal(check.quickCheck, "ok");
        assert.equal(check.fts5IntegrityOk, true);
        assert.equal(check.messagesCount, 0);
        assert.equal(check.ftsCount, 0);
      } finally {
        db.close();
      }
    } finally {
      rmSync(tmp, { recursive: true, force: true });
    }
  });

  it("SUB-973-2: индексирует 100 000 сообщений в 10 параллельных потоков в SQLite WAL с конкурентным поиском FTS5 и PRAGMA integrity_check = ok", async () => {
    const { runFts5WalStressTest } = await import("../../scripts/stress-fts5.mjs");

    const tmp = mkdtempSync(join(tmpdir(), "fts5-100k-973-"));
    try {
      const dbPath = join(tmp, "stress-100k.db");
      const result = await runFts5WalStressTest({
        dbPath,
        totalMessages: 100_000,
        parallelThreads: 10,
        batchSize: 2_500,
        useWorkerThreads: true,
      });

      assert.equal(result.passed, true, `Ошибки стресс-теста: ${result.reasons.join("; ")}`);
      assert.equal(result.totalMessages, 100_000);
      assert.equal(result.parallelThreads, 10);
      assert.equal(result.insertedMessages, 100_000);
      assert.equal(result.indexedFtsRows, 100_000);
      assert.equal(result.deadlocks, 0, "Дедлоки при конкурентной записи в WAL недопустимы");
      assert.equal(result.failedTransactions, 0);
      assert.equal(result.integrity.journalMode, "wal");
      assert.equal(result.integrity.integrityCheck, "ok");
      assert.equal(result.integrity.quickCheck, "ok");
      assert.equal(result.integrity.fts5IntegrityOk, true);
      assert.equal(result.workers.length, 10);

      for (const w of result.workers) {
        assert.equal(w.inserted, 10_000);
        assert.ok(w.searchQueries > 0, `Поток ${w.workerId} должен выполнить параллельные FTS5-запросы`);
      }

      assert.ok(result.concurrentSearchQueries >= 40, "Конкурентные поисковые запросы FTS5 должны выполняться во время записи");
      assert.ok(result.searchVerification.keywordHits > 0, "Полнотекстовый поиск FTS5 по ключевому токену должен находить записи");
      assert.equal(result.searchVerification.keywordHits, 2_000); // 100 000 / 50 токенов = ровно 2000 совпадений
      assert.ok(result.throughputMsgsPerSec > 5_000, "Пропускная способность записи должна превышать 5 000 сообщ/сек");

      // Независимый оракул: прямое подключение к файлу SQLite через node:sqlite и проверка таблиц/индексов
      const oracleDb = new DatabaseSync(dbPath, { readOnly: true });
      try {
        const msgRow = oracleDb.prepare("SELECT COUNT(*) AS cnt FROM messages WHERE deleted = 0").get();
        const ftsRow = oracleDb.prepare("SELECT COUNT(*) AS cnt FROM messages_fts").get();
        const matchRow = oracleDb
          .prepare("SELECT COUNT(*) AS hits FROM messages_fts WHERE messages_fts MATCH ?")
          .get("token_42");
        const pragmaRow = oracleDb.prepare("PRAGMA integrity_check").get();
        assert.equal(Number(msgRow?.cnt), 100_000);
        assert.equal(Number(ftsRow?.cnt), 100_000);
        assert.equal(Number(matchRow?.hits), 2_000);
        assert.equal(pragmaRow?.integrity_check, "ok");
      } finally {
        oracleDb.close();
      }
    } finally {
      rmSync(tmp, { recursive: true, force: true });
    }
  });

  it("SUB-973-3: проверяет граничные значения (Snowflake 0 и u64::MAX, длина 2000 символов, пагинация offset=9975) и негативные входы", async () => {
    const {
      initFts5WalDatabase,
      insertMessageWithFts5,
      searchMessagesFts5,
      validateMessageInput,
      sanitizeFts5Query,
    } = await import("../../scripts/stress-fts5.mjs");

    const tmp = mkdtempSync(join(tmpdir(), "fts5-bounds-973-"));
    try {
      const dbPath = join(tmp, "bounds.db");
      const db = initFts5WalDatabase(dbPath);
      try {
        // 1. Граничные Snowflake ID: "0" (min) и "18446744073709551615" (u64::MAX)
        const content2000 = `${"А".repeat(1_977)} boundary_token_max2000`;
        assert.equal(content2000.length, 2_000);

        insertMessageWithFts5(db, {
          id: "0",
          channel_id: "ch-bounds",
          author_id: "0",
          author_name: "min_snowflake_user",
          content: "Минимальный Snowflake 0 boundary_token_zero",
          timestamp: 0,
        });

        insertMessageWithFts5(db, {
          id: "18446744073709551615",
          channel_id: "ch-bounds",
          author_id: "18446744073709551615",
          author_name: "max_snowflake_user",
          content: content2000,
          timestamp: 1_700_000_000_000,
        });

        const zeroRes = searchMessagesFts5(db, { query: "boundary_token_zero" });
        assert.equal(zeroRes.status, 200);
        assert.equal(zeroRes.totalHits, 1);
        assert.equal(zeroRes.messages[0]?.id, "0");

        const maxRes = searchMessagesFts5(db, { query: "boundary_token_max2000" });
        assert.equal(maxRes.status, 200);
        assert.equal(maxRes.totalHits, 1);
        assert.equal(maxRes.messages[0]?.id, "18446744073709551615");
        assert.equal(maxRes.messages[0]?.content.length, 2_000);

        // 2. Негативные сценарии валидации сообщений:
        // - Длина 2001 символ (> 2000)
        assert.throws(
          () =>
            validateMessageInput({
              id: "100",
              channel_id: "ch-bounds",
              author_id: "1",
              author_name: "user",
              content: "Б".repeat(2_001),
              timestamp: 1,
            }),
          /2000/,
        );

        // - Snowflake > u64::MAX (18446744073709551616) и отрицательный Snowflake
        assert.throws(
          () =>
            validateMessageInput({
              id: "18446744073709551616",
              channel_id: "ch-bounds",
              author_id: "1",
              author_name: "user",
              content: "overflow snowflake",
              timestamp: 1,
            }),
          /Snowflake/i,
        );
        assert.throws(
          () =>
            validateMessageInput({
              id: "-1",
              channel_id: "ch-bounds",
              author_id: "1",
              author_name: "user",
              content: "negative snowflake",
              timestamp: 1,
            }),
          /Snowflake/i,
        );

        // - Внутренний NUL-байт (\0) отвергается по контракту discord-store
        assert.throws(
          () =>
            insertMessageWithFts5(db, {
              id: "200",
              channel_id: "ch-bounds",
              author_id: "1",
              author_name: "user",
              content: "bad\0null",
              timestamp: 1,
            }),
          /NUL/i,
        );

        // 3. Граничная пагинация поиска Discord: offset = 9975, limit = 25 (offset + limit <= 10 000)
        const pageAtLimit = searchMessagesFts5(db, {
          query: "boundary_token_zero",
          offset: 9_975,
          limit: 25,
        });
        assert.equal(pageAtLimit.status, 200);
        assert.equal(pageAtLimit.messages.length, 0);
        assert.equal(pageAtLimit.totalHits, 1);

        // Превышение offset > 9975 или offset + limit > 10 000 возвращает ошибку валидации пагинации
        assert.throws(
          () =>
            searchMessagesFts5(db, {
              query: "boundary_token_zero",
              offset: 9_976,
              limit: 25,
            }),
          /offset/i,
        );

        // limit = 0 возвращает пустую страницу без падения
        const zeroLimit = searchMessagesFts5(db, {
          query: "boundary_token_zero",
          offset: 0,
          limit: 0,
        });
        assert.equal(zeroLimit.messages.length, 0);

        // 4. Негативные/враждебные FTS5 запросы (непарные кавычки, SQL-инъекции, операторы NEAR/AND/OR)
        for (const hostileQuery of [
          '"unbalanced quote',
          'test" OR 1=1 --',
          "NEAR(a b, 5",
          "*** () AND OR NOT ^ : ; DROP TABLE messages;",
          "привет\0мир",
        ]) {
          const sanitized = sanitizeFts5Query(hostileQuery);
          assert.ok(!sanitized.includes("\0"));
          const res = searchMessagesFts5(db, { query: hostileQuery });
          assert.equal(res.status, 200);
          assert.ok(Array.isArray(res.messages));
        }

        // 5. Пустые запросы (empty / whitespace)
        const emptyRes = searchMessagesFts5(db, { query: "   " });
        assert.equal(emptyRes.status, 200);
        assert.equal(emptyRes.totalHits, 0);
        assert.deepEqual(emptyRes.messages, []);
      } finally {
        db.close();
      }
    } finally {
      rmSync(tmp, { recursive: true, force: true });
    }
  });

  it("SUB-973-4: проверяет HTTP 202 indexing state, права доступа (VIEW_CHANNEL / AGE_GATED_SPACES), Quick Switcher FTS5, IPC, откат и маскирование секретов", async () => {
    const {
      PERMISSION_VIEW_CHANNEL,
      PERMISSION_READ_MESSAGE_HISTORY,
      initFts5WalDatabase,
      upsertChannel,
      insertMessageWithFts5,
      searchMessagesFts5,
      indexQuickSwitcherItems,
      searchQuickSwitcherFts5,
      handleSearchIpcCommand,
      rebuildAndCheckpointFts5,
      maskSensitiveTokens,
    } = await import("../../scripts/stress-fts5.mjs");

    const tmp = mkdtempSync(join(tmpdir(), "fts5-acl-973-"));
    try {
      const dbPath = join(tmp, "acl.db");
      const db = initFts5WalDatabase(dbPath);
      try {
        // 1. Загрузочное состояние индексации (HTTP 202 Accepted + retry_after_ms)
        const indexingRes = searchMessagesFts5(db, {
          query: "rust",
          indexingState: { indexing: true, retryAfterMs: 1_500 },
        });
        assert.equal(indexingRes.status, 202);
        assert.equal(indexingRes.indexing, true);
        assert.equal(indexingRes.retry_after_ms, 1_500);
        assert.deepEqual(indexingRes.messages, []);

        // 2. Права доступа: VIEW_CHANNEL | READ_MESSAGE_HISTORY и AGE_GATED_SPACES (nsfw)
        const requiredMask = PERMISSION_VIEW_CHANNEL | PERMISSION_READ_MESSAGE_HISTORY;
        upsertChannel(db, {
          id: "ch-public",
          guild_id: "g-1",
          name: "general",
          kind: 0,
          position: 0,
          required_permissions: String(requiredMask),
          nsfw: false,
        });
        upsertChannel(db, {
          id: "ch-secret",
          guild_id: "g-1",
          name: "admin-vault",
          kind: 0,
          position: 1,
          required_permissions: String(requiredMask | (1n << 3n)), // + ADMINISTRATOR
          nsfw: false,
        });
        upsertChannel(db, {
          id: "ch-nsfw",
          guild_id: "g-1",
          name: "age-gated-lounge",
          kind: 0,
          position: 2,
          required_permissions: String(requiredMask),
          nsfw: true,
        });

        insertMessageWithFts5(db, {
          id: "1001",
          channel_id: "ch-public",
          author_id: "u-1",
          author_name: "alice",
          content: "Общий отчёт project_omega опубликован",
          timestamp: 100,
        });
        insertMessageWithFts5(db, {
          id: "1002",
          channel_id: "ch-secret",
          author_id: "u-2",
          author_name: "bob",
          content: "Секретный отчёт project_omega только для админов",
          timestamp: 200,
        });
        insertMessageWithFts5(db, {
          id: "1003",
          channel_id: "ch-nsfw",
          author_id: "u-3",
          author_name: "carol",
          content: "Возрастной канал project_omega 18+",
          timestamp: 300,
        });

        // Обычный участник без прав администратора и без верификации возраста видит только ch-public
        const memberSearch = searchMessagesFts5(db, {
          query: "project_omega",
          userPermissions: requiredMask,
          ageVerified: false,
        });
        assert.equal(memberSearch.totalHits, 1);
        assert.equal(memberSearch.messages[0]?.channel_id, "ch-public");

        // Участник с подтверждённым возрастом видит ch-public и ch-nsfw (2 сообщения)
        const adultSearch = searchMessagesFts5(db, {
          query: "project_omega",
          userPermissions: requiredMask,
          ageVerified: true,
        });
        assert.equal(adultSearch.totalHits, 2);

        // 3. Quick Switcher FTS5 с префиксами (* серверы, #/! каналы, @ пользователи)
        indexQuickSwitcherItems(db, [
          { id: "g-1", kind: "server", name: "Rust Engineers", subtitle: "Guild" },
          { id: "ch-public", kind: "channel", name: "general-rust", subtitle: "Text Channel" },
          { id: "u-1", kind: "user", name: "rustacean_dev", subtitle: "Online" },
        ]);
        assert.equal(searchQuickSwitcherFts5(db, "*Rust").length, 1);
        assert.equal(searchQuickSwitcherFts5(db, "*Rust")[0]?.kind, "server");
        assert.equal(searchQuickSwitcherFts5(db, "#general").length, 1);
        assert.equal(searchQuickSwitcherFts5(db, "#general")[0]?.kind, "channel");
        assert.equal(searchQuickSwitcherFts5(db, "@rustacean").length, 1);
        assert.equal(searchQuickSwitcherFts5(db, "@rustacean")[0]?.kind, "user");

        // 4. IPC-команды поиска из /home/sanya/re-deep/backend-ipc.md (core/#4355)
        const delItemRes = handleSearchIpcCommand(db, "DISCORD_SEARCH_DELETE_ITEMS", {
          ids: ["1001"],
        });
        assert.equal(delItemRes.ok, true);
        assert.equal(delItemRes.deletedCount, 1);

        const afterDelSearch = searchMessagesFts5(db, {
          query: "project_omega",
          userPermissions: requiredMask,
          ageVerified: false,
        });
        assert.equal(afterDelSearch.totalHits, 0);

        // 5. Восстановление FTS5 индекса (rebuild + optimize + wal_checkpoint)
        // Искусственно удалим запись только из FTS5 и проверим, что rebuild восстанавливает синхронность с messages
        db.exec("DELETE FROM messages_fts");
        const recovery = rebuildAndCheckpointFts5(db);
        assert.equal(recovery.rebuiltRows, 2); // остались 1002 и 1003
        assert.equal(recovery.integrityCheck, "ok");

        // 6. Маскирование чувствительных токенов (Security)
        const rawLog = "Auth failed for mfa.abCDef1234567890_secret and Bot MTIzNDU2Nzg5MDEyMzQ1Njc4.GhIjKl.MnOpQrStUvWxYz123456";
        const masked = maskSensitiveTokens(rawLog);
        assert.ok(!masked.includes("abCDef1234567890_secret"));
        assert.ok(!masked.includes("MnOpQrStUvWxYz123456"));
        assert.match(masked, /mfa\.\*\*\*/);
        assert.match(masked, /Bot \*\*\*/);
      } finally {
        db.close();
      }
    } finally {
      rmSync(tmp, { recursive: true, force: true });
    }
  });

  it("SUB-973-5: запускает CLI scripts/stress-fts5.mjs, проверяет JSON/MD отчёты и коды выхода 0 / 1 / 2", () => {
    const tmp = mkdtempSync(join(tmpdir(), "fts5-cli-973-"));
    try {
      const dbPath = join(tmp, "cli-stress.db");
      const outJson = join(tmp, "stress-fts5-973.json");
      const outMd = join(tmp, "STRESS-FTS5-REPORT.md");

      const run = spawnSync(
        process.execPath,
        [
          SCRIPT_PATH,
          "--db",
          dbPath,
          "--messages",
          "100000",
          "--threads",
          "10",
          "--batch-size",
          "2500",
          "--out-json",
          outJson,
          "--out-md",
          outMd,
        ],
        { cwd: ROOT, encoding: "utf8" },
      );

      assert.equal(run.status, 0, `CLI завершился с ошибкой: ${run.stderr}`);
      assert.ok(existsSync(outJson), "JSON-отчёт должен быть создан");
      assert.ok(existsSync(outMd), "Markdown-отчёт должен быть создан");

      const summary = JSON.parse(readFileSync(outJson, "utf8"));
      assert.equal(summary.taskId, "TASK-973");
      assert.equal(summary.status, "passed");
      assert.equal(summary.totalMessages, 100_000);
      assert.equal(summary.parallelThreads, 10);
      assert.equal(summary.insertedMessages, 100_000);
      assert.equal(summary.indexedFtsRows, 100_000);
      assert.equal(summary.deadlocks, 0);
      assert.equal(summary.integrity.integrityCheck, "ok");

      const md = readFileSync(outMd, "utf8");
      assert.match(md, /TASK-973/);
      assert.match(md, /PRAGMA integrity_check/);
      assert.match(md, /100\s*000/);
      assert.match(md, /PASS/);

      // Код выхода 1 при неверном аргументе CLI
      const badArg = spawnSync(process.execPath, [SCRIPT_PATH, "--unknown-flag"], {
        cwd: ROOT,
        encoding: "utf8",
      });
      assert.equal(badArg.status, 1);

      // Код выхода 2 при недоступной среде (несуществующая директория с --require-existing-dir)
      const missingEnv = spawnSync(
        process.execPath,
        [SCRIPT_PATH, "--db", "/nonexistent-dir-973/stress.db", "--require-existing-dir"],
        { cwd: ROOT, encoding: "utf8" },
      );
      assert.equal(missingEnv.status, 2);
    } finally {
      rmSync(tmp, { recursive: true, force: true });
    }
  });
});
