"""Independently verify actual VH/HH/HV originals and native project rasters.

Consumes the private native receipt, not synthetic raster fixtures. Partial
mode accepts only completed original/output records and stays explicitly partial.
GDAL, PROJ and GEOS are test dependencies; they are not application dependencies.
"""
import argparse
import hashlib
import json
import math
import importlib.util
from pathlib import Path
from datetime import datetime, timezone

import numpy as np
import rasterio
from rasterio.windows import Window
from pyproj import Transformer
from shapely import contains_xy
from shapely.geometry import shape
_preview_spec = importlib.util.spec_from_file_location('radar_original', Path(__file__).with_name('verify-radar-original.py'))
_preview_module = importlib.util.module_from_spec(_preview_spec)
_preview_spec.loader.exec_module(_preview_module)
verify_preview = _preview_module.verify_preview


def digest(path):
    with Path(path).open('rb') as source:
        return hashlib.file_digest(source, 'sha256').hexdigest()


def profile(key):
    return {'product': 'sentinel-1-iw-rtc', 'polarization': key.upper(),
            'quantity': 'gamma0', 'unit': 'linear'}


def verify_geometry(source, metadata, key):
    assert source.count == 1 and source.dtypes == ('float32',)
    assert source.nodata == metadata['nodata'] == -32768
    assert source.crs.to_string() == metadata['crs']
    assert source.width == metadata['width'] and source.height == metadata['height']
    assert list(source.bounds) == metadata['bounds']
    assert list(source.res) == metadata['pixelSize'] == [10, 10]
    assert source.tags().get('AREA_OR_POINT') == 'Area'
    assert all(metadata['radar'][k] == v for k, v in profile(key).items())
    assert metadata['radar']['displayUnit'] == 'dB'


def verify_pixels(source, entry):
    checks = []
    for pixel in entry['pixels']:
        col, row = pixel['pixel']
        value = float(source.read(1, window=Window(col, row, 1, 1))[0, 0])
        assert pixel['value'] == value
        assert pixel['sha256'] == entry['job']['sha256']
        assert pixel['crs'] == source.crs.to_string()
        assert pixel['isNoData'] == (value == -32768)
        assert pixel['label'] == ('No data' if value == -32768 else 'Gamma0 · ' + entry['job']['assetKey'].upper())
        x, y = source.xy(row, col)
        assert pixel['coordinate'] == [x, y]
        assert pixel['center'] == [x, y]
        if value > 0:
            assert math.isclose(pixel['decibels'], 10 * math.log10(value), rel_tol=0, abs_tol=1e-11)
        else:
            assert 'decibels' not in pixel
        checks.append({'pixel': [col, row], 'gamma0': value, 'decibels': pixel.get('decibels'),
                       'exactFloat32Match': True})
    return checks


def verify_images(path, entry):
    preview = verify_preview(path, entry['metadata'])
    thumb = entry['thumbnail']
    assert thumb['jobId'] == entry['job']['id'] and thumb['sha256'] == entry['job']['sha256']
    thumbnail = verify_preview(path, {'previewWidth': thumb['width'], 'previewHeight': thumb['height'],
                                     'previewDataUrl': thumb['dataUrl']})
    return {'preview': preview, 'thumbnail': thumbnail}


def verify_original(entry, items):
    job, metadata = entry['job'], entry['metadata']
    assert job['kind'] == 'download' and job['assetKey'] in ['vh', 'hh', 'hv']
    assert job['status'] == 'succeeded' and job['settled']
    item = items[job['itemId']]
    assert job['href'] == item['assets'][job['assetKey']]['href']
    path = Path(job['outputPath'])
    assert path.stat().st_size == job['bytesDownloaded'] == job['totalBytes']
    assert digest(path) == job['sha256']
    with rasterio.open(path) as source:
        verify_geometry(source, metadata, job['assetKey'])
        assert source.crs.to_epsg() == item['properties']['proj:epsg']
        assert [source.height, source.width] == item['properties']['proj:shape']
        assert list(source.transform) == item['properties']['proj:transform']
        pixels = verify_pixels(source, entry)
    return {'jobId': job['id'], 'itemId': job['itemId'], 'key': job['assetKey'],
            'bytes': job['bytesDownloaded'], 'sha256': job['sha256'], 'pixels': pixels,
            **verify_images(path, entry)}


