"""Retain a bounded receipt for an actual fresh WCS acquisition through MCP."""
import argparse
import hashlib
import json
from pathlib import Path


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('root', type=Path)
args = parser.parse_args()
root = args.root.resolve()
assert root.parent == Path('.verification').resolve() and root.name.startswith('wcs-mcp-public-')
evidence = root / 'evidence'
reports = []


def read(name):
    raw = (evidence / name).read_bytes()
    reports.append({'file': 'evidence/' + name, 'sha256': sha(raw)})
    return json.loads(raw)


report, offline = read('report.json'), read('offline-mcp-verification.json')
job, project, plan = read('job.json'), read('project.json'), read('plan.json')
reference, points = read('rasterio-reference.json'), read('mcp-pixels.json')
summary, pages = read('mcp-connection-summary.json'), read('mcp-catalog-pages.json')
assert report['status'] == offline['status'] == 'passed'
assert report['nativeBinarySha256'] == offline['binarySha256']
assert report['freshStoreBeforeConnect'] and not report['syntheticData'] and not report['usedUserDesktop']
assert job['id'] == report['jobId'] == offline['jobId'] and job['status'] == 'succeeded' and job['settled']
assert project['id'] == report['projectId'] == offline['projectId']
assert plan['id'] == report['planId'] == offline['planId'] == job['wcsSource']['planId']
assert any(entry['planId'] == plan['id'] for entry in project['wcsItems'])
assert report['sha256'] == job['sha256'] and report['bytes'] == job['bytesDownloaded'] == job['totalBytes']
native, independent = (evidence / 'native-subset.tif').read_bytes(), (evidence / 'independent-subset.tif').read_bytes()
assert native == independent and sha(native) == job['sha256'] and len(native) == report['bytes']
assert reference['width'] == report['width'] == plan['width']
assert reference['height'] == report['height'] == plan['height']
assert reference['crs'] == report['crs'] and reference['bands'] == report['bands']
assert sum(band['samples'] for band in reference['bands']) == report['independentSamplesCompared']
assert len(points) == report['mcpPixelChecks']
assert len(pages) == report['catalogPages']
assert sum(len(page['coverages']) for page in pages) == summary['coverageCount'] == report['coveragesCompared']
assert len(offline['sessions']) == report['offlineSessions'] == 5
assert len(offline['independentPixelChecks']) == report['offlinePixelChecks'] == 25
assert offline['registryBytesUnchanged'] and offline['blockedProxyAttempts'] == report['offlineBlockedProxyRequests'] == []
for item in report['metadata']:
    raw = (evidence / item['file']).read_bytes()
    assert sha(raw) == item['sha256'] and len(raw) == item['bytes']
receipt = {
    'schema': 'geod-wcs-mcp-public-receipt/v1', 'status': 'passed', 'checkedAt': report['finishedAt'],
    'scope': 'Fresh bounded public WCS subset acquired through actual loopback stdio MCP, then recovered in standalone and loopback offline sessions.',
    'nativeBinarySha256': report['nativeBinarySha256'], 'evidenceReports': reports,
    'endpoint': report['endpoint'], 'coverageId': report['coverageId'],
    'requestedBounds': report['requestedBounds'], 'jobId': job['id'], 'projectId': project['id'], 'planId': plan['id'],
    'freshStoreBeforeConnect': True, 'publicMetadataAndGetCoverageViaMcp': True, 'catalogPages': len(pages),
    'coveragesComparedToSourceXml': summary['coverageCount'], 'metadata': report['metadata'],
    'output': {'kind': 'server-generated coverage subset', 'bytes': len(native), 'sha256': job['sha256'],
               'width': reference['width'], 'height': reference['height'], 'crs': reference['crs'], 'bands': reference['bands']},
    'independent': {'publicHttpPayloadIdentical': True, 'samplesCompared': report['independentSamplesCompared'],
                    'gridComparedToOriginalXml': True, 'onlineMcpPixelsCompared': len(points)},
    'offline': {'sessions': offline['sessions'], 'pixelChecks': len(offline['independentPixelChecks']),
                'registryBytesUnchanged': True, 'blockedProxyAttempts': [], 'checks': offline['checks']},
    'declaredFields': report['declaredFields'], 'automaticRetries': report['automaticRetries'],
    'usedUserDesktop': False, 'nativeWindowTested': False, 'syntheticData': False,
    'limitations': ['This is a coverage subset, not an original survey archive.',
                    'The Depth field declares W.m-2.Sr-1; the TIFF has no unit tag. No guessed unit or scientific calibration.',
                    'Linked source license metadata was not fetched or verified.',
                    'No installed WebView manual acceptance or other WCS versions, dimensions, formats or authentication.']}
destination = Path('prototype/qa/wcs-mcp-public-verification.json')
destination.write_text(json.dumps(receipt, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
print(json.dumps({'status': receipt['status'], 'samples': receipt['independent']['samplesCompared'],
                  'offlineSessions': len(receipt['offline']['sessions'])}))
