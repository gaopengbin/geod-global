"""Independent GDAL/PROJ/GEOS validation of real managed NAIP files.

Use --partial while the native driver is still transferring other originals.
That mode accepts only the completed entries it actually checks, never the full
provider matrix. No credentials, signed URLs or original imagery are bundled.
"""
import argparse
import base64
import hashlib
import io
import json
import os
import subprocess
import time
from datetime import datetime, timezone
from pathlib import Path

import numpy as np
import rasterio
import requests
import shapely
import tifffile
from PIL import Image
from pyproj import Transformer
from rasterio.windows import Window
from shapely.geometry import shape

parser = argparse.ArgumentParser()
parser.add_argument('root')
parser.add_argument('port', type=int, nargs='?', default=4608)
parser.add_argument('--partial', action='store_true')
args = parser.parse_args()
workspace = Path.cwd().resolve()
root = Path(args.root).resolve()
assert root.parent == workspace / '.verification' and root.name.startswith('naip-resolutions-')
report = json.loads((root / 'native-resolution-verification.json').read_text(encoding='utf-8'))
assert args.partial or report['status'] == 'passed'
entries = report['originals'] + report['outputs']
assert entries, 'No completed native entries are available for independent acceptance yet'
base = f'http://127.0.0.1:{args.port}'
session = requests.Session()
session.trust_env = False
session.headers['X-GeoD-Client'] = 'geod-global'
runtime = None
runtime_log = None


def normalized_path(value):
    text = str(value)
    if os.name == 'nt' and text.startswith('\\\\?\\'):
        text = text[4:]
    return Path(text).resolve()


def file_sha(file):
    with Path(file).open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def get(route):
    response = session.get(base + route, timeout=(5, 180))
    assert response.ok, (response.status_code, response.text[:600])
    return response.json()


def ready():
    for attempt in range(100):
        try:
            health = get('/health')
            assert normalized_path(health['storageRoot']) == root
            return
        except (requests.RequestException, AssertionError):
            if runtime is None or runtime.poll() is not None or attempt == 99:
                raise
            time.sleep(.1)


def png_array(data_url):
    prefix = 'data:image/png;base64,'
    assert data_url.startswith(prefix)
    raw = base64.b64decode(data_url[len(prefix):], validate=True)
    image = Image.open(io.BytesIO(raw))
    assert image.mode == 'RGBA'
    return np.asarray(image), raw


def source_preview(file, edge):
    # Match the documented native selection rule against independent GDAL
    # dimensions, including rounding in embedded overview grids.
    with rasterio.open(file) as original:
        best = None
        best_extent = max(original.width, original.height)
        for index in range(len(original.overviews(1))):
            with rasterio.open(file, OVERVIEW_LEVEL=index) as candidate:
                extent = max(candidate.width, candidate.height)
                if edge <= extent < best_extent:
                    best = index
                    best_extent = extent
    options = {} if best is None else {'OVERVIEW_LEVEL': best}
    with rasterio.open(file, **options) as selected:
        values = selected.read([1, 2, 3]).transpose(1, 2, 0)
        mask = selected.dataset_mask() if 'per_dataset' in str(selected.mask_flag_enums) else None
        return values, mask, best


def verify_images(entry, job, file, masked):
    result = {}
    for label, edge, value in [('preview', 768, entry['metadata']), ('thumbnail', 160, entry['thumbnail'])]:
        url = value['previewDataUrl'] if label == 'preview' else value['dataUrl']
        png, encoded = png_array(url)
        values, mask, overview = source_preview(file, edge)
        yy = np.arange(png.shape[0]) * values.shape[0] // png.shape[0]
        xx = np.arange(png.shape[1]) * values.shape[1] // png.shape[1]
        np.testing.assert_array_equal(png[:, :, :3], values[np.ix_(yy, xx)])
        if masked:
            assert mask is not None
            np.testing.assert_array_equal(png[:, :, 3], mask[np.ix_(yy, xx)])
        else:
            # A legacy TIFF alpha tag must never make the real NIR values into
            # display transparency or premultiply the source RGB channels.
            assert np.all(png[:, :, 3] == 255)
        (root / f'{job["id"]}.{label}.png').write_bytes(encoded)
        result[label] = {'dimensions': [png.shape[1], png.shape[0]], 'pixelsCompared': int(png.shape[0] * png.shape[1]),
                         'pngSha256': hashlib.sha256(encoded).hexdigest(), 'gdalOverviewIndex': overview,
                         'exactRgbEqual': True, 'exactAlphaEqual': True}
    return result


verified_sources = {}


