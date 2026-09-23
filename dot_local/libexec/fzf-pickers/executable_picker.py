#!/usr/bin/env python3
import argparse
import hashlib
import importlib
import json
import os
from pathlib import Path
import shlex
import subprocess
import sys
import tempfile

sys.dont_write_bytecode = True

import git_files

MACOS = {'contacts', 'contact-fields', 'tabs', 'recent', 'shortcuts', 'logs', 'crashes'}
MESSAGES = {'messages', 'chats', 'message-history', 'message-links', 'message-attachments'}
NETWORK = {'network', 'devices', 'captures', 'url'}
COMMON = {'ctrl-/': 'Help', 'alt-/': 'Help', 'alt-p': 'Preview', 'alt-w': 'Wrap preview',
          'alt-W': 'Wrap results', 'ctrl-r': 'Relevance/source order', 'alt-r': 'Refresh',
          'alt-y': 'Copy selected/current values', 'ctrl-n/ctrl-p': 'Next/previous',
          'ctrl-alt-n/ctrl-alt-p': 'Page down/up', 'tab/shift-tab': 'Select/unselect',
          'enter': 'Accept', 'esc/ctrl-c': 'Cancel'}
LOCATION_KINDS = {'files', 'directories', 'parents', 'bookmarks', 'contents', 'tracked',
                  'git-files', 'changed', 'ignored', 'conflicts', 'git-config', 'quickfix',
                  'diagnostics', 'chezmoi', 'recent', 'crashes', 'dev-context', 'dev-instructions'}
SEARCH_DETAILS = {'network', 'devices', 'network-hosts', 'contacts'}
TOGGLES = {'alt-h', 'alt-u', 'alt-x', 'alt-s', 'alt-B', 'alt-0', 'alt-1', 'alt-2', 'alt-3', 'alt-4', 'alt-5', 'alt-6'}


def module(kind):
    if kind in git_files.KINDS:
        return git_files
    if kind in MACOS or kind.startswith('macos-'):
        return importlib.import_module('macos')
    if kind in MESSAGES or kind.startswith('messages-'):
        return importlib.import_module('messages')
    return importlib.import_module('network' if kind in NETWORK or kind.startswith('network-') else 'developer')


def clean(text):
    return ''.join(c if c >= ' ' and c != '\x7f' else {'\n': ' ↵ ', '\t': '  '}.get(c, '') for c in str(text))


def save(path, value):
    with tempfile.NamedTemporaryFile(mode='w', dir=path.parent, prefix='.write-', delete=False) as stream:
        json.dump(value, stream, ensure_ascii=True)
        temporary = Path(stream.name)
    temporary.replace(path)


def load(session):
    return json.loads((session / 'state.json').read_text())


def row_ids(session, ids):
    rows = []
    for identity in ids:
        if len(identity) == 64 and all(c in '0123456789abcdef' for c in identity):
            path = session / (identity + '.json')
            if path.is_file():
                row = json.loads(path.read_text())
                if not row.get('error'):
                    rows.append(row)
    return rows


def controls(kind, state):
    custom = module(kind).bindings(kind, state)
    result = dict(COMMON)
    if kind in ('contents', 'log-search'):
        result.pop('ctrl-r')
    if kind in LOCATION_KINDS:
        result.update({'ctrl-o': 'Prepare opening', 'ctrl-s': 'Prepare sudo edit',
                       'alt-c': 'Prepare containing directory', 'alt-q': 'Export selected locations'})
    result.update(custom)
    return result


def header(session):
    state = load(session)
    modes = [state['kind'].replace('-', ' ').title()]
    if state['kind'] == 'contents':
        project = state.get('worktree') or git_files.root(state['cwd'])
        modes += ['Untracked only' if state.get('untracked') else 'Tracked only'] if project else ['Current directory']
        modes += ['Regex' if state.get('regex') else 'Literal', 'Hidden' if state.get('hidden') else 'Visible']
    for field in ('browser', 'contact_mode'):
        if state.get(field):
            modes.append(str(state[field]))
    if state.get('notice'):
        modes.append(state['notice'])
    status_file = session / 'status.json'
    status = json.loads(status_file.read_text()) if status_file.exists() else {}
    modes.append(status.get('status', 'Loading'))
    return clean(' | '.join(modes))


def filter_rows(session, query, toggle=False):
    state = load(session)
    if toggle:
        state['source_order'] = not state.get('source_order')
        save(session / 'state.json', state)
    index = session / 'index'
    identities = index.read_text().splitlines() if index.exists() else []
    data = bytearray()
    for identity in identities:
        for row in row_ids(session, [identity]):
            data.extend(record(identity, row))
    args = ['fzf', '--read0', '--print0', '--delimiter=\t', '--nth=2..', '--filter=' + query]
    if state.get('source_order'):
        args.append('--no-sort')
    result = subprocess.run(args, input=data, stdout=subprocess.PIPE,
                            env={**os.environ, 'FZF_DEFAULT_OPTS': '', 'FZF_DEFAULT_OPTS_FILE': ''})
    if result.returncode not in (0, 1):
        raise RuntimeError('Could not filter picker records.')
    sys.stdout.buffer.write(result.stdout)


