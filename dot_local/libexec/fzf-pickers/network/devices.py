import concurrent.futures
import ipaddress
import re
import time

from . import inputs
from .process import bounded_output, run
from .rows import device_row, note, ready
from .unifi import unifi_devices


def parse_neighbors(output, source):
    rows = []
    for line in output.splitlines():
        if source == "arp":
            match = re.match(r"(.+?) \(([^)]+)\) at (.+?) on (\S+)", line)
            if not match:
                continue
            name, address, mac, interface = match.groups()
            name = "" if name == "?" else name
            state = "incomplete" if "incomplete" in mac else "cached"
        else:
            parts = line.split()
            if len(parts) < 5 or ":" not in parts[0]:
                continue
            address, mac, interface = parts[:3]
            name, state = "", " ".join(parts[3:])
        rows.append(
            device_row(
                source.upper(),
                f"{address}:{interface}",
                name,
                address,
                {"Name": name, "IP": address, "MAC": mac, "Interface": interface, "State": state},
            )
        )
    return rows


def mdns_devices():
    # ponytail: browse three common service types; discover types when broader coverage is needed.
    types = ("_workstation._tcp", "_ssh._tcp", "_http._tcp")
    with concurrent.futures.ThreadPoolExecutor(max_workers=3) as pool:
        outputs = list(
            pool.map(
                lambda service: bounded_output(["dns-sd", "-B", service, "local."], 1.2), types
            )
        )
    instances = []
    for service, output in zip(types, outputs):
        for line in output.splitlines():
            parts = line.split(None, 6)
            if len(parts) == 7 and parts[1] == "Add":
                instances.append((parts[6], service, parts[4]))
    deadline = time.monotonic() + 2
    for name, service, domain in dict.fromkeys(instances):
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            yield note("mdns:partial", "mDNS discovery window ended; use Alt-R to refresh")
            return
        output = bounded_output(["dns-sd", "-L", name, service, domain], min(remaining, 0.25))
        match = re.search(r"can be reached at (.+?):(\d+)", output)
        address = match.group(1).rstrip(".") if match else ""
        yield device_row(
            "mDNS",
            f"{service}:{domain}:{name}",
            name,
            address,
            {
                "Service": service,
                "Domain": domain,
                "Host": address,
                "Port": match.group(2) if match else "",
                "State": "observed during bounded browse",
            },
        )


def devices(state):
    yield note(
        "devices:scope",
        "Known devices only: controller snapshots, neighbour caches and mDNS (workstation, SSH, HTTP)",
    )
    for source, argv in (("arp", ["arp", "-an"]), ("ndp", ["ndp", "-an"])):
        try:
            yield from parse_neighbors(run(argv), source)
        except ValueError as error:
            yield note(f"{source}:error", str(error))
    yield from mdns_devices()
    yield from unifi_devices(state)


def discover(state):
    subnet = inputs.validate("cidr", inputs.prompt("Subnet to discover, CIDR notation"))
    return ready(
        "Discover devices on " + subnet,
        [
            "nmap",
            *(["-6"] if ipaddress.ip_network(subnet).version == 6 else []),
            "-sn",
            "-n",
            subnet,
        ],
    )