def verify_output(entry, originals, project, items):
    job, metadata, key = entry['job'], entry['metadata'], entry['key']
    plan = job['mosaicOutput']
    expected_profile = profile(key)
    assert job['assetKey'] == key and plan['radar'] == expected_profile
    assert all(plan.get(k) is None for k in ['elevation', 'calibration', 'aerial', 'modis', 'quality'])
    assert job['mosaic']['projectId'] == project['id'] and job['status'] == 'succeeded' and job['settled']
    path = Path(job['outputPath'])
    assert path.stat().st_size == job['bytesDownloaded'] and digest(path) == job['sha256']
    pins = job['mosaic']['sources']
    chronology = lambda pin: (items[originals[pin['jobId']]['job']['itemId']]['properties']['datetime'],
                              originals[pin['jobId']]['job']['itemId'])
    assert pins == sorted(pins, key=chronology)
    assert len(pins) == (1 if entry['case'] == 'single' else 2)
    overlap = {'olderValidUnderNewerNoData': 0, 'newerValidReplacesDifferentOlderValue': 0}
    with rasterio.open(path) as output:
        verify_geometry(output, metadata, key)
        assert list(output.bounds) == plan['bounds'] and output.crs.to_string() == plan['crs']
        assert output.tags().get('PRODUCT') == expected_profile['product']
        assert output.tags(1).get('POLARIZATION') == key.upper()
        assert output.units == ('gamma0 (linear)',)
        actual = output.read(1)
        expected = np.full(actual.shape, -32768, dtype=np.float32)
        for pin in pins:
            original = originals[pin['jobId']]['job']
            assert original['assetKey'] == key and original['sha256'] == pin['sha256']
            with rasterio.open(original['outputPath']) as source:
                assert source.crs == output.crs and source.res == output.res
                col, row = (~source.transform) * (output.bounds.left, output.bounds.top)
                assert abs(col - round(col)) < 1e-5 and abs(row - round(row)) < 1e-5
                values = source.read(1, window=Window(round(col), round(row), output.width, output.height),
                                     boundless=True, fill_value=-32768)
                assert np.isfinite(values).all() and ((values >= 0) | (values == -32768)).all()
                previous_valid, current_valid = expected != -32768, values != -32768
                overlap['olderValidUnderNewerNoData'] += int((previous_valid & ~current_valid).sum())
                overlap['newerValidReplacesDifferentOlderValue'] += int((previous_valid & current_valid
                    & (values.view(np.uint32) != expected.view(np.uint32))).sum())
                np.copyto(expected, values, where=current_valid)
        masked = 0
        if project.get('geometry'):
            assert entry['case'] == 'polygon'
            # Straight geographic edges must be tested in their original CRS.
            transform = Transformer.from_crs(output.crs, 'EPSG:4326', always_xy=True)
            columns, rows = np.meshgrid(np.arange(output.width) + .5, np.arange(output.height) + .5)
            x, y = output.transform * (columns, rows)
            longitude, latitude = transform.transform(x, y)
            outside = ~contains_xy(shape(project['geometry']), longitude, latitude)
            masked = int(outside.sum())
            assert masked > 0
            expected[outside] = -32768
        assert np.array_equal(actual.view(np.uint32), expected.view(np.uint32))
        assert plan['coveredPixels'] == int((expected != -32768).sum())
        assert plan['maskedPixels'] == masked
        pixels = verify_pixels(output, entry)
    manifest = json.loads(Path(job['manifestPath']).read_text(encoding='utf8'))
    assert manifest['plan'] == plan and manifest['output']['sha256'] == job['sha256']
    assert all(source['radar'] == expected_profile and source['additionalCalibrationApplied'] is False
               and source['speckleFilteringApplied'] is False for source in manifest['sources'])
    return {'jobId': job['id'], 'projectId': project['id'], 'case': entry['case'], 'key': key,
            'bytes': job['bytesDownloaded'], 'sha256': job['sha256'], 'plan': plan,
            'float32ValuesCompared': int(actual.size), 'allFloat32BitsMatch': True,
            'overlapChecks': overlap, 'pixels': pixels,
            'sourceAcquisitions': [{'itemId': originals[pin['jobId']]['job']['itemId'],
                                    'datetime': chronology(pin)[0]} for pin in pins],
            **verify_images(path, entry)}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('root', type=Path)
    parser.add_argument('--partial', action='store_true')
    parser.add_argument('--completed-projects', action='store_true')
    parser.add_argument('--completed-originals', action='store_true')
    parser.add_argument('--output', type=Path, help='Separate private receipt without replacing an accepted cohort')
    args = parser.parse_args()
    args.root = args.root.resolve()
    assert args.root.parent == Path('.verification').resolve() and args.root.name.startswith('radar-polarizations-')
    assert not (args.completed_projects and args.completed_originals)
    if args.completed_projects or args.completed_originals:
        assert args.partial, 'A ready project subset cannot replace full-matrix acceptance'
    native_file = ('early-projects-native-verification.json' if args.completed_projects else
                   'early-originals-native-verification.json' if args.completed_originals else 'native-polarizations-verification.json')
    native_raw = (args.root / native_file).read_bytes()
    native = json.loads(native_raw)
    native_sha = hashlib.sha256(native_raw).hexdigest()
    snapshot_file = 'evidence-snapshots/' + native_sha + '.json'
    snapshot = args.root / snapshot_file
    snapshot.parent.mkdir(exist_ok=True)
    try:
        with snapshot.open('xb') as destination:
            destination.write(native_raw)
    except FileExistsError:
        assert snapshot.read_bytes() == native_raw, 'The immutable receipt snapshot changed'
    if not args.partial:
        assert native['status'] == 'passed' and len(native['originals']) == 6 and len(native['outputs']) == 9
    assert native['originals'], 'No completed real originals are available'
    binary = Path('target/debug/geod-runtime.exe')
    assert digest(binary) == native['nativeBinarySha256']
    items = {}
    for catalog in native['catalogs']:
        data = json.loads((args.root / catalog['file']).read_text(encoding='utf8'))
        items.update({item['id']: item for item in data['features']})
    originals = {entry['job']['id']: entry for entry in native['originals']}
    original_checks = [verify_original(entry, items) for entry in native['originals']]
    for result in original_checks:
        print(json.dumps({'original': result['itemId'], 'key': result['key'], 'bytes': result['bytes'], 'sha256': result['sha256']}), flush=True)
    outputs = []
    for entry in native['outputs']:
        project = next(case['project'] for case in native['cases']
                       if case['group'] == entry['group'] and case['kind'] == entry['case'])
        result = verify_output(entry, originals, project, items)
        outputs.append(result)
        print(json.dumps({'case': result['case'], 'key': result['key'],
                          'valuesCompared': result['float32ValuesCompared']}), flush=True)
    report = {'schema': 'geod-radar-polarizations-independent/v1', 'status': 'partial' if args.partial else 'passed',
              'checkedAt': datetime.now(timezone.utc).isoformat(),
              'nativeBinarySha256': native['nativeBinarySha256'],
              'nativeReceiptSha256': native_sha,
              'nativeReceiptFile': native_file,
              'nativeReceiptSnapshot': snapshot_file,
              'independentLibraries': {'rasterio': rasterio.__version__, 'gdal': rasterio.__gdal_version__},
              'originals': original_checks, 'outputs': outputs,
              'float32ValuesCompared': sum(entry['float32ValuesCompared'] for entry in outputs),
              'pixelChecks': sum(len(entry['pixels']) for entry in original_checks + outputs),
              'rgbaPixelsCompared': sum(entry[k]['previewPixelsCompared'] for entry in original_checks + outputs
                                        for k in ['preview', 'thumbnail']),
              'usedUserDesktop': False, 'terrainCorrectionAccuracyAssessed': False,
              'additionalCalibrationApplied': False, 'speckleFilteringApplied': False}
    if args.partial:
        report['pending'] = 'Remaining actual VH/HH/HV originals, nine project outputs, full offline restart, production UI and MCP.'
    output_file = ('early-projects-independent-verification.json' if args.completed_projects else
                   'early-originals-independent-verification.json' if args.completed_originals else 'independent-polarizations-verification.json')
    output = args.output.resolve() if args.output else args.root / output_file
    if args.output:
        assert output.parent == args.root and output.suffix == '.json'
        assert output.name not in [native_file, 'native-polarizations-verification.json'], 'Do not replace the acquisition receipt'
    temporary = output.with_suffix(output.suffix + '.tmp')
    temporary.write_text(json.dumps(report, indent=2) + '\n', encoding='utf8')
    temporary.replace(output)
    print(json.dumps({k: report[k] for k in ['status', 'float32ValuesCompared', 'pixelChecks', 'rgbaPixelsCompared']}))


if __name__ == '__main__':
    main()
