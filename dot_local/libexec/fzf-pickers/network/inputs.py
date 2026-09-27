import ipaddress
import re


def prompt(label, default=""):
    with open("/dev/tty", "r+") as tty:
        tty.write(f"{label}" + (f" [{default}]" if default else "") + ": ")
        tty.flush()
        value = tty.readline()
    if not value:
        raise KeyboardInterrupt
    return value.strip() or default


def validate(name, value):
    value = str(value).strip()
    if name in ("port", "count", "duration"):
        if not value.isdecimal() or int(value) < 1 or (name == "port" and int(value) > 65535):
            raise ValueError(
                f"{name} must be a positive integer" + (" up to 65535" if name == "port" else "")
            )
    elif name == "ip":
        ipaddress.ip_address(value)
    elif name == "cidr":
        if "/" not in value:
            raise ValueError("Choose an explicit subnet, such as 192.168.1.0/24")
        value = str(ipaddress.ip_network(value, strict=False))
    elif name == "host" and ":" in value:
        address, _, scope = value.partition("%")
        ipaddress.IPv6Address(address)
        if scope and not re.fullmatch(r"[A-Za-z0-9_.-]+", scope):
            raise ValueError("Invalid IPv6 scope")
    elif name in ("host", "interface"):
        if not re.fullmatch(r"[A-Za-z0-9_][A-Za-z0-9_.:%-]*", value):
            raise ValueError(f"Invalid {name}")
    elif not value or any(ord(char) < 32 for char in value):
        raise ValueError(f"Invalid {name}")
    return value
