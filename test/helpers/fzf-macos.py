import io
import sys
from pathlib import Path
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
MODULES = ROOT / "dot_local/libexec/fzf-pickers"
sys.path.insert(0, str(MODULES))
import macos
import messages
from macos import bridge, contacts, discovery
from messages import attachments as message_attachments
from messages import client, views

person = {
    "id": "contact-1",
    "name": "Zoë Doe",
    "fields": [
        {"id": "name", "group": "name", "label": "Name", "value": "Zoë Doe"},
        {"id": "p1", "group": "phone", "label": "mobile", "value": "+1 (202) 555-0100"},
        {"id": "p2", "group": "phone", "label": "work", "value": "+44 20 7946 0000"},
        {"id": "e1", "group": "email", "label": "home", "value": "Zoe@example.com"},
        {"id": "empty", "group": "work", "label": "Department", "value": ""},
        {"id": "note", "group": "other", "label": "Notes", "value": "Two\nlines"},
    ],
}
row = contacts.contact_row(person, {})
assert (
    row["value"]
    == "Name: Zoë Doe\nmobile: +1 (202) 555-0100\nwork: +44 20 7946 0000\nhome: Zoe@example.com\nNotes: Two\nlines"
)
assert (
    contacts.contact_row(person, {"contact_mode": "phone"})["value"]
    == "+1 (202) 555-0100\n+44 20 7946 0000"
)
assert macos.accept("contacts", [row], "ctrl-t", {})["kind"] == "macos-contact-call"
assert macos.accept("contacts", [row], "alt-3", {}) == {
    "type": "reload",
    "state": {"contact_mode": "email"},
}
assert macos.accept("contacts", [row], "alt-7", {})["state"]["contact"]["id"] == "contact-1"
assert len(list(macos.collect("contact-fields", {"contact": person}))) == 5
assert macos.accept("macos-contact-call", [{"value": "+1 (202) 555-0100"}], "enter", {}) == {
    "type": "open",
    "url": "tel:+12025550100",
}
assert views.normalize_address("Zoe@EXAMPLE.com") == "zoe@example.com"
assert views.normalize_address("+1 (202) 555-0100") == "+12025550100"
assert views.normalize_address("020 7946 0000") != views.normalize_address("+44 20 7946 0000")

calls = []


def pages(path, body, state):
    calls.append(dict(body))
    return {
        "data": [{"guid": "a", "text": "first"}, {"guid": "b", "text": "second"}]
        if body["offset"] == 0
        else [],
        "metadata": {},
    }


with patch.object(client, "request", pages):
    assert [m["guid"] for m in client.paginate("/message/query", {}, {}, limit=2)] == ["a", "b"]
assert [c["offset"] for c in calls] == [0, 2]


def repeated(path, body, state):
    return {"data": [{"guid": "same"}]}


with patch.object(client, "request", repeated):
    try:
        list(client.paginate("/message/query", {}, {}, limit=1))
    except RuntimeError as exc:
        assert "advance" in str(exc)
    else:
        raise AssertionError("repeating pages must not loop")
message = {
    "guid": "m1",
    "text": "Visit https://example.com/a?q=1",
    "dateCreated": 1700000000000,
    "handle": {"address": "friend@example.com"},
    "chats": [{"guid": "chat1"}],
    "attachments": [{"guid": "att1", "transferName": "résumé.pdf"}],
}
r = views.message_row(message)
assert r["value"] == message["text"] and "friend@example.com" in r["detail"]
assert messages.accept("message-history", [r], "alt-a", {})["text"].endswith(message["text"])
with patch.object(client, "paginate", return_value=iter([message])):
    links = list(messages.collect("message-links", {}))
assert links[0]["value"] == "https://example.com/a?q=1" and links[0]["message_guid"] == "m1"
with (
    patch.object(client, "paginate", return_value=iter([message])),
    patch.object(message_attachments, "attachment_path", return_value=None),
):
    attachments = list(messages.collect("message-attachments", {}))
assert "unavailable" in attachments[0]["detail"].lower()
try:
    messages.accept("message-attachments", attachments, "enter", {})
except RuntimeError as exc:
    assert "unavailable" in str(exc).lower()
else:
    raise AssertionError("missing attachments cannot be opened")
with patch.object(
    bridge, "run", return_value="A shortcut (12345678-1234-1234-1234-123456789abc)\n"
):
    shortcuts = list(macos.collect("shortcuts", {}))
