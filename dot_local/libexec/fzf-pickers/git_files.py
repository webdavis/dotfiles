import base64
import json
import os
import re
from pathlib import Path
import shlex
import subprocess

KINDS = {'files', 'directories', 'parents', 'bookmarks', 'contents', 'tracked', 'git-files',
         'changed', 'historical', 'historical-files', 'commits', 'refs', 'stashes', 'reflog',
         'log-search', 'worktrees', 'ignored', 'conflicts', 'git-config'}


def run(args, cwd, allowed=(0,)):
    result = subprocess.run(args, cwd=cwd, capture_output=True)
    if result.returncode not in allowed:
        raise RuntimeError(result.stderr.decode(errors='replace').strip() or f'{args[0]} exited {result.returncode}')
    return result.stdout


def git(cwd, *args, allowed=(0,)):
    return run(['git', '--no-pager', '-c', 'core.quotePath=false', *args], cwd, allowed)


def root(cwd):
    result = subprocess.run(['git', 'rev-parse', '--show-toplevel'], cwd=cwd, capture_output=True)
    return os.fsdecode(result.stdout).rstrip('\n') if result.returncode == 0 else None


def strings(data):
    return [os.fsdecode(p) for p in data.split(b'\0') if p]


def paths(cwd, state, directories=False):
    start = state.get('root', cwd)
    start = str(Path.home()) if start == 'home' else start
    start = root(cwd) if start == 'git' else start
    if not start:
        raise RuntimeError('This directory is outside a Git repository.')
    start = os.path.abspath(start)
    max_depth = 3 if start == '/' else 2 if start == str(Path.home()) else None
    for directory, dirs, files in os.walk(start):
        dirs[:] = [d for d in dirs if d != '.git' and (state.get('hidden') or not d.startswith('.'))]
        depth = len(Path(directory).relative_to(start).parts)
        if max_depth is not None and depth >= max_depth:
            dirs[:] = []
        for name in dirs if directories else files:
            if state.get('hidden') or not name.startswith('.'):
                path = os.path.join(directory, name)
                patterns = state.get('file_patterns', [])
                if patterns and not any(Path(path).match(pattern if '*' in pattern else '*' + pattern) for pattern in patterns):
                    continue
                yield path


def file_row(path, cwd, label=None, **extra):
    return {'id': path, 'label': label or os.path.relpath(path, cwd), 'path': path,
            'value': os.path.relpath(path, cwd), 'action': 'edit', **extra}


