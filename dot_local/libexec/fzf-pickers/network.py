import concurrent.futures
import datetime
import glob
import ipaddress
import json
import os
from pathlib import Path
import re
import shlex
import shutil
import ssl
import subprocess
import time
import tomllib
import urllib.error
import urllib.parse
import urllib.request


def run(argv, timeout=3):
    try:
        result = subprocess.run(argv, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                text=True, errors='replace', timeout=timeout, check=False)
        if result.returncode:
            raise ValueError(f'{Path(argv[0]).name} unavailable (exit {result.returncode})')
        return result.stdout
    except subprocess.TimeoutExpired as error:
        raise ValueError(f'{Path(argv[0]).name} timed out') from error
    except OSError as error:
        raise ValueError(f'{Path(argv[0]).name} unavailable') from error


def prompt(label, default=''):
    with open('/dev/tty', 'r+') as tty:
        tty.write(f'{label}' + (f' [{default}]' if default else '') + ': ')
        tty.flush()
        value = tty.readline()
    if not value:
        raise KeyboardInterrupt
    return value.strip() or default


def validate(name, value):
    value = str(value).strip()
    if name in ('port', 'count', 'duration'):
        if not value.isdecimal() or int(value) < 1 or (name == 'port' and int(value) > 65535):
            raise ValueError(f'{name} must be a positive integer' + (' up to 65535' if name == 'port' else ''))
    elif name == 'ip':
        ipaddress.ip_address(value)
    elif name == 'cidr':
        if '/' not in value:
            raise ValueError('Choose an explicit subnet, such as 192.168.1.0/24')
        value = str(ipaddress.ip_network(value, strict=False))
    elif name == 'host' and ':' in value:
        address, _, scope = value.partition('%')
        ipaddress.IPv6Address(address)
        if scope and not re.fullmatch(r'[A-Za-z0-9_.-]+', scope):
            raise ValueError('Invalid IPv6 scope')
    elif name in ('host', 'interface'):
        if not re.fullmatch(r'[A-Za-z0-9_][A-Za-z0-9_.:%-]*', value):
            raise ValueError(f'Invalid {name}')
    elif not value or any(ord(char) < 32 for char in value):
        raise ValueError(f'Invalid {name}')
    return value


def command_row(identity, title, argv=None, text=None, purpose='', settings=''):
    command = text if text is not None else shlex.join(argv)
    row = {'id': identity, 'label': title, 'detail': '\n'.join(filter(None, [title, command, purpose, settings, 'Enter prepares the command.'])),
           'value': command, 'action': 'command', 'search': ' '.join([command, purpose, settings])}
    if argv is not None:
        row['argv'] = argv
    else:
        row['text'] = command
    return row


def note(identity, message):
    return {'id': identity, 'label': message, 'detail': message, 'value': '', 'action': 'none'}


def catalog(state):
    deployed = Path.home() / '.config/fzf-pickers/network-actions.toml'
    source = Path(__file__).resolve().parents[3] / 'dot_config/fzf-pickers/network-actions.toml'
    path = Path(state.get('network_catalog', source if source.is_file() else deployed))
    with path.open('rb') as stream:
        return tomllib.load(stream)['action']


def network_actions(state):
    for item in catalog(state):
        title = f"{item['title']}  [{item['tool']}]"
        if item.get('next'):
            yield {'id': item['id'], 'label': title, 'detail': item['purpose'] + '\nEnter opens the selector.',
                   'value': item['title'], 'search': item['purpose'], 'action': 'next', 'next': item['next']}
        else:
            row = command_row(item['id'], title, text=item['command'], purpose=item['purpose'],
                              settings='Choose ' + ', '.join(item['params']) + ' on Enter.' if item.get('params') else '')
            row['params'] = item.get('params', [])
            row['search'] = item['purpose'] + ' ' + item['command']
            yield row


