"""Verify a few actual public TMS tiles, their bottom-origin grid, and offline recovery.

TileMap XML is independently retained for acceptance; this does not claim that
the product discovers XML automatically. Never acquire a whole tile pyramid.
"""
import argparse
import base64
from datetime import datetime, timezone
import hashlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import importlib.util
import io
import json
import math
import os
from pathlib import Path
import socket
import subprocess
import threading
import time
from urllib.request import build_opener, ProxyHandler, Request
import urllib.error
import xml.etree.ElementTree as ET
import zipfile

import numpy as np
from PIL import Image
import rasterio


spec = importlib.util.spec_from_file_location('xyz_reference', Path(__file__).with_name('verify-xyz-public.py'))
xyz = importlib.util.module_from_spec(spec)
spec.loader.exec_module(xyz)


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', required=True, type=Path)
    parser.add_argument('--executable', required=True, type=Path)
    parser.add_argument('--tilemap', required=True)
    parser.add_argument('--attribution', required=True)
    parser.add_argument('--policy', required=True)
    parser.add_argument('--case', required=True, action='append', help='zoom,west,south,east,north')
    parser.add_argument('--port', type=int, default=4614)
    args = parser.parse_args()
    root, binary = args.root.resolve(), args.executable.resolve(strict=True)
    assert root.parent == Path('.verification').resolve() and root.name.startswith('tms-public-') and not root.exists()
    assert 1 <= args.port <= 65535 and 1 <= len(args.case) <= 2
    with socket.socket() as probe:
        assert probe.connect_ex(('127.0.0.1', args.port)) != 0
    store, evidence = root / 'store', root / 'evidence'
    store.mkdir(parents=True)
    evidence.mkdir()
    (store / 'proxy-settings.json').write_text(json.dumps({'mode': 'system'}), encoding='utf-8')
    report = {'schema': 'geod-tms-public-verification/v1', 'status': 'running',
              'startedAt': datetime.now(timezone.utc).isoformat(), 'nativeBinarySha256': sha(binary.read_bytes()),
              'tileMapUrl': args.tilemap, 'attribution': args.attribution, 'sourcePolicyUrl': args.policy,
              'usedUserDesktop': False, 'syntheticData': False, 'automaticCapabilitiesDiscovery': False,
              'scientificOriginalClaimed': False, 'cases': []}
    report_file = evidence / 'report.json'

    def save():
        report_file.write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')

    local, public = build_opener(ProxyHandler({})), build_opener()
    base = f'http://127.0.0.1:{args.port}'

    def api(route, payload=None):
        body = json.dumps(payload).encode() if payload is not None else None
        headers = {'Content-Type': 'application/json', 'X-GeoD-Client': 'geod-global'}
        try:
            with local.open(Request(base + route, data=body, headers=headers), timeout=140) as response:
                return response.read()
        except urllib.error.HTTPError as error:
            error.add_note(error.read(4096).decode('utf-8', errors='replace'))
            raise

    def api_json(route, payload=None):
        return json.loads(api(route, payload))

    def owner():
        value = api_json('/health')['storageRoot']
        if os.name == 'nt' and value.startswith('\\\\?\\'):
            value = value[4:]
        assert Path(value).samefile(store)

    server, pending, trap = None, False, None
    stdout, stderr = (root / 'runtime.stdout.log').open('wb'), (root / 'runtime.stderr.log').open('wb')

    def start():
        child = subprocess.Popen([str(binary), 'serve', '--data-dir', str(store), '--port', str(args.port)],
                                 stdout=stdout, stderr=stderr, creationflags=getattr(subprocess, 'CREATE_NO_WINDOW', 0))
        for i in range(100):
            assert child.poll() is None, 'Private runtime exited before readiness'
            try:
                owner()
                return child
            except OSError:
                if i == 99:
                    child.terminate()
                    child.wait(timeout=10)
                    raise
                time.sleep(.1)

    try:
        save()
        with public.open(Request(args.tilemap, headers={'User-Agent': 'GeoD-Global-TMS-Verification/1'}), timeout=60) as response:
            xml = response.read(256 * 1024 + 1)
            assert response.status == 200 and response.geturl() == args.tilemap and len(xml) <= 256 * 1024
        (evidence / 'tilemap.xml').write_bytes(xml)
        declaration = ET.fromstring(xml)
        assert declaration.tag == 'TileMap' and declaration.attrib['version'] == '1.0.0'
        assert declaration.findtext('SRS') in ('EPSG:3857', 'EPSG:900913')
        half = math.pi * 6378137
        bbox = declaration.find('BoundingBox').attrib
        origin = declaration.find('Origin').attrib
        # GeoWebCache may round the published world bounds by millimetres.
        assert np.allclose([float(bbox[k]) for k in ['minx', 'miny', 'maxx', 'maxy']], [-half, -half, half, half], rtol=0, atol=.004)
        assert np.allclose([float(origin[k]) for k in ['x', 'y']], [-half, -half], rtol=0, atol=.004)
        format_ = declaration.find('TileFormat').attrib
        assert format_ == {'width': '256', 'height': '256', 'mime-type': 'image/png', 'extension': 'png'}
        levels = {int(level.attrib['order']): level.attrib for level in declaration.findall('TileSets/TileSet')}
        assert levels and min(levels) == 0 and set(levels) == set(range(max(levels) + 1))
        for z, level in levels.items():
            assert math.isclose(float(level['units-per-pixel']), 2 * half / (256 * 2 ** z), rel_tol=1e-9)
            assert level['href'] == args.tilemap.rstrip('/') + '/' + str(z)
        template = args.tilemap.rstrip('/') + '/{z}/{x}/{y}.png'
        report['declaredGrid'] = {'sha256': sha(xml), 'bytes': len(xml), 'srs': declaration.findtext('SRS'),
                                  'origin': origin, 'boundingBox': bbox, 'levels': levels, 'format': format_}
        server = start()
        assert api_json('/map-images') == [] and api_json('/map-services') == [] and api_json('/jobs') == []
        config = {'tileSize': 256, 'minZoom': 0, 'maxZoom': max(levels), 'zoomOffset': 0, 'format': 'image/png',
                  'attribution': args.attribution, 'accessConstraints': 'Rendered basemap; source use policy: ' + args.policy}
        service = api_json('/map-services', {'name': declaration.findtext('Title'), 'url': template, 'protocol': 'TMS', 'tileConfig': config})
        for i, raw_case in enumerate(args.case):
            values = [float(value) for value in raw_case.split(',')]
            assert len(values) == 5 and int(values[0]) == values[0]
            z, bounds = int(values[0]), values[1:]
            assert z in levels and -180 <= bounds[0] < bounds[2] <= 180 and -85 <= bounds[1] < bounds[3] <= 85
            window = xyz.plan(bounds, 256, z)
            assert 0 < window[2] <= 1024 and 0 < window[3] <= 1024
            request = {'serviceId': service['id'], 'layerName': 'tiles', 'style': '', 'time': None, 'bounds': bounds,
                       'width': window[2], 'height': window[3], 'tileMatrixSet': 'WebMercator', 'tileMatrix': str(z), 'areaGeometry': None}
            pending = True
            try:
                asset = api_json('/map-images', request)
            except urllib.error.HTTPError:
                pending = False  # An explicit native response, not an observer timeout.
                raise
            pending = False
            snap = asset['source']['xyz']
            assert snap['pixelWindow'] == window and snap['configuration'] == {'scheme': 'TMS', 'urlTemplate': template, 'grid': config}
            assert 1 <= len(snap['tiles']) <= 8
            view = api_json('/map-images/' + asset['id'])
            assert view['asset'] == asset
            png = base64.b64decode(view['imageUrl'].split(',', 1)[1])
            assert sha(png) == asset['sha256'] and len(png) == asset['bytes']
            actual = np.array(Image.open(io.BytesIO(png)).convert('RGBA'))
            assert actual.shape == (window[3], window[2], 4)
            expected = np.zeros_like(actual)
            package = api('/map-images/' + asset['id'] + '/export')
            folder = evidence / ('case-' + str(i))
            folder.mkdir()
            (folder / 'map.png').write_bytes(png)
            (folder / 'export.zip').write_bytes(package)
            (folder / 'asset.json').write_text(json.dumps(asset, ensure_ascii=False, indent=2), encoding='utf-8')
            tile_checks = []
            with zipfile.ZipFile(io.BytesIO(package)) as exported:
                assert exported.read('map.png') == png and json.loads(exported.read('source.json')) == asset
                for line in exported.read('checksums.sha256').decode().splitlines():
                    checksum, filename = line.split('  ')
                    assert sha(exported.read(filename)) == checksum
                archive = exported.read('source-tiles.zip')
                assert sha(archive) == snap['archiveSha256'] and len(archive) == snap['archiveBytes']
                with zipfile.ZipFile(io.BytesIO(archive)) as tiles:
                    assert len(tiles.namelist()) == len(snap['tiles'])
                    for receipt in snap['tiles']:
                        row, col = receipt['row'], receipt['col']
                        resolution = 2 * half / (256 * 2 ** z)
                        centre_y = half - (row + .5) * 256 * resolution
                        # Derive server row from the independently declared bottom
                        # origin and resolution, not the product's row-flip helper.
                        bottom_row = math.floor((centre_y - float(origin['y'])) / (256 * float(levels[z]['units-per-pixel'])))
                        assert bottom_row == 2 ** z - 1 - row and bottom_row != row
                        url = levels[z]['href'] + f'/{col}/{bottom_row}.png'
                        assert url == receipt['requestUrl']
                        original = tiles.read(f'{row}-{col}.png')
                        with public.open(Request(url, headers={'User-Agent': 'GeoD-Global-TMS-Verification/1'}), timeout=60) as response:
                            independently_fetched = response.read(4 * 1024 * 1024 + 1)
                            assert response.status == 200 and response.geturl() == url
                        assert independently_fetched == original and sha(original) == receipt['sha256'] and len(original) == receipt['bytes']
                        rgba = np.array(Image.open(io.BytesIO(original)).convert('RGBA'))
                        assert rgba.shape == (256, 256, 4)
                        tx, ty = col * 256, row * 256
                        left, right = max(window[0], tx), min(window[0] + window[2], tx + 256)
                        top, bottom = max(window[1], ty), min(window[1] + window[3], ty + 256)
                        expected[top-window[1]:bottom-window[1], left-window[0]:right-window[0]] = rgba[top-ty:bottom-ty, left-tx:right-tx]
                        tile_checks.append({'logicalRow': row, 'column': col, 'serverBottomRow': bottom_row,
                                            'bytes': len(original), 'sha256': receipt['sha256'], 'httpPayloadIdentical': True})
                resolution = 2 * half / (256 * 2 ** z)
                extent = [-half + window[0] * resolution, half - (window[1] + window[3]) * resolution,
                          -half + (window[0] + window[2]) * resolution, half - window[1] * resolution]
                assert np.allclose(asset['imageExtent'], extent, rtol=0, atol=1e-8)
                assert np.allclose([float(value) for value in exported.read('map.pgw').splitlines()],
                                   [resolution, 0, 0, -resolution, extent[0]+resolution/2, extent[3]-resolution/2], rtol=0, atol=1e-8)
                (folder / 'map.pgw').write_bytes(exported.read('map.pgw'))
                (folder / 'map.png.aux.xml').write_bytes(exported.read('map.png.aux.xml'))
            assert np.array_equal(actual, expected) and actual[:, :, 3].max() > 0
            assert len(np.unique(actual[:, :, :3].reshape(-1, 3), axis=0)) > 40
            with rasterio.open(folder / 'map.png') as dataset:
                assert dataset.crs.to_epsg() == 3857 and np.allclose(tuple(dataset.bounds), extent, rtol=0, atol=1e-8)
                assert np.array_equal(np.moveaxis(dataset.read(), 0, -1), actual)
            report['cases'].append({'imageId': asset['id'], 'serviceId': service['id'], 'zoom': z, 'bounds': bounds,
                                     'width': asset['width'], 'height': asset['height'], 'pixels': asset['width'] * asset['height'],
                                     'sha256': asset['sha256'], 'imageExtent': extent, 'tiles': tile_checks,
                                     'allRgbaChannelsIdentical': True, 'independentGdalCrsGridAndPixels': True, 'packageHashesValid': True})
            save()
            print(json.dumps({'stage': 'public-tms', 'case': i, 'tiles': len(tile_checks), 'pixels': asset['width'] * asset['height']}), flush=True)
        assert len(api_json('/map-images')) == len(args.case) and len(api_json('/map-services')) == 1
        server.terminate()
        server.wait(timeout=10)
        server = None
        attempts = []

        class DenyProxy(BaseHTTPRequestHandler):
            def blocked(self):
                attempts.append({'method': self.command, 'target': self.path})
                self.send_error(502)
            do_CONNECT = do_GET = do_POST = blocked
            def log_message(self, *_):
                pass

        trap = ThreadingHTTPServer(('127.0.0.1', 0), DenyProxy)
        threading.Thread(target=trap.serve_forever, daemon=True).start()
        (store / 'proxy-settings.json').write_text(json.dumps({'mode': 'custom', 'url': f'http://127.0.0.1:{trap.server_port}'}), encoding='utf-8')
        server = start()
        assert api_json('/map-services') == [service]
        restored = []
        for i, case in enumerate(report['cases']):
            folder = evidence / ('case-' + str(i))
            inspection = api_json('/map-images/' + case['imageId'])
            assert inspection['asset'] == json.loads((folder / 'asset.json').read_text(encoding='utf-8'))
            assert base64.b64decode(inspection['imageUrl'].split(',', 1)[1]) == (folder / 'map.png').read_bytes()
            assert api('/map-images/' + case['imageId'] + '/export') == (folder / 'export.zip').read_bytes()
            restored.append({'imageId': case['imageId'], 'metadataPngAndExportIdentical': True})
        assert attempts == []
        report.update(status='passed', publicTmsAcquisitionVerified=True,
                      pixelsCompared=sum(case['pixels'] for case in report['cases']),
                      tilesCompared=sum(len(case['tiles']) for case in report['cases']),
                      restart={'actualProcessRestart': True, 'rejectingProxyAttempts': attempts, 'cases': restored})
    except Exception as error:
        report.update(status='failed', error=f'{type(error).__name__}: {error}')
        raise
    finally:
        if server is not None and server.poll() is None:
            if not pending:
                owner()
                server.terminate()
                server.wait(timeout=10)
            else:
                report['ownerRetainedForUnknownAcquisition'] = server.pid
        if trap is not None:
            trap.shutdown()
            trap.server_close()
        stdout.close()
        stderr.close()
        report['finishedAt'] = datetime.now(timezone.utc).isoformat()
        save()
    print(json.dumps({'status': report['status'], 'tiles': report['tilesCompared'], 'pixels': report['pixelsCompared']}))


if __name__ == '__main__':
    main()
