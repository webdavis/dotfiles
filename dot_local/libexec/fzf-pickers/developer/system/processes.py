import developer.rows as row_model

from .. import execution


def process_rows(processes, sockets):
    connections = {}
    pid = None
    for line in sockets.splitlines():
        if line.startswith("p") and line[1:].isdigit():
            pid = line[1:]
        elif pid and line.startswith("n"):
            connections.setdefault(pid, []).append(line[1:])
        elif pid and line.startswith("TST=") and connections.get(pid):
            connections[pid][-1] += " " + line[4:]
    rows = []
    for line in processes.splitlines():
        parts = line.split(None, 3)
        if len(parts) != 4 or not parts[0].isdigit():
            continue
        pid, parent, user, executable = parts
        ports = "\n".join(connections.get(pid, []))
        detail = f"PID: {pid}\nParent: {parent}\nUser: {user}\nExecutable: {executable}"
        if ports:
            detail += "\n\nConnections and ports:\n" + ports
        rows.append(
            row_model.row(
                ("pid", pid, executable),
                f"{pid:>7}  {executable}  {ports.replace(chr(10), '; ')}",
                detail,
                pid,
                action="insert",
            )
        )
    return rows


def collect(cwd, state):
    processes = execution.run(["ps", "-axo", "pid=,ppid=,user=,comm="])
    sockets = execution.run(["lsof", "-nP", "-i", "-FpcnT"], check=False)
    return process_rows(processes, sockets)


def accept(rows, key, state):
    if rows and key == "alt-d":
        return {"type": "command", "argv": ["lsof", "-nP", "-a", "-p", rows[0]["value"], "-i"]}
    return None