def pages(get, path):
    offset = 0
    seen = set()
    while True:
        page = get(path, offset)
        if not isinstance(page, dict) or not isinstance(page.get('data'), list):
            raise ValueError('Malformed UniFi page; results are incomplete')
        batch = page['data']
        limit = page.get('limit', 200)
        if type(limit) is not int or limit < 0 or (limit == 0 and batch):
            raise ValueError('Malformed UniFi limit; results are incomplete')
        if page.get('offset', offset) != offset:
            raise ValueError('UniFi pagination did not advance; results are incomplete')
        for item in batch:
            if not isinstance(item, dict):
                raise ValueError('Malformed UniFi device; results are incomplete')
            identity = item.get('id') or json.dumps(item, sort_keys=True)
            if identity in seen:
                raise ValueError('UniFi repeated a result; results are incomplete')
            seen.add(identity)
            yield item
        total = page.get('totalCount')
        if total is not None and (not isinstance(total, int) or total < 0):
            raise ValueError('Malformed UniFi total; results are incomplete')
        offset += len(batch)
        if total is not None and offset >= total:
            return
        if not batch:
            if total is not None and offset < total:
                raise ValueError('UniFi returned an empty early page; results are incomplete')
            return
        if total is None and len(batch) < limit:
            return


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        return None


def unifi_get(state):
    path = Path(state.get('config', Path.home() / '.config/fzf-pickers/config.toml'))
    if path.stat().st_mode & 0o077:
        raise ValueError('Picker credentials must be readable only by their owner (chmod 600)')
    with path.open('rb') as stream:
        config = tomllib.load(stream).get('unifi', {})
    base = str(config.get('url', '')).rstrip('/')
    parsed = urllib.parse.urlsplit(base)
    if parsed.scheme != 'https' or not parsed.hostname or parsed.username or parsed.password or parsed.query or parsed.fragment:
        raise ValueError('UniFi requires an HTTPS controller URL without credentials')
    key = config.get('api_key')
    if not isinstance(key, str) or not key:
        raise ValueError('UniFi API key is not configured')
    context = ssl.create_default_context(cafile=config.get('ca_file') or None)
    opener = urllib.request.build_opener(NoRedirect(), urllib.request.HTTPSHandler(context=context))

    def get(path, offset):
        request = urllib.request.Request(base + '/proxy/network/integration/v1' + path + '?' +
                                         urllib.parse.urlencode({'offset': offset, 'limit': 200}),
                                         headers={'X-API-Key': key, 'Accept': 'application/json'})
        try:
            with opener.open(request, timeout=4) as response:
                return json.load(response)
        except urllib.error.HTTPError as error:
            raise ValueError(f'UniFi HTTP {error.code}; results are incomplete') from None
        except (urllib.error.URLError, ssl.SSLError):
            raise ValueError('UniFi connection or certificate validation failed; configure a trusted CA if needed') from None
        except (json.JSONDecodeError, UnicodeError):
            raise ValueError('UniFi returned invalid JSON; results are incomplete') from None
    return get


def device_row(source, identity, name, address, fields):
    now = datetime.datetime.now(datetime.timezone.utc).isoformat(timespec='seconds')
    details = [f'Source: {source}', f'Observed: {now}']
    details.extend(f'{key}: {value}' for key, value in fields.items() if value not in ('', None, [], {}))
    return {'id': f'{source}:{identity}', 'label': f'{name or address or identity}  {address}',
            'detail': '\n'.join(details), 'search': ' '.join(details), 'value': address or name or identity, 'action': 'insert'}


def unifi_devices(state):
    try:
        get = unifi_get(state)
        for site in pages(get, '/sites'):
            site_id = urllib.parse.quote(str(site['id']), safe='')
            site_name = site.get('name', site['id'])
            for category in ('devices', 'clients'):
                try:
                    for device in pages(get, f'/sites/{site_id}/{category}'):
                        fields = {'Site': site_name, **device}
                        yield device_row(f'UniFi {category}', f"{site['id']}:{device.get('id', device.get('macAddress', ''))}",
                                         device.get('name', ''), device.get('ipAddress', ''), fields)
                except (ValueError, KeyError) as error:
                    yield note(f'unifi:{site_id}:{category}:error', f'{site_name} {category}: {error}')
    except (OSError, ValueError, KeyError):
        yield note('unifi:error', 'UniFi unavailable or incomplete: check private picker config, API access and trusted CA')


