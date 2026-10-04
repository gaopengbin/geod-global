"""Independent acceptance of explicitly chosen public STAC/COG sources.

Contacts only an explicitly chosen loopback native runtime. Public discovery and
transfers occur through that runtime. No built-in public endpoint or retry.
Use --metadata-only for a second view of an already-downloaded asset, or --job-id
and --snapshot-id to inspect existing work without an upstream request.
"""
import argparse
import base64
from datetime import datetime, timezone
import hashlib
import ipaddress
import json
import math
from pathlib import Path
import sys
import time
import urllib.error
import urllib.parse
import urllib.request

import numpy as np
import rasterio


def require(condition, message):
    if not condition:
        raise ValueError(message)


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def save_json(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2, allow_nan=False) + '\n', encoding='utf-8')


def read_json(path):
    return json.loads(path.read_bytes())


def number(value):
    if value is None:
        return None
    if isinstance(value, (int, np.integer)):
        return int(value)
    if isinstance(value, str) and value.lstrip('-').isdigit():
        return int(value)
    value = float(value)
    return value if math.isfinite(value) else ('nan' if math.isnan(value) else ('inf' if value > 0 else '-inf'))


def near(left, right):
    return len(left) == len(right) and all(math.isclose(a, b, rel_tol=1e-11, abs_tol=1e-10) for a, b in zip(left, right))


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        return None


class Runtime:
    def __init__(self, server, output):
        url = urllib.parse.urlsplit(server)
        require(url.scheme == 'http' and not url.username and not url.password and not url.query and not url.fragment and url.path in ('', '/'), 'Use an HTTP loopback runtime root')
        try:
            local = ipaddress.ip_address(url.hostname).is_loopback
        except ValueError:
            local = url.hostname == 'localhost'
        require(local, '--server must be loopback')
        self.server, self.output, self.receipts = server.rstrip('/'), output, []
        self.opener = urllib.request.build_opener(urllib.request.ProxyHandler({}), NoRedirect())

    def call(self, path, body=None):
        encoded = None if body is None else json.dumps(body, ensure_ascii=False).encode('utf-8')
        request = urllib.request.Request(self.server + path, data=encoded,
            headers={'Content-Type': 'application/json', 'Accept': 'application/json', 'X-GeoD-Client': 'geod-global'})
        label = f'{len(self.receipts) + 1:03d}'
        entry = {'path': path, 'method': 'GET' if body is None else 'POST', 'body': body,
                 'startedAt': datetime.now(timezone.utc).isoformat(), 'responseFile': f'http-{label}.json'}
        self.receipts.append(entry)
        try:
            with self.opener.open(request, timeout=180) as response:
                entry.update(status=response.status, responseHeaders=dict(response.headers))
                raw = response.read(24 * 1024 * 1024 + 1)
            require(len(raw) <= 24 * 1024 * 1024, 'Runtime JSON response exceeded QA cap')
        except urllib.error.HTTPError as error:
            raw = error.read(65536)
            entry.update(status=error.code, responseHeaders=dict(error.headers))
            (self.output / entry['responseFile']).write_bytes(raw)
            entry.update(bytes=len(raw), sha256=sha(raw))
            save_json(self.output / 'http-receipts.json', self.receipts)
            raise RuntimeError(f'Runtime HTTP {error.code}: {raw.decode("utf-8", errors="replace")}') from error
        (self.output / entry['responseFile']).write_bytes(raw)
        entry.update(bytes=len(raw), sha256=sha(raw))
        save_json(self.output / 'http-receipts.json', self.receipts)
        return json.loads(raw)


