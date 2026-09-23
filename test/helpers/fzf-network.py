import importlib.util
import pathlib
import shlex
import sys
import tempfile
from unittest.mock import patch

root = pathlib.Path(sys.argv[1])
module = root / 'dot_local/libexec/fzf-pickers/network.py'
assert module.exists(), 'network provider is missing'
spec = importlib.util.spec_from_file_location('network', module)
n = importlib.util.module_from_spec(spec)
spec.loader.exec_module(n)

# Pagination must retain early data when a later page stalls.
responses = {
    0: {'offset': 0, 'count': 2, 'totalCount': 3, 'data': [{'id': 'a'}, {'id': 'b'}]},
    2: {'offset': 2, 'count': 1, 'totalCount': 3, 'data': [{'id': 'c'}]},
}
assert list(n.pages(lambda path, offset: responses[offset], '/sites')) == [{'id': 'a'}, {'id': 'b'}, {'id': 'c'}]
stream = n.pages(lambda path, offset: responses[0], '/sites')
assert next(stream) == {'id': 'a'}
assert next(stream) == {'id': 'b'}
try:
    next(stream)
except ValueError:
    pass
else:
    raise AssertionError('nonadvancing page accepted')

# Quoting is applied after gathering targets, with no shell evaluation.
rows = list(n.collect('network', {'cwd': str(root)}))
ssh = next(row for row in rows if row['id'] == 'port-22')
with patch.object(n, 'prompt', return_value='example.com'):
    accepted = n.accept('network', [ssh], 'enter', {'cwd': str(root)})
assert accepted == {'type': 'command', 'text': 'nc -vz -w3 example.com 22'}
with patch.object(n, 'prompt', return_value='$(touch /tmp/no)'):
    try:
        n.accept('network', [ssh], 'enter', {})
    except ValueError:
        pass
    else:
        raise AssertionError('invalid hostname accepted')
for row in rows:
    assert row['detail']
    assert '__bash_bindings' not in row['detail']

assert 'TCP port 22' in ssh['search']
assert 'nc -vz -w3' in ssh['search']
with patch.object(n, 'prompt', return_value='192.168.1.0/24'):
    action = n.accept('devices', [], 'alt-d', {})
assert action['state']['command_argv'] == ['nmap', '-sn', '-n', '192.168.1.0/24']

assert n.validate('host', '::1') == '::1'
assert n.validate('host', 'fe80::1%en0') == 'fe80::1%en0'
with patch.object(n, 'prompt', return_value='2001:db8::/64'):
    action = n.accept('devices', [], 'alt-d', {})
assert action['state']['command_argv'] == ['nmap', '-6', '-sn', '-n', '2001:db8::/64']
discover = next(row for row in rows if row['id'] == 'discovery')
with patch.object(n, 'prompt', return_value='2001:db8::/64'):
    action = n.accept('network', [discover], 'enter', {})
assert action['text'] == 'nmap -6 -sn -n 2001:db8::/64'
try:
    list(n.pages(lambda path, offset: {'data': [{'id': 'a'}], 'limit': 'wrong'}, '/sites'))
except ValueError:
    pass
else:
    raise AssertionError('malformed pagination limit accepted')

# Capture count/duration and filters remain separate options.
argv = n.capture_command({'interface': 'en0', 'host': '192.0.2.1', 'protocol': 'tcp', 'port': '443', 'count': '20', 'duration': '10', 'output': '/tmp/space capture.pcapng'})
assert argv == ['sudo', 'tshark', '-n', '-i', 'en0', '-c', '20', '-a', 'duration:10', '-w', '/tmp/space capture.pcapng', '-f', 'host 192.0.2.1 and tcp and port 443']
try:
    n.capture_command({'interface': 'en0', 'count': '0'})
except ValueError:
    pass
else:
    raise AssertionError('unbounded capture accepted')

# URL inspection must verify the peer and never print credentials.
commands = n.url_commands('https://example.com:8443/a?b=1')
assert '--globoff' in commands[0]['argv']
assert commands[0]['argv'][-1] == 'https://example.com:8443/a?b=1'
tls = commands[1]['argv']
assert tls[tls.index('-connect') + 1] == 'example.com:8443'
assert tls[tls.index('-verify_hostname') + 1] == 'example.com'
assert '-verify_return_error' in tls
for value in ('file:///etc/passwd', 'https://user:pass@example.com', 'https://example.com\nfoo'):
    try:
        n.url_commands(value)
    except ValueError:
        pass
    else:
        raise AssertionError('unsafe URL accepted')

