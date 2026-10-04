"""Offline restart/export acceptance for previously completed public probes.

Stop the server owning --data-dir first. CLI list/inspect/export do not query
remote services; every command opens and closes the persisted runtime store.
"""
import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import subprocess


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--data-dir', required=True)
    parser.add_argument('--evidence-dir', required=True)
    parser.add_argument('--output', required=True)
    args = parser.parse_args()
    root = Path.cwd()
    storage = Path(args.data_dir).resolve()
    evidence = Path(args.evidence_dir).resolve()
    out = Path(args.output).resolve()
    out.mkdir(parents=True, exist_ok=True)
    exe = root / 'target/debug/geod-runtime.exe'
    public = json.loads((evidence / 'arcgis-public/report.json').read_text(encoding='utf-8'))
    shapes = json.loads((evidence / 'arcgis-shapes/report.json').read_text(encoding='utf-8'))
    ui = json.loads((evidence / 'arcgis-ui/report.json').read_text(encoding='utf-8'))
    ids = [public['asset']['id'], public['emptyAssetId'],
           *(c['assetId'] for c in shapes['cases']), *(c['assetId'] for c in ui['reports'])]

    def command(*values):
        completed = subprocess.run([str(exe), *values, '--data-dir', str(storage)],
            text=True, encoding='utf-8', capture_output=True, check=True)
        return json.loads(completed.stdout)

    services = command('feature-services', 'list')
    assets = command('vectors', 'list')
    results = []
    for asset_id in ids:
        asset = next(a for a in assets if a['id'] == asset_id)
        data = command('vectors', 'inspect', '--id', asset_id)
        assert data['asset'] == asset and data['geojson']['geodSource'] == asset['remoteSource']
        assert any(s['url'] == asset['remoteSource']['serviceUrl'] for s in services)
        source = storage / 'vectors' / (asset_id + '.json')
        raw = source.read_bytes()
        assert hashlib.sha256(raw).hexdigest() == asset['sourceSha256']
        export = out / (asset_id + '.geojson')
        command('vectors', 'export', '--id', asset_id, '--out', str(export))
        assert json.loads(export.read_text(encoding='utf-8')) == data['geojson']
        assert source.read_bytes() == raw
        results.append({'assetId': asset_id, 'features': asset['featureCount'],
            'sourceSha256': asset['sourceSha256'], 'completeProvenanceRecovered': True,
            'allNativeExportFeaturesEqual': True, 'managedSnapshotUnchanged': True})
    report = {'checkedAt': datetime.now(timezone.utc).isoformat(), 'serviceRegistryRecovered': True,
        'mode': 'Fresh native CLI processes, server stopped; list/inspect/export only',
        'networkRequiredForReopen': False, 'snapshots': results}
    (out / 'report.json').write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    print(json.dumps({'offlineSnapshots': len(results), 'allNativeExportsEqual': True}))


if __name__ == '__main__':
    main()
