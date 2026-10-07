"""Independently read Agent-produced RGB/VI TIFFs against real pinned originals.

No GeoD decoder, model calls, network or user desktop. GDAL/NumPy evaluate the
original integer samples, QA and same-observation selection at every output cell.
"""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import numpy as np
import rasterio
from rasterio.windows import Window

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--root', required=True, type=Path)
args = parser.parse_args()
base = args.root.resolve()
receipt = json.loads((base / 'native-acceptance.json').read_text(encoding='utf-8'))
assert receipt['status'] == 'passed'
jobs = json.loads((base / 'core/jobs.json').read_text(encoding='utf-8'))
inputs = json.loads((base / 'inputs.json').read_text(encoding='utf-8'))

def sha(path):
    with Path(path).open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()

def plane_sha(value, dtype):
    return hashlib.sha256(value.astype(dtype).tobytes()).hexdigest()

for source in inputs['sources']:
    assert sha(source['originalPath']) == source['sha256']
    assert Path(source['originalPath']).stat().st_mtime_ns == source['mtimeNs']
    assert sha(jobs[source['jobId']]['outputPath']) == source['sha256']

module = importlib.util.spec_from_file_location('landsat_oracle', Path(__file__).with_name('verify-landsat-coupled.py'))
oracle = importlib.util.module_from_spec(module)
module.loader.exec_module(oracle)
rgb = jobs[receipt['rgbId']]
expected, counts, _, _, _ = oracle.coherent_oracle(rgb['rgbSpec'], jobs)
assert sha(rgb['outputPath']) == rgb['sha256'] == receipt['rgbSha256']
with rasterio.open(rgb['outputPath']) as ds:
    assert ds.dtypes == ('uint16',) * 3 and ds.nodata == 0
    assert ds.scales == (.0000275,) * 3 and ds.offsets == (-.2,) * 3
    assert ds.crs.to_string() == rgb['rgbSpec']['grid']['crs']
    assert list(ds.bounds) == rgb['rgbSpec']['grid']['bounds']
    assert np.array_equal(ds.read(), expected)
assert rgb['rgbOutput']['qualityMask'] == counts
assert rgb['rgbOutput']['samplesSha256'] == [plane_sha(b, '<u2') for b in expected]
report = {'schema': 'geod-agent-science-independent/v1', 'status': 'running', 'reader': f'Rasterio {rasterio.__version__} / GDAL {rasterio.__gdal_version__}', 'originalFilesUnchanged': True, 'sourceFilesChecked': len(inputs['sources']), 'rgb': {'jobId': rgb['id'], 'sha256': rgb['sha256'], 'width': expected.shape[2], 'height': expected.shape[1], 'exactSamples': int(expected.size), 'qualityCounts': counts}, 'indices': [], 'modelCalls': 0, 'usedUserDesktop': False}

def original_window(job, dst, fill):
    with rasterio.open(job['outputPath']) as source:
        # MODIS original/output WKT datum labels differ; compare every numeric
        # projection parameter and the exact sample spacing, not display names.
        assert source.crs.to_dict() == dst.crs.to_dict() and source.res == dst.res
        dx = (source.transform.c - dst.transform.c) / source.transform.a
        dy = (source.transform.f - dst.transform.f) / source.transform.e
        assert abs(dx - round(dx)) < 1e-5 and abs(dy - round(dy)) < 1e-5
        dx, dy = round(dx), round(dy)
        result = np.full((dst.height, dst.width), fill, dtype=source.dtypes[0])
        x0, x1, y0, y1 = max(0, dx), min(dst.width, dx + source.width), max(0, dy), min(dst.height, dy + source.height)
        if x1 > x0 and y1 > y0:
            result[y0:y1, x0:x1] = source.read(1, window=Window(x0 - dx, y0 - dy, x1 - x0, y1 - y0))
        return result

for id in receipt['outputIds']:
    job = jobs[id]
    if job['kind'] != 'raster_mosaic':
        continue
    spec = job['mosaic']['viSelection']
    assert spec['policy'] == 'good' and spec.get('geometry') is None
    assert sha(job['outputPath']) == job['sha256']
    with rasterio.open(job['outputPath']) as dst:
        assert dst.dtypes == ('int16',) and dst.nodata == -3000
        shape = (dst.height, dst.width)
        ndvi = np.full(shape, -3000, dtype=np.int16)
        evi = ndvi.copy()
        winner = np.zeros(shape, dtype=np.uint32)
        latest = winner.copy()
        for index, scene in enumerate(spec['scenes'], 1):
            originals = [jobs[s['jobId']] for s in scene['sources']]
            assert [j['assetKey'] for j in originals] == ['ndvi', 'evi', 'vi_quality', 'vi_reliability']
            assert all(j['itemId'] == scene['itemId'] for j in originals)
            a, b, qa, rank = [original_window(j, dst, fill) for j, fill in zip(originals, [-3000, -3000, 65535, -1])]
            complete = (a >= -2000) & (a <= 10000) & (b >= -2000) & (b <= 10000)
            quality = (qa != 65535) & np.isin((qa >> 2) & 15, [0, 1, 2, 4, 8, 9, 10]) & (((qa >> 6) & 3) != 3) & ((qa & ((1 << 8) | (1 << 10) | (1 << 14) | (1 << 15))) == 0)
            accepted = complete & quality & ((qa & 3) == 0) & (rank == 0)
            latest[complete] = index
            winner[accepted] = index
            ndvi[accepted], evi[accepted] = a[accepted], b[accepted]
        selected = ndvi if job['assetKey'] == 'ndvi' else evi
        assert np.array_equal(dst.read(1), selected)
        result = job['mosaicOutput']['viQuality']
        assert result['indicesSha256'] == [plane_sha(ndvi, '<i2'), plane_sha(evi, '<i2')]
        assert result['selectionSha256'] == plane_sha(winner, '<u4') == receipt['viSelectionSha256']
        assert result['inputCommonValidPixels'] == int((latest > 0).sum())
        assert result['rejectedPixels'] == int((winner == 0).sum())
        assert result['fallbackPixels'] == int(((winner > 0) & (winner < latest)).sum())
        assert result['sceneValidPixels'] == [int((winner == i).sum()) for i in range(1, len(spec['scenes']) + 1)]
        report['indices'].append({'jobId': id, 'index': job['assetKey'], 'sha256': job['sha256'], 'width': dst.width, 'height': dst.height, 'exactSamples': int(selected.size), 'selectionSha256': result['selectionSha256'], 'fallbackPixels': result['fallbackPixels']})
assert len(report['indices']) == 2
report['status'] = 'passed'
(base / 'independent-acceptance.json').write_text(json.dumps(report, indent=2), encoding='utf-8')
print(json.dumps(report))