def parse_neighbors(output, source):
    rows = []
    for line in output.splitlines():
        if source == 'arp':
            match = re.match(r'(.+?) \(([^)]+)\) at (.+?) on (\S+)', line)
            if not match:
                continue
            name, address, mac, interface = match.groups()
            name = '' if name == '?' else name
            state = 'incomplete' if 'incomplete' in mac else 'cached'
        else:
            parts = line.split()
            if len(parts) < 5 or ':' not in parts[0]:
                continue
            address, mac, interface = parts[:3]
            name, state = '', ' '.join(parts[3:])
        rows.append(device_row(source.upper(), f'{address}:{interface}', name, address,
                               {'Name': name, 'IP': address, 'MAC': mac, 'Interface': interface, 'State': state}))
    return rows


def bounded_output(argv, timeout):
    try:
        return subprocess.run(argv, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
                              timeout=timeout, check=False).stdout.decode(errors='replace')
    except subprocess.TimeoutExpired as error:
        return (error.stdout or b'').decode(errors='replace')
    except OSError:
        return ''


def mdns_devices():
    # ponytail: browse three common service types; discover types when broader coverage is needed.
    types = ('_workstation._tcp', '_ssh._tcp', '_http._tcp')
    with concurrent.futures.ThreadPoolExecutor(max_workers=3) as pool:
        outputs = list(pool.map(lambda service: bounded_output(['dns-sd', '-B', service, 'local.'], 1.2), types))
    instances = []
    for service, output in zip(types, outputs):
        for line in output.splitlines():
            parts = line.split(None, 6)
            if len(parts) == 7 and parts[1] == 'Add':
                instances.append((parts[6], service, parts[4]))
    deadline = time.monotonic() + 2
    for name, service, domain in dict.fromkeys(instances):
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            yield note('mdns:partial', 'mDNS discovery window ended; use Alt-R to refresh')
            return
        output = bounded_output(['dns-sd', '-L', name, service, domain], min(remaining, 0.25))
        match = re.search(r'can be reached at (.+?):(\d+)', output)
        address = match.group(1).rstrip('.') if match else ''
        yield device_row('mDNS', f'{service}:{domain}:{name}', name, address,
                         {'Service': service, 'Domain': domain, 'Host': address,
                          'Port': match.group(2) if match else '', 'State': 'observed during bounded browse'})


def devices(state):
    yield note('devices:scope', 'Known devices only: controller snapshots, neighbour caches and mDNS (workstation, SSH, HTTP)')
    for source, argv in (('arp', ['arp', '-an']), ('ndp', ['ndp', '-an'])):
        try:
            yield from parse_neighbors(run(argv), source)
        except ValueError as error:
            yield note(f'{source}:error', str(error))
    yield from mdns_devices()
    yield from unifi_devices(state)


def ssh_hosts(path, seen=None, base=None):
    seen = set() if seen is None else seen
    path = Path(path).expanduser().resolve()
    base = path.parent if base is None else base
    if path in seen or not path.is_file():
        return []
    seen.add(path)
    result, current = [], []
    try:
        lines = path.read_text(errors='replace').splitlines()
    except OSError:
        return []
    for line in lines:
        try:
            parts = shlex.split(line, comments=True)
        except ValueError:
            continue
        if not parts:
            continue
        keyword, *values = parts
        if '=' in keyword:
            keyword, value = keyword.split('=', 1)
            values.insert(0, value)
        keyword = keyword.lower()
        if keyword == 'include':
            for pattern in values:
                candidate = Path(pattern).expanduser()
                if not candidate.is_absolute():
                    candidate = base / candidate
                for included in sorted(glob.glob(str(candidate))):
                    result.extend(ssh_hosts(included, seen, base))
        elif keyword == 'host':
            current = []
            for host in values:
                if any(char in host for char in '*?!'):
                    continue
                try:
                    validate('host', host)
                except ValueError:
                    continue
                row = command_row(f'ssh:{path}:{host}', host + '  [SSH]', ['ssh', host],
                                  purpose=f'Static host alias from {path}; SSH evaluates the full configuration when run.')
                row['value'] = host
                result.append(row)
                current.append(row)
        elif keyword == 'match':
            current = []
        elif keyword in ('hostname', 'user', 'port', 'proxyjump'):
            for row in current:
                row['detail'] += f'\n{keyword}: ' + ' '.join(values)
                row['search'] += ' ' + ' '.join(values)
    return result


