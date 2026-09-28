import os
import sqlite3
from pathlib import Path


def attachment_path(guid, state):
    path = Path(state.get("messages_db", "~/Library/Messages/chat.db")).expanduser().resolve()
    try:
        with sqlite3.connect(path.as_uri() + "?mode=ro", uri=True, timeout=1) as connection:
            row = connection.execute(
                "SELECT filename FROM attachment WHERE guid = ?", (guid,)
            ).fetchone()
    except sqlite3.Error:
        return None
    if not row or not row[0]:
        return None
    filename = os.path.expanduser(row[0])
    return filename if Path(filename).is_file() else None
