import json
import stat
from pathlib import Path
from urllib.error import HTTPError, URLError
from urllib.parse import urlencode, urlsplit
from urllib.request import HTTPRedirectHandler, Request, build_opener

import tomllib


class NoRedirect(HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        raise RuntimeError(
            "BlueBubbles redirected the request. Update its URL in the picker configuration."
        )


def request(path, body, state):
    config_path = Path(state.get("config", "~/.config/fzf-pickers/config.toml")).expanduser()
    try:
        if stat.S_IMODE(config_path.stat().st_mode) & 0o077:
            raise RuntimeError(
                "Picker credentials must be private. Set config.toml permissions to 0600."
            )
        with config_path.open("rb") as stream:
            config = tomllib.load(stream)["bluebubbles"]
        base, server_password = config["url"], config["server_password"]
        if not isinstance(base, str):
            raise ValueError("invalid URL type")
        base = base.rstrip("/")
    except (OSError, KeyError, TypeError, ValueError) as exc:
        raise RuntimeError(
            "BlueBubbles requires url and server_password in ~/.config/fzf-pickers/config.toml."
        ) from exc
    parts = urlsplit(base)
    if not isinstance(server_password, str) or not server_password:
        raise RuntimeError("BlueBubbles server_password is empty in the picker configuration.")
    if parts.username or parts.password or parts.query or parts.fragment or not parts.hostname:
        raise RuntimeError(
            "BlueBubbles URL must contain only its scheme, host, port, and optional base path."
        )
    if parts.scheme != "https" and not (
        parts.scheme == "http" and parts.hostname in ("localhost", "127.0.0.1", "::1")
    ):
        raise RuntimeError("BlueBubbles requires HTTPS outside this Mac.")
    req = Request(
        base + "/api/v1" + path + "?" + urlencode({"password": server_password}),
        data=json.dumps(body).encode(),
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    try:
        with build_opener(NoRedirect()).open(req, timeout=20) as response:
            result = json.load(response)
    except HTTPError as exc:
        raise RuntimeError(
            f"BlueBubbles request failed (HTTP {exc.code}). Check its URL and server password."
        ) from None
    except (URLError, OSError, ValueError):
        raise RuntimeError(
            "BlueBubbles is unavailable or returned an invalid response. Check its server and picker configuration."
        ) from None
    if not isinstance(result, dict) or not isinstance(result.get("data"), list):
        raise RuntimeError("BlueBubbles returned an unexpected response.")
    return result


def paginate(path, body, state, limit=200):
    offset = 0
    seen = set()
    verify_chats = path == "/chat/query"
    first_ids = None
    expected_total = None
    while True:
        result = request(path, {**body, "offset": offset, "limit": limit}, state)
        page = result["data"]
        total = result.get("metadata", {}).get("total")
        identities = []
        for item in page:
            if (
                not isinstance(item, dict)
                or not isinstance(item.get("guid"), str)
                or not item["guid"]
            ):
                raise RuntimeError("BlueBubbles returned a record without a stable identifier.")
            identities.append(item["guid"])
        if verify_chats:
            if type(total) is not int or total < 0:
                raise RuntimeError("BlueBubbles omitted its chat count; results are incomplete.")
            if first_ids is None:
                first_ids, expected_total = set(identities), total
            if (
                total != expected_total
                or seen.intersection(identities)
                or len(set(identities)) != len(identities)
            ):
                raise RuntimeError(
                    "BlueBubbles conversations changed during loading; results are incomplete. Press Alt-R to refresh."
                )
        if not page:
            break
        fresh = 0
        for item, identity in zip(page, identities):
            if identity in seen:
                continue
            seen.add(identity)
            fresh += 1
            yield item
        if not fresh:
            raise RuntimeError("BlueBubbles pagination did not advance; results are incomplete.")
        offset += len(page)
        if isinstance(total, int) and offset >= total:
            break
    if verify_chats:
        check = request(path, {**body, "offset": 0, "limit": limit}, state)
        current_ids = {item.get("guid") for item in check["data"] if isinstance(item, dict)}
        if (
            len(seen) != expected_total
            or check.get("metadata", {}).get("total") != expected_total
            or current_ids != first_ids
        ):
            raise RuntimeError(
                "BlueBubbles conversations changed during loading; results are incomplete. Press Alt-R to refresh."
            )
