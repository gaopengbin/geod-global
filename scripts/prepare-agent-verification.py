"""Copy one existing verified SCL file into an isolated Agent acceptance store."""
import hashlib
import json
from pathlib import Path
import shutil

base = Path(__file__).resolve().parent.parent
source = base / '.verification/cli-e2e-roundtrip/store'
destination = base / '.verification/agent-native-20261004/core'
destination.mkdir(parents=True, exist_ok=True)
(destination / 'assets').mkdir(exist_ok=True)
jobs = json.loads((source / 'jobs.json').read_text(encoding='utf-8'))
job = next(value for value in jobs.values() if value['status'] == 'succeeded' and value['assetKey'] == 'scl' and value['kind'] == 'download')
original = Path(job['outputPath'])
digest = hashlib.sha256(original.read_bytes()).hexdigest()
assert digest == job['sha256']
target = (destination / 'assets' / (job['id'] + '.tif')).resolve()
shutil.copyfile(original, target)
job['outputPath'] = str(target)
(destination / 'jobs.json').write_text(json.dumps({job['id']: job}, ensure_ascii=False, indent=2), encoding='utf-8')
(destination.parent / 'source.json').write_text(json.dumps({'scope': 'copy of an existing completed, checksum-verified real Sentinel SCL download; no new source request', 'jobId': job['id'], 'sha256': digest, 'bytes': target.stat().st_size}, indent=2), encoding='utf-8')
print(json.dumps({'prepared': True, 'jobId': job['id'], 'sha256': digest, 'bytes': target.stat().st_size}))
