"""Compare NAIP RGB/CIR/NIR previews against independent GDAL samples.

Uses a separate offline ledger with hard links to completed, independently
accepted public files. Only GET requests are allowed. Source TIFFs are never
changed, and display-only previews never replace recorded processing profiles.
"""
import argparse
import base64
import hashlib
import io
import json
import os
import shutil
import socket
import subprocess
import time
from datetime import datetime, timezone
from pathlib import Path

import numpy as np
import rasterio
import requests
from PIL import Image

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('root', type=Path)
parser.add_argument('--source-root', type=Path, default=Path('.verification/naip-resolutions-20261004'))
parser.add_argument('--binary', type=Path, default=Path('.verification/naip-native-target/debug/geod-runtime.exe'))
parser.add_argument('--port', type=int, default=4614)
parser.add_argument('--legacy-060', action='store_true', help='Reuse the four previously accepted California 0.6 m files; no new download')
args = parser.parse_args()
workspace = Path.cwd().resolve()
root, source_root = args.root.resolve(), args.source_root.resolve()
assert root.parent == source_root.parent == workspace / '.verification'
assert root.name.startswith('naip-views-') and root != source_root
assert 1 <= args.port <= 65535
assert not (root / 'jobs.json').exists(), 'Use a fresh isolated acceptance store'


def sha(file):
    with Path(file).open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def normal_path(value):
    return Path(str(value).removeprefix('\\\\?\\')).resolve()


def snapshot(file):
    file = Path(file).resolve()
    assert file.is_relative_to(workspace)
    raw = file.read_bytes()
    digest = hashlib.sha256(raw).hexdigest()
    target = root / 'evidence-snapshots' / (digest + '.json')
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_bytes(raw)
    return {'file': str(Path(file).relative_to(workspace)).replace('\\', '/'),
            'sha256': digest, 'snapshot': str(target.relative_to(root)).replace('\\', '/')}


ledger = json.loads((source_root / 'jobs.json').read_text(encoding='utf8'))
root.mkdir(parents=True, exist_ok=True)
if args.legacy_060:
    assert source_root.name == 'naip-native-20261002'
    source_file = Path('prototype/qa/naip-native-verification.json')
    independent_file = Path('prototype/qa/naip-processing-verification.json')
    source, independent = (json.loads(file.read_bytes()) for file in [source_file, independent_file])
    assert source['nativeOriginal']['status'] == 'succeeded'
    assert source['independentLocalVerification']['pngMatchesIndependentOverview']
    assert independent['singleSceneClip']['allFourChannelsEqual']
    source_pins = [(item['jobId'], item['sha256'], 'original') for item in independent['sources']]
    source_pins += [(independent['singleSceneClip']['jobId'], independent['singleSceneClip']['sha256'], 'single'),
                    (independent['jobId'], independent['sha256'], 'polygon')]
    accepted = {id: digest for id, digest, _ in source_pins}
    assert len(accepted) == 4
    assert accepted[source['independentLocalVerification']['jobId']] == source['nativeOriginal']['sha256']
    entries = [{'group': '0.6m', 'case': kind, 'job': ledger[id]} for id, _, kind in source_pins]
    acquisition_binary_sha = None  # The historical acquisition did not pin its executable.
else:
    source_file = source_root / 'native-resolution-verification.json'
    independent_file = source_root / 'independent-resolution-verification.json'
    source, independent = (json.loads(file.read_bytes()) for file in [source_file, independent_file])
    assert source['status'] == independent['status'] == 'passed'
    assert source['nativeBinarySha256'] == independent['nativeBinarySha256']
    accepted = {item['jobId']: item['sha256'] for item in independent['originals'] + independent['outputs']}
    entries = source['originals'] + source['outputs']
    assert len(entries) == 8 and len(source['originals']) == 3 and len(source['outputs']) == 5
    acquisition_binary_sha = source['nativeBinarySha256']