def record(identity, row):
    label = clean(row.get('label', row.get('value', '')))
    search = clean(row.get('search', row.get('detail', '')))
    return (identity + '\t' + label + '\t' + search + '\0').encode(errors='replace')


def emit_rows(session, query, key=None):
    state = load(session)
    kind = state['kind']
    state['query'] = query
    if key:
        action = module(kind).accept(kind, [], key, state)
        if action and action.get('type') == 'reload':
            state.update(action.get('state', {}))
    if key:
        state.pop('notice', None)
        save(session / 'state.json', state)
    save(session / 'status.json', {'status': 'Loading'})
    count = 0
    seen = set()
    (session / 'index').write_text('')
    try:
        for row in module(kind).collect(kind, state):
            identity = hashlib.sha256(str(row.get('id', row.get('value', row.get('label')))).encode(errors='surrogateescape')).hexdigest()
            if identity in seen:
                continue
            seen.add(identity)
            save(session / (identity + '.json'), row)
            with (session / 'index').open('a') as index:
                index.write(identity + '\n')
            if kind not in SEARCH_DETAILS or not query:
                sys.stdout.buffer.write(record(identity, row))
                sys.stdout.buffer.flush()
            count += 1
        state['status'] = f'{count} items | Complete'
    except (RuntimeError, OSError, ValueError, KeyError, subprocess.SubprocessError) as exc:
        state['status'] = f'{count} items | Incomplete: {clean(exc)}'
        identity = '0' * 64
        save(session / (identity + '.json'), {'error': True, 'label': 'Unavailable', 'detail': clean(exc)})
        sys.stdout.buffer.write((identity + '\t' + clean(exc) + '\t\0').encode(errors='replace'))
        sys.stdout.buffer.flush()
    save(session / 'status.json', {'status': state['status']})
    if kind in SEARCH_DETAILS and query:
        filter_rows(session, query)


def preview(session, identity):
    state = load(session)
    if state.get('help'):
        return '\n'.join(f'{key:25} {description}' for key, description in controls(state['kind'], state).items())
    path = session / (identity + '.json')
    if not path.is_file():
        return 'No selection'
    row = json.loads(path.read_text())
    content = row.get('detail', '')
    if not content and state['kind'] in git_files.KINDS:
        content = git_files.preview(row)
    if not content and row.get('path') and Path(row['path']).is_file():
        try:
            with Path(row['path']).open('rb') as stream:
                raw = stream.read(200000)
            if b'\0' in raw:
                content = '[Binary file]'
            else:
                lines = raw.decode(errors='replace').splitlines()
                number = row.get('line', 1)
                content = '\n'.join(f'{i + 1:5} {line}' for i, line in enumerate(lines) if max(0, number - 15) <= i < number + 150)
        except OSError as exc:
            content = str(exc)
    if not content:
        content = shlex.join(row['argv']) if row.get('argv') else str(row.get('value', ''))
    return '\n'.join([clean(row.get('label', '')), '', *[clean(line) for line in str(content).splitlines()[:500]]])


def copy_text(text):
    if not text.strip():
        raise RuntimeError('No populated value to copy.')
    subprocess.run(['pbcopy'], input=text.encode(), check=True)


def values(rows):
    return [str(value) for row in rows for value in (row.get('value') if isinstance(row.get('value'), list) else [row.get('value', '')])]


