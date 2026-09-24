import os
import pty
import select
import signal
import subprocess
import sys
import time
from pathlib import Path

root = Path(__file__).resolve().parents[2]
mode, chord = sys.argv[1:]
master, slave = pty.openpty()
process = subprocess.Popen(
    ["bash", "--noprofile", "--norc", "-i"],
    stdin=slave,
    stdout=slave,
    stderr=slave,
    cwd=root,
    env={**os.environ, "TERM": "xterm", "INPUTRC": "/dev/null", "PS1": "CHECK> "},
)
os.close(slave)


def read_until(token, timeout):
    result = b""
    deadline = time.monotonic() + timeout
    while token not in result and time.monotonic() < deadline:
        if select.select([master], [], [], 0.01)[0]:
            result += os.read(master, 65536)
    return result


try:
    setup = (
        "source dot_bash_bindings_functions; source dot_bash_bindings; source dot_fzf_bindings; "
        '__mark() { printf "\\nFIRED\\n"; }; fzf-file-widget() { __mark; }; '
        "bind -m " + mode + " -x '\"\\C-gss\": __mark'; "
        'bind "set keyseq-timeout 200"; bind "set enable-bracketed-paste off"; '
        'bind "set keymap ' + mode + '"; printf "\\nREADY\\n"\n'
    )
    os.write(master, setup.encode())
    assert b"\r\nREADY\r\nCHECK> " in read_until(b"\r\nREADY\r\nCHECK> ", 0.4), (
        "Shell did not initialize"
    )
    prefix, rest = (b"\x07", b"ss") if chord == "git" else (b"\x14", b"\x14")
    os.write(master, prefix)
    assert b"FIRED" not in read_until(b"FIRED", 0.3), "Prefix executed before its next key"
    os.write(master, rest)
    result = read_until(b"FIRED", 0.5)
    assert b"FIRED" in result, "Delayed chord was lost: " + repr(result)
finally:
    os.close(master)
    process.send_signal(signal.SIGHUP)
    process.wait(timeout=1)
print("OK")