file_count = len(entries)
cloned = {}
(root / 'assets').mkdir(exist_ok=True)
for entry in entries:
    job = entry['job']
    assert accepted[job['id']] == job['sha256']
    local = ledger[job['id']]
    # settled is derived by the running manager, rather than stored in jobs.json.
    assert local['status'] == 'succeeded' and (args.legacy_060 or job['settled']) and local['sha256'] == job['sha256']
    old = normal_path(local['outputPath'])
    assert old == source_root / 'assets' / (job['id'] + '.tif')
    assert old.stat().st_size == local['bytesDownloaded'] == local['totalBytes']
    assert sha(old) == local['sha256']
    new = root / 'assets' / old.name
    os.link(old, new)
    cloned[job['id']] = {**local, 'outputPath': str(new)}
    if local['manifestPath'] is not None:
        manifest = normal_path(local['manifestPath'])
        assert manifest == source_root / 'assets' / (job['id'] + '.metadata.json')
        new_manifest = root / 'assets' / manifest.name
        shutil.copyfile(manifest, new_manifest)
        assert sha(manifest) == sha(new_manifest)
        cloned[job['id']]['manifestPath'] = str(new_manifest)
(root / 'jobs.json').write_text(json.dumps(cloned, indent=2) + '\n', encoding='utf8')
shutil.copyfile(source_root / 'projects.json', root / 'projects.json')
(root / 'proxy-settings.json').write_text(json.dumps({'mode': 'custom', 'url': 'http://127.0.0.1:9'}), encoding='utf8')
binary_sha = sha(args.binary)
binary = root / ('runtime-' + binary_sha[:16] + '.exe')
shutil.copyfile(args.binary, binary)
assert sha(binary) == binary_sha
report = {'schema': 'geod-naip-views/v1', 'status': 'running', 'checkedAt': datetime.now(timezone.utc).isoformat(),
          'nativeBinarySha256': binary_sha, 'acquisitionNativeBinarySha256': acquisition_binary_sha,
          'sourceReceipt': snapshot(source_file), 'sourceIndependentReceipt': snapshot(independent_file),
          'cohort': '060' if args.legacy_060 else '030-100',
          'scope': 'Previously accepted actual 0.6 m originals and aligned outputs; no new acquisition.' if args.legacy_060 else 'Actual accepted 0.3 m and 1 m originals and aligned derived grids; RGB/CIR/NIR display only.',
          'readOnly': True, 'usedUserDesktop': False, 'networkDisabledByProxy': True,
          'tools': {'rasterio': rasterio.__version__, 'numpy': np.__version__}, 'entries': [], 'requests': []}
target = root / 'views-verification.json'


def write_report():
    temporary = target.with_suffix('.part')
    temporary.write_text(json.dumps(report, indent=2) + '\n', encoding='utf8')
    temporary.replace(target)


session = requests.Session()
session.trust_env = False
base = f'http://127.0.0.1:{args.port}'


def get(route, **kwargs):
    report['requests'].append({'method': 'GET', 'route': route})
    response = session.get(base + route, timeout=(5, 180), **kwargs)
    assert response.ok, (response.status_code, response.text[:600])
    return response.json()


def start():
    with socket.socket() as check:
        assert check.connect_ex(('127.0.0.1', args.port)) != 0, 'Verification port already belongs to another process'
    process = subprocess.Popen([str(binary), 'serve', '--data-dir', str(root), '--port', str(args.port)],
                               stdout=log, stderr=log, creationflags=subprocess.CREATE_NO_WINDOW if os.name == 'nt' else 0)
    for attempt in range(100):
        try:
            assert normal_path(get('/health')['storageRoot']) == root
            assert get('/proxy') == {'mode': 'custom', 'url': 'http://127.0.0.1:9'}
            return process
        except requests.ConnectionError:
            assert process.poll() is None
            assert attempt < 99
            time.sleep(.1)
    raise AssertionError('Native service did not become ready')


def stop(process):
    assert all(job['status'] not in ['queued', 'running'] for job in get('/jobs'))
    process.terminate()
    process.wait(timeout=20)


def rgba(data_url):
    prefix = 'data:image/png;base64,'
    assert data_url.startswith(prefix)
    encoded = base64.b64decode(data_url[len(prefix):], validate=True)
    image = Image.open(io.BytesIO(encoded))
    assert image.mode == 'RGBA'
    return np.asarray(image), encoded


