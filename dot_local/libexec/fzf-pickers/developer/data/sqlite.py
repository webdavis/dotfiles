import sqlite3

import developer.rows as row_model


def sqlite_rows(path):
    uri = path.as_uri() + "?mode=ro"
    rows = []
    with sqlite3.connect(uri, uri=True, timeout=1) as connection:
        connection.execute("PRAGMA query_only = ON")
        schema = connection.execute(
            "SELECT type, name, tbl_name, sql FROM sqlite_schema WHERE name NOT LIKE 'sqlite_%' ORDER BY type, name"
        ).fetchall()
        for kind, name, table, definition in schema:
            quoted = '"' + name.replace('"', '""') + '"'
            detail = definition or name
            if kind in ("table", "view"):
                columns = connection.execute(
                    'SELECT name, type, "notnull", dflt_value, pk FROM pragma_table_info(?)',
                    (name,),
                ).fetchall()
                detail += "\n\nColumns:\n" + "\n".join(
                    f"{column[0]}  {column[1]}  {'NOT NULL' if column[2] else ''}  {'PRIMARY KEY' if column[4] else ''}"
                    for column in columns
                )
                indexes = [item[1] for item in schema if item[0] == "index" and item[2] == name]
                if indexes:
                    detail += "\n\nIndexes: " + ", ".join(indexes)
                query = f"SELECT * FROM {quoted} LIMIT 100;"
            else:
                query = (
                    "SELECT sql FROM sqlite_schema WHERE name = "
                    + "'"
                    + name.replace("'", "''")
                    + "';"
                )
            rows.append(
                row_model.command(
                    ("sqlite", str(path), kind, name),
                    f"{kind}  {name}",
                    ["sqlite3", "-readonly", "-header", "-column", str(path), query],
                    detail,
                    sql_name=name,
                    value=name,
                )
            )
    return rows
