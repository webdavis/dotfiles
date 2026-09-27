import json
import sqlite3

import developer as dev


def test_json_values_and_expressions(cwd, state):
    data = cwd / "odd name.json"
    data.write_text(json.dumps({"a.b": {'quo"te': [None, False, "hello\nworld"]}}))
    rows = dev.collect("json", dict(state, file=str(data)))
    leaf = next(row for row in rows if row.get("json_path") == ["a.b", 'quo"te', 2])
    assert leaf["value"] == "hello\nworld"
    assert dev.accept("json", [leaf], "alt-j", state)["text"] == 'getpath(["a.b","quo\\"te",2])'
    assert any(row["value"] == "null" for row in rows)


def test_sqlite_readonly_schema_and_file_selection(cwd, state):
    db = cwd / "space.sqlite"
    with sqlite3.connect(db) as connection:
        connection.execute('CREATE TABLE "say""hi" ("a b" TEXT, n INTEGER)')
        connection.execute('CREATE INDEX named_idx ON "say""hi" (n)')
    before = db.read_bytes()
    rows = dev.collect("sqlite", dict(state, file=str(db)))
    table = next(row for row in rows if row.get("sql_name") == 'say"hi')
    assert "a b" in table["detail"] and "named_idx" in table["detail"]
    assert table["argv"][-1] == 'SELECT * FROM "say""hi" LIMIT 100;'
    assert db.read_bytes() == before
    assert dev.collect("sqlite", state)[0]["next"] == "files"