def collect(kind, state):
    cwd = state['cwd']
    project = state.get('worktree') or root(cwd)
    if kind in ('files', 'directories'):
        rows = [file_row(p, cwd, action='cd' if kind == 'directories' else 'edit')
                for p in paths(cwd, state, kind == 'directories')]
        return sorted(rows, key=lambda r: os.path.getmtime(r['path']) if state.get('sort') else r['path'], reverse=bool(state.get('sort')))
    if kind in ('parents', 'bookmarks'):
        names = [str(p) for p in Path(cwd).parents] if kind == 'parents' else state.get('paths', [])
        return [file_row(p, cwd, label=p, action='cd') for p in dict.fromkeys(names) if os.path.isdir(p)]
    if kind == 'contents':
        return contents(project or cwd, state, bool(project))
    if not project:
        raise RuntimeError('This picker needs a Git repository.')
    if kind in ('tracked', 'git-files'):
        args = ['ls-files', '-z', '--cached']
        if kind == 'git-files':
            args += ['--others', '--exclude-standard']
        return [file_row(os.path.join(project, p), cwd) for p in dict.fromkeys(strings(git(project, *args))) if '.git' not in Path(p).parts]
    if kind == 'changed':
        names = strings(git(project, 'diff', '--name-only', '--no-renames', '-z'))
        names += strings(git(project, 'diff', '--cached', '--name-only', '--no-renames', '-z'))
        names += strings(git(project, 'ls-files', '-z', '--others', '--exclude-standard'))
        return [file_row(os.path.join(project, p), cwd, action='insert', value=p,
                         project=project, changed=True)
                for p in dict.fromkeys(names)]
    if kind in ('commits', 'historical', 'reflog', 'log-search'):
        args = ['reflog'] if kind == 'reflog' else ['log']
        args += ['-500', '--format=%H%x00%h %s (%cr, %an)%x00']
        if kind == 'log-search':
            if not state.get('query'):
                return []
            args += ['--no-ext-diff', '--no-textconv', '-G' + state['query']]
        values = strings(git(project, *args))
        rows = []
        for i in range(0, len(values) - 1, 2):
            commit, label = values[i].strip(), values[i + 1]
            row = {'id': commit, 'label': label, 'value': commit, 'commit': commit, 'project': project, 'action': 'insert'}
            if kind == 'historical':
                row.update(action='next', next='historical-files', state={'commit': commit})
            rows.append(row)
        return list(reversed(rows)) if state.get('sort') else rows
    if kind == 'historical-files':
        commit = state['commit']
        rows = []
        for item in git(project, 'ls-tree', '-rz', commit).split(b'\0'):
            if not item:
                continue
            metadata, name = item.split(b'\t', 1)
            mode, kind_name, blob = metadata.decode().split()
            if kind_name != 'blob':
                continue
            path = os.fsdecode(name)
            rows.append({'id': commit + ':' + path, 'label': path, 'value': path,
                         'blob': blob, 'project': project, 'action': 'historical'})
        return rows
    if kind == 'refs':
        records = git(project, 'for-each-ref', '--format=%(refname)%09%(refname:short)%09%(subject)', 'refs/heads', 'refs/remotes', 'refs/tags')
        return [{'id': parts[0], 'label': f'{parts[1]}  {parts[2]}', 'value': parts[0], 'commit': parts[0], 'project': project, 'action': 'insert'}
                for line in records.decode(errors='replace').splitlines() if len(parts := line.split('\t', 2)) == 3 and not parts[0].endswith('/HEAD')]
    if kind == 'stashes':
        values = strings(git(project, 'stash', 'list', '--format=%gd%x00%gs%x00'))
        return [{'id': values[i].strip(), 'label': f'{values[i].strip()} {values[i + 1]}', 'value': values[i].strip(),
                 'commit': values[i].strip(), 'project': project, 'action': 'insert'} for i in range(0, len(values) - 1, 2)]
    if kind == 'worktrees':
        records = strings(git(project, 'worktree', 'list', '--porcelain', '-z'))
        return [{'id': p, 'label': p, 'value': p, 'action': 'next', 'next': 'contents', 'state': {'worktree': p, 'query': '', 'hidden': True}}
                for record in records if record.startswith('worktree ') and (p := record[9:]) != project]
    if kind == 'ignored':
        names = git(project, 'ls-files', '--others', '--ignored', '--exclude-standard', '-z')
        result = subprocess.run(['git', 'check-ignore', '-z', '-v', '--stdin'], cwd=project, input=names, capture_output=True)
        if result.returncode not in (0, 1):
            raise RuntimeError(result.stderr.decode(errors='replace').strip())
        values = strings(result.stdout)
        return [file_row(os.path.abspath(os.path.join(project, values[i])), cwd,
                         label=values[i + 3], id=values[i + 3], line=int(values[i + 1]),
                         detail=f'{values[i + 3]}\nRule: {values[i + 2]}\nSource: {values[i]}:{values[i + 1]}')
                for i in range(0, len(values) - 3, 4)]
    if kind == 'conflicts':
        stages = {}
        for record in git(project, 'ls-files', '-u', '-z').split(b'\0'):
            if record:
                metadata, name = record.split(b'\t', 1)
                mode, blob, stage = metadata.decode().split()
                stages.setdefault(os.fsdecode(name), {})[stage] = blob
        return [file_row(os.path.join(project, p), cwd, stages=s, project=project) for p, s in stages.items()]
    if kind == 'git-config':
        values = strings(git(project, 'config', '--null', '--list', '--show-origin', '--show-scope'))
        rows = []
        for i in range(0, len(values) - 2, 3):
            scope, origin, entry = values[i:i + 3]
            name, _, value = entry.partition('\n')
            sensitive = any(word in name.lower() for word in ('password', 'token', 'secret', 'extraheader', 'credential'))
            def redact(text):
                text = re.sub(r'(?i)(https?://)[^/\s@]+@', r'\1[redacted]@', text)
                return re.sub(r'(?i)([?&](?:token|access_token|password|api_key|apikey)=)[^&\s]+', r'\1[redacted]', text)
            safe_name = redact(name)
            shown = '[redacted]' if sensitive else redact(value)
            row = {'id': f'{scope}:{origin}:{name}', 'label': f'{scope}  {safe_name} = {shown}',
                   'detail': f'{origin}\n{safe_name} = {shown}', 'value': safe_name, 'action': 'insert'}
            if origin.startswith('file:'):
                row.update(path=os.path.abspath(os.path.join(project, origin[5:])), action='edit')
            rows.append(row)
        return rows
    raise RuntimeError(f'Unknown Git picker: {kind}')


