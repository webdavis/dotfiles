import json
import ssl
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path

import tomllib

from .rows import device_row, note


def pages(get, path):
    offset = 0
    seen = set()
    while True:
        page = get(path, offset)
        if not isinstance(page, dict) or not isinstance(page.get("data"), list):
            raise ValueError("Malformed UniFi page; results are incomplete")
        batch = page["data"]
        limit = page.get("limit", 200)
        if type(limit) is not int or limit < 0 or (limit == 0 and batch):
            raise ValueError("Malformed UniFi limit; results are incomplete")
        if page.get("offset", offset) != offset:
            raise ValueError("UniFi pagination did not advance; results are incomplete")
        for item in batch:
            if not isinstance(item, dict):
                raise ValueError("Malformed UniFi device; results are incomplete")
            identity = item.get("id") or json.dumps(item, sort_keys=True)
            if identity in seen:
                raise ValueError("UniFi repeated a result; results are incomplete")
            seen.add(identity)
            yield item
        total = page.get("totalCount")
        if total is not None and (not isinstance(total, int) or total < 0):
            raise ValueError("Malformed UniFi total; results are incomplete")
        offset += len(batch)
        if total is not None and offset >= total:
            return
        if not batch:
            if total is not None and offset < total:
                raise ValueError("UniFi returned an empty early page; results are incomplete")
            return
        if total is None and len(batch) < limit:
            return


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        return None


def unifi_get(state):
    path = Path(state.get("config", Path.home() / ".config/fzf-pickers/config.toml"))
    if path.stat().st_mode & 0o077:
        raise ValueError("Picker credentials must be readable only by their owner (chmod 600)")
    with path.open("rb") as stream:
        config = tomllib.load(stream).get("unifi", {})
    base = str(config.get("url", "")).rstrip("/")
    parsed = urllib.parse.urlsplit(base)
    if (
        parsed.scheme != "https"
        or not parsed.hostname
        or parsed.username
        or parsed.password
        or parsed.query
        or parsed.fragment
    ):
        raise ValueError("UniFi requires an HTTPS controller URL without credentials")
    key = config.get("api_key")
    if not isinstance(key, str) or not key:
        raise ValueError("UniFi API key is not configured")
    context = ssl.create_default_context(cafile=config.get("ca_file") or None)
    opener = urllib.request.build_opener(NoRedirect(), urllib.request.HTTPSHandler(context=context))

    def get(path, offset):
        request = urllib.request.Request(
            base
            + "/proxy/network/integration/v1"
            + path
            + "?"
            + urllib.parse.urlencode({"offset": offset, "limit": 200}),
            headers={"X-API-Key": key, "Accept": "application/json"},
        )
        try:
            with opener.open(request, timeout=4) as response:
                return json.load(response)
        except urllib.error.HTTPError as error:
            raise ValueError(f"UniFi HTTP {error.code}; results are incomplete") from None
        except (urllib.error.URLError, ssl.SSLError):
            raise ValueError(
                "UniFi connection or certificate validation failed; configure a trusted CA if needed"
            ) from None
        except (json.JSONDecodeError, UnicodeError):
            raise ValueError("UniFi returned invalid JSON; results are incomplete") from None

    return get


def unifi_devices(state):
    try:
        get = unifi_get(state)
        for site in pages(get, "/sites"):
            site_id = urllib.parse.quote(str(site["id"]), safe="")
            site_name = site.get("name", site["id"])
            for category in ("devices", "clients"):
                try:
                    for device in pages(get, f"/sites/{site_id}/{category}"):
                        fields = {"Site": site_name, **device}
                        yield device_row(
                            f"UniFi {category}",
                            f"{site['id']}:{device.get('id', device.get('macAddress', ''))}",
                            device.get("name", ""),
                            device.get("ipAddress", ""),
                            fields,
                        )
                except (ValueError, KeyError) as error:
                    yield note(
                        f"unifi:{site_id}:{category}:error", f"{site_name} {category}: {error}"
                    )
    except (OSError, ValueError, KeyError):
        yield note(
            "unifi:error",
            "UniFi unavailable or incomplete: check private picker config, API access and trusted CA",
        )
