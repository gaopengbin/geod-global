"""Independently compare real native MODIS QA inspections against GDAL samples.

Reads only the originals and the native receipt in the specified QA directory.
Every grid cell is counted; every PNG pixel and all recorded raw bit fields are
compared independently. This is not provider accuracy or RGB mask acceptance.
"""
import argparse
import base64
import hashlib
import io
import json
from pathlib import Path

import numpy as np
import rasterio
from PIL import Image


def decoded(raw, key):
    if key == 'modis_qc':
        fields = [('MODLAND quality', 0, 1)]
        fields.extend((f'Band {b + 1} quality', 2 + b * 4, 5 + b * 4) for b in range(7))
        fields.extend([('Atmospheric correction', 30, 30), ('Adjacency correction', 31, 31)])
    else:
        fields = [('Cloud state', 0, 1), ('Cloud shadow', 2, 2), ('Land / water', 3, 5),
                  ('Aerosol correction uncertainty', 6, 7), ('Cirrus', 8, 9),
                  ('Internal cloud', 10, 10), ('Internal fire', 11, 11), ('MOD35 snow / ice', 12, 12),
                  ('Adjacent to cloud', 13, 13), ('Salt pan', 14, 14), ('Internal snow', 15, 15)]
    return [(name, first, last, (raw >> first) & ((1 << (last - first + 1)) - 1)) for name, first, last in fields]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('root')
    args = parser.parse_args()
    root = Path(args.root).resolve()
    assert root.parent == Path('.verification').resolve() and root.name.startswith('modis-quality-')
    report = json.loads((root / 'native-verification.json').read_text(encoding='utf-8'))
    assert report['status'] == 'passed' and len(report['cases']) == 2
    result = {'schema': 'geod-modis-quality-gdal/v1', 'independentDecoder': f'Rasterio {rasterio.__version__} / GDAL {rasterio.__gdal_version__}',
              'definition': 'NASA MOD09 User Guide C61 tables 10 and 13', 'cases': []}
    for case in report['cases']:
        key, job, meta = case['key'], case['job'], case['metadata']
        value = job['outputPath']
        source = Path(value[4:] if value.startswith(chr(92)*2+'?'+chr(92)) else value).resolve()
        assert source.is_relative_to(root) and hashlib.sha256(source.read_bytes()).hexdigest() == job['sha256']
        bits = 32 if key == 'modis_qc' else 16
        fill = (1 << bits) - 1
        with rasterio.open(source) as dataset:
            assert dataset.count == 1 and (dataset.width, dataset.height) == (2400, 2400)
            assert dataset.dtypes == (f'uint{bits}',) and dataset.nodata == fill
            assert dataset.tags()['AREA_OR_POINT'] == 'Area'
            crs = dataset.crs.to_dict()
            assert crs['proj'] == 'sinu' and abs(crs.get('R', crs.get('a', 0)) - 6371007.181) < 1e-6
            assert np.allclose(dataset.bounds, meta['bounds'], atol=1e-7, rtol=0)
            assert np.allclose(dataset.res, meta['pixelSize'], atol=1e-9, rtol=0)
            raw = dataset.read(1)
            valid = raw != fill
            counts = [int(np.count_nonzero(valid & ((raw & 3) == value))) for value in range(4)]
            assert counts == [c['count'] for c in meta['classes']]
            assert int(valid.sum()) == meta['quality']['validSampleCount']
            unknown = 0
            for pixel in case['pixels']:
                col, row = pixel['pixel']; value = int(raw[row, col]); assert value == pixel['value']
                assert pixel['isNoData'] == (value == fill)
                assert 'reflectance' not in pixel and 'values' not in pixel
                q = pixel['quality']; assert q['hex'] == f'0x{value:0{bits // 4}X}' and q['binary'] == f'{value:0{bits}b}'
                if value == fill:
                    assert q['fields'] == []
                    continue
                expected = decoded(value, key)
                assert [(f['name'], f['startBit'], f['endBit'], f['value']) for f in q['fields']] == expected
                for field in q['fields']:
                    undefined = field['name'].startswith('Band ') and 1 <= field['value'] <= 6
                    assert field['defined'] != undefined
                    if undefined:
                        assert field['label'] == 'Undocumented quality code'; unknown += 1
                center = dataset.transform * (col + .5, row + .5)
                assert np.allclose(center, pixel['center'], atol=1e-7, rtol=0)
            palette = np.array([[int(c['color'][i:i+2],16) for i in (1,3,5)] + [255] for c in meta['classes']],dtype=np.uint8)
            png_pixels = 0
            for kind, url in [('preview',meta['previewDataUrl']), ('thumbnail',case['thumbnail']['dataUrl'])]:
                image = np.asarray(Image.open(io.BytesIO(base64.b64decode(url.split(',')[1]))).convert('RGBA'))
                height,width = image.shape[:2]
                selected = raw[np.arange(height) * 2400 // height][:, np.arange(width) * 2400 // width]
                expected = palette[selected & 3].copy(); expected[selected == fill] = [0,0,0,0]
                assert np.array_equal(image, expected), kind
                png_pixels += width * height
            result['cases'].append({'assetKey':key,'sha256':job['sha256'],'bytes':source.stat().st_size,
              'dtype':dataset.dtypes[0],'originalPixelsCounted':raw.size,'validPixels':int(valid.sum()),'counts':counts,
              'rawPixelsCompared':len(case['pixels']),'originalMaximum':int(raw[valid].max()),
              'undefinedBandCodesPreserved':unknown,'pngPixelsCompared':png_pixels,'originalGridChecked':True})
    result['status']='passed'
    (root / 'independent-verification.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8')
    print(json.dumps(result))


if __name__ == '__main__':
    main()