def contents(project, state, in_git):
    query = state.get('query', '')
    if not query:
        return []
    if in_git:
        args = ['ls-files', '-z', '--others', '--exclude-standard'] if state.get('untracked') else ['ls-files', '-z', '--cached']
        names = strings(git(project, *args))
    else:
        names = list(paths(project, {**state, 'root': project}))
    names = [p for p in names if os.path.isfile(os.path.join(project, p)) and '.git' not in Path(p).parts and (state.get('hidden', True) or not any(part.startswith('.') for part in Path(os.path.relpath(p, project) if os.path.isabs(p) else p).parts))]
    rows = []
    for offset in range(0, len(names), 128):
        args = ['rg', '--json', '--smart-case', '--hidden', '--no-ignore']
        if not state.get('regex'):
            args.append('--fixed-strings')
        args += ['--', query, *names[offset:offset + 128]]
        result = subprocess.run(args, cwd=project, capture_output=True)
        if result.returncode not in (0, 1):
            raise RuntimeError(result.stderr.decode(errors='replace').strip())
        for line in result.stdout.splitlines():
            event = json.loads(line)
            if event['type'] != 'match':
                continue
            data = event['data']
            def content(value):
                return value['text'] if 'text' in value else os.fsdecode(base64.b64decode(value['bytes']))
            path = os.path.abspath(os.path.join(project, content(data['path'])))
            number = data['line_number']
            text = content(data['lines']).rstrip('\n')
            rows.append(file_row(path, project, label=f'{os.path.relpath(path, project)}:{number}: {text}',
                                 id=f'{path}:{number}', line=number, text=text, project=project))
    if state.get('sort'):
        rows.sort(key=lambda row: (os.path.getmtime(row['path']), row['line']), reverse=True)
    return rows


def preview(row):
    project = row.get('project')
    if row.get('changed'):
        relative = os.path.relpath(row['path'], project)
        sections = []
        for label, extra in [('Staged', ['--cached']), ('Unstaged', [])]:
            diff = git(project, 'diff', '--no-color', '--no-ext-diff', '--no-textconv', *extra, '--', relative).decode(errors='replace')
            if diff:
                sections.append(label + '\n' + diff)
        if sections:
            return '\n'.join(sections)
    if row.get('blob'):
        return git(project, 'cat-file', 'blob', row['blob']).decode(errors='replace')
    if row.get('commit'):
        return git(project, 'show', '--no-color', '--no-ext-diff', '--no-textconv', '--stat', '--patch', row['commit']).decode(errors='replace')
    if row.get('stages'):
        parts = []
        for number, label in [('1', 'Base'), ('2', 'Ours'), ('3', 'Theirs')]:
            blob = row['stages'].get(number)
            text = git(project, 'cat-file', 'blob', blob).decode(errors='replace') if blob else '(stage absent)'
            parts.append(f'{label}\n{text}')
        return '\n'.join(parts)
    return None


def bindings(kind, state):
    keys = {}
    if kind in ('files', 'directories', 'contents'):
        keys.update({'alt-h': 'Show/hide hidden files', 'alt-s': 'Path/modification time'})
    if kind == 'contents':
        keys['alt-x'] = 'Literal/regular expression'
        if state.get('worktree') or root(state['cwd']):
            keys.update({'alt-u': 'Tracked only/untracked only', 'alt-l': 'Committed line history'})
    if kind in ('commits', 'reflog', 'log-search', 'historical'):
        keys['alt-s'] = 'Newest/oldest first'
    if kind in ('commits', 'reflog', 'log-search', 'stashes', 'refs'):
        keys['alt-i'] = 'Inspect in pager'
    return keys


def accept(kind, rows, key, state):
    toggles = {'alt-h': 'hidden', 'alt-u': 'untracked', 'alt-x': 'regex', 'alt-s': 'sort'}
    if key in toggles:
        name = toggles[key]
        return {'type': 'reload', 'state': {name: not state.get(name)}}
    if not rows:
        return None
    row = rows[0]
    if key == 'alt-l':
        project, path = row['project'], row['path']
        relative = os.path.relpath(path, project)
        current = Path(path).read_bytes()
        committed = git(project, 'show', 'HEAD:' + relative, allowed=(0, 128))
        if committed != current:
            raise RuntimeError('The file differs from HEAD. Select a committed version before tracing line history.')
        number = row['line']
        return {'type': 'command', 'argv': ['git', '-C', project, 'log', '-L', f'{number},{number}:{relative}']}
    if key == 'alt-i':
        with open('/dev/tty', 'r+') as tty:
            subprocess.run(['git', '--paginate', '-C', row['project'], 'show', row['commit']],
                           stdin=tty, stdout=tty, stderr=tty, check=True)
        return {'type': 'reload'}
    if kind == 'historical-files' and key == 'enter':
        return {'type': 'command', 'text': shlex.join(['git', '-C', row['project'], 'cat-file', 'blob', row['blob']]) + ' | nvim -R -'}
    if kind == 'changed' and key == 'enter':
        return {'type': 'insert', 'values': [os.path.relpath(r['path'], state['cwd']) for r in rows]}
    if state.get('checkout') and key == 'enter':
        target = row['value']
        if target.startswith('refs/heads/'):
            target = target[len('refs/heads/'):]
        return {'type': 'command', 'argv': ['git', '-C', row['project'], 'checkout', target]}
    if kind == 'files' and state.get('return_kind') and key == 'enter':
        return {'type': 'next', 'kind': state['return_kind'], 'state': {'file': row['path'], 'return_kind': None}}
    return None
