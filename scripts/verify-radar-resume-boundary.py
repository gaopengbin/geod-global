"""Check a live radar retry against its checkpoint and an independent byte Range.

This only reads the active store and a bounded official public response. It
never stops, retries or edits the acquisition job, and does not accept a whole
original. Temporary public SAS authorization stays in memory.
"""
import argparse
import hashlib
import json
import uuid
import urllib.request
from datetime import datetime, timezone
from pathlib import Path
from urllib.parse import urlsplit, urlunsplit


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, request, response, code, message, headers, url):
        return None


parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('root', type=Path)
parser.add_argument('--port', type=int, default=4605)
args = parser.parse_args()
root = args.root.resolve()
assert root.parent == Path('.verification').resolve() and root.name.startswith('radar-polarizations-')
assert 1 <= args.port <= 65535
local = urllib.request.build_opener(urllib.request.ProxyHandler({}), NoRedirect())


def read_json(opener, address, headers=None):
    try:
        with opener.open(urllib.request.Request(address, headers=headers or {}), timeout=30) as response:
            raw = response.read(4 * 1024 * 1024 + 1)
            assert len(raw) <= 4 * 1024 * 1024
            return json.loads(raw)
    except Exception as error:
        code = getattr(error, 'code', None)
        suffix = ' status=' + str(code) if isinstance(code, int) else ''
        raise RuntimeError('Bounded JSON request failed: ' + type(error).__name__ + suffix) from None


base = f'http://127.0.0.1:{args.port}'
health = read_json(local, base + '/health')
storage = health['storageRoot'].removeprefix('\\\\?\\')
assert Path(storage).resolve() == root
queued_raw = (root / 'hv-retry-queued-verification.json').read_bytes()
live_raw = (root / 'hv-retry-live-verification.json').read_bytes()
queued, live = json.loads(queued_raw), json.loads(live_raw)
job_id = str(uuid.UUID(queued['jobId']))
assert live['jobId'] == job_id and queued['key'] == live['key'] == 'hv'
assert queued['sameJobId'] and live['sameJobId'] and not queued['originalAccepted'] and not live['originalAccepted']
binary_sha = hashlib.sha256(Path('target/debug/geod-runtime.exe').read_bytes()).hexdigest()
assert binary_sha == queued['nativeBinarySha256']
job = read_json(local, base + '/jobs/' + job_id)
assert job['id'] == job_id and job['kind'] == 'download' and job['assetKey'] == 'hv'
assert job['status'] == 'running' and not job['settled']
assert job['transfer'] == {'mode': 'resumed', 'resumedBytes': live['resumedBytes']}
prefix = live['resumedBytes']
assert prefix == queued['previousReportedBytes'] and 32768 <= prefix < job['bytesDownloaded'] - 32768
checkpoint_raw = (root / 'assets' / (job_id + '.part.resume.json')).read_bytes()
checkpoint = json.loads(checkpoint_raw)
assert checkpoint['schema'] == 'geod-partial-transfer/v1'
assert job.get('stacSource') is None and job.get('wcsSource') is None
binding_fields = [job[key] for key in ['id', 'kind', 'itemId', 'assetKey', 'href', 'mediaType']] + [None, None]
binding = hashlib.sha256(json.dumps(binding_fields, separators=(',', ':'), ensure_ascii=False).encode()).hexdigest()
assert checkpoint['binding'] == binding
assert checkpoint['total'] == job['totalBytes'] == queued['totalBytes'] == live['totalBytes']
assert prefix <= checkpoint['bytes'] < checkpoint['total']
etag = checkpoint['etag']
assert etag.startswith('"') and etag.endswith('"') and not any(char in etag for char in '\r\n')
partial = root / 'assets' / (job_id + '.part')
with partial.open('rb') as file:
    digest = hashlib.sha256()
    remaining = checkpoint['bytes']
    while remaining:
        chunk = file.read(min(4 * 1024 * 1024, remaining))
        assert chunk, 'The saved checkpoint exceeds the readable prefix'
        digest.update(chunk)
        remaining -= len(chunk)
    assert digest.hexdigest() == checkpoint['sha256']
    start, end = prefix - 32768, prefix + 32767
    file.seek(start)
    actual = file.read(65536)
    assert len(actual) == 65536

