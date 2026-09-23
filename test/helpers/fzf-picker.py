import importlib.util
import json
from pathlib import Path
import shlex
import sys
import tempfile
import tracemalloc

helpers = Path(__file__).resolve().parents[2] / 'dot_local/libexec/fzf-pickers'
sys.path.insert(0, str(helpers))
spec = importlib.util.spec_from_file_location('picker', helpers / 'executable_picker.py')
m = importlib.util.module_from_spec(spec)
spec.loader.exec_module(m)
assert m.clean('a\x1b[31m\n\tb') == 'a[31m ↵   b'
row = {'path': '/tmp/a\nb; $(echo bad)', 'line': 3, 'label': 'path', 'action': 'edit'}
a = m.default_action([row], 'enter', {})
assert shlex.split(shlex.join(a['argv']))[-1] == row['path']
assert m.default_action([{'value': ['a b', 'c']}], 'enter', {})['values'] == ['a b', 'c']
assert m.default_action([], 'enter', {}) is None
second = {**row, 'path': '/tmp/second'}
assert m.default_action([row, second], 'enter', {})['argv'][-2:] == [row['path'], second['path']]
try:
    m.copy_text('')
    raise AssertionError('Empty copy must be refused')
except RuntimeError:
    pass
with tempfile.TemporaryDirectory() as directory:
    session = Path(directory)
    m.save(session / 'state.json', {'kind': 'files', 'cwd': directory})
    identity = 'f' * 64
    m.save(session / (identity + '.json'), row)
    assert m.row_ids(session, ['../../private', identity]) == [row]
    m.save(session / ('0' * 64 + '.json'), {'error': True})
    assert not m.row_ids(session, ['0' * 64])
    assert m.preview(session, identity).startswith('path')
    large = session / 'large.bin'
    with large.open('wb') as stream:
        stream.truncate(32 * 1024 * 1024)
    m.save(session / (identity + '.json'), {**row, 'path': str(large)})
    tracemalloc.start()
    assert '[Binary file]' in m.preview(session, identity)
    _, peak = tracemalloc.get_traced_memory()
    tracemalloc.stop()
    assert peak < 8 * 1024 * 1024, 'Preview read the whole large file'
print('Picker transport checks passed')