def hosts(state):
    yield from ssh_hosts(Path.home() / '.ssh/config')
    try:
        for line in Path('/etc/hosts').read_text().splitlines():
            parts = line.split('#', 1)[0].split()
            if len(parts) > 1:
                for name in parts[1:]:
                    yield device_row('/etc/hosts', name, name, parts[0], {'Name': name, 'IP': parts[0]})
    except OSError:
        yield note('hosts:error', '/etc/hosts is unavailable')
    try:
        data = json.loads(run(['tailscale', 'status', '--json']))
        peers = [data.get('Self', {}), *data.get('Peer', {}).values()]
        for peer in peers:
            name = peer.get('DNSName') or peer.get('HostName') or peer.get('ID', '')
            if not name:
                continue
            addresses = peer.get('TailscaleIPs', [])
            yield device_row('Tailscale', peer.get('ID', name), name, addresses[0] if addresses else '',
                             {key: peer.get(key) for key in ('HostName', 'DNSName', 'TailscaleIPs', 'OS', 'Online', 'LastSeen', 'Relay', 'CurAddr')})
    except (ValueError, AttributeError, TypeError):
        yield note('tailscale:error', 'Tailscale status is unavailable')


def capture_command(options):
    interface = validate('interface', options.get('interface', 'en0'))
    count = validate('count', options.get('count', '20'))
    duration = options.get('duration', '')
    protocol = options.get('protocol', 'any')
    if protocol not in ('any', 'tcp', 'udp', 'icmp', 'icmp6'):
        raise ValueError('Choose any, tcp, udp, icmp or icmp6')
    clauses = []
    if options.get('host'):
        clauses.append('host ' + validate('host', options['host']))
    if protocol != 'any':
        clauses.append(protocol)
    if options.get('port'):
        clauses.append('port ' + validate('port', options['port']))
    tool = options.get('tool', 'tshark')
    if tool not in ('tshark', 'tcpdump'):
        raise ValueError('Choose tshark or tcpdump')
    if tool == 'tcpdump' and duration:
        raise ValueError('Use tshark for a duration-limited capture')
    argv = ['sudo', tool, '-n', '-i', interface, '-c', count]
    if duration:
        argv.extend(['-a', 'duration:' + validate('duration', duration)])
    if options.get('output'):
        output = str(Path(options['output']).expanduser().absolute())
        if Path(output).exists():
            raise ValueError('Capture output already exists; choose a new filename')
        argv.extend(['-w', validate('output', output)])
    if clauses:
        if tool == 'tshark':
            argv.append('-f')
        argv.append(' and '.join(clauses))
    return argv


def capture_builder(state):
    try:
        interfaces = run(['ifconfig', '-l']).split()
    except ValueError:
        interfaces = ['en0']
    for interface in interfaces:
        yield {'id': f'interface:{interface}', 'label': f'Capture on {interface}',
               'detail': 'Choose packet count, optional duration, output and capture filter on Enter.\nThe final command is shown for confirmation.',
               'value': interface, 'action': 'capture'}


def saved_captures(state):
    records = []
    root = Path(state.get('capture_root', state.get('cwd', os.getcwd()))).expanduser()
    for directory, dirs, files in os.walk(root):
        dirs[:] = [name for name in dirs if (state.get('hidden') or not name.startswith('.')) and name not in ('node_modules', 'target', '.build', '.git')]
        for name in sorted(files):
            path = Path(directory) / name
            if (state.get('hidden') or not name.startswith('.')) and path.suffix.lower() in ('.pcap', '.pcapng', '.cap'):
                try:
                    info = path.stat()
                    size = info.st_size
                except OSError:
                    continue
                records.append({'id': str(path), 'label': str(path.relative_to(root)), 'mtime': info.st_mtime, 'detail': f'{path}\n{size:,} bytes\nEnter chooses summary, packets, conversations or protocols.\nNo capture analysis runs while hovering.',
                       'value': str(path), 'path': str(path), 'action': 'next', 'next': 'network-capture-analysis'})
    yield from sorted(records, key=(lambda row: (-row['mtime'], row['path'])) if state.get('sort') else (lambda row: row['path']))


