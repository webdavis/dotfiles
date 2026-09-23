import hashlib
import json
import os
from pathlib import Path
import plistlib
import re
import subprocess
from urllib.parse import quote


MODES = {'alt-0': 'all', 'alt-1': 'name', 'alt-2': 'phone', 'alt-3': 'email',
         'alt-4': 'address', 'alt-5': 'work', 'alt-6': 'link'}


def run(argv, timeout=30, binary=False):
    try:
        result = subprocess.run(argv, capture_output=True, timeout=timeout, check=False)
    except (OSError, subprocess.TimeoutExpired) as exc:
        raise RuntimeError(f'{Path(argv[0]).name} unavailable or timed out. Check app access in Privacy & Security and dismiss pending app dialogs.') from exc
    if result.returncode:
        error = result.stderr.decode('utf-8', 'replace').strip()
        raise RuntimeError(f'{Path(argv[0]).name}: {error or "request failed"}')
    return result.stdout if binary else result.stdout.decode('utf-8', 'replace')


def native(action, **kwargs):
    script = Path(__file__).with_suffix('.js')
    return json.loads(run(['osascript', '-l', 'JavaScript', str(script), json.dumps({'action': action, **kwargs})]))


def contact_row(person, state):
    fields = [field for field in person['fields'] if str(field.get('value', '')).strip()]
    mode = state.get('contact_mode', 'all')
    chosen = fields if mode == 'all' else [field for field in fields if field['group'] == mode]
    card = '\n'.join(f"{field['label']}: {field['value']}" for field in fields)
    copied = card if mode == 'all' else '\n'.join(field['value'] for field in chosen)
    return {'id': person['id'], 'label': person['name'] or '(unnamed contact)',
            'detail': card + f'\n\nCopy: {mode}. Enter copies; Alt-Y copies and stays.',
            'value': copied, 'action': 'copy', 'contact': person}


def contact_fields(person, group=None):
    for field in person['fields']:
        if not str(field.get('value', '')).strip() or (group and field['group'] != group):
            continue
        yield {'id': person['id'] + ':' + field['id'], 'label': f"{field['label']}: {field['value']}",
               'detail': f"{person['name']}\n{field['label']}: {field['value']}",
               'value': field['value'], 'action': 'copy', 'group': field['group']}


def recent_files(state):
    scopes = state.get('recent_scopes', [str(Path.home() / 'Documents'), str(Path.home() / 'Downloads')])
    found = set()
    records = []
    query = 'kMDItemContentTypeTree == "public.data"'
    for scope in scopes:
        raw = run(['mdfind', '-0', '-onlyin', str(Path(scope).expanduser()), query], binary=True)
        found.update(os.fsdecode(path) for path in raw.split(b'\0') if path)
    excluded = [Path(path).expanduser().resolve() for path in state.get('recent_exclude', [])]
    for name in found:
        path = Path(name)
        if not path.is_file() or any(path.is_relative_to(exclusion) for exclusion in excluded):
            continue
        try:
            metadata = plistlib.loads(run(['mdls', '-plist', '-', str(path)], binary=True))
            last = metadata.get('kMDItemLastUsedDate')
            order = last.timestamp() if hasattr(last, 'timestamp') else path.stat().st_mtime
            detail = f"{path}\nKind: {metadata.get('kMDItemKind', 'unknown')}\nLast used: {last or 'not indexed'}"
            if metadata.get('kMDItemWhereFroms'):
                detail += '\nOrigin: ' + '\n'.join(metadata['kMDItemWhereFroms'])
            records.append((order, {'id': str(path), 'label': str(path), 'detail': detail,
                                   'path': str(path), 'value': str(path), 'action': 'command', 'argv': ['open', str(path)]}))
        except (OSError, plistlib.InvalidFileException):
            continue
    yield from (row for _, row in sorted(records, key=lambda pair: pair[0], reverse=True))


