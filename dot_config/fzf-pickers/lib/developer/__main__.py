import argparse
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from developer.capture import capture

parser = argparse.ArgumentParser()
commands = parser.add_subparsers(dest="command", required=True)
recorder = commands.add_parser("capture")
recorder.add_argument("--category", choices=["tests", "diagnostics"], required=True)
recorder.add_argument("--output", required=True)
recorder.add_argument("--cwd", required=True)
recorder.add_argument("--framework", required=True)
recorder.add_argument("argv", nargs=argparse.REMAINDER)
arguments = parser.parse_args()
argv = arguments.argv[1:] if arguments.argv[:1] == ["--"] else arguments.argv
if not argv:
    parser.error("capture needs a command after --")
try:
    sys.exit(
        capture(
            arguments.category,
            arguments.output,
            arguments.cwd,
            arguments.framework,
            argv,
        )
    )
except OSError as error:
    print(str(error), file=sys.stderr)
    sys.exit(1)
