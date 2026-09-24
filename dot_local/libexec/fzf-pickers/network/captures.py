import os
import shlex
from pathlib import Path

from . import inputs
from .inputs import validate
from .process import run
from .rows import command_row, ready


def capture_command(options):
    interface = validate("interface", options.get("interface", "en0"))
    count = validate("count", options.get("count", "20"))
    duration = options.get("duration", "")
    protocol = options.get("protocol", "any")
    if protocol not in ("any", "tcp", "udp", "icmp", "icmp6"):
        raise ValueError("Choose any, tcp, udp, icmp or icmp6")
    clauses = []
    if options.get("host"):
        clauses.append("host " + validate("host", options["host"]))
    if protocol != "any":
        clauses.append(protocol)
    if options.get("port"):
        clauses.append("port " + validate("port", options["port"]))
    tool = options.get("tool", "tshark")
    if tool not in ("tshark", "tcpdump"):
        raise ValueError("Choose tshark or tcpdump")
    if tool == "tcpdump" and duration:
        raise ValueError("Use tshark for a duration-limited capture")
    argv = ["sudo", tool, "-n", "-i", interface, "-c", count]
    if duration:
        argv.extend(["-a", "duration:" + validate("duration", duration)])
    if options.get("output"):
        output = str(Path(options["output"]).expanduser().absolute())
        if Path(output).exists():
            raise ValueError("Capture output already exists; choose a new filename")
        argv.extend(["-w", validate("output", output)])
    if clauses:
        if tool == "tshark":
            argv.append("-f")
        argv.append(" and ".join(clauses))
    return argv


def capture_builder(state):
    try:
        interfaces = run(["ifconfig", "-l"]).split()
    except ValueError:
        interfaces = ["en0"]
    for interface in interfaces:
        yield {
            "id": f"interface:{interface}",
            "label": f"Capture on {interface}",
            "detail": "Choose packet count, optional duration, output and capture filter on Enter.\nThe final command is shown for confirmation.",
            "value": interface,
            "action": "capture",
        }


def saved_captures(state):
    records = []
    root = Path(state.get("capture_root", state.get("cwd", os.getcwd()))).expanduser()
    for directory, dirs, files in os.walk(root):
        dirs[:] = [
            name
            for name in dirs
            if (state.get("hidden") or not name.startswith("."))
            and name not in ("node_modules", "target", ".build", ".git")
        ]
        for name in sorted(files):
            path = Path(directory) / name
            if (state.get("hidden") or not name.startswith(".")) and path.suffix.lower() in (
                ".pcap",
                ".pcapng",
                ".cap",
            ):
                try:
                    info = path.stat()
                    size = info.st_size
                except OSError:
                    continue
                records.append(
                    {
                        "id": str(path),
                        "label": str(path.relative_to(root)),
                        "mtime": info.st_mtime,
                        "detail": f"{path}\n{size:,} bytes\nEnter chooses summary, packets, conversations or protocols.\nNo capture analysis runs while hovering.",
                        "value": str(path),
                        "path": str(path),
                        "action": "next",
                        "next": "network-capture-analysis",
                    }
                )
    yield from sorted(
        records,
        key=(lambda row: (-row["mtime"], row["path"]))
        if state.get("sort")
        else (lambda row: row["path"]),
    )


def capture_analysis(state):
    path = state["capture_path"]
    yield command_row("capture-summary", "Capture file summary", ["capinfos", path])
    yield command_row(
        "capture-packets", "Packets (numeric addresses)", ["tshark", "-n", "-r", path]
    )
    yield command_row(
        "capture-conversations",
        "TCP and UDP conversations",
        ["tshark", "-n", "-r", path, "-q", "-z", "conv,tcp", "-z", "conv,udp"],
    )
    yield command_row(
        "capture-protocols",
        "Protocol hierarchy",
        ["tshark", "-n", "-r", path, "-q", "-z", "io,phs"],
    )
    row = command_row(
        "capture-filter",
        "Packets matching a display filter",
        text=shlex.join(["tshark", "-n", "-r", path]) + " -Y <display-filter>",
        purpose="Choose a Wireshark display filter on Enter. Capture filters use -f; this saved-file view uses -Y.",
    )
    row["action"] = "display-filter"
    yield row


def toggle(state, field):
    return {"type": "reload", "state": {field: not state.get(field, False)}}


def choose_root(state):
    root = (
        Path(inputs.prompt("Capture search directory", state.get("cwd", os.getcwd())))
        .expanduser()
        .absolute()
    )
    if not root.is_dir():
        raise ValueError("Capture search directory does not exist")
    return {"type": "reload", "state": {"capture_root": str(root)}}


def open_capture(row, state):
    return {
        "type": "next",
        "kind": "network-capture-analysis",
        "state": {"capture_path": row["path"]},
    }


def prepare_capture(row, state):
    options = {
        "interface": row["value"],
        "tool": inputs.prompt("Tool: tshark or tcpdump", "tshark"),
        "host": inputs.prompt("Host filter (blank for any)"),
        "protocol": inputs.prompt("Protocol: any/tcp/udp/icmp/icmp6", "any"),
        "port": inputs.prompt("Port filter (blank for any)"),
        "count": inputs.prompt("Maximum packets", "20"),
        "duration": inputs.prompt("Maximum seconds (tshark only, blank for packet limit)"),
        "output": inputs.prompt("Save to new file (blank for terminal output)"),
    }
    return ready("Packet capture", capture_command(options))


def prepare_filter(row, state):
    query = inputs.prompt("Wireshark display filter")
    validate("display filter", query)
    return ready("Filtered capture", ["tshark", "-n", "-r", state["capture_path"], "-Y", query])
