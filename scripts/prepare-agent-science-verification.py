"""Clone actual checksum-verified source cohorts for isolated Agent acceptance.

Reuses previously downloaded original products. This is neither fixture imagery
nor evidence of a new provider transfer; upstream traffic is disabled.
"""
import argparse
import copy
import hashlib
import json
import shutil
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--root', required=True, type=Path)
args = parser.parse_args()
repo = Path(__file__).resolve().parent.parent
base = args.root.resolve()
assert base.parent == (repo / '.verification').resolve() and base.name.startswith('agent-science-') and not base.exists()
core = base / 'core'
(core / 'assets').mkdir(parents=True)

def read(path):
    return json.loads(path.read_text(encoding='utf-8'))

def sha(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()

rgb_root = repo / '.verification/landsat-coupled-final-20261004'
rgb_jobs = read(rgb_root / 'jobs.json')
reference = rgb_jobs['5f26e4b2-d792-4678-8cae-e0746c27f460']['rgbSpec']
rgb_ids = [s['jobId'] for s in reference['sources']]
quality_ids = [s['jobId'] for s in reference['qualityMask']['sources']]
rgb_needed = set(rgb_ids + quality_ids)
rgb_needed.update(s['jobId'] for scene in reference['qualityMask']['coupled']['scenes'] for s in scene['sources'])
rgb_project = read(rgb_root / 'projects.json')[reference['projectId']]

vi_root = repo / '.verification/modis-vi-quality-native-r4-20261004'
vi_jobs = read(vi_root / 'jobs.json')
vi_project = read(vi_root / 'projects.json')['5706ca9d-81e2-4f2f-8e43-2e83319cf02a']
vi_needed = set()
for scene in vi_project['scenes']:
    for key in ['ndvi', 'evi', 'vi_quality', 'vi_reliability']:
        job = next(j for j in vi_jobs.values() if j['kind'] == 'download' and j['itemId'] == scene['itemId'] and j['assetKey'] == key and j['href'] == scene['assets'][key]['href'])
        vi_needed.add(job['id'])

copied, sources = {}, []
for root, records, ids in [(rgb_root, rgb_jobs, rgb_needed), (vi_root, vi_jobs, vi_needed)]:
    for id in sorted(ids):
        job = copy.deepcopy(records[id])
        assert job['status'] == 'succeeded'
        original = Path(job['outputPath'])
        assert sha(original) == job['sha256'] and original.stat().st_size == job['bytesDownloaded']
        sources.append({'jobId': id, 'kind': job['kind'], 'assetKey': job['assetKey'], 'sha256': job['sha256'], 'bytes': job['bytesDownloaded'], 'originalPath': str(original), 'mtimeNs': original.stat().st_mtime_ns})
        target = core / 'assets' / original.name
        shutil.copy2(original, target)
        job['outputPath'] = str(target)
        if job.get('manifestPath'):
            manifest = Path(job['manifestPath'])
            target_manifest = core / 'assets' / manifest.name
            shutil.copy2(manifest, target_manifest)
            job['manifestPath'] = str(target_manifest)
        copied[id] = job

def write(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2), encoding='utf-8')

write(core / 'jobs.json', copied)
write(core / 'projects.json', {p['id']: p for p in [rgb_project, vi_project]})
write(core / 'proxy-settings.json', {'mode': 'custom', 'url': 'http://127.0.0.1:9'})
request = {'jobIds': rgb_ids, 'projectId': rgb_project['id'], 'name': 'Agent actual Landsat QA RGB', 'qualityMask': {'qaPixelJobId': quality_ids[0], 'qaRadsatJobId': quality_ids[1], 'policy': 'cloud_free_conservative', 'excludeSnow': False}}
write(base / 'inputs.json', {'schema': 'geod-agent-science-inputs/v1', 'scope': 'Copied actual original downloads and pinned native intermediates; no new transfer; upstream disabled', 'rgbRequest': request, 'viProjectId': vi_project['id'], 'sources': sources})
print(json.dumps({'prepared': True, 'directory': str(base), 'jobs': len(copied), 'originals': sum(j['kind'] == 'download' for j in copied.values()), 'bytesCopied': sum(j['bytesDownloaded'] for j in copied.values())}))
