"""Retain a bounded receipt for actual public TMS acquisition and offline UI."""
import argparse
import hashlib
import io
import json
from pathlib import Path
import xml.etree.ElementTree as ET
import zipfile

from PIL import Image


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('root', type=Path)
args = parser.parse_args()
root = args.root.resolve()
assert root.parent == Path('.verification').resolve() and root.name.startswith('tms-public-')
reports = []


def read(relative):
    raw = (root / relative).read_bytes()
    reports.append({'file': relative, 'sha256': sha(raw)})
    return json.loads(raw)


native, ui = read('evidence/report.json'), read('ui/ui-verification.json')
assert native['status'] == ui['status'] == 'passed'
assert native['nativeBinarySha256'] == ui['nativeBinarySha256']
assert not native['syntheticData'] and not native['usedUserDesktop'] and not ui['usedUserDesktop']
assert not native['automaticCapabilitiesDiscovery'] and not native['scientificOriginalClaimed']
assert ui['readOnly'] and not ui['nativeWindowTested'] and ui['errors'] == ui['remoteRequests'] == []
assert native['publicTmsAcquisitionVerified'] and native['restart']['actualProcessRestart']
assert native['restart']['rejectingProxyAttempts'] == []
xml = (root / 'evidence/tilemap.xml').read_bytes()
assert sha(xml) == native['declaredGrid']['sha256'] and len(xml) == native['declaredGrid']['bytes']
declaration = ET.fromstring(xml)
assert declaration.findtext('SRS') == native['declaredGrid']['srs'] == 'EPSG:3857'
assert declaration.find('Origin').attrib == native['declaredGrid']['origin']
assert declaration.find('BoundingBox').attrib == native['declaredGrid']['boundingBox']
assert declaration.find('TileFormat').attrib == native['declaredGrid']['format']
levels = {int(item.attrib['order']): item.attrib for item in declaration.findall('TileSets/TileSet')}
assert levels == {int(key): value for key, value in native['declaredGrid']['levels'].items()}
assert len(native['cases']) == len(native['restart']['cases']) == 2
images = []
for index, case in enumerate(native['cases']):
    folder = root / 'evidence' / ('case-' + str(index))
    asset = read('evidence/case-' + str(index) + '/asset.json')
    assert asset['id'] == case['imageId'] and asset['crs'] == 'EPSG:3857'
    assert asset['sha256'] == case['sha256'] and asset['imageExtent'] == case['imageExtent']
    assert asset['width'] == case['width'] and asset['height'] == case['height']
    assert case['pixels'] == case['width'] * case['height']
    assert case['allRgbaChannelsIdentical'] and case['independentGdalCrsGridAndPixels'] and case['packageHashesValid']
    assert any(item['imageId'] == asset['id'] and item['metadataPngAndExportIdentical']
               for item in native['restart']['cases'])
    snap = asset['source']['xyz']
    config = snap['configuration']
    assert config['scheme'] == 'TMS' and config['grid']['tileSize'] == 256 and config['grid']['zoomOffset'] == 0
    assert config['grid']['minZoom'] == min(levels) and config['grid']['maxZoom'] == max(levels)
    assert config['grid']['format'] == 'image/png' and snap['logicalZoom'] == case['zoom']
    assert snap['requestedBounds'] == case['bounds'] and len(snap['tiles']) == len(case['tiles'])
    png, package = (folder / 'map.png').read_bytes(), (folder / 'export.zip').read_bytes()
    assert sha(png) == asset['sha256'] and len(png) == asset['bytes']
    image = Image.open(io.BytesIO(png))
    assert image.size == (case['width'], case['height']) and image.mode == 'RGBA'
    with zipfile.ZipFile(io.BytesIO(package)) as exported:
        assert exported.read('map.png') == png and json.loads(exported.read('source.json')) == asset
        for line in exported.read('checksums.sha256').decode().splitlines():
            checksum, filename = line.split('  ')
            assert sha(exported.read(filename)) == checksum
        archive = exported.read('source-tiles.zip')
        assert sha(archive) == snap['archiveSha256'] and len(archive) == snap['archiveBytes']
        with zipfile.ZipFile(io.BytesIO(archive)) as tiles:
            assert len(tiles.namelist()) == len(snap['tiles'])
            for original, checked in zip(snap['tiles'], case['tiles']):
                assert original['row'] == checked['logicalRow'] and original['col'] == checked['column']
                assert checked['serverBottomRow'] == 2 ** case['zoom'] - 1 - original['row']
                assert original['requestUrl'] == levels[case['zoom']]['href'] + f"/{original['col']}/{checked['serverBottomRow']}.png"
                raw = tiles.read(f"{original['row']}-{original['col']}.png")
                assert len(raw) == original['bytes'] == checked['bytes']
                assert sha(raw) == original['sha256'] == checked['sha256'] and checked['httpPayloadIdentical']
    images.append({**case, 'pngBytes': len(png), 'exportBytes': len(package), 'exportSha256': sha(package),
                   'originalTileArchiveBytes': len(archive), 'originalTileArchiveSha256': sha(archive)})
