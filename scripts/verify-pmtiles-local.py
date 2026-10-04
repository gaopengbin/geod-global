"""Independent local PMTiles import/read/export verifier. Verification-only packages.

First run --import-file FILE (repeatable) without --server; later runs can verify
the saved references through a restarted loopback runtime using --server URL.
Requires pmtiles and mapbox-vector-tile on PYTHONPATH; neither is a product dependency.
"""
import argparse
import base64
import gzip
import hashlib
import importlib.metadata
import io
import json
from pathlib import Path
import subprocess
import tempfile
import urllib.request
import zipfile
from datetime import datetime, timezone
from pmtiles.reader import Reader, MemorySource, all_tiles
from pmtiles.tile import Compression
import mapbox_vector_tile

def sha(b):
    return hashlib.sha256(b).hexdigest()

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--binary', type=Path, default=Path('target/debug/geod-runtime.exe'))
    ap.add_argument('--data-dir', type=Path, required=True)
    ap.add_argument('--reference-dir', type=Path, required=True)
    ap.add_argument('--report', type=Path, required=True)
    ap.add_argument('--import-file', type=Path, action='append', default=[])
    ap.add_argument('--server')
    args = ap.parse_args()
    refs = args.reference_dir
    refs.mkdir(parents=True, exist_ok=True)
    manifest = refs / 'packages.json'
    recorded = json.loads(manifest.read_text(encoding='utf8')) if manifest.exists() else []
    binary = str(args.binary.resolve())
    def cli(group, command, *options):
        return json.loads(subprocess.check_output([binary, group, command, *options, '--data-dir', str(args.data_dir)], encoding='utf8'))
    if args.import_file:
        assert not args.server, 'Import through CLI only while the runtime is stopped'
        for path in args.import_file:
            original = path.read_bytes()
            asset = cli('tile-packages', 'open', '--file', str(path))
            assert asset['sha256'] == sha(original)
            assert asset['source']['local'] == {'fileName': path.name, 'sha256': sha(original)}
            assert asset['source']['url'] == asset['source']['etag'] == ''
            (refs / (asset['id'] + '.pmtiles')).write_bytes(original)
            recorded.append(asset)
        manifest.write_text(json.dumps(recorded, ensure_ascii=False, indent=2), encoding='utf8')
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
    def get(path):
        with opener.open(args.server.rstrip('/') + path, timeout=190) as r:
            return r.read()
    def inspect(asset):
        return json.loads(get('/tile-packages/' + asset['id'])) if args.server else cli('tile-packages', 'inspect', '--id', asset['id'])
    reports = []
    for asset in recorded:
        package_id = asset['id']
        original = (refs / (package_id + '.pmtiles')).read_bytes()
        inspected = inspect(asset)
        assert inspected['asset'] == asset, 'Persisted import receipt changed'
        reader = Reader(MemorySource(original))
        assert inspected['metadata'] == reader.metadata()
        assert asset['sha256'] == sha(original) and asset['bytes'] == len(original)
        expected = dict(all_tiles(MemorySource(original)))
        actual = {(t['coordinate']['z'], t['coordinate']['x'], t['coordinate']['y']): t for t in asset['tiles']}
        assert expected.keys() == actual.keys()
        counts = 0
        for (z, x, y), compressed in expected.items():
            t = actual[z, x, y]
            assert t['sourceOffset'] == t['packageOffset']
            assert original[t['packageOffset']:t['packageOffset'] + t['bytes']] == compressed
            assert sha(compressed) == t['sha256']
            raw = gzip.decompress(compressed) if reader.header()['tile_compression'] == Compression.GZIP else compressed
            if args.server:
                tile = json.loads(get(f'/tile-packages/{package_id}/tiles/{z}/{x}/{y}'))
            else:
                request = refs / 'tile-request.json'
                request.write_text(json.dumps({'id': package_id, 'z': z, 'x': x, 'y': y}), encoding='utf8')
                tile = cli('tile-packages', 'tile', '--request', str(request))
            assert base64.b64decode(tile['dataBase64']) == raw
            assert tile['sha256'] == sha(raw)
            decoded = mapbox_vector_tile.decode(raw)
            layers = [{'name': name, 'extent': v['extent'], 'version': v['version'], 'features': len(v['features'])} for name, v in sorted(decoded.items())]
            assert layers == sorted(tile['layers'], key=lambda v: v['name']) == sorted(t['layers'], key=lambda v: v['name'])
            counts += sum(v['features'] for v in layers)
        if args.server:
            exported = get('/tile-packages/' + package_id + '/export')
        else:
            with tempfile.TemporaryDirectory(dir=refs) as tmp:
                output = Path(tmp) / 'export.zip'
                cli('tile-packages', 'export', '--id', package_id, '--out', str(output))
                exported = output.read_bytes()
        with zipfile.ZipFile(io.BytesIO(exported)) as z:
            assert set(z.namelist()) == {'tiles.pmtiles', 'source.json', 'README.txt', 'checksums.sha256'}
            assert z.read('tiles.pmtiles') == original
            assert json.loads(z.read('source.json')) == asset
            for line in z.read('checksums.sha256').decode().splitlines():
                digest, name = line.split('  ', 1)
                assert digest == sha(z.read(name))
        baseline = refs / (package_id + '.zip')
        if baseline.exists():
            assert exported == baseline.read_bytes(), 'Export changed after restart'
        else:
            baseline.write_bytes(exported)
        reports.append({'id': package_id, 'fileName': asset['source']['local']['fileName'], 'bytes': len(original), 'sha256': sha(original), 'tiles': len(expected), 'featureRecordsAcrossLevels': counts, 'exactOriginalArchive': True, 'exactLocalTiles': True, 'independentMvtDecode': True, 'exportVerified': True})
    report = {'checkedAt': datetime.now(timezone.utc).isoformat(), 'mode': 'restarted loopback runtime' if args.server else 'direct native CLI', 'referenceVersions': {n: importlib.metadata.version(n) for n in ['pmtiles', 'mapbox-vector-tile']}, 'packages': reports}
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding='utf8')
    print(json.dumps(report, ensure_ascii=False))

if __name__ == '__main__':
    main()