settings = read_json(local, base + '/proxy')
assert settings['mode'] in ['system', 'direct', 'manual']
proxies = {} if settings['mode'] == 'direct' else urllib.request.getproxies()
if settings['mode'] == 'manual':
    proxies = {'http': settings['url'], 'https': settings['url']}
remote = urllib.request.build_opener(urllib.request.ProxyHandler(proxies), NoRedirect())
asset = urlsplit(job['href'])
assert asset.scheme == 'https' and asset.hostname == 'sentinel1euwestrtc.blob.core.windows.net'
assert not asset.query and not asset.fragment and asset.path.startswith('/sentinel1-grd-rtc/')
assert asset.path.endswith('/measurement/iw-hv.rtc.tiff')
authorization = read_json(remote, 'https://planetarycomputer.microsoft.com/api/sas/v1/token/sentinel1euwestrtc/sentinel1-grd-rtc')
token = authorization['token'].lstrip('?')
assert token and '\n' not in token and '\r' not in token
signed = urlunsplit((asset.scheme, asset.netloc, asset.path, token, ''))
try:
    request = urllib.request.Request(signed, headers={'Range': f'bytes={start}-{end}', 'If-Range': etag, 'Accept-Encoding': 'identity'})
    with remote.open(request, timeout=45) as response:
        assert response.status == 206
        assert response.headers['Content-Range'] == f'bytes {start}-{end}/{checkpoint["total"]}'
        assert response.headers['ETag'] == etag
        assert response.headers.get('Content-Encoding', 'identity') == 'identity'
        expected = response.read(65537)
        assert len(expected) == 65536 and actual == expected
except Exception as error:
    raise RuntimeError('Bounded conditional Range check failed: ' + type(error).__name__) from None
after = read_json(local, base + '/jobs/' + job_id)
assert after['id'] == job_id and after['bytesDownloaded'] >= job['bytesDownloaded']
snapshot_sha = hashlib.sha256(checkpoint_raw).hexdigest()
snapshot_file = 'evidence-snapshots/' + snapshot_sha + '.json'
snapshot = root / snapshot_file
snapshot.parent.mkdir(exist_ok=True)
try:
    with snapshot.open('xb') as output:
        output.write(checkpoint_raw)
except FileExistsError:
    assert snapshot.read_bytes() == checkpoint_raw
report = {'schema': 'geod-radar-retry-boundary/v1', 'checkedAt': datetime.now(timezone.utc).isoformat(),
          'status': 'partial', 'scope': 'Independent conditional Range across the worker-reported retry boundary and full saved-prefix fingerprint; no whole-original acceptance or native restart.',
          'nativeBinarySha256': binary_sha, 'jobId': job_id, 'itemId': job['itemId'], 'key': 'hv', 'href': job['href'],
          'sameJobId': True, 'workerRequestCaptured': False, 'workerReportedResumeBytes': prefix,
          'checkpoint': {'snapshot': snapshot_file, 'sha256': snapshot_sha, 'binding': binding,
                         'prefixBytes': checkpoint['bytes'], 'prefixSha256': checkpoint['sha256'], 'independentHashMatch': True},
          'etag': etag, 'totalBytes': checkpoint['total'], 'observedBytesAfterCheck': after['bytesDownloaded'],
          'independentBoundary': {'from': start, 'to': end, 'bytes': 65536, 'sha256': hashlib.sha256(actual).hexdigest(),
                                  'conditionalRangeAccepted': True, 'exactSourceBytes': True},
          'evidenceReports': [{'file': 'hv-retry-queued-verification.json', 'sha256': hashlib.sha256(queued_raw).hexdigest()},
                              {'file': 'hv-retry-live-verification.json', 'sha256': hashlib.sha256(live_raw).hexdigest()}],
          'originalAccepted': False, 'nativeRestarted': False, 'signaturesPersisted': False, 'usedUserDesktop': False}
output = root / 'hv-retry-boundary-verification.json'
temporary = output.with_suffix('.json.tmp')
temporary.write_text(json.dumps(report, indent=2) + '\n', encoding='utf8')
temporary.replace(output)
print(json.dumps({'status': report['status'], 'sameJobId': True, 'prefixBytes': checkpoint['bytes'],
                  'boundaryBytes': 65536, 'conditionalRangeAccepted': True, 'exactSourceBytes': True, 'nativeRestarted': False}))