def default_action(rows, key, state):
    if not rows:
        return None
    row = rows[0]
    if key == 'alt-y':
        return {'type': 'copy', 'text': '\n'.join(values(rows))}
    if key == 'alt-q':
        located = [{'filename': r['path'], 'lnum': r.get('line', 1), 'text': r.get('text', r['label'])} for r in rows if r.get('path')]
        if not located:
            raise RuntimeError('No file locations in the selection.')
        target = Path.home() / '.local/state/fzf-pickers/quickfix.json'
        target.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
        save(target, located)
        target.chmod(0o600)
        expression = "setqflist(json_decode('" + json.dumps(located).replace("'", "''") + "'))"
        socket = os.environ.get('NVIM')
        if socket:
            subprocess.run(['nvim', '--server', socket, '--remote-expr', expression], check=True, stdout=subprocess.DEVNULL)
            return {'type': 'reload'}
        return {'type': 'command', 'argv': ['nvim', '-c', 'call ' + expression, '-c', 'copen']}
    if key in ('ctrl-o', 'ctrl-s', 'alt-c') or row.get('action') in ('edit', 'cd'):
        path = row.get('path')
        if not path:
            raise RuntimeError('This selection has no local path.')
        if key == 'alt-c' or row.get('action') == 'cd' and key == 'enter':
            if len(rows) != 1:
                raise RuntimeError('Select one directory for this action.')
            directory = path if Path(path).is_dir() else str(Path(path).parent)
            return {'type': 'command', 'argv': ['cd', '--', directory]}
        if key == 'ctrl-o':
            return {'type': 'command', 'argv': ['open' if sys.platform == 'darwin' else 'xdg-open', *dict.fromkeys(r['path'] for r in rows if r.get('path'))]}
        argv = shlex.split(os.environ.get('EDITOR_CMD') or os.environ.get('EDITOR') or 'nvim')
        if row.get('line'):
            argv += ['+' + str(int(row['line']))]
        argv += ['--', *dict.fromkeys(r['path'] for r in rows if r.get('path'))]
        return {'type': 'command', 'argv': (['sudo'] if key == 'ctrl-s' else []) + argv}
    action = row.get('action', 'insert')
    if action == 'none':
        return {'type': 'reload'}
    if action == 'next':
        return {'type': 'next', 'kind': row['next'], 'state': row.get('state', {})}
    if action == 'command':
        return {'type': 'command', **{name: row[name] for name in ('argv', 'text') if name in row}}
    if action == 'copy':
        return {'type': 'copy', 'text': '\n'.join(values(rows))}
    if action == 'insert':
        return {'type': 'insert', 'values': values(rows)}
    raise RuntimeError('This item has no selectable action.')


def choose_action(session, rows, key):
    state = load(session)
    return module(state['kind']).accept(state['kind'], rows, key, state) or default_action(rows, key, state)


def picker(state):
    with tempfile.TemporaryDirectory(prefix='fzf-picker-') as directory:
        session = Path(directory)
        save(session / 'state.json', state)
        command = shlex.join([sys.executable, str(Path(__file__).resolve())])
        argument = shlex.quote(directory)
        while True:
            state = load(session)
            kind = state['kind']
            keys = controls(kind, state)
            reload_command = f'{command} rows {argument} {{q}}'
            args = ['fzf', '--read0', '--print0', '--print-query', '--delimiter=\t', '--with-nth=2', '--nth=1',
                    '--id-nth=1', '--track', '--multi', '--no-sort', '--query=' + state.get('query', ''),
                    '--header=' + header(session), '--footer= ',
                    '--footer-border=rounded', '--footer-label= Ctrl-/ Alt-/: help | Alt-Y: copy | Enter: accept | Esc: cancel ', '--footer-label-pos=0',
                    '--preview=' + f'{command} preview {argument} {{1}} {{q}}',
                    '--bind=' + f'load:transform-header({command} header {argument})',
                    '--bind=' + f'alt-r:reload-sync({reload_command})',
                    '--bind=' + f'alt-y:execute-silent({command} copy {argument} {{+1}})+transform-header({command} header {argument})',
                    '--bind=' + f'ctrl-/:execute-silent({command} help {argument})+refresh-preview',
                    '--bind=' + f'ctrl-_:execute-silent({command} help {argument})+refresh-preview',
                    '--bind=' + f'alt-/:execute-silent({command} help {argument})+refresh-preview']
            for key in TOGGLES & keys.keys():
                args += ['--bind=' + f'{key}:reload-sync({reload_command} {key})']
            if state.get('checkout') or kind in ('directories', 'parents', 'bookmarks', 'worktrees', 'historical', 'macos-contact-call'):
                args.append('--no-multi')
            custom = set(module(kind).bindings(kind, state)) - TOGGLES - {'alt-y'}
            if kind in LOCATION_KINDS:
                custom |= {'ctrl-o', 'ctrl-s', 'alt-c', 'alt-q'}
            args += ['--expect=' + ','.join(sorted(custom | {'enter'}))]
            if kind in SEARCH_DETAILS:
                source_keys = ['alt-r', *sorted(TOGGLES & keys.keys())]
                paused = ','.join(['change', 'ctrl-r', *source_keys])
                gate = f'unbind({paused})+rebind(load)'
                args += ['--disabled', '--bind=start:' + gate,
                         '--bind=' + f'load:transform-header({command} header {argument})+unbind(load)+rebind({paused})+trigger(change)',
                         '--bind=' + f'change:reload-sync({command} filter {argument} {{q}})',
                         '--bind=' + f'ctrl-r:reload-sync({command} filter {argument} {{q}} toggle)']
                for key in source_keys:
                    suffix = '' if key == 'alt-r' else ' ' + key
                    args += ['--bind=' + f'{key}:{gate}+reload-sync({reload_command}{suffix})']
            if kind in ('contents', 'log-search'):
                args += ['--disabled', '--bind=' + f'change:reload({reload_command})']
            producer = subprocess.Popen([sys.executable, str(Path(__file__).resolve()), 'rows', directory, state.get('query', '')], stdout=subprocess.PIPE)
            result = subprocess.run(args, stdin=producer.stdout, stdout=subprocess.PIPE)
            producer.stdout.close()
            if producer.poll() is None:
                producer.terminate()
            producer.wait()
            if result.returncode != 0:
                return None
            output = result.stdout.decode(errors='surrogateescape').split('\0')
            if len(output) < 3:
                return None
            state = load(session)
            state['query'] = output[0]
            save(session / 'state.json', state)
            key = output[1] or 'enter'
            rows = row_ids(session, [record.split('\t', 1)[0] for record in output[2:] if record])
            try:
                action = choose_action(session, rows, key)
                if not action:
                    continue
                action_type = action['type']
                if action_type == 'done':
                    return None
                if action_type == 'copy':
                    copy_text(action['text'])
                    return None
                if action_type == 'open':
                    subprocess.run(['open', action['url']], check=True)
                    return None
                if action_type in ('next', 'reload'):
                    if action_type == 'next':
                        state.update(kind=action['kind'], query='')
                    state.update(action.get('state', {}))
                    save(session / 'state.json', state)
                    continue
                return action
            except (RuntimeError, OSError, ValueError, subprocess.SubprocessError) as exc:
                state['notice'] = clean(exc)
                save(session / 'state.json', state)
                with open('/dev/tty', 'w') as tty:
                    print(clean(exc), file=tty)


