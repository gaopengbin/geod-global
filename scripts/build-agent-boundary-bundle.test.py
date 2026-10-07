"""Cross-platform gzip and corruption regressions; all data are synthetic."""
import gzip
import hashlib
import importlib.util
import json
from pathlib import Path
import struct
import tempfile
import unittest

spec = importlib.util.spec_from_file_location('boundary_bundle', Path(__file__).with_name('build-agent-boundary-bundle.py'))
bundle = importlib.util.module_from_spec(spec)
spec.loader.exec_module(bundle)


def split(archive):
    size = struct.unpack('<I', archive[6:10])[0]
    return json.loads(archive[10:10 + size]), bytearray(archive[10 + size:])


def pack(entries, payload):
    header = json.dumps(entries, separators=(',', ':')).encode()
    return bundle.MAGIC + struct.pack('<I', len(header)) + header + payload


class BoundaryBundleTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.source = Path(self.temp.name)
        (self.source / 'AAA.geojson').write_bytes(b'{"features":[' + b'{"geometry":null},' * 500 + b'{}]}')
        (self.source / 'BBB.geojson').write_bytes(b'{"features":[]}')
        self.archive = bundle.build_archive(self.source)

    def test_generation_is_repeatable_with_portable_gzip_headers(self):
        self.assertEqual(self.archive, bundle.build_archive(self.source))
        entries, payload = split(self.archive)
        for entry in entries.values():
            self.assertEqual(payload[entry['offset'] + 4:entry['offset'] + 8], b'\0' * 4)
            self.assertEqual(payload[entry['offset'] + 9], 255)
        self.assertEqual(bundle.verify_archive(self.source, self.archive)['countries'], 2)

    def test_platform_os_bytes_and_equivalent_compression_are_accepted(self):
        entries, payload = split(self.archive)
        for os_byte in (3, 10, 255):
            for entry in entries.values():
                payload[entry['offset'] + 9] = os_byte
            bundle.verify_archive(self.source, pack(entries, payload))
        # Distinct deflate encodings pass only when every source byte matches.
        entries, payload = {}, bytearray()
        for name, raw in bundle.country_sources(self.source).items():
            member = gzip.compress(raw, compresslevel=1, mtime=0)
            entries[name] = {'offset': len(payload), 'bytes': len(member), 'sha256': hashlib.sha256(raw).hexdigest()}
            payload.extend(member)
        alternative = pack(entries, payload)
        self.assertNotEqual(alternative, self.archive)
        bundle.verify_archive(self.source, alternative)

    def test_changed_source_and_forged_index_hash_are_rejected(self):
        (self.source / 'BBB.geojson').write_bytes(b'{"changed":true}')
        with self.assertRaisesRegex(ValueError, 'source hash'):
            bundle.verify_archive(self.source, self.archive)
        entries, payload = split(self.archive)
        entries['BBB']['sha256'] = hashlib.sha256((self.source / 'BBB.geojson').read_bytes()).hexdigest()
        with self.assertRaisesRegex(ValueError, 'geometry differs'):
            bundle.verify_archive(self.source, pack(entries, payload))

    def test_bad_crc_truncation_and_unindexed_or_member_trailing_bytes_are_rejected(self):
        entries, payload = split(self.archive)
        payload[entries['AAA']['bytes'] - 8] ^= 1
        with self.assertRaisesRegex(ValueError, 'compressed native boundary'):
            bundle.verify_archive(self.source, pack(entries, payload))
        for changed in (self.archive[:-1], self.archive + b'hidden'):
            with self.assertRaises(ValueError):
                bundle.verify_archive(self.source, changed)
        entries, payload = split(self.archive)
        entries['BBB']['bytes'] += 6
        with self.assertRaisesRegex(ValueError, 'geometry differs'):
            bundle.verify_archive(self.source, pack(entries, payload + b'hidden'))

    def test_country_sets_ranges_and_duplicate_keys_are_rejected(self):
        entries, payload = split(self.archive)
        for changed in ({'AAA': entries['AAA']}, {**entries, 'CCC': entries['BBB']}):
            with self.assertRaisesRegex(ValueError, 'country set'):
                bundle.verify_archive(self.source, pack(changed, payload))
        for field, value in (('offset', 0), ('offset', True), ('bytes', -1), ('bytes', True)):
            entries, payload = split(self.archive)
            entries['BBB'][field] = value
            with self.assertRaisesRegex(ValueError, 'archive range'):
                bundle.verify_archive(self.source, pack(entries, payload))
        header = b'{"AAA":{},"AAA":{}}'
        with self.assertRaisesRegex(ValueError, 'archive index'):
            bundle.verify_archive(self.source, bundle.MAGIC + struct.pack('<I', len(header)) + header)


if __name__ == '__main__':
    unittest.main()