# Incomplete neighbours stay visible with their source instead of fabricated names.
arp = n.parse_neighbors('? (192.0.2.4) at aa:bb:cc:dd:ee:ff on en0 ifscope [ethernet]\n? (192.0.2.5) at (incomplete) on en0 ifscope [ethernet]\n', 'arp')
assert arp[0]['value'] == '192.0.2.4'
assert 'aa:bb:cc:dd:ee:ff' in arp[0]['detail']
assert 'aa:bb:cc:dd:ee:ff' in arp[0]['search']
assert 'incomplete' in arp[1]['detail']
ndp = n.parse_neighbors('Neighbor Linklayer Address Netif Expire S Flags\nfe80::1%en0 aa:bb:cc:dd:ee:ff en0 23h59m59s S R\n', 'ndp')
assert ndp[0]['value'] == 'fe80::1%en0'

# Repeated data and empty early pages cannot silently look complete.
for bad in ({'offset': 0, 'totalCount': 2, 'data': []}, {'offset': 0, 'totalCount': 2, 'data': [{'id': 'a'}, {'id': 'a'}]}):
    try:
        list(n.pages(lambda path, offset: bad, '/sites'))
    except ValueError:
        pass
    else:
        raise AssertionError('incomplete result presented as complete')

# TLS credentials stay in a header, and certificate errors omit exception contents.
with tempfile.TemporaryDirectory() as tmp:
    config = pathlib.Path(tmp) / 'config.toml'
    config.write_text('[unifi]\nurl="https://controller.example"\napi_key="SENSITIVE_VALUE"\n')
    config.chmod(0o600)
    class Response:
        def __enter__(self):
            return self
        def __exit__(self, *args):
            pass
        def read(self):
            return b'{"data": [], "offset": 200, "totalCount": 200}'
    class Opener:
        def open(self, req, timeout):
            assert req.full_url == 'https://controller.example/proxy/network/integration/v1/sites?offset=200&limit=200'
            assert req.get_header('X-api-key') == 'SENSITIVE_VALUE'
            return Response()
    with patch.object(n.urllib.request, 'build_opener', return_value=Opener()):
        get = n.unifi_get({'config': str(config)})
        assert get('/sites', 200)['offset'] == 200
    config.chmod(0o644)
    try:
        n.unifi_get({'config': str(config)})
    except ValueError:
        pass
    else:
        raise AssertionError('readable credentials accepted')

# Capture scope toggles include hidden files and preserve canonical paths.
with tempfile.TemporaryDirectory() as tmp:
    p = pathlib.Path(tmp)
    (p / 'visible.pcap').write_bytes(b'')
    (p / '.hidden.pcap').write_bytes(b'')
    state = {'cwd': tmp}
    assert [r['label'] for r in n.collect('captures', state)] == ['visible.pcap']
    change = n.accept('captures', [], 'alt-h', state)
    state.update(change['state'])
    assert {r['label'] for r in n.collect('captures', state)} == {'.hidden.pcap', 'visible.pcap'}
    assert n.accept('captures', [], 'alt-s', state)['state']['sort'] is True

# Static SSH includes are parsed without executing Match exec.
with tempfile.TemporaryDirectory() as tmp:
    p = pathlib.Path(tmp)
    (p / 'sub').mkdir()
    (p / 'extra').write_text('Host second\n  HostName second.example\n')
    (p / 'sub' / 'nested').write_text('Include extra\nHost nested\n')
    (p / 'config').write_text('Include sub/nested\nHost first *.wild\n  HostName first.example\nMatch exec "touch /tmp/no"\nHost third\n')
    hosts = n.ssh_hosts(p / 'config')
    assert {r['value'] for r in hosts} == {'first', 'second', 'third', 'nested'}
    assert any('second.example' in r['detail'] for r in hosts)
print('PASS fzf network: pagination, command preparation, capture limits, TLS, neighbours, static SSH')