def retain_metadata(root, snapshot, output):
    """Verify independently against exact managed metadata, never reconstructed JSON."""
    directory = output / 'metadata'
    directory.mkdir(exist_ok=True)
    ident = snapshot['id']
    path = root / 'stac' / f'snapshot-{ident}.json'
    record_raw = path.read_bytes()
    require(sha(record_raw) == ident, 'Snapshot ID is not the exact record SHA-256')
    record = json.loads(record_raw)
    digest = record['documentSha256']
    document_raw = (root / 'stac' / f'document-{digest}.json').read_bytes()
    require(sha(document_raw) == digest == snapshot['documentSha256'], 'Original metadata hash differs')
    require(record['connectionId'] == snapshot['connectionId'] and record['retrievedAt'] == snapshot['retrievedAt'], 'Snapshot provenance differs')
    datetime.fromisoformat(record['retrievedAt'].replace('Z', '+00:00'))
    doc = json.loads(document_raw)
    if record['kind'] == 'raster':
        require(doc['kind'] == 'direct-raster' and doc['href'] == record['documentUrl'], 'Direct raster probe provenance differs')
        require(doc['cogLayout'] == 'not-validated', 'Probe must not claim full COG validation')
        require(snapshot['assets'][0]['href'] == doc['href'], 'Raster URL changed')
        original = doc
    else:
        original = doc['features'][record['itemIndex']] if record['itemIndex'] is not None else doc
        require(snapshot['itemId'] == original['id'] and snapshot['collectionId'] == original.get('collection'), 'Item/collection identity changed')
        require(snapshot['geometry'] == original['geometry'] and snapshot['bbox'] == original.get('bbox'), 'Source footprint changed')
        require(snapshot['properties'] == original['properties'], 'Source properties lost or changed')
        require(len(snapshot['assets']) == len(original['assets']), 'Source asset metadata was dropped')
        for asset in snapshot['assets']:
            source = original['assets'][asset['key']]
            require(asset['metadata'] == source and asset['roles'] == source.get('roles', []), 'Asset fields or roles changed')
            require(asset['href'] == urllib.parse.urljoin(record['documentUrl'], source['href']), 'Resolved asset URL differs')
        for field in ('datetime', 'start_datetime', 'end_datetime'):
            camel = {'start_datetime': 'startDatetime', 'end_datetime': 'endDatetime'}.get(field, field)
            require(snapshot.get(camel) == original['properties'].get(field), 'Source time was fabricated or changed')
        if 'temporalStatus' in snapshot:
            expected = 'instant' if snapshot.get('datetime') else ('interval' if snapshot.get('startDatetime') and snapshot.get('endDatetime') else 'missing')
            require(snapshot['temporalStatus'] == expected, 'Source temporal compatibility status differs')
    (directory / path.name).write_bytes(record_raw)
    document_name = f'document-{digest}.json'
    (directory / document_name).write_bytes(document_raw)
    save_json(directory / f'item-{ident}.json', original)
    return {'snapshotId': ident, 'recordFile': f'metadata/{path.name}', 'recordBytes': len(record_raw),
            'documentFile': f'metadata/{document_name}', 'documentBytes': len(document_raw), 'documentSha256': digest,
            'documentUrl': record['documentUrl'], 'kind': record['kind'], 'itemId': snapshot['itemId'],
            'collectionId': snapshot.get('collectionId'), 'originalFieldsPreserved': True}


