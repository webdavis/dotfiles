import glob
import json
import shlex
from pathlib import Path

from .inputs import validate
from .process import run
from .rows import command_row, device_row, note


def ssh_hosts(path, seen=None, base=None):
    seen = set() if seen is None else seen
    path = Path(path).expanduser().resolve()
    base = path.parent if base is None else base
    if path in seen or not path.is_file():
        return []
    seen.add(path)
    result, current = [], []
    try:
        lines = path.read_text(errors="replace").splitlines()
    except OSError:
        return []
    for line in lines:
        try:
            parts = shlex.split(line, comments=True)
        except ValueError:
            continue
        if not parts:
            continue
        keyword, *values = parts
        if "=" in keyword:
            keyword, value = keyword.split("=", 1)
            values.insert(0, value)
        keyword = keyword.lower()
        if keyword == "include":
            for pattern in values:
                candidate = Path(pattern).expanduser()
                if not candidate.is_absolute():
                    candidate = base / candidate
                for included in sorted(glob.glob(str(candidate))):
                    result.extend(ssh_hosts(included, seen, base))
        elif keyword == "host":
            current = []
            for host in values:
                if any(char in host for char in "*?!"):
                    continue
                try:
                    validate("host", host)
                except ValueError:
                    continue
                row = command_row(
                    f"ssh:{path}:{host}",
                    host + "  [SSH]",
                    ["ssh", host],
                    purpose=f"Static host alias from {path}; SSH evaluates the full configuration when run.",
                )
                row["value"] = host
                result.append(row)
                current.append(row)
        elif keyword == "match":
            current = []
        elif keyword in ("hostname", "user", "port", "proxyjump"):
            for row in current:
                row["detail"] += f"\n{keyword}: " + " ".join(values)
                row["search"] += " " + " ".join(values)
    return result


def hosts(state):
    yield from ssh_hosts(Path.home() / ".ssh/config")
    try:
        for line in Path("/etc/hosts").read_text().splitlines():
            parts = line.split("#", 1)[0].split()
            if len(parts) > 1:
                for name in parts[1:]:
                    yield device_row(
                        "/etc/hosts", name, name, parts[0], {"Name": name, "IP": parts[0]}
                    )
    except OSError:
        yield note("hosts:error", "/etc/hosts is unavailable")
    try:
        data = json.loads(run(["tailscale", "status", "--json"]))
        peers = [data.get("Self", {}), *data.get("Peer", {}).values()]
        for peer in peers:
            name = peer.get("DNSName") or peer.get("HostName") or peer.get("ID", "")
            if not name:
                continue
            addresses = peer.get("TailscaleIPs", [])
            yield device_row(
                "Tailscale",
                peer.get("ID", name),
                name,
                addresses[0] if addresses else "",
                {
                    key: peer.get(key)
                    for key in (
                        "HostName",
                        "DNSName",
                        "TailscaleIPs",
                        "OS",
                        "Online",
                        "LastSeen",
                        "Relay",
                        "CurAddr",
                    )
                },
            )
    except (ValueError, AttributeError, TypeError):
        yield note("tailscale:error", "Tailscale status is unavailable")
