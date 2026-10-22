#!/usr/bin/env python3
"""Независимый оракул файла БД `discord-store` (RW-31, W31b).

Читает **только сам файл** через stdlib `sqlite3` (Python 3, системная libsqlite3 3.54) и
печатает построчный отчёт `ключ=значение`, поля через `|`. Никакого доступа к Rust-коду и
никакого доверия к его утверждениям: схема берётся из `sqlite_master`/`PRAGMA table_info`,
данные — обычным SQL, а порядок и keyset-пагинация **перепроверяются в Python** (сортировка
кортежей и фильтр «строго меньше курсора»), результат сравнения — `order_matches` и
`keyset_matches`.

Формат полей:
  table=<имя>
  index=<таблица>|<имя>
  index_column=<индекс>|<seqno>|<колонка>
  column=<таблица>|<колонка>|<тип>|<notnull>|<pk>
  message=<id>|<channel_id>|<author_id>|<timestamp>|<edited|->|<flags>|<deleted>|<content_hex>
  channel=<id>|<guild_id|->|<name_hex>|<kind>|<position>
  kv=<key>|<value>

Запуск:
    python3 waves/RW-31/sqlite/oracle.py /path/to/store.db

Код выхода: 0 — отчёт напечатан; 1 — БД не читается или схема не соответствует v1;
2 — ошибка аргументов.
"""

import sqlite3
import sys

MESSAGE_COLUMNS = (
    "id",
    "channel_id",
    "author_id",
    "timestamp",
    "edited_timestamp",
    "flags",
    "deleted",
    "content",
)


def quote(name):
    """Идентификатор для PRAGMA (параметры там не поддерживаются)."""
    return '"' + name.replace('"', '""') + '"'


def emit(lines, key, value):
    lines.append("%s=%s" % (key, value))


def enc(text):
    return "" if text is None else str(text).encode("utf-8").hex()


def dec(row):
    return {
        "id": row[0],
        "channel_id": row[1],
        "author_id": row[2],
        "timestamp": row[3],
        "edited_timestamp": row[4],
        "flags": row[5],
        "deleted": row[6],
        "content": row[7],
    }


def report(conn, lines):
    emit(lines, "sqlite_version", sqlite3.sqlite_version)
    emit(lines, "user_version", conn.execute("PRAGMA user_version").fetchone()[0])
    emit(lines, "journal_mode", conn.execute("PRAGMA journal_mode").fetchone()[0].lower())
    emit(lines, "integrity_check", conn.execute("PRAGMA integrity_check").fetchone()[0])
    emit(lines, "foreign_key_violations", len(conn.execute("PRAGMA foreign_key_check").fetchall()))

    tables = [
        name
        for (name,) in conn.execute(
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name"
        )
    ]
    for name in tables:
        emit(lines, "table", name)
        for cid, column, kind, notnull, _default, pk in conn.execute(
            "PRAGMA table_info(%s)" % quote(name)
        ):
            emit(lines, "column", "%s|%s|%s|%d|%d" % (name, column, kind or "", notnull, pk))

    for name, table in conn.execute(
        "SELECT name, tbl_name FROM sqlite_master WHERE type = 'index' AND name NOT LIKE 'sqlite_%' ORDER BY name"
    ):
        emit(lines, "index", "%s|%s" % (table, name))
        for seqno, _cid, column in conn.execute("PRAGMA index_info(%s)" % quote(name)):
            emit(lines, "index_column", "%s|%d|%s" % (name, seqno, column))

    if "channels" in tables:
        for row in conn.execute(
            "SELECT id, guild_id, name, kind, position FROM channels ORDER BY id"
        ):
            emit(
                lines,
                "channel",
                "%s|%s|%s|%d|%d"
                % (row[0], "-" if row[1] is None else row[1], enc(row[2]), row[3], row[4]),
            )

    if "messages" in tables:
        cursor = conn.execute(
            "SELECT %s FROM messages ORDER BY timestamp DESC, id DESC" % ", ".join(MESSAGE_COLUMNS)
        )
        rows = [dec(row) for row in cursor]
        for row in rows:
            emit(
                lines,
                "message",
                "%s|%s|%s|%d|%s|%d|%d|%s"
                % (
                    row["id"],
                    row["channel_id"],
                    row["author_id"],
                    row["timestamp"],
                    "-" if row["edited_timestamp"] is None else row["edited_timestamp"],
                    row["flags"],
                    row["deleted"],
                    enc(row["content"]),
                ),
            )
        emit(
            lines,
            "duplicate_ids",
            conn.execute(
                "SELECT count(*) FROM (SELECT id FROM messages GROUP BY id HAVING count(*) > 1)"
            ).fetchone()[0],
        )
        # Независимая проверка порядка: SQL ORDER BY против сортировки кортежей в Python.
        expected = sorted(rows, key=lambda row: (row["timestamp"], row["id"]), reverse=True)
        emit(lines, "order_matches", int([r["id"] for r in rows] == [r["id"] for r in expected]))
        # Независимая проверка keyset-пагинации от самого нового сообщения.
        if expected:
            head = expected[0]
            keyset = [
                row_id
                for (row_id,) in conn.execute(
                    "SELECT id FROM messages WHERE deleted = 0"
                    " AND (timestamp < ? OR (timestamp = ? AND id < ?))"
                    " ORDER BY timestamp DESC, id DESC",
                    (head["timestamp"], head["timestamp"], head["id"]),
                )
            ]
            predicted = [
                row["id"]
                for row in expected
                if (row["timestamp"], row["id"]) < (head["timestamp"], head["id"])
            ]
            emit(lines, "keyset_matches", int(keyset == predicted))
            emit(lines, "keyset_size", len(keyset))
        else:
            emit(lines, "keyset_matches", 1)
            emit(lines, "keyset_size", 0)

    if "kv" in tables:
        for key, value in conn.execute("SELECT key, value FROM kv ORDER BY key"):
            emit(lines, "kv", "%s|%s" % (key, value))

    return tables


def main(argv):
    if len(argv) != 2:
        print("usage: oracle.py <db-path>", file=sys.stderr)
        return 2
    path = argv[1]
    try:
        conn = sqlite3.connect(path)
    except sqlite3.Error as error:
        print("не удалось открыть %s: %s" % (path, error), file=sys.stderr)
        return 1
    lines = []
    try:
        tables = report(conn, lines)
    except sqlite3.Error as error:
        print("ошибка чтения схемы/данных: %s" % error, file=sys.stderr)
        return 1
    finally:
        conn.close()

    missing = [name for name in ("channels", "messages", "kv") if name not in tables]
    if missing:
        print("нет таблиц схемы v1: %s" % ", ".join(missing), file=sys.stderr)
        return 1
    sys.stdout.write("\n".join(lines) + "\n")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
