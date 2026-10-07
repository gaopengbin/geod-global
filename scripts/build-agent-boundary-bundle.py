"""Package unchanged owned Natural Earth ADM1 shapes for native offline reads.

Each country is compressed independently. Only the requested country's geometry
is inflated, with source hashes and a deterministic archive header.
"""
import gzip
import hashlib
import json
import struct
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
source = ROOT / 'prototype/public/basemaps/admin1-10m'
entries = {}
payload = bytearray()
for path in sorted(source.glob('*.geojson')):
    raw = path.read_bytes()
    compressed = gzip.compress(raw, compresslevel=9, mtime=0)
    entries[path.stem] = {'offset': len(payload), 'bytes': len(compressed),
                          'sha256': hashlib.sha256(raw).hexdigest()}
    payload.extend(compressed)
header = json.dumps(entries, ensure_ascii=True, separators=(',', ':')).encode()
target = ROOT / 'crates/geod-runtime/fixtures/boundaries/admin1.negb'
target.parent.mkdir(parents=True, exist_ok=True)
archive = b'NEGB1\0' + struct.pack('<I', len(header)) + header + payload
if '--check' in sys.argv:
    if target.read_bytes() != archive:
        raise SystemExit('Native boundary archive differs from owned source geometries.')
else:
    target.write_bytes(archive)
print(json.dumps({'countries': len(entries), 'archiveBytes': target.stat().st_size}))