def raster_reference(path):
    """Read every native-resolution band/sample with independent GDAL/rasterio."""
    with rasterio.Env(GDAL_DISABLE_READDIR_ON_OPEN='EMPTY_DIR', GDAL_PAM_ENABLED='NO'):
        with rasterio.open(path) as ds:
            require(ds.width * ds.height * sum(np.dtype(t).itemsize for t in ds.dtypes) <= 128 * 1024 * 1024, 'QA decoded raster cap exceeded')
            bands, samples = [], []
            positions = {(0, 0), (ds.width - 1, ds.height - 1), (ds.width // 2, ds.height // 2),
                         (ds.width // 4, ds.height // 4), (3 * ds.width // 4, 3 * ds.height // 4)}
            arrays, masks = [], []
            for index in ds.indexes:
                array = ds.read(index)
                mask = ds.read_masks(index)
                arrays.append(array)
                masks.append(mask)
                valid = (mask != 0) & np.isfinite(array)
                valid_values = array[valid]
                for choose in (valid, ~valid):
                    found = np.flatnonzero(choose)
                    if found.size:
                        row, column = divmod(int(found[0]), ds.width)
                        positions.add((column, row))
                dtype = array.dtype.newbyteorder('<')
                bands.append({'index': index, 'dataType': ds.dtypes[index-1], 'nodata': number(ds.nodatavals[index-1]),
                    'samples': int(array.size), 'validSamples': int(valid.sum()), 'maskSha256': sha(mask.tobytes(order='C')),
                    'sampleSha256': sha(array.astype(dtype, copy=False).tobytes(order='C')),
                    'sampleHashEncoding': 'row-major little-endian unscaled native sample dtype',
                    'minimum': number(valid_values.min()) if valid_values.size else None,
                    'maximum': number(valid_values.max()) if valid_values.size else None,
                    'scale': ds.scales[index-1], 'offset': ds.offsets[index-1], 'unit': ds.units[index-1],
                    'colorInterpretation': ds.colorinterp[index-1].name, 'overviews': ds.overviews(index),
                    'blockShape': list(ds.block_shapes[index-1])})
            for column, row in sorted(positions):
                samples.append({'column': column, 'row': row,
                    'values': [number(array[row, column]) for array in arrays],
                    'noData': [bool(mask[row, column] == 0) for mask in masks]})
            return {'width': ds.width, 'height': ds.height, 'bandCount': ds.count,
                'crs': ds.crs.to_string() if ds.crs else None, 'crsWkt': ds.crs.to_wkt() if ds.crs else None,
                'transform': list(ds.transform)[:6], 'bounds': list(ds.bounds),
                'pixelInterpretation': ds.tags().get('AREA_OR_POINT'), 'bands': bands, 'samples': samples,
                'tiled': ds.is_tiled, 'compression': str(ds.compression), 'imageStructure': ds.tags(ns='IMAGE_STRUCTURE'),
                'method': 'rasterio/GDAL full native-resolution decode; independent of native Rust TIFF implementation',
                'rasterioVersion': rasterio.__version__, 'gdalVersion': rasterio.__gdal_version__,
                'cogConformanceClaimed': False}


def verify_inspection(native, reference, job):
    require(native['jobId'] == job['id'] and native['sha256'] == job['sha256'], 'Inspection points at another original')
    require(native['width'] == reference['width'] and native['height'] == reference['height'], 'Native dimensions differ from GDAL')
    require(len(native['bands']) == reference['bandCount'], 'Native band count differs')
    for got, expected in zip(native['bands'], reference['bands']):
        require(got['index'] == expected['index'] and got['dataType'].lower() == expected['dataType'].lower(), 'Native sample type/index differs')
        require(number(got['nodata']) == expected['nodata'], 'Native nodata differs')
    require(native['crs'] == reference['crs'], 'Native CRS differs from GDAL')
    require(native['transform'] is not None and near(native['transform'], reference['transform']), 'Native affine grid differs from GDAL')
    require(native['bounds'] is not None and near(native['bounds'], reference['bounds']), 'Native full raster envelope differs from GDAL')
    got = (native.get('pixelInterpretation') or '').lower().replace('pixelis', '')
    expected = (reference.get('pixelInterpretation') or '').lower()
    require(got == expected, 'Native PixelIsPoint/Area differs')


def verify_pixel(native, reference, job):
    require(native['jobId'] == job['id'] and native['sha256'] == job['sha256'], 'Pixel response source differs')
    require(native['column'] == reference['column'] and native['row'] == reference['row'], 'Pixel position differs')
    require(native['noData'] == reference['noData'] and len(native['values']) == len(reference['values']), 'Native valid-data mask differs')
    for got, expected, no_data in zip(native['values'], reference['values'], reference['noData']):
        if no_data and got is None:
            continue
        require(number(got) == expected, 'Native raw sample differs from independent decode')


def verify_declared_grid(item, asset, reference):
    """Compare declarations when present; never infer missing scientific metadata."""
    values = dict(item.get('properties', {}))
    values.update(asset['metadata'])
    checks = []
    for key, expected in [('proj:shape', [reference['height'], reference['width']]),
                          ('proj:code', reference['crs'])]:
        if key in values and values[key] is not None:
            require(values[key] == expected, f'Original STAC {key} disagrees with decoded TIFF')
            checks.append(key)
    if values.get('proj:epsg') is not None:
        require('EPSG:' + str(values['proj:epsg']) == reference['crs'], 'Original STAC EPSG differs')
        checks.append('proj:epsg')
    if values.get('proj:transform') is not None:
        require(near(values['proj:transform'][:6], reference['transform']), 'Original STAC transform differs')
        checks.append('proj:transform')
    bands = values.get('bands', values.get('raster:bands'))
    if bands:
        require(len(bands) == reference['bandCount'], 'Original STAC band count differs')
        for declared, actual in zip(bands, reference['bands']):
            for key in ('nodata', 'data_type'):
                if key in declared:
                    expected = actual['nodata'] if key == 'nodata' else actual['dataType']
                    got = number(declared[key]) if key == 'nodata' else declared[key].lower()
                    require(got == expected, f'Original STAC band {key} differs')
        checks.append('bands')
    return checks


def bounds(value):
    b = [float(v) for v in value.split(',')]
    require(len(b) == 4 and all(math.isfinite(v) for v in b) and -180 <= b[0] < b[2] <= 180 and -90 <= b[1] < b[3] <= 90, 'Use finite increasing W,S,E,N bounds')
    return b


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--server', required=True)
    parser.add_argument('--data-dir', required=True, type=Path)
    parser.add_argument('--endpoint', required=True)
    parser.add_argument('--kind', required=True, choices=('api', 'item', 'raster'))
    parser.add_argument('--asset-key', required=True)
    parser.add_argument('--bounds', required=True, type=bounds)
    parser.add_argument('--collection')
    parser.add_argument('--datetime')
    parser.add_argument('--item-id')
    parser.add_argument('--page-size', type=int, default=4)
    parser.add_argument('--max-items', type=int, default=10)
    parser.add_argument('--all-pages', action='store_true')
    parser.add_argument('--connection-id')
    parser.add_argument('--snapshot-id')
    parser.add_argument('--job-id')
    parser.add_argument('--metadata-only', action='store_true')
    parser.add_argument('--expected-bytes', type=int, help='Prior bounded HEAD evidence; required before a new download')
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    root, output = args.data_dir.resolve(), args.output.resolve()
    require(root.is_dir() and not output.exists() and not output.is_relative_to(root), 'Use an existing runtime and new evidence directory outside it')
    require(1 <= args.page_size <= 10 and 1 <= args.max_items <= 20, 'QA page/item caps are 10/20')
    require(not args.job_id or args.snapshot_id, 'Existing job inspection requires its exact snapshot ID')
    require(args.metadata_only or args.job_id or args.expected_bytes and 0 < args.expected_bytes <= 32 * 1024 * 1024, 'Choose a bounded source with prior size evidence before downloading')
    output.mkdir(parents=True)
    report = {'status': 'running', 'startedAt': datetime.now(timezone.utc).isoformat(), 'mode': 'Actual native public STAC/COG acquisition; no synthetic sources',
        'dataDirectory': str(root), 'server': args.server, 'endpoint': args.endpoint, 'kind': args.kind,
        'assetKey': args.asset_key, 'bounds': args.bounds, 'metadataOnly': args.metadata_only,
        'automaticRetries': 0, 'desktopWebViewAccepted': False, 'cogConformanceClaimed': False, 'snapshots': [], 'jobs': []}
    runtime = Runtime(args.server, output)
    save_json(output / 'report.json', report)
    try:
        connections = runtime.call('/stac/connections')
        matches = [c for c in connections if c['url'].rstrip('/') == args.endpoint.rstrip('/') and c['kind'] == args.kind and (not args.connection_id or c['id'] == args.connection_id)]
        require(not args.connection_id or len(matches) == 1, 'Requested connection does not match endpoint/kind')
        if matches:
            require(len(matches) == 1, 'Ambiguous connections')
            connection = matches[0]
        else:
            require(not args.snapshot_id, 'Existing snapshot inspection must not connect upstream')
            connection = runtime.call('/stac/connections', {'name': f'STAC QA {args.kind}', 'url': args.endpoint, 'kind': args.kind})
        registry = read_json(root / 'stac-connections.json')
        require(registry['connections'][connection['id']] == connection, 'Runtime and data directory differ')
        save_json(output / 'connection.json', connection)
        for digest in connection['metadataSha256']:
            raw = (root / 'stac' / f'document-{digest}.json').read_bytes()
            require(sha(raw) == digest, 'Discovery metadata checksum differs')
            (output / f'discovery-{digest}.json').write_bytes(raw)
        items = []
        if args.snapshot_id:
            items = [runtime.call('/stac/snapshots/' + args.snapshot_id)]
            require(items[0]['connectionId'] == connection['id'], 'Snapshot belongs to another connection')
            report['discovery'] = 'Reused saved snapshot; no source search'
        elif args.kind == 'api':
            require(args.collection, 'API search requires --collection')
            cursor, seen = None, set()
            page_documents, matched_count = [], None
            while True:
                before_documents = set((root / 'stac').glob('document-*.json'))
                page = runtime.call('/stac/search', {'connectionId': connection['id'], 'collectionId': args.collection, 'bounds': args.bounds,
                    'datetime': args.datetime, 'limit': args.page_size, 'cursor': cursor})
                if page['items']:
                    digest = page['items'][0]['documentSha256']
                    source_path = root / 'stac' / f'document-{digest}.json'
                else:
                    fresh = set((root / 'stac').glob('document-*.json')) - before_documents
                    candidates = [p for p in fresh if read_json(p).get('type') == 'FeatureCollection' and read_json(p).get('features') == []]
                    require(len(candidates) == 1, 'Cannot independently identify the terminal empty source page')
                    source_path = candidates[0]
                    digest = source_path.stem.removeprefix('document-')
                source_raw = source_path.read_bytes()
                require(sha(source_raw) == digest, 'Original search page hash differs')
                source = json.loads(source_raw)
                require([(f.get('collection'), f['id']) for f in source['features']] == [(i['collectionId'], i['itemId']) for i in page['items']], 'Native page omitted/reordered source identities')
                returned = source.get('numberReturned', source.get('context', {}).get('returned'))
                require(returned is None or returned == len(page['items']), 'Original returned count differs')
                declared = source.get('numberMatched', source.get('context', {}).get('matched'))
                if isinstance(declared, int):
                    require(matched_count is None or matched_count == declared, 'Source matching count changed across pages')
                    matched_count = declared
                next_links = [l for l in source.get('links', []) if l.get('rel') == 'next']
                require(page['complete'] == (len(next_links) == 0), 'Native completeness disagrees with original continuation links')
                filename = f'search-page-{len(page_documents) + 1:02d}.json'
                (output / filename).write_bytes(source_raw)
                page_documents.append({'file': filename, 'sha256': digest, 'bytes': len(source_raw), 'returned': len(page['items']), 'matched': declared, 'next': next_links})
                for item in page['items']:
                    identity = (item['collectionId'], item['itemId'])
                    require(identity not in seen, 'Duplicate source identity across search pages')
                    seen.add(identity)
                    items.append(item)
                    require(len(items) <= args.max_items, 'Bounded metadata query exceeded acceptance item cap; no asset requested')
                report['searchComplete'], report['searchLimitReached'] = page['complete'], page['limitReached']
                cursor = page['nextCursor']
                if page['complete'] or not args.all_pages:
                    break
                require(cursor and not page['limitReached'], 'Search stopped before completion')
                require(len(items) <= args.max_items, 'No additional search pages allowed')
            report['searchItems'] = len(items)
            if report['searchComplete'] and matched_count is not None:
                require(matched_count == len(items), 'Complete search differs from original matched count')
            report['searchPages'] = page_documents
        else:
            items = [runtime.call('/stac/snapshots/' + ident) for ident in connection['snapshotIds']]
        require(items, 'Source returned no items')
        for item in items:
            report['snapshots'].append(retain_metadata(root, item, output))
        save_json(output / 'snapshots.json', items)
        selected = [i for i in items if not args.item_id or i['itemId'] == args.item_id]
        require(len(selected) == 1 or args.item_id is None, 'Selected original item is missing or ambiguous')
        item = selected[0]
        assets = [a for a in item['assets'] if a['key'] == args.asset_key]
        require(len(assets) == 1 and assets[0]['eligible'], 'Selected asset is absent or not eligible')
        asset = assets[0]
        pin = {'snapshotId': item['id'], 'assetKey': args.asset_key}
        report.update(selectedSnapshotId=item['id'], selectedItemId=item['itemId'], selectedAssetHref=asset['href'], connectionId=connection['id'])
        project = runtime.call('/stac/project', {'name': f'Public STAC verification {args.kind}', 'bounds': args.bounds, 'selections': [pin]})
        require(len(project['stacItems']) == 1 and project['stacItems'][0]['snapshotId'] == item['id'] and project['stacItems'][0]['assetKey'] == args.asset_key, 'Project did not persist exact selection')
        save_json(output / 'project.json', project)
        report['projectId'] = project['id']
        if not args.metadata_only:
            if args.job_id:
                job = runtime.call('/jobs/' + args.job_id)
            else:
                queued = runtime.call('/stac/downloads', {'projectId': project['id'], 'selections': [pin]})
                require(len(queued['jobs']) == 1, 'Expected exactly one original transfer')
                job = queued['jobs'][0]
                deadline = time.monotonic() + 300
                while job['status'] in ('queued', 'running') or job.get('settled') is False:
                    require(time.monotonic() < deadline, 'Original transfer did not finish within five minutes')
                    time.sleep(1)
                    job = runtime.call('/jobs/' + job['id'])
            require(job['status'] == 'succeeded', 'Original transfer failed: ' + str(job.get('error')))
            job.pop('settled', None)
            require(job['stacSource'] == pin and job['href'] == asset['href'] and job['itemId'] == item['itemId'], 'Job/source identity differs')
            path = Path(job['outputPath'])
            require(path.samefile(root / 'assets' / f"{job['id']}.tif"), 'Output is not the managed original')
            raw = path.read_bytes()
            require(len(raw) == job['bytesDownloaded'] == job['totalBytes'] and sha(raw) == job['sha256'], 'Original file size/hash differs')
            require(args.expected_bytes is None or len(raw) == args.expected_bytes, 'Original differs from candidate HEAD size')
            (output / 'original.tif').write_bytes(raw)
            save_json(output / 'job.json', job)
            reference = raster_reference(path)
            report['verifiedStacRasterDeclarations'] = verify_declared_grid(item, asset, reference)
            save_json(output / 'rasterio-reference.json', reference)
            native = runtime.call('/stac/jobs/' + job['id'] + '/inspect')
            verify_inspection(native, reference, job)
            save_json(output / 'native-inspection.json', native)
            if native.get('previewDataUrl'):
                prefix, encoded = native['previewDataUrl'].split(',', 1)
                require(prefix == 'data:image/png;base64', 'Unexpected preview encoding')
                (output / 'native-preview.png').write_bytes(base64.b64decode(encoded, validate=True))
            native_pixels = []
            for sample in reference['samples']:
                pixel = runtime.call(f"/stac/jobs/{job['id']}/pixel?column={sample['column']}&row={sample['row']}")
                verify_pixel(pixel, sample, job)
                native_pixels.append(pixel)
            save_json(output / 'native-pixels.json', native_pixels)
            require(runtime.call('/stac/snapshots/' + item['id']) == item, 'Saved source changed after transfer')
            report['jobs'].append({'jobId': job['id'], 'bytes': len(raw), 'sha256': sha(raw), 'originalFile': 'original.tif',
                'managedPath': str(path), 'sampleCount': sum(b['samples'] for b in reference['bands']),
                'pixelProbeCount': len(native_pixels), 'fullIndependentDecode': True,
                'width': reference['width'], 'height': reference['height'], 'crs': reference['crs']})
        report['status'] = 'passed'
    except Exception as error:
        report.update(status='failed', error=f'{type(error).__name__}: {error}')
        raise
    finally:
        report['finishedAt'] = datetime.now(timezone.utc).isoformat()
        report['localHttpRequests'] = len(runtime.receipts)
        save_json(output / 'report.json', report)
    print(json.dumps({'status': report['status'], 'report': str(output / 'report.json'), 'snapshots': len(report['snapshots']), 'jobs': report['jobs']}, ensure_ascii=False))


if __name__ == '__main__':
    try:
        main()
    except Exception as error:
        print(str(error), file=sys.stderr)
        sys.exit(1)
