"""Fetch seven small public thumbnails for this fixed design fixture, not bulk data."""
import json, hashlib, urllib.request, datetime
from pathlib import Path
from concurrent.futures import ThreadPoolExecutor
root = Path(__file__).parent / 'public' / 'samples'
source = json.loads((root / 'earth-search-response.json').read_text(encoding='utf-8'))
def fetch(feature):
    url = feature['assets']['thumbnail']['href']
    assert url.startswith('https://sentinel-cogs.s3.us-west-2.amazonaws.com/')
    filename = feature['id'] + '.jpg'
    data = urllib.request.urlopen(url, timeout=45).read()
    (root / filename).write_bytes(data)
    return dict(id=feature['id'], date=feature['properties']['datetime'], cloud=feature['properties']['eo:cloud_cover'],
        thumbnail='samples/' + filename, source=url, sha256=hashlib.sha256(data).hexdigest(), bbox=feature['bbox'],
        properties=feature['properties'], assets=feature['assets'], geometry=feature['geometry'])
with ThreadPoolExecutor(max_workers=3) as pool:
    scenes = list(pool.map(fetch, source['features']))
manifest = dict(retrievedAt=datetime.datetime.now(datetime.timezone.utc).isoformat(),
    query='https://earth-search.aws.element84.com/v1/search?collections=sentinel-2-l2a&bbox=-122.55,37.68,-122.32,37.84&datetime=2025-06-01T00:00:00Z/2025-06-30T23:59:59Z&limit=8',
    attribution='Contains Copernicus Sentinel data (2025). Earth Search / Element 84.',
    registry='https://registry.opendata.aws/sentinel-2-l2a-cogs/',
    boundary='Fixed catalog snapshot; scene-wide cloud cover; thumbnail preview only. No analytical raster processing.', scenes=scenes)
(root / 'manifest.json').write_text(json.dumps(manifest, indent=2), encoding='utf-8')
print(f'Saved {len(scenes)} real catalog scenes and thumbnails.')
