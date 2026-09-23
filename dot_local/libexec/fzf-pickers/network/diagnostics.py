import re

from . import inputs
from .inputs import validate
from .rows import ready


def dns_records(state):
    for record in ("A", "AAAA", "CNAME", "MX", "TXT", "NS", "SOA", "SRV", "CAA"):
        yield {
            "id": record,
            "label": record,
            "detail": f"dig <host> {record}\nChoose a host and optional DNS server on Enter.",
            "value": record,
            "action": "dns",
        }


def iperf_modes(state):
    for mode in ("client", "reverse", "bidirectional", "udp", "server"):
        yield {
            "id": mode,
            "label": mode,
            "detail": "Choose duration, port and host as applicable.\nEnter shows the exact command before preparing it.",
            "value": mode,
            "action": "iperf",
        }


def prepare_dns(row, state):
    host = validate("host", inputs.prompt("Host"))
    server = inputs.prompt("DNS server (blank for system default)")
    argv = ["dig", "+time=2", "+tries=1"]
    if server:
        argv.append("@" + validate("host", server))
    return ready("DNS " + row["value"], [*argv, host, row["value"]])


def prepare_iperf(row, state):
    mode = row["value"]
    port = validate("port", inputs.prompt("Port", "5201"))
    argv = ["iperf3", "--json", "--port", port]
    if mode == "server":
        argv += ["--server", "--one-off"]
    else:
        host = validate("host", inputs.prompt("iperf3 server"))
        duration = validate("duration", inputs.prompt("Duration in seconds", "10"))
        argv += ["--client", host, "--time", duration]
        if mode == "reverse":
            argv += ["--reverse"]
        elif mode == "bidirectional":
            argv += ["--bidir"]
        elif mode == "udp":
            bitrate = inputs.prompt("UDP bitrate, such as 10M", "10M")
            if not re.fullmatch(r"[1-9][0-9]*[KMG]?", bitrate):
                raise ValueError("Invalid bitrate")
            argv += ["--udp", "--bitrate", bitrate]
    return ready("iperf3 " + mode, argv)
