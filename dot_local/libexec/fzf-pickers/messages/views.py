import re
import time
from collections import deque
from datetime import datetime
from itertools import islice
from pathlib import Path

from . import attachments, client


def normalize_address(address):
    address = address.strip()
    if "@" in address:
        return address.casefold()
    return re.sub(r"[\s().-]", "", address)


def message_row(message):
    sender = (
        "Me"
        if message.get("isFromMe")
        else (message.get("handle") or {}).get("address", "(unknown sender)")
    )
    raw_date = message.get("dateCreated")
    try:
        date = (
            datetime.fromtimestamp(float(raw_date) / 1000)
            .astimezone()
            .isoformat(timespec="seconds")
        )
    except (TypeError, ValueError, OverflowError):
        date = "(unknown date)"
    text = message.get("text") or ""
    attachments = message.get("attachments") or []
    summary = text or ("[attachments]" if attachments else "[message without text]")
    chats = message.get("chats") or []
    return {
        "id": message["guid"],
        "label": f"{date} {sender}: {summary}",
        "detail": f"{date}\n{sender}\n"
        + ", ".join(chat.get("guid", "") for chat in chats)
        + "\n\n"
        + text,
        "value": text,
        "action": "copy",
        "message_guid": message["guid"],
        "sender": sender,
        "date": date,
        "chat_guid": chats[0].get("guid") if chats else None,
        "attachments": attachments,
    }


def collect(kind, state):
    if kind == "messages":
        for target, label, detail in (
            ("chats", "Conversations", "Choose a conversation, then search its history."),
            (
                "message-history",
                "All messages",
                "Search normalized message text across conversations.",
            ),
            ("message-links", "Links", "Search links with their parent message and sender."),
            (
                "message-attachments",
                "Attachments",
                "Search attachments and prepare opening the original file.",
            ),
        ):
            yield {
                "id": target,
                "label": label,
                "detail": detail,
                "value": target,
                "action": "next",
                "next": target,
            }
        return
    if kind == "chats":
        addresses = {normalize_address(address) for address in state.get("contact_addresses", [])}
        for chat in client.paginate(
            "/chat/query", {"with": ["participants", "lastMessage"], "sort": "lastmessage"}, state
        ):
            participants = [person.get("address", "") for person in chat.get("participants", [])]
            if addresses and not addresses.intersection(
                normalize_address(person) for person in participants
            ):
                continue
            label = chat.get("displayName") or ", ".join(participants) or chat["guid"]
            last = chat.get("lastMessage") or {}
            yield {
                "id": chat["guid"],
                "label": label,
                "detail": label
                + "\n"
                + "\n".join(participants)
                + "\n\n"
                + (last.get("text") or ""),
                "value": chat["guid"],
                "chat_guid": chat["guid"],
                "action": "next",
                "next": "message-history",
                "state": {"chat_guid": chat["guid"]},
            }
        return
    if kind not in ("message-history", "message-links", "message-attachments"):
        raise RuntimeError(f"Unknown Messages picker: {kind}")
    before = state.get("messages_before") or int(time.time() * 1000)
    body = {
        "with": ["chats", "chats.participants", "attachments"],
        "sort": "DESC",
        "before": before,
        "convertAttachments": False,
    }
    if state.get("chat_guid"):
        body["chatGuid"] = state["chat_guid"]
    previous = deque(maxlen=2)
    source = iter(client.paginate("/message/query", body, state))
    upcoming = deque(islice(source, 3))
    while upcoming:
        message = upcoming.popleft()
        row = message_row(message)
        if not row["chat_guid"]:
            row["chat_guid"] = state.get("chat_guid")
        neighbors = [*previous, *(message_row(item) for item in upcoming)]
        context = "\n\n".join(
            f"{item['date']} {item['sender']}\n{item['value']}"
            for item in neighbors
            if (item["chat_guid"] or state.get("chat_guid")) == row["chat_guid"]
        )
        if kind == "message-history":
            if context:
                row["detail"] += "\n\nNearby messages in this conversation:\n" + context
            yield row
        elif kind == "message-links":
            for index, match in enumerate(re.finditer(r"https?://[^\s<>\uFFFC]+", row["value"])):
                url = match.group().rstrip(".,;!?)]}")
                yield {
                    **row,
                    "id": f"{row['id']}:{index}",
                    "label": f"{url}  {row['sender']}  {row['date']}",
                    "value": url,
                    "detail": url + "\n\n" + row["detail"],
                }
        else:
            for attachment in row["attachments"]:
                guid = attachment.get("guid")
                if not guid:
                    continue
                path = attachments.attachment_path(guid, state)
                name = attachment.get("transferName") or guid
                yield {
                    **row,
                    "id": f"{row['id']}:{guid}",
                    "label": f"{name}  {row['sender']}  {row['date']}",
                    "value": path or "",
                    "path": path,
                    "action": "command",
                    "argv": ["open", path] if path else [],
                    "detail": f"{name}\n{path or 'Original attachment unavailable or Messages database access denied.'}\n\n{row['detail']}",
                }
        previous.append(row)
        upcoming.extend(islice(source, 1))


def accept(kind, rows, key, state):
    if not rows:
        return None
    row = rows[0]
    if kind == "chats" and key == "enter":
        return {"type": "next", "kind": "message-history", "state": {"chat_guid": row["chat_guid"]}}
    if key == "alt-a" and kind in ("message-history", "message-links"):
        return {
            "type": "copy",
            "text": "\n\n".join(
                f"{item['date']} {item['sender']}\n{item['value']}" for item in rows
            ),
        }
    if key == "alt-m" and row.get("chat_guid"):
        return {"type": "next", "kind": "message-history", "state": {"chat_guid": row["chat_guid"]}}
    if kind == "message-attachments" and key in ("enter", "ctrl-o"):
        paths = [item.get("path") for item in rows]
        if not all(path and Path(path).is_file() for path in paths):
            raise RuntimeError(
                "Original attachment unavailable. Check Messages Full Disk Access and downloaded attachments."
            )
        return {"type": "command", "argv": ["open", *paths]}
    if key == "enter" and kind in ("message-history", "message-links"):
        return {"type": "copy", "text": "\n\n".join(item["value"] for item in rows)}
    return None


def bindings(kind, state):
    if kind in ("message-history", "message-links"):
        return {"alt-a": "Copy with date and sender", "alt-m": "Open conversation history"}
    return {}
