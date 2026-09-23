from collections import deque
from datetime import datetime
from itertools import islice
import json
import os
from pathlib import Path
import re
import sqlite3
import stat
import time
import tomllib
from urllib.error import HTTPError, URLError
from urllib.parse import quote, urlencode, urlsplit
from urllib.request import HTTPRedirectHandler, Request, build_opener


class NoRedirect(HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        raise RuntimeError('BlueBubbles redirected the request. Update its URL in the picker configuration.')


def request(path, body, state):
    config_path = Path(state.get('config', '~/.config/fzf-pickers/config.toml')).expanduser()
    try:
        if stat.S_IMODE(config_path.stat().st_mode) & 0o077:
            raise RuntimeError('Picker credentials must be private. Set config.toml permissions to 0600.')
        with config_path.open('rb') as stream:
            config = tomllib.load(stream)['bluebubbles']
        base, server_password = config['url'], config['server_password']
        if not isinstance(base, str):
            raise ValueError('invalid URL type')
        base = base.rstrip('/')
    except (OSError, KeyError, TypeError, ValueError) as exc:
        raise RuntimeError('BlueBubbles requires url and server_password in ~/.config/fzf-pickers/config.toml.') from exc
    parts = urlsplit(base)
    if not isinstance(server_password, str) or not server_password:
        raise RuntimeError('BlueBubbles server_password is empty in the picker configuration.')
    if parts.username or parts.password or parts.query or parts.fragment or not parts.hostname:
        raise RuntimeError('BlueBubbles URL must contain only its scheme, host, port, and optional base path.')
    if parts.scheme != 'https' and not (parts.scheme == 'http' and parts.hostname in ('localhost', '127.0.0.1', '::1')):
        raise RuntimeError('BlueBubbles requires HTTPS outside this Mac.')
    req = Request(base + '/api/v1' + path + '?' + urlencode({'password': server_password}),
                  data=json.dumps(body).encode(), headers={'Content-Type': 'application/json'}, method='POST')
    try:
        with build_opener(NoRedirect()).open(req, timeout=20) as response:
            result = json.load(response)
    except HTTPError as exc:
        raise RuntimeError(f'BlueBubbles request failed (HTTP {exc.code}). Check its URL and server password.') from None
    except (URLError, OSError, ValueError) as exc:
        raise RuntimeError('BlueBubbles is unavailable or returned an invalid response. Check its server and picker configuration.') from None
    if not isinstance(result, dict) or not isinstance(result.get('data'), list):
        raise RuntimeError('BlueBubbles returned an unexpected response.')
    return result


def paginate(path, body, state, limit=200):
    offset = 0
    seen = set()
    verify_chats = path == '/chat/query'
    first_ids = None
    expected_total = None
    while True:
        result = request(path, {**body, 'offset': offset, 'limit': limit}, state)
        page = result['data']
        total = result.get('metadata', {}).get('total')
        identities = []
        for item in page:
            if not isinstance(item, dict) or not isinstance(item.get('guid'), str) or not item['guid']:
                raise RuntimeError('BlueBubbles returned a record without a stable identifier.')
            identities.append(item['guid'])
        if verify_chats:
            if type(total) is not int or total < 0:
                raise RuntimeError('BlueBubbles omitted its chat count; results are incomplete.')
            if first_ids is None:
                first_ids, expected_total = set(identities), total
            if total != expected_total or seen.intersection(identities) or len(set(identities)) != len(identities):
                raise RuntimeError('BlueBubbles conversations changed during loading; results are incomplete. Press Alt-R to refresh.')
        if not page:
            break
        fresh = 0
        for item, identity in zip(page, identities):
            if identity in seen:
                continue
            seen.add(identity)
            fresh += 1
            yield item
        if not fresh:
            raise RuntimeError('BlueBubbles pagination did not advance; results are incomplete.')
        offset += len(page)
        if isinstance(total, int) and offset >= total:
            break
    if verify_chats:
        check = request(path, {**body, 'offset': 0, 'limit': limit}, state)
        current_ids = {item.get('guid') for item in check['data'] if isinstance(item, dict)}
        if (len(seen) != expected_total or check.get('metadata', {}).get('total') != expected_total
                or current_ids != first_ids):
            raise RuntimeError('BlueBubbles conversations changed during loading; results are incomplete. Press Alt-R to refresh.')


def normalize_address(address):
    address = address.strip()
    if '@' in address:
        return address.casefold()
    return re.sub(r'[\s().-]', '', address)


def message_row(message):
    sender = 'Me' if message.get('isFromMe') else (message.get('handle') or {}).get('address', '(unknown sender)')
    raw_date = message.get('dateCreated')
    try:
        date = datetime.fromtimestamp(float(raw_date) / 1000).astimezone().isoformat(timespec='seconds')
    except (TypeError, ValueError, OverflowError):
        date = '(unknown date)'
    text = message.get('text') or ''
    attachments = message.get('attachments') or []
    summary = text or ('[attachments]' if attachments else '[message without text]')
    chats = message.get('chats') or []
    return {'id': message['guid'], 'label': f'{date} {sender}: {summary}',
            'detail': f'{date}\n{sender}\n' + ', '.join(chat.get('guid', '') for chat in chats) + '\n\n' + text,
            'value': text, 'action': 'copy', 'message_guid': message['guid'], 'sender': sender,
            'date': date, 'chat_guid': chats[0].get('guid') if chats else None, 'attachments': attachments}


def attachment_path(guid, state):
    path = Path(state.get('messages_db', '~/Library/Messages/chat.db')).expanduser().resolve()
    try:
        with sqlite3.connect(path.as_uri() + '?mode=ro', uri=True, timeout=1) as connection:
            row = connection.execute('SELECT filename FROM attachment WHERE guid = ?', (guid,)).fetchone()
    except sqlite3.Error:
        return None
    if not row or not row[0]:
        return None
    filename = os.path.expanduser(row[0])
    return filename if Path(filename).is_file() else None


def collect(kind, state):
    if kind == 'messages':
        for target, label, detail in (
            ('chats', 'Conversations', 'Choose a conversation, then search its history.'),
            ('message-history', 'All messages', 'Search normalized message text across conversations.'),
            ('message-links', 'Links', 'Search links with their parent message and sender.'),
            ('message-attachments', 'Attachments', 'Search attachments and prepare opening the original file.'),
        ):
            yield {'id': target, 'label': label, 'detail': detail, 'value': target, 'action': 'next', 'next': target}
        return
    if kind == 'chats':
        addresses = {normalize_address(address) for address in state.get('contact_addresses', [])}
        for chat in paginate('/chat/query', {'with': ['participants', 'lastMessage'], 'sort': 'lastmessage'}, state):
            participants = [person.get('address', '') for person in chat.get('participants', [])]
            if addresses and not addresses.intersection(normalize_address(person) for person in participants):
                continue
            label = chat.get('displayName') or ', '.join(participants) or chat['guid']
            last = chat.get('lastMessage') or {}
            yield {'id': chat['guid'], 'label': label, 'detail': label + '\n' + '\n'.join(participants) + '\n\n' + (last.get('text') or ''),
                   'value': chat['guid'], 'chat_guid': chat['guid'], 'action': 'next', 'next': 'message-history',
                   'state': {'chat_guid': chat['guid']}}
        return
    if kind not in ('message-history', 'message-links', 'message-attachments'):
        raise RuntimeError(f'Unknown Messages picker: {kind}')
    before = state.get('messages_before') or int(time.time() * 1000)
    body = {'with': ['chats', 'chats.participants', 'attachments'], 'sort': 'DESC',
            'before': before, 'convertAttachments': False}
    if state.get('chat_guid'):
        body['chatGuid'] = state['chat_guid']
    previous = deque(maxlen=2)
    source = iter(paginate('/message/query', body, state))
    upcoming = deque(islice(source, 3))
    while upcoming:
        message = upcoming.popleft()
        row = message_row(message)
        if not row['chat_guid']:
            row['chat_guid'] = state.get('chat_guid')
        neighbors = [*previous, *(message_row(item) for item in upcoming)]
        context = '\n\n'.join(f"{item['date']} {item['sender']}\n{item['value']}" for item in neighbors if (item['chat_guid'] or state.get('chat_guid')) == row['chat_guid'])
        if kind == 'message-history':
            if context:
                row['detail'] += '\n\nNearby messages in this conversation:\n' + context
            yield row
        elif kind == 'message-links':
            for index, match in enumerate(re.finditer(r'https?://[^\s<>\uFFFC]+', row['value'])):
                url = match.group().rstrip('.,;!?)]}')
                yield {**row, 'id': f"{row['id']}:{index}", 'label': f"{url}  {row['sender']}  {row['date']}",
                       'value': url, 'detail': url + '\n\n' + row['detail']}
        else:
            for attachment in row['attachments']:
                guid = attachment.get('guid')
                if not guid:
                    continue
                path = attachment_path(guid, state)
                name = attachment.get('transferName') or guid
                yield {**row, 'id': f"{row['id']}:{guid}", 'label': f"{name}  {row['sender']}  {row['date']}",
                       'value': path or '', 'path': path, 'action': 'command',
                       'argv': ['open', path] if path else [],
                       'detail': f"{name}\n{path or 'Original attachment unavailable or Messages database access denied.'}\n\n{row['detail']}"}
        previous.append(row)
        upcoming.extend(islice(source, 1))


def accept(kind, rows, key, state):
    if not rows:
        return None
    row = rows[0]
    if kind == 'chats' and key == 'enter':
        return {'type': 'next', 'kind': 'message-history', 'state': {'chat_guid': row['chat_guid']}}
    if key == 'alt-a' and kind in ('message-history', 'message-links'):
        return {'type': 'copy', 'text': '\n\n'.join(f"{item['date']} {item['sender']}\n{item['value']}" for item in rows)}
    if key == 'alt-m' and row.get('chat_guid'):
        return {'type': 'next', 'kind': 'message-history', 'state': {'chat_guid': row['chat_guid']}}
    if kind == 'message-attachments' and key in ('enter', 'ctrl-o'):
        paths = [item.get('path') for item in rows]
        if not all(path and Path(path).is_file() for path in paths):
            raise RuntimeError('Original attachment unavailable. Check Messages Full Disk Access and downloaded attachments.')
        return {'type': 'command', 'argv': ['open', *paths]}
    if key == 'enter' and kind in ('message-history', 'message-links'):
        return {'type': 'copy', 'text': '\n\n'.join(item['value'] for item in rows)}
    return None


def bindings(kind, state):
    if kind in ('message-history', 'message-links'):
        return {'alt-a': 'Copy with date and sender', 'alt-m': 'Open conversation history'}
    return {}