def capture_analysis(state):
    path = state['capture_path']
    yield command_row('capture-summary', 'Capture file summary', ['capinfos', path])
    yield command_row('capture-packets', 'Packets (numeric addresses)', ['tshark', '-n', '-r', path])
    yield command_row('capture-conversations', 'TCP and UDP conversations', ['tshark', '-n', '-r', path, '-q', '-z', 'conv,tcp', '-z', 'conv,udp'])
    yield command_row('capture-protocols', 'Protocol hierarchy', ['tshark', '-n', '-r', path, '-q', '-z', 'io,phs'])
    row = command_row('capture-filter', 'Packets matching a display filter', text=shlex.join(['tshark', '-n', '-r', path]) + ' -Y <display-filter>',
                      purpose='Choose a Wireshark display filter on Enter. Capture filters use -f; this saved-file view uses -Y.')
    row['action'] = 'display-filter'
    yield row


def url_commands(value):
    if any(ord(char) < 32 for char in value):
        raise ValueError('Invalid URL')
    url = urllib.parse.urlsplit(value)
    if url.scheme not in ('https', 'http') or not url.hostname or url.username is not None or url.password is not None:
        raise ValueError('Choose an HTTP or HTTPS URL without credentials')
    host = validate('host', url.hostname)
    port = url.port or (443 if url.scheme == 'https' else 80)
    rows = [command_row('url-timing', 'HTTP timings and response headers',
                        ['curl', '--globoff', '--proto', '=http,https', '--connect-timeout', '10', '--max-time', '30', '--silent', '--show-error',
                         '--output', '/dev/null', '--dump-header', '-', '--write-out',
                         'DNS: %{time_namelookup}s\nConnect: %{time_connect}s\nTLS: %{time_appconnect}s\nFirst byte: %{time_starttransfer}s\nTotal: %{time_total}s\nStatus: %{http_code}\nRemote: %{remote_ip}\n', '--url', value],
                        purpose='Send one request and report phase timings. Redirects are shown without following them.')]
    if url.scheme == 'https':
        openssl = shutil.which('openssl') or 'openssl'
        try:
            ipaddress.ip_address(host.split('%', 1)[0])
            check = '-verify_ip'
        except ValueError:
            check = '-verify_hostname'
        connect = f'[{host}]:{port}' if ':' in host else f'{host}:{port}'
        argv = [openssl, 's_client', '-connect', connect, '-servername', host,
                '-showcerts', '-verify_return_error', check, host]
        rows.append(command_row('url-certificate', 'Inspect and verify TLS certificates', argv,
                                purpose='Connect explicitly, verify the certificate chain and peer name, and show certificates.'))
    return rows


def collect(kind, state):
    if kind == 'network':
        yield from network_actions(state)
    elif kind == 'devices':
        yield from devices(state)
    elif kind == 'network-hosts':
        yield from hosts(state)
    elif kind == 'network-capture':
        yield from capture_builder(state)
    elif kind == 'captures':
        yield from saved_captures(state)
    elif kind == 'network-capture-analysis':
        yield from capture_analysis(state)
    elif kind == 'url':
        if state.get('url'):
            yield from url_commands(state['url'])
        else:
            yield {'id': 'url-enter', 'label': 'Choose a URL', 'detail': 'Enter an HTTP or HTTPS URL, then choose timing or certificate inspection.\nNo requests run before the prepared command is executed.', 'value': '', 'action': 'url'}
    elif kind == 'network-dns':
        for record in ('A', 'AAAA', 'CNAME', 'MX', 'TXT', 'NS', 'SOA', 'SRV', 'CAA'):
            yield {'id': record, 'label': record, 'detail': f'dig <host> {record}\nChoose a host and optional DNS server on Enter.', 'value': record, 'action': 'dns'}
    elif kind == 'network-iperf':
        for mode in ('client', 'reverse', 'bidirectional', 'udp', 'server'):
            yield {'id': mode, 'label': mode, 'detail': 'Choose duration, port and host as applicable.\nEnter shows the exact command before preparing it.', 'value': mode, 'action': 'iperf'}
    elif kind == 'network-ready':
        yield command_row('ready', state.get('command_title', 'Prepared network command'), state.get('command_argv'), state.get('command_text'))


def ready(title, argv):
    return {'type': 'next', 'kind': 'network-ready', 'state': {'command_title': title, 'command_argv': argv, 'command_text': None}}