def collect(kind, state):
    if kind == 'contacts':
        for person in native('contacts'):
            yield contact_row(person, state)
    elif kind in ('contact-fields', 'macos-contact-call'):
        yield from contact_fields(state['contact'], 'phone' if kind == 'macos-contact-call' else None)
    elif kind == 'tabs':
        browser = state.get('browser', 'arc')
        for tab in native('tabs', browser=browser):
            space = tab.get('space') or {}
            yield {'id': browser + ':' + tab['id'], 'label': f"{tab['title']}  {tab['url']}",
                   'detail': f"{browser.title()}\n{tab['title']}\n{tab['url']}\n{space.get('title', '')} {tab.get('location', '')}\n\nEnter focuses this tab. Alt-Y copies its URL.",
                   'value': tab['url'], 'tab': tab, 'browser': browser}
    elif kind == 'recent':
        yield from recent_files(state)
    elif kind == 'shortcuts':
        for line in run(['shortcuts', 'list', '--show-identifiers']).splitlines():
            match = re.fullmatch(r'(.*) \(([0-9A-Fa-f-]{36})\)', line)
            if match:
                name, identifier = match.groups()
                yield {'id': identifier, 'label': name, 'detail': f'{name}\n{identifier}\n\nEnter prepares shortcuts run. Ctrl-O opens its editor.',
                       'value': identifier, 'action': 'command', 'argv': ['shortcuts', 'run', identifier]}
    elif kind == 'logs':
        yield {'id': 'unified', 'label': 'Unified logs from the last hour', 'detail': 'Read the local unified log. May require additional permissions.',
               'value': 'unified', 'action': 'next', 'next': 'macos-unified-logs'}
        yield {'id': 'crashes', 'label': 'Crash and hang reports', 'detail': 'Browse accessible DiagnosticReports and prepare opening the original file in Console.',
               'value': 'crashes', 'action': 'next', 'next': 'crashes'}
    elif kind == 'macos-unified-logs':
        argv = ['/usr/bin/log', 'show', '--style', 'ndjson', '--last', state.get('log_window', '1h'), '--no-pager']
        if state.get('log_predicate'):
            argv.extend(['--predicate', state['log_predicate']])
        for index, line in enumerate(run(argv, timeout=60).splitlines()):
            try:
                event = json.loads(line)
            except json.JSONDecodeError:
                continue
            text = event.get('eventMessage', '')
            timestamp = event.get('timestamp', '')
            process = event.get('processImagePath', '')
            identity = hashlib.sha256(line.encode()).hexdigest()
            yield {'id': f'{identity}:{index}', 'label': f'{timestamp} {Path(process).name} {text}',
                   'detail': json.dumps(event, indent=2, ensure_ascii=False), 'value': text, 'action': 'copy'}
    elif kind == 'crashes':
        reports = []
        for folder in (Path.home() / 'Library/Logs/DiagnosticReports', Path('/Library/Logs/DiagnosticReports')):
            if folder.is_dir():
                reports.extend(path for path in folder.rglob('*') if path.suffix in ('.ips', '.spin', '.crash', '.hang'))
        for path in sorted(reports, key=lambda item: item.stat().st_mtime, reverse=True):
            try:
                detail = path.read_text(errors='replace')
            except OSError:
                detail = 'Report unavailable. Check file permissions.'
            yield {'id': str(path), 'label': path.name, 'detail': detail, 'value': str(path), 'path': str(path),
                   'action': 'command', 'argv': ['open', '-a', 'Console', str(path)]}
    else:
        raise RuntimeError(f'Unknown macOS picker: {kind}')


def accept(kind, rows, key, state):
    if kind == 'contacts' and key in MODES:
        return {'type': 'reload', 'state': {'contact_mode': MODES[key]}}
    if kind == 'tabs' and key == 'alt-B':
        return {'type': 'reload', 'state': {'browser': 'arc' if state.get('browser', 'arc') == 'chrome' else 'chrome'}}
    if not rows:
        return None
    row = rows[0]
    if kind == 'contacts':
        person = row['contact']
        if key == 'alt-7':
            return {'type': 'next', 'kind': 'contact-fields', 'state': {'contact': person}}
        if key == 'ctrl-t':
            if len(rows) != 1:
                raise RuntimeError('Select one contact to call.')
            phones = list(contact_fields(person, 'phone'))
            if not phones:
                raise RuntimeError('This contact has no phone number.')
            if len(phones) == 1:
                return accept('macos-contact-call', phones, 'enter', state)
            return {'type': 'next', 'kind': 'macos-contact-call', 'state': {'contact': person}}
        if key == 'alt-m':
            addresses = [field['value'] for field in person['fields'] if field['group'] in ('phone', 'email') and field['value']]
            if not addresses:
                raise RuntimeError('This contact has no phone number or email address to match.')
            return {'type': 'next', 'kind': 'chats', 'state': {'contact_addresses': addresses}}
    if kind == 'macos-contact-call' and key == 'enter':
        number = re.sub(r'[\s().-]', '', row['value'])
        if not re.fullmatch(r'\+?[0-9]+(?:[;,][0-9]+)*', number):
            raise RuntimeError('The selected phone number cannot be used as a telephone URL.')
        return {'type': 'open', 'url': 'tel:' + quote(number, safe='+;,')}
    if kind == 'tabs' and key == 'enter':
        native('focus', browser=row['browser'], item=row['tab'])
        return {'type': 'done'}
    if kind == 'shortcuts' and key == 'ctrl-o':
        return {'type': 'command', 'argv': ['shortcuts', 'view', row['id']]}
    if key == 'enter' and kind in ('contacts', 'contact-fields'):
        return {'type': 'copy', 'text': '\n'.join(str(item['value']) for item in rows)}
    return None


def bindings(kind, state):
    if kind == 'contacts':
        return {**{key: 'Copy ' + mode for key, mode in MODES.items()}, 'alt-7': 'Choose populated fields',
                'ctrl-t': 'Call phone number', 'alt-m': 'Find conversations'}
    if kind == 'tabs':
        return {'alt-B': 'Switch Arc / Chrome'}
    if kind == 'shortcuts':
        return {'ctrl-o': 'Prepare opening Shortcut editor'}
    return {}