def source_samples(file, edge=768):
    with rasterio.open(file) as original:
        best, extent = None, max(original.width, original.height)
        for index in range(len(original.overviews(1))):
            with rasterio.open(file, OVERVIEW_LEVEL=index) as candidate:
                current = max(candidate.width, candidate.height)
                if edge <= current < extent:
                    best, extent = index, current
    with rasterio.open(file, **({} if best is None else {'OVERVIEW_LEVEL': best})) as selected:
        values = selected.read([1, 2, 3, 4]).transpose(1, 2, 0)
        mask = selected.dataset_mask() if 'per_dataset' in str(selected.mask_flag_enums) else None
    return values, mask, best


def cache_identity():
    return {str(file.relative_to(root)).replace('\\', '/'): {'sha256': sha(file), 'fileIdentity': file.stat().st_ino}
            for file in (root / 'cache' / 'thumbnails').rglob('*.json')}


write_report()
process = None
thumbnails = {}
log = (root / 'native-views.log').open('ab')
try:
    process = start()
    restored = {id: get('/jobs/' + id) for id in cloned}
    assert all(restored[id]['status'] == 'succeeded' and restored[id]['settled'] for id in cloned)
    for entry in entries:
        job = cloned[entry['job']['id']]
        thumbnails[job['id']] = get('/jobs/' + job['id'] + '/thumbnail')
        assert thumbnails[job['id']]['sha256'] == job['sha256']
        if args.legacy_060:
            entry['metadata'] = get('/jobs/' + job['id'] + '/raster')
            with rasterio.open(normal_path(job['outputPath'])) as original:
                metadata = entry['metadata']
                assert (metadata['width'], metadata['height'], metadata['bandCount']) == (original.width, original.height, original.count)
                assert original.count == 4 and original.dtypes == ('uint8',) * 4 and metadata['dataType'] == 'UInt8'
                assert metadata['crs'] == original.crs.to_string() == 'EPSG:26910'
                assert metadata['bounds'] == list(original.bounds)
                assert metadata['pixelSize'] == [original.transform.a, -original.transform.e] == [.6, .6]
                assert metadata['nodata'] == original.nodata is None
                assert metadata['aerial']['bands'] == ['red', 'green', 'blue', 'nir']
                assert metadata['aerial']['displayBands'] == [1, 2, 3]
            thumbnail_values, thumbnail_mask, _ = source_samples(normal_path(job['outputPath']), edge=160)
            thumbnail_rgba, _ = rgba(thumbnails[job['id']]['dataUrl'])
            yy = np.arange(thumbnail_rgba.shape[0]) * thumbnail_values.shape[0] // thumbnail_rgba.shape[0]
            xx = np.arange(thumbnail_rgba.shape[1]) * thumbnail_values.shape[1] // thumbnail_rgba.shape[1]
            np.testing.assert_array_equal(thumbnail_rgba[:, :, :3], thumbnail_values[np.ix_(yy, xx)][:, :, :3])
            np.testing.assert_array_equal(thumbnail_rgba[:, :, 3], thumbnail_mask[np.ix_(yy, xx)] if thumbnail_mask is not None else np.full(thumbnail_rgba.shape[:2], 255))
        else:
            assert thumbnails[job['id']]['dataUrl'] == entry['thumbnail']['dataUrl']
    baseline = cache_identity()
    assert len(baseline) == file_count
    profile_fields = ['width', 'height', 'bandCount', 'dataType', 'crs', 'bounds', 'pixelSize', 'nodata', 'sha256', 'classes']
    for entry in entries:
        job = cloned[entry['job']['id']]
        file = normal_path(job['outputPath'])
        values, mask, overview = source_samples(file)
        proof = {'jobId': job['id'], 'sha256': job['sha256'], 'group': entry['group'], 'case': entry.get('case', 'original'),
                 'sourceBytes': job['bytesDownloaded'], 'views': [], 'rawPixels': [], 'gdalOverviewIndex': overview}
        for view, bands in {'rgb': [1, 2, 3], 'cir': [4, 1, 2], 'nir': [4, 4, 4]}.items():
            metadata = get('/jobs/' + job['id'] + '/raster', params={'aerialView': view})
            assert all(metadata[field] == entry['metadata'][field] for field in profile_fields)
            assert metadata['aerial'] == {**entry['metadata']['aerial'], 'displayBands': bands}
            pixels, encoded = rgba(metadata['previewDataUrl'])
            yy = np.arange(pixels.shape[0]) * values.shape[0] // pixels.shape[0]
            xx = np.arange(pixels.shape[1]) * values.shape[1] // pixels.shape[1]
            sampled = values[np.ix_(yy, xx)]
            np.testing.assert_array_equal(pixels[:, :, :3], sampled[:, :, np.array(bands) - 1])
            if job['kind'] == 'raster_mosaic':
                assert mask is not None
                np.testing.assert_array_equal(pixels[:, :, 3], mask[np.ix_(yy, xx)])
            else:
                assert np.all(pixels[:, :, 3] == 255)
            name = f'{job["id"]}.{view}.png'
            (root / name).write_bytes(encoded)
            proof['views'].append({'view': view, 'displayBands': bands, 'pngFile': name,
                                   'pngSha256': hashlib.sha256(encoded).hexdigest(),
                                   'dimensions': [pixels.shape[1], pixels.shape[0]], 'pixelsCompared': int(pixels.shape[0] * pixels.shape[1]),
                                   'exactChannelsEqual': True, 'exactIndependentAlphaEqual': True,
                                   'opaqueZeroNirPreviewPixels': int(np.count_nonzero((sampled[:, :, 3] == 0) & (pixels[:, :, 3] == 255)))})
        with rasterio.open(file) as original:
            for column, row in [(0, 0), (original.width // 2, original.height // 2), (original.width - 1, original.height - 1)]:
                x, y = original.xy(row, column)
                sample = get('/jobs/' + job['id'] + '/pixel', params={'x': x, 'y': y})
                raw = original.read(window=rasterio.windows.Window(column, row, 1, 1))[:, 0, 0].tolist()
                assert sample['values'] == raw[:3] and sample['nearInfrared'] == raw[3]
                assert sample['sha256'] == job['sha256'] and sample['pixel'] == [column, row]
                proof['rawPixels'].append(sample)
        assert (get('/jobs/' + job['id'])['mosaicOutput'] or {}).get('aerial', {}).get('displayBands', [1, 2, 3]) == [1, 2, 3]
        report['entries'].append(proof)
        write_report()
        print(json.dumps({'stage': 'real-view-channels', 'group': proof['group'], 'case': proof['case'], 'views': 3}), flush=True)
    assert cache_identity() == baseline
    report['displaySelectionLeavesDefaultThumbnailCacheUnchanged'] = True
    bad = session.get(base + '/jobs/' + entries[0]['job']['id'] + '/raster?aerialView=ndvi', timeout=(5, 10))
    assert bad.status_code == 400
    report['unknownViewRejected'] = True
    stop(process)
    process = None
    process = start()
    for id, expected in thumbnails.items():
        assert get('/jobs/' + id + '/thumbnail') == expected
    assert cache_identity() == baseline
    report['restart'] = {'offline': True, 'filesRestored': file_count, 'unchangedRgbThumbnails': file_count, 'unchangedCacheFileIdentityAndBytes': True}
    assert all(sha(normal_path(job['outputPath'])) == job['sha256'] for job in cloned.values())
    assert sha(binary) == binary_sha
    report['sourceBytesUnchanged'] = True
    report['status'] = 'passed'
    report['completedAt'] = datetime.now(timezone.utc).isoformat()
    write_report()
    print(json.dumps({'stage': 'naip-views-complete', 'status': 'passed', 'files': file_count, 'views': file_count * 3,
                      'rgbaPixelsCompared': sum(v['pixelsCompared'] for p in report['entries'] for v in p['views'])}), flush=True)
except Exception as error:
    report['status'] = 'failed'
    report['failure'] = {'message': str(error), 'checkedAt': datetime.now(timezone.utc).isoformat()}
    write_report()
    raise
finally:
    if process is not None and process.poll() is None:
        stop(process)
    log.close()