def validated_file(job):
    assert job['status'] == 'succeeded'
    file = normalized_path(job['outputPath'])
    assert file == root / 'assets' / f'{job["id"]}.tif'
    assert file.stat().st_size == job['bytesDownloaded'] == job['totalBytes']
    if job['id'] not in verified_sources:
        assert file_sha(file) == job['sha256']
        verified_sources[job['id']] = job['sha256']
    assert verified_sources[job['id']] == job['sha256']
    return file


def source_tags(file):
    with tifffile.TiffFile(file) as tiff:
        page = tiff.pages[0]
        return {name: page.tags[name].value if name in page.tags else None for name in
                ['Compression', 'PhotometricInterpretation', 'SamplesPerPixel', 'ExtraSamples', 'Predictor', 'TileWidth', 'TileLength', 'GDAL_NODATA']}


def verify_original(entry):
    job = get('/jobs/' + entry['job']['id'])
    file = validated_file(job)
    metadata = entry['metadata']
    tags = source_tags(file)
    extra = list(tags['ExtraSamples'])
    assert extra in ([0], [2]) and tags['GDAL_NODATA'] is None
    assert metadata['aerial'].get('sourceExtraSample', 0) == extra[0]
    if extra == [2]:
        assert job['itemId'].split('_')[5] == '1' and metadata['pixelSize'] == [1, 1]
    points = []
    with rasterio.open(file) as source:
        assert source.count == 4 and source.dtypes == ('uint8',) * 4
        assert str(source.crs) == metadata['crs'] and source.nodata is None
        assert [source.width, source.height] == [metadata['width'], metadata['height']]
        np.testing.assert_allclose(source.res, metadata['pixelSize'], rtol=0, atol=1e-9)
        np.testing.assert_allclose(tuple(source.bounds), metadata['bounds'], rtol=0, atol=1e-7)
        for column, row in [(0, 0), (511, 511), (512, 512), (source.width - 1, 0), (0, source.height - 1),
                            (source.width - 1, source.height - 1), (source.width // 2, source.height // 2)]:
            x, y = source.xy(row, column)
            values = source.read(window=Window(column, row, 1, 1)).reshape(-1).tolist()
            actual = get(f'/jobs/{job["id"]}/pixel?x={x}&y={y}')
            assert actual['pixel'] == [column, row] and actual['values'] + [actual['nearInfrared']] == values
            assert actual['isNoData'] is False
            points.append({'pixel': [column, row], 'values': values, 'isNoData': False})
        color_interpretation = [value.name for value in source.colorinterp]
    return {'jobId': job['id'], 'itemId': job['itemId'], 'href': job['href'], 'bytes': job['bytesDownloaded'], 'sha256': job['sha256'],
            'grid': {key: metadata[key] for key in ['width', 'height', 'bandCount', 'dataType', 'crs', 'bounds', 'pixelSize', 'nodata']},
            'physicalSourceTags': tags, 'gdalSourceColorInterpretation': color_interpretation,
            'bandRoles': 'red,green,blue,nir as bound to the exact official NAIP catalogue item',
            'sourceBytesUnchanged': file_sha(file) == job['sha256'], 'points': points, 'images': verify_images(entry, job, file, False)}


def verify_output(entry):
    job = get('/jobs/' + entry['job']['id'])
    file = validated_file(job)
    plan = job['mosaicOutput']
    project = next(case['project'] for case in report['cases'] if case['project']['id'] == job['mosaic']['projectId'])
    with rasterio.open(file) as target:
        assert target.count == 4 and target.dtypes == ('uint8',) * 4 and target.nodata is None
        assert str(target.crs) == plan['crs'] and 'per_dataset' in str(target.mask_flag_enums)
        np.testing.assert_allclose(target.res, plan['pixelSize'], rtol=0, atol=1e-9)
        actual = target.read()
        actual_mask = target.dataset_mask()
        expected = np.zeros_like(actual)
        coverage = np.zeros((target.height, target.width), dtype=bool)
        pins = []
        for pin in job['mosaic']['sources']:
            source_job = get('/jobs/' + pin['jobId'])
            original = validated_file(source_job)
            assert pin['sha256'] == source_job['sha256']
            with rasterio.open(original) as source:
                assert source.count == 4 and source.crs == target.crs and source.nodata is None
                np.testing.assert_allclose(source.res, target.res, rtol=0, atol=1e-9)
                column = round((source.transform.c - target.transform.c) / target.res[0])
                row = round((target.transform.f - source.transform.f) / target.res[1])
                left, top = max(0, column), max(0, row)
                right, bottom = min(target.width, column + source.width), min(target.height, row + source.height)
                if right > left and bottom > top:
                    expected[:, top:bottom, left:right] = source.read(window=Window(left - column, top - row, right - left, bottom - top))
                    coverage[top:bottom, left:right] = True
                pins.append({'jobId': source_job['id'], 'itemId': source_job['itemId'], 'sha256': pin['sha256'], 'window': [left, top, right, bottom]})
        inside = np.ones_like(coverage)
        if project.get('geometry'):
            xx, yy = np.meshgrid(target.transform.c + (np.arange(target.width) + .5) * target.res[0],
                                 target.transform.f - (np.arange(target.height) + .5) * target.res[1])
            longitude, latitude = Transformer.from_crs(target.crs, 'EPSG:4326', always_xy=True).transform(xx, yy)
            inside = shapely.contains_xy(shape(project['geometry']), longitude, latitude)
        coverage &= inside
        expected[:, ~coverage] = 0
        np.testing.assert_array_equal(actual, expected)
        np.testing.assert_array_equal(actual_mask, coverage.astype('uint8') * 255)
        assert int(coverage.sum()) == plan['coveredPixels']
        assert int((~inside).sum()) == plan['maskedPixels']
        points = []
        for valid in [False, True]:
            locations = np.argwhere(coverage == valid)
            if not len(locations):
                continue
            for index in [0, len(locations) // 2, -1]:
                row, column = map(int, locations[index])
                x, y = target.xy(row, column)
                pixel = get(f'/jobs/{job["id"]}/pixel?x={x}&y={y}')
                assert pixel['pixel'] == [column, row] and pixel['values'] + [pixel['nearInfrared']] == actual[:, row, column].tolist()
                assert pixel['isNoData'] == (not valid)
                points.append({'pixel': [column, row], 'values': actual[:, row, column].tolist(), 'isNoData': not valid})
    assert list(source_tags(file)['ExtraSamples']) == [0]
    return {'jobId': job['id'], 'group': entry['group'], 'case': entry['case'], 'bytes': job['bytesDownloaded'], 'sha256': job['sha256'],
            'grid': plan, 'sourcePins': pins, 'points': points,
            'fourChannelSamplesCompared': int(actual.size), 'maskPixelsCompared': int(actual_mask.size),
            'exactFourChannelsEqual': True, 'exactCoverageMaskEqual': True,
            'polygonRule': 'inverse-project every native pixel centre into the original WGS84 polygon and its hole',
            'images': verify_images(entry, job, file, True)}


proof = {'schema': 'geod-naip-resolution-independent/v1', 'checkedAt': datetime.now(timezone.utc).isoformat(),
         'nativeBinarySha256': report['nativeBinarySha256'], 'tools': {'rasterio': rasterio.__version__, 'shapely': shapely.__version__},
         'status': 'in_progress', 'originals': [], 'outputs': []}
try:
    try:
        ready()
    except requests.RequestException:
        assert not args.partial and report['status'] == 'passed'
        exe = root / f'runtime-{report["nativeBinarySha256"][:16]}.exe'
        assert file_sha(exe) == report['nativeBinarySha256']
        runtime_log = (root / 'independent-runtime.log').open('ab')
        runtime = subprocess.Popen([str(exe), 'serve', '--data-dir', str(root), '--port', str(args.port)],
                                   stdout=runtime_log, stderr=runtime_log,
                                   creationflags=subprocess.CREATE_NO_WINDOW if os.name == 'nt' else 0)
        ready()
    for entry in report['originals']:
        proof['originals'].append(verify_original(entry))
        print(json.dumps({'stage': 'independent-original', 'itemId': entry['job']['itemId'], 'bytes': entry['job']['bytesDownloaded']}), flush=True)
    for entry in report['outputs']:
        proof['outputs'].append(verify_output(entry))
        print(json.dumps({'stage': 'independent-output', 'group': entry['group'], 'case': entry['case'], 'samples': proof['outputs'][-1]['fourChannelSamplesCompared']}), flush=True)
    proof['status'] = 'passed' if report['status'] == 'passed' and len(proof['originals']) == 3 and len(proof['outputs']) == 5 else 'partial'
    proof['completedAt'] = datetime.now(timezone.utc).isoformat()
    (root / 'independent-resolution-verification.json').write_text(json.dumps(proof, indent=2) + '\n', encoding='utf-8')
    print(json.dumps({'stage': 'independent-complete', 'status': proof['status'], 'originals': len(proof['originals']), 'outputs': len(proof['outputs'])}), flush=True)
finally:
    if runtime is not None:
        runtime.terminate()
        runtime.wait(timeout=30)
    if runtime_log is not None:
        runtime_log.close()
