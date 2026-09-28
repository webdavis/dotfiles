import ipaddress
import shutil
import urllib.parse

from . import inputs
from .inputs import validate
from .rows import command_row


def url_commands(value):
    if any(ord(char) < 32 for char in value):
        raise ValueError("Invalid URL")
    url = urllib.parse.urlsplit(value)
    if (
        url.scheme not in ("https", "http")
        or not url.hostname
        or url.username is not None
        or url.password is not None
    ):
        raise ValueError("Choose an HTTP or HTTPS URL without credentials")
    host = validate("host", url.hostname)
    port = url.port or (443 if url.scheme == "https" else 80)
    rows = [
        command_row(
            "url-timing",
            "HTTP timings and response headers",
            [
                "curl",
                "--globoff",
                "--proto",
                "=http,https",
                "--connect-timeout",
                "10",
                "--max-time",
                "30",
                "--silent",
                "--show-error",
                "--output",
                "/dev/null",
                "--dump-header",
                "-",
                "--write-out",
                "DNS: %{time_namelookup}s\nConnect: %{time_connect}s\nTLS: %{time_appconnect}s\nFirst byte: %{time_starttransfer}s\nTotal: %{time_total}s\nStatus: %{http_code}\nRemote: %{remote_ip}\n",
                "--url",
                value,
            ],
            purpose="Send one request and report phase timings. Redirects are shown without following them.",
        )
    ]
    if url.scheme == "https":
        openssl = shutil.which("openssl") or "openssl"
        try:
            ipaddress.ip_address(host.split("%", 1)[0])
            check = "-verify_ip"
        except ValueError:
            check = "-verify_hostname"
        connect = f"[{host}]:{port}" if ":" in host else f"{host}:{port}"
        argv = [
            openssl,
            "s_client",
            "-connect",
            connect,
            "-servername",
            host,
            "-showcerts",
            "-verify_return_error",
            check,
            host,
        ]
        rows.append(
            command_row(
                "url-certificate",
                "Inspect and verify TLS certificates",
                argv,
                purpose="Connect explicitly, verify the certificate chain and peer name, and show certificates.",
            )
        )
    return rows


def collect(state):
    if state.get("url"):
        yield from url_commands(state["url"])
    else:
        yield {
            "id": "url-enter",
            "label": "Choose a URL",
            "detail": "Enter an HTTP or HTTPS URL, then choose timing or certificate inspection.\nNo requests run before the prepared command is executed.",
            "value": "",
            "action": "url",
        }


def edit(state):
    value = inputs.prompt("URL", state.get("url", "https://example.com"))
    url_commands(value)
    return {"type": "reload", "state": {"url": value}}


def choose(row, state):
    return edit({})
