"""Independent GDAL verification of synthetic native VIIRS outputs only.

Requires Rasterio and NumPy for this development check, never for GeoD runtime.
Generate outputs with GEOD_VIIRS_PREPARE_EVIDENCE and the native prepare tests.
"""
import argparse
import hashlib
import json
import math
from pathlib import Path

import numpy as np
import rasterio
from rasterio.windows import Window

parser = argparse.ArgumentParser()
parser.add_argument('outputs', type=Path)
args = parser.parse_args()
root = Path(__file__).resolve().parents[1]
expected = json.loads((root / 'crates/geod-runtime/fixtures/viirs/expected.json').read_text(encoding='utf-8'))['fixtures']
checks = []
sample_count = 0
for index, fixture in enumerate(expected):
    for channel, key in enumerate(['red', 'green', 'blue']):
        with rasterio.open(args.outputs / f'{index}-{key}.tif') as data:
            pixels = data.read(1)
            assert data.dtypes == ('int16',) and data.count == 1 and data.shape == (1200, 1200)
            assert data.nodata == -28672 and data.scales == (0.0001,) and data.offsets == (0.0,)
            assert data.tags()['PRODUCT'] == 'viirs-09a1-v002'
            assert data.tags()['AREA_OR_POINT'] == 'Area'
            crs = data.crs.to_dict()
            assert crs['proj'] == 'sinu' and abs(crs['R'] - 6371007.181) < 1e-8 and crs['units'] == 'm'
            size = math.pi * 6371007.181 / 18
            assert abs(data.transform.a - size / 1200) < 1e-7 and abs(data.transform.e + size / 1200) < 1e-7
            assert np.allclose(list(data.bounds), [-10 * size, 3 * size, -9 * size, 4 * size], rtol=0, atol=0.02)
            digest = hashlib.sha256(pixels.astype('<i2').tobytes()).hexdigest()
            assert digest == fixture['bands'][channel]['samplesSha256']
            sample_count += pixels.size
            checks.append({'platform': fixture['itemId'].split('.')[0], 'band': key, 'samples': pixels.size,
                           'samplesSha256': digest, 'gridCalibrationNoDataMatch': True})
with rasterio.open(args.outputs / '1-red.tif') as source, rasterio.open(args.outputs / 'project-red-clip.tif') as clip:
    col = round((clip.transform.c - source.transform.c) / source.transform.a)
    row = round((clip.transform.f - source.transform.f) / source.transform.e)
    reference = source.read(1, window=Window(col, row, clip.width, clip.height))
    assert np.array_equal(clip.read(1), reference)
    assert source.crs == clip.crs and source.scales == clip.scales and source.offsets == clip.offsets and source.nodata == clip.nodata
    clipped = clip.width * clip.height
report = {'provenance': 'Synthetic VIIRS fixtures only; not NASA observations or production authorization',
          'reader': f'Rasterio {rasterio.__version__}/GDAL {rasterio.__gdal_version__} + NumPy',
          'preparedSamplesCompared': int(sample_count), 'prepared': checks,
          'projectClipSamplesCompared': clipped, 'projectClipExact': True}
(args.outputs / 'independent-report.json').write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
print(json.dumps({'preparedSamplesCompared': int(sample_count), 'clipSamplesCompared': clipped, 'allPassed': True}))