def main():
    operation = sys.argv[1]
    if operation == 'cleanup':
        path = Path(sys.argv[2])
        if path.name.startswith(('fzf-result.', 'fzf-provenance.')) and path.is_file():
            path.unlink()
        return
    if operation != 'run':
        session = Path(sys.argv[2])
        if operation == 'rows':
            emit_rows(session, sys.argv[3], sys.argv[4] if len(sys.argv) > 4 else None)
        elif operation == 'filter':
            filter_rows(session, sys.argv[3], len(sys.argv) > 4)
        elif operation == 'header':
            print(header(session))
        elif operation == 'preview':
            print(preview(session, sys.argv[3]))
        elif operation == 'help':
            state = load(session)
            state['help'] = not state.get('help')
            save(session / 'state.json', state)
        elif operation == 'copy':
            state = load(session)
            try:
                action = choose_action(session, row_ids(session, sys.argv[3:]), 'alt-y')
                if action and action['type'] == 'copy':
                    copy_text(action['text'])
                    state['notice'] = 'Copied selection'
            except (RuntimeError, OSError, ValueError, subprocess.SubprocessError) as exc:
                state['notice'] = clean(exc)
            save(session / 'state.json', state)
        return
    parser = argparse.ArgumentParser()
    parser.add_argument('kind')
    parser.add_argument('--root')
    parser.add_argument('--hidden', action='store_true', default=None)
    parser.add_argument('--checkout', action='store_true')
    parser.add_argument('--paths', nargs='*')
    parser.add_argument('--provenance')
    options = vars(parser.parse_args(sys.argv[2:]))
    state = {k: v for k, v in options.items() if v is not None}
    state.update(cwd=os.getcwd(), config=str(Path.home() / '.config/fzf-pickers/config.toml'), query='')
    state.setdefault('hidden', state['kind'] in ('contents', 'directories'))
    state['browser'] = 'arc' if state['kind'] == 'tabs' else None
    if state.get('provenance'):
        raw = Path(state['provenance']).read_bytes().decode(errors='replace').split('\0')
        records = [{'name': raw[i], 'kind': raw[i + 1].strip(), 'detail': raw[i + 2], 'value': raw[i]} for i in range(0, len(raw) - 2, 3)]
        Path(state['provenance']).write_text(json.dumps(records))
        state['provenance_file'] = state['provenance']
    action = picker(state)
    if action and action['type'] in ('insert', 'command'):
        fields = action['values'] if action['type'] == 'insert' else [action.get('text') or shlex.join(action['argv'])]
        sys.stdout.buffer.write(('\0'.join([action['type'], *fields]) + '\0').encode(errors='surrogateescape'))


if __name__ == '__main__':
    try:
        main()
    except (BrokenPipeError, KeyboardInterrupt):
        sys.exit(130)
    except (RuntimeError, OSError, ValueError, KeyError, subprocess.SubprocessError) as error:
        print('fzf picker: ' + clean(error), file=sys.stderr)
        sys.exit(1)
