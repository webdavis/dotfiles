import hashlib
import importlib.util
import io
import json
import os
import shlex
import signal
import sys
import tempfile
import unittest
from contextlib import redirect_stdout
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

ENTRY = Path(__file__).resolve().parents[2] / "dot_local/libexec/fzf-pickers/executable_picker.py"
sys.path.insert(0, str(ENTRY.parent))
spec = importlib.util.spec_from_file_location("picker_entry", ENTRY)
entry = importlib.util.module_from_spec(spec)
spec.loader.exec_module(entry)


class RuntimeTests(unittest.TestCase):
    def setUp(self):
        self.scratch = tempfile.TemporaryDirectory(prefix="fzf-runtime-")
        self.addCleanup(self.scratch.cleanup)
        self.root = Path(self.scratch.name).resolve()
        self.files = self.root / "files"
        self.files.mkdir()
        self.file = self.files / "odd name; 'quoted'.txt"
        self.file.write_text("hello world\n")
        (self.files / ".hidden").write_text("hidden\n")
        self.session = self.root / "session"
        self.session.mkdir()
        self.state = {"kind": "files", "cwd": str(self.files), "hidden": False}
        self.write_state()
        self.env = {key: value for key, value in os.environ.items() if not key.startswith("GIT_")}
        self.env.update(
            HOME=str(self.root),
            GIT_CONFIG_GLOBAL="/dev/null",
            GIT_CONFIG_SYSTEM="/dev/null",
            PYTHONDONTWRITEBYTECODE="1",
            EDITOR="nvim",
            EDITOR_CMD="nvim",
        )

    def write_state(self):
        (self.session / "state.json").write_text(json.dumps(self.state))

    def invoke(self, *args):
        output = io.BytesIO()
        with (
            patch.object(sys, "argv", [str(ENTRY), *map(str, args)]),
            patch.dict(os.environ, self.env, clear=True),
            patch("os.getcwd", return_value=str(self.files)),
            patch("git_files.root", return_value=None),
            redirect_stdout(io.TextIOWrapper(output, write_through=True)),
        ):
            entry.main()
            return SimpleNamespace(stdout=output.getvalue())

    def test_rows_preview_and_help_share_the_session(self):
        result = self.invoke("rows", self.session, "")
        records = result.stdout.rstrip(b"\0").split(b"\0")
        self.assertEqual(len(records), 1)
        identity = hashlib.sha256(str(self.file).encode()).hexdigest()
        self.assertEqual(records[0].decode().split("\t")[0], identity)
        self.assertIn(self.file.name.encode(), result.stdout)
        self.assertIn(b"hello world", self.invoke("preview", self.session, identity).stdout)
        self.assertIn(b"1 items | Complete", self.invoke("header", self.session).stdout)

    def test_help_toggles_preview_content(self):
        identity = hashlib.sha256(str(self.file).encode()).hexdigest()
        self.invoke("rows", self.session, "")
        self.invoke("help", self.session)
        self.assertIn(b"Wrap preview", self.invoke("preview", self.session, identity).stdout)
        self.invoke("help", self.session)
        self.assertNotIn(b"Wrap preview", self.invoke("preview", self.session, identity).stdout)

    def test_hidden_toggle_reloads_rows(self):
        self.invoke("rows", self.session, "", "alt-h")
        self.assertIn(b"2 items | Complete", self.invoke("header", self.session).stdout)

    def fake_fzf(self, cancel=False):
        selected = False

        def select(args, state, session, entrypoint):
            nonlocal selected
            self.assertFalse(selected, "The picker must finish after this selection.")
            selected = True
            self.assertIn("--read0", args)
            self.assertIn("--print0", args)
            if cancel:
                return SimpleNamespace(returncode=130, stdout=b"")
            output = io.BytesIO()
            with redirect_stdout(io.TextIOWrapper(output, write_through=True)):
                entry.emit_rows(session, "")
                first = output.getvalue().split(b"\0")[0]
            return SimpleNamespace(returncode=0, stdout=b"\0enter\0" + first + b"\0")

        replacing = patch("runtime.session.select", side_effect=select)
        replacing.start()
        self.addCleanup(replacing.stop)

    def test_selection_returns_a_quoted_command(self):
        self.fake_fzf()
        result = self.invoke("run", "files")
        fields = result.stdout.decode().split("\0")
        self.assertEqual(fields[0], "command")
        self.assertEqual(shlex.split(fields[1]), ["nvim", "--", str(self.file)])
        self.assertEqual(self.file.read_text(), "hello world\n")

    def test_metadata_filter_searches_details_and_emits_matches(self):
        from runtime import records

        identity = "f" * 64
        row = {
            "label": "nc -vz host 22",
            "detail": "Inspect SSH connectivity",
            "value": "host",
        }
        records.save(self.session / (identity + ".json"), row)
        (self.session / "index").write_text(identity + "\n")
        matched = records.record(identity, row)
        with patch(
            "runtime.records.subprocess.run",
            return_value=SimpleNamespace(returncode=0, stdout=matched),
        ) as filter:
            self.assertEqual(self.invoke("filter", self.session, "SSH").stdout, matched)
            self.assertIn(b"Inspect SSH connectivity", filter.call_args.kwargs["input"])
            self.assertIn("--filter=SSH", filter.call_args.args[0])

    def test_missing_selector_reaps_the_row_producer(self):
        from runtime.window import select

        with (
            patch("runtime.window.subprocess.Popen") as start,
            patch("runtime.window.subprocess.run", side_effect=FileNotFoundError),
            patch("os.killpg") as stop,
        ):
            producer = start.return_value
            producer.poll.return_value = None
            with self.assertRaises(FileNotFoundError):
                select(["missing-fzf"], self.state, self.session, ENTRY)
            producer.stdout.close.assert_called_once()
            stop.assert_called_once_with(producer.pid, signal.SIGKILL)
            self.assertTrue(start.call_args.kwargs["start_new_session"])
            producer.wait.assert_called_once()

    def test_selection_cleans_descendants_even_after_the_producer_exits(self):
        from runtime.window import select

        for stop_error in (None, ProcessLookupError):
            with (
                self.subTest(stop_error=stop_error),
                patch("runtime.window.subprocess.Popen") as start,
                patch("runtime.window.subprocess.run") as selector,
                patch("os.killpg", side_effect=stop_error) as stop,
            ):
                producer = start.return_value
                producer.poll.return_value = 0
                result = select(["fzf"], self.state, self.session, ENTRY)
                self.assertIs(result, selector.return_value)
                stop.assert_called_once_with(producer.pid, signal.SIGKILL)
                self.assertTrue(start.call_args.kwargs["start_new_session"])
                producer.wait.assert_called_once()

    def test_cancellation_returns_no_action(self):
        self.fake_fzf(cancel=True)
        self.assertEqual(self.invoke("run", "files").stdout, b"")


if __name__ == "__main__":
    unittest.main(verbosity=2)
