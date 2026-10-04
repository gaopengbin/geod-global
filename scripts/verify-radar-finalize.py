"""Finish the real radar matrix after the acquisition owner releases its store.

The waiting phase only reads receipts and health. It never interrupts or
restarts an acquisition. Each dependent verifier must succeed before the next
starts; a failed observer leaves the data and previous evidence intact.
"""
import argparse
import hashlib
import json
import subprocess
import sys
import time
import urllib.request
from pathlib import Path
from urllib.error import URLError

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('root', type=Path)
parser.add_argument('--port', type=int, default=4605)
parser.add_argument('--ui-port', type=int, default=4606)
args = parser.parse_args()
root = args.root.resolve()
assert root.parent == Path('.verification').resolve() and root.name.startswith('radar-polarizations-')
assert 1 <= args.port <= 65535 and 1 <= args.ui_port <= 65535 and args.port != args.ui_port
opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))


def owner_present():
    try:
        with opener.open(f'http://127.0.0.1:{args.port}/health', timeout=5) as response:
            health = json.load(response)
    except URLError as error:
        if isinstance(error.reason, ConnectionRefusedError):
            return False
        raise RuntimeError('Owner health is unknown; no native process was changed') from None
    assert Path(health['storageRoot'].removeprefix('\\\\?\\')).resolve() == root
    return True


last = None
missing_owner = 0
while True:
    try:
        native_raw = (root / 'native-polarizations-verification.json').read_bytes()
        native = json.loads(native_raw)
    except json.JSONDecodeError:
        # The old owner writes its evolving receipt directly; never accept a
        # read made in the middle of that write.
        time.sleep(1)
        continue
    state = (native.get('status'), len(native['originals']), len(native['outputs']))
    if state != last:
        print(json.dumps({'stage': 'waiting-for-matrix', 'status': state[0], 'originals': state[1], 'outputs': state[2]}), flush=True)
        last = state
    present = owner_present()
    if native.get('status') == 'passed':
        assert len(native['originals']) == 6 and len(native['outputs']) == 9
        assert all(sum(entry['job']['assetKey'] == key for entry in native['originals']) == 2 for key in ['vh', 'hh', 'hv'])
        assert {(entry['case'], entry['key']) for entry in native['outputs']} == {
            (kind, key) for kind in ['single', 'mosaic', 'polygon'] for key in ['vh', 'hh', 'hv']}
        assert all(entry['job']['status'] == 'succeeded' and entry['job']['settled'] for entry in native['originals'] + native['outputs'])
        if not present:
            break
    missing_owner = missing_owner + 1 if not present else 0
    assert missing_owner < 6, 'The acquisition owner disappeared before its complete matrix receipt; evidence was preserved'
    time.sleep(5)

binary_sha = hashlib.sha256(Path('target/debug/geod-runtime.exe').read_bytes()).hexdigest()
assert binary_sha == native['nativeBinarySha256']
receipt_sha = hashlib.sha256(native_raw).hexdigest()
commands = [
    ('independent-full-matrix', [sys.executable, '-X', 'utf8', 'scripts/verify-radar-polarizations.py', str(root)]),
    ('mcp-direct-and-loopback', [sys.executable, '-X', 'utf8', 'scripts/verify-radar-polarizations-mcp.py', str(root), '--port', str(args.port)]),
    ('production-ui-and-offline', ['node', 'scripts/verify-radar-polarizations-ui.mjs', str(root), str(args.port), str(args.ui_port)]),
    ('bound-summary', [sys.executable, '-X', 'utf8', 'scripts/summarize-radar-polarizations.py', str(root)]),
]
for stage, command in commands:
    assert hashlib.sha256((root / 'native-polarizations-verification.json').read_bytes()).hexdigest() == receipt_sha
    assert hashlib.sha256(Path('target/debug/geod-runtime.exe').read_bytes()).hexdigest() == binary_sha
    print(json.dumps({'stage': stage, 'status': 'starting'}), flush=True)
    result = subprocess.run(command, cwd=Path.cwd())
    assert result.returncode == 0, f'{stage} failed; no successful later stage or whole-matrix acceptance was reported'
    print(json.dumps({'stage': stage, 'status': 'passed'}), flush=True)
summary = json.loads(Path('prototype/qa/radar-polarizations-verification.json').read_text(encoding='utf8'))
assert summary['status'] == 'passed'
print(json.dumps({'stage': 'radar-matrix-complete', 'status': 'passed', 'originals': 6, 'outputs': 9,
                  'scope': 'The tested RTC polarization matrix, not every planned provider capability or a release.'}), flush=True)