def accept(kind, rows, key, state):
    if kind == 'captures' and key in ('alt-h', 'alt-s'):
        field = 'hidden' if key == 'alt-h' else 'sort'
        return {'type': 'reload', 'state': {field: not state.get(field, False)}}
    if key == 'alt-d' and kind == 'devices':
        subnet = validate('cidr', prompt('Subnet to discover, CIDR notation'))
        return ready('Discover devices on ' + subnet, ['nmap', *(['-6'] if ipaddress.ip_network(subnet).version == 6 else []), '-sn', '-n', subnet])
    if key == 'alt-f' and kind == 'captures':
        root = Path(prompt('Capture search directory', state.get('cwd', os.getcwd()))).expanduser().absolute()
        if not root.is_dir():
            raise ValueError('Capture search directory does not exist')
        return {'type': 'reload', 'state': {'capture_root': str(root)}}
    if key == 'alt-e' and kind == 'url':
        value = prompt('URL', state.get('url', 'https://example.com'))
        url_commands(value)
        return {'type': 'reload', 'state': {'url': value}}
    if key != 'enter' or not rows:
        return None
    row = rows[0]
    action = row.get('action')
    if action == 'none':
        return {'type': 'reload', 'state': {}}
    if kind == 'captures':
        return {'type': 'next', 'kind': 'network-capture-analysis', 'state': {'capture_path': row['path']}}
    if action == 'url':
        value = prompt('URL', 'https://example.com')
        url_commands(value)
        return {'type': 'reload', 'state': {'url': value}}
    if action == 'capture':
        options = {'interface': row['value'], 'tool': prompt('Tool: tshark or tcpdump', 'tshark'),
                   'host': prompt('Host filter (blank for any)'), 'protocol': prompt('Protocol: any/tcp/udp/icmp/icmp6', 'any'),
                   'port': prompt('Port filter (blank for any)'), 'count': prompt('Maximum packets', '20'),
                   'duration': prompt('Maximum seconds (tshark only, blank for packet limit)'),
                   'output': prompt('Save to new file (blank for terminal output)')}
        return ready('Packet capture', capture_command(options))
    if action == 'display-filter':
        query = prompt('Wireshark display filter')
        validate('display filter', query)
        return ready('Filtered capture', ['tshark', '-n', '-r', state['capture_path'], '-Y', query])
    if action == 'dns':
        host = validate('host', prompt('Host'))
        server = prompt('DNS server (blank for system default)')
        argv = ['dig', '+time=2', '+tries=1']
        if server:
            argv.append('@' + validate('host', server))
        return ready('DNS ' + row['value'], [*argv, host, row['value']])
    if action == 'iperf':
        mode = row['value']
        port = validate('port', prompt('Port', '5201'))
        argv = ['iperf3', '--json', '--port', port]
        if mode == 'server':
            argv += ['--server', '--one-off']
        else:
            host = validate('host', prompt('iperf3 server'))
            duration = validate('duration', prompt('Duration in seconds', '10'))
            argv += ['--client', host, '--time', duration]
            if mode == 'reverse':
                argv += ['--reverse']
            elif mode == 'bidirectional':
                argv += ['--bidir']
            elif mode == 'udp':
                bitrate = prompt('UDP bitrate, such as 10M', '10M')
                if not re.fullmatch(r'[1-9][0-9]*[KMG]?', bitrate):
                    raise ValueError('Invalid bitrate')
                argv += ['--udp', '--bitrate', bitrate]
        return ready('iperf3 ' + mode, argv)
    if action == 'command':
        text = row.get('text')
        if text is not None:
            for parameter in row.get('params', []):
                value = validate(parameter, prompt(parameter.capitalize()))
                text = text.replace('<' + parameter + '>', shlex.quote(value))
                if parameter == 'cidr' and ipaddress.ip_network(value).version == 6:
                    text = text.replace('nmap ', 'nmap -6 ', 1)
            return {'type': 'command', 'text': text}
        return {'type': 'command', 'argv': row['argv']}
    return None


def bindings(kind, state):
    if kind == 'devices':
        return {'alt-d': 'Prepare discovery of a chosen subnet'}
    if kind == 'captures':
        return {'alt-f': 'Choose capture search directory', 'alt-h': 'Show/hide hidden captures', 'alt-s': 'Path/modification time'}
    if kind == 'url':
        return {'alt-e': 'Choose another URL'}
    return {}