assert native['pixelsCompared'] == sum(item['pixels'] for item in images)
assert native['tilesCompared'] == sum(len(item['tiles']) for item in images)
assert {(item['width'], item['locale'], item['theme']) for item in ui['cases']} == {
    (1440, 'en', 'light'), (1024, 'zh-CN', 'dark')}
for case in ui['cases']:
    assert case['actualLocalThumbnails'] == len(images) and case['officialEncodedTemplateAccepted']
    assert case['configuredGridVisible'] and case['noHorizontalOverflow'] and case['verifiedTmsPresetOnlyFillsDraft']
    assert {item['imageId']: item['sha256'] for item in case['workspaceImages']} == {
        item['imageId']: item['sha256'] for item in images}
    assert all(item['paintedRaster'] and item['sourceBottomOriginVisible'] for item in case['workspaceImages'])
    assert all(call['method'] == 'GET' for call in case['calls'])
assert ui['frontendFiles'] and all(len(item['sha256']) == 64 and '..' not in item['file'] for item in ui['frontendFiles'])
receipt = {
    'schema': 'geod-tms-public-receipt/v1', 'status': 'passed', 'checkedAt': native['finishedAt'],
    'scope': 'Two bounded actual public DLR TMS rendered map images; bottom-origin requests, all pixels, native grid, export and offline recovery.',
    'nativeBinarySha256': native['nativeBinarySha256'], 'evidenceReports': reports,
    'tileMapUrl': native['tileMapUrl'], 'attribution': native['attribution'], 'sourcePolicyUrl': native['sourcePolicyUrl'],
    'declaredGrid': native['declaredGrid'], 'automaticCapabilitiesDiscovery': False, 'scientificOriginalClaimed': False,
    'images': images, 'independent': {'publicHttpTilePayloadsIdentical': True, 'originalTilesCompared': native['tilesCompared'],
                                    'rgbaPixelsCompared': native['pixelsCompared'], 'nativeGridAndGdalPixelsCompared': True},
    'restart': native['restart'],
    'desktopFrontend': {key: ui[key] for key in ['renderer', 'readOnly', 'nativeWindowTested', 'cases', 'frontendFiles', 'errors', 'remoteRequests']},
    'usedUserDesktop': False, 'syntheticData': False,
    'limitations': ['Rendered basemap colours are not scientific original bands.',
                    'TileMap XML was independently read for acceptance; the product still uses an explicit template and grid.',
                    'Only a global EPSG:3857 grid, 256-pixel PNG and zero zoom offset were acquired from this provider.',
                    '512-pixel tiles and nonzero zoom offsets retain separate synthetic protocol tests.',
                    'No installed WebView manual acceptance or UI remote acquisition was performed in these read-only UI cases.']}
destination = Path('prototype/qa/tms-public-verification.json')
destination.write_text(json.dumps(receipt, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
print(json.dumps({'status': receipt['status'], 'tiles': native['tilesCompared'], 'pixels': native['pixelsCompared'], 'uiCases': len(ui['cases'])}))
