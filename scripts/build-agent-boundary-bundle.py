"""Package unchanged owned Natural Earth ADM1 shapes for native offline reads.

Verify source bytes and archive structure, rather than a platform-specific gzip
encoding. Python/zlib versions may emit different gzip headers or deflate bytes.
"""
import gzip
import hashlib
import io
import json
import struct
import sys
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MAGIC = b'NEGB1\0'


def country_sources(source):
    return {path.stem: path.read_bytes() for path in sorted(source.glob('*.geojson'))}


def build_archive(source):
    entries = {}
    payload = bytearray()
    for country, raw in country_sources(source).items():
        output = io.BytesIO()
        # GzipFile always writes an OS-independent header; do not inherit zlib's
        # OS byte via gzip.compress(mtime=0) on Python 3.11/3.12.
        with gzip.GzipFile(filename='', fileobj=output, mode='wb', compresslevel=9, mtime=0) as stream:
            stream.write(raw)
        compressed = output.getvalue()
        entries[country] = {'offset': len(payload), 'bytes': len(compressed),
                            'sha256': hashlib.sha256(raw).hexdigest()}
        payload.extend(compressed)
    header = json.dumps(entries, ensure_ascii=True, separators=(',', ':')).encode()
    return MAGIC + struct.pack('<I', len(header)) + header + payload


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError('Duplicate boundary archive key: ' + key)
        result[key] = value
    return result


def verify_archive(source, archive):
    if archive[:6] != MAGIC or len(archive) < 10:
        raise ValueError('Invalid native boundary archive signature.')
    header_size = struct.unpack('<I', archive[6:10])[0]
    if header_size < 2 or header_size > min(1024 * 1024, len(archive) - 10):
        raise ValueError('Invalid native boundary archive header size.')
    try:
        entries = json.loads(archive[10:10 + header_size], object_pairs_hook=unique_object)
    except (ValueError, UnicodeError) as error:
        raise ValueError('Invalid native boundary archive index.') from error
    sources = country_sources(source)
    if not isinstance(entries, dict) or entries.keys() != sources.keys():
        raise ValueError('Native boundary archive country set differs from owned sources.')
    payload = archive[10 + header_size:]
    cursor = 0
    for country, raw in sources.items():
        entry = entries[country]
        if (not isinstance(entry, dict) or set(entry) != {'offset', 'bytes', 'sha256'}
                or type(entry['offset']) is not int or type(entry['bytes']) is not int
                or entry['offset'] != cursor or entry['bytes'] < 18
                or entry['bytes'] > len(payload) - cursor):
            raise ValueError('Invalid native boundary archive range: ' + country)
        if entry['sha256'] != hashlib.sha256(raw).hexdigest():
            raise ValueError('Native boundary archive source hash differs: ' + country)
        member = payload[cursor:cursor + entry['bytes']]
        # Bound inflation by owned source size. Check CRC, exact bytes and one
        # complete member; compression level and OS metadata are irrelevant.
        decoder = zlib.decompressobj(wbits=31)
        try:
            decoded = decoder.decompress(member, len(raw) + 1)
        except zlib.error as error:
            raise ValueError('Invalid compressed native boundary: ' + country) from error
        if (decoded != raw or not decoder.eof or decoder.unconsumed_tail or decoder.unused_data):
            raise ValueError('Native boundary archive geometry differs: ' + country)
        cursor += entry['bytes']
    if cursor != len(payload):
        raise ValueError('Native boundary archive has unindexed payload bytes.')
    return {'countries': len(sources), 'archiveBytes': len(archive)}


def main():
    source = ROOT / 'prototype/public/basemaps/admin1-10m'
    target = ROOT / 'crates/geod-runtime/fixtures/boundaries/admin1.negb'
    if '--check' in sys.argv:
        try:
            result = verify_archive(source, target.read_bytes())
        except (ValueError, OSError) as error:
            raise SystemExit(str(error)) from error
    else:
        target.parent.mkdir(parents=True, exist_ok=True)
        archive = build_archive(source)
        result = verify_archive(source, archive)
        target.write_bytes(archive)
    print(json.dumps(result))


if __name__ == '__main__':
    main()