assert shortcuts[0]["argv"] == ["shortcuts", "run", "12345678-1234-1234-1234-123456789abc"]
assert macos.accept("tabs", [], "alt-B", {}) == {"type": "reload", "state": {"browser": "chrome"}}
assert macos.accept("tabs", [], "alt-B", {"browser": "chrome"})["state"]["browser"] == "arc"
with (
    patch.object(discovery.Path, "is_file", return_value=True),
    patch.object(discovery.Path, "stat") as stat,
):
    stat.return_value.st_mtime = 100

    def metadata(argv, **kwargs):
        if argv[0] == "mdfind":
            return b"/tmp/report.pdf\0"
        assert argv == ["mdls", "-plist", "-", "/tmp/report.pdf"], argv
        return discovery.plistlib.dumps({"kMDItemKind": "PDF"})

    with patch.object(bridge, "run", side_effect=metadata):
        recent = list(macos.collect("recent", {"recent_scopes": ["/tmp"]}))
    assert recent[0]["argv"] == ["open", "/tmp/report.pdf"]
with patch.object(
    client,
    "paginate",
    return_value=iter([{**message, "guid": str(i), "text": str(i)} for i in range(4)]),
):
    history = list(messages.collect("message-history", {}))
assert "Nearby messages" in history[0]["detail"] and history[0]["detail"].endswith("2")
assert history[3]["detail"].count("Nearby messages") == 1
with patch.object(bridge, "native", return_value=True):
    assert macos.accept("tabs", [{"browser": "arc", "tab": {}}], "enter", {}) == {"type": "done"}
with patch.object(client.Path, "stat") as stat, patch.object(client.Path, "open") as opening:
    stat.return_value.st_mode = 0o644
    try:
        client.request("/chat/query", {}, {})
    except RuntimeError as exc:
        assert "0600" in str(exc)
    else:
        raise AssertionError("shared-readable credentials must be refused")
    stat.return_value.st_mode = 0o600
    opening.return_value.__enter__.return_value = io.BytesIO(
        b'[bluebubbles]\nurl="https://example.com"\nserver_password="TEST-SECRET"\n'
    )
    error = client.HTTPError(
        "https://example.com?password=TEST-SECRET", 401, "TEST-SECRET", {}, None
    )
    with patch.object(client, "build_opener") as opener:
        opener.return_value.open.side_effect = error
        try:
            client.request("/chat/query", {}, {})
        except RuntimeError as exc:
            assert "401" in str(exc) and "TEST-SECRET" not in str(exc)
        else:
            raise AssertionError("HTTP failures must not return success")
        assert (
            opener.return_value.open.call_args.args[0].full_url
            == "https://example.com/api/v1/chat/query?password=TEST-SECRET"
        )


def assert_chat_changed(pages):
    with patch.object(client, "request", side_effect=pages):
        try:
            list(
                client.paginate(
                    "/chat/query", {"with": ["participants", "lastMessage"]}, {}, limit=2
                )
            )
        except RuntimeError as exc:
            assert "incomplete" in str(exc).lower()
        else:
            raise AssertionError("changing chat pages must not claim complete results")


assert_chat_changed(
    [
        {"data": [{"guid": "a"}, {"guid": "b"}], "metadata": {"total": 4}},
        {"data": [{"guid": "b"}, {"guid": "c"}], "metadata": {"total": 4}},
    ]
)
assert_chat_changed(
    [
        {"data": [{"guid": "a"}, {"guid": "b"}], "metadata": {"total": 3}},
        {"data": [{"guid": "c"}], "metadata": {"total": 4}},
    ]
)
assert_chat_changed(
    [
        {"data": [{"guid": "a"}, {"guid": "b"}], "metadata": {"total": 3}},
        {"data": [{"guid": "c"}], "metadata": {"total": 3}},
        {"data": [{"guid": "d"}, {"guid": "b"}], "metadata": {"total": 3}},
    ]
)
with patch.object(
    client,
    "request",
    side_effect=[
        {"data": [{"guid": "a"}, {"guid": "b"}], "metadata": {"total": 3}},
        {"data": [{"guid": "c"}], "metadata": {"total": 3}},
        {"data": [{"guid": "b"}, {"guid": "a"}], "metadata": {"total": 3}},
    ],
):
    assert [r["guid"] for r in client.paginate("/chat/query", {}, {}, limit=2)] == ["a", "b", "c"]
print("macOS picker checks passed")
