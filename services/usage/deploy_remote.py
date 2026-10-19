"""Deploy the independent Global usage receiver from a locally uploaded archive.

Run on the reviewed Tencent host as ubuntu. No dependency or GitHub downloads.
"""
import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import re
import shutil
import sqlite3
import subprocess
import tarfile
import time
import urllib.request


ROOT = Path('/srv/laogao')
NAME = 'geod-global-usage'
PORT = 9119
SITE = Path('/etc/nginx/sites-available/geod-global')
UNIT = Path('/etc/systemd/system/geod-global-usage.service')


def run(*args):
    return subprocess.run(args, check=True, text=True, capture_output=True).stdout.strip()


def health():
    for _ in range(20):
        try:
            with urllib.request.urlopen(f'http://127.0.0.1:{PORT}/health', timeout=2) as response:
                value = json.load(response)
                if value.get('product') == 'geod-global' and value.get('ready'):
                    return
        except (OSError, ValueError):
            pass
        time.sleep(.25)
    raise RuntimeError('Usage receiver failed health verification.')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--release', required=True)
    parser.add_argument('--sha256', required=True)
    args = parser.parse_args()
    assert re.fullmatch(r'\d{8}T\d{6}Z', args.release)
    assert re.fullmatch(r'[a-f0-9]{64}', args.sha256)
    stage = ROOT / 'staging' / (NAME + '-' + args.release)
    assert Path(__file__).resolve().parent == stage
    archive = stage / 'receiver.tar.gz'
    assert hashlib.sha256(archive.read_bytes()).hexdigest() == args.sha256
    assert shutil.disk_usage(ROOT).free > 2 * 1024**3
    release = ROOT / 'releases' / NAME / args.release
    assert not release.exists()
    release.mkdir(parents=True, mode=0o700)
    expected = {'services/usage/server.py', 'services/usage/test_service.py', 'schemas/usage-events.json'}
    with tarfile.open(archive) as package:
        members = package.getmembers()
        assert {member.name for member in members} == expected
        assert all(member.isfile() for member in members)
        package.extractall(release, filter='data')
    run('python3', '-m', 'unittest', 'discover', '-s', str(release / 'services/usage'), '-p', 'test_service.py')
    data = ROOT / 'data' / NAME
    data.mkdir(parents=True, exist_ok=True, mode=0o700)
    data.chmod(0o700)
    database = data / 'events.sqlite'
    current = ROOT / 'current' / NAME
    assert not current.exists() or current.is_symlink()
    previous = current.resolve() if current.is_symlink() else None
    if previous:
        assert previous.is_relative_to(ROOT / 'releases' / NAME)
    snapshot = ROOT / 'backups' / NAME / args.release
    snapshot.mkdir(parents=True, mode=0o700)
    if database.exists():
        with sqlite3.connect(database) as source, sqlite3.connect(snapshot / 'events.sqlite') as target:
            assert source.execute('PRAGMA quick_check').fetchone()[0] == 'ok'
            source.backup(target)
    nginx = run('sudo', 'cat', str(SITE)) + '\n'
    assert 'server_name geod-global.laogao.xyz;' in nginx
    (snapshot / 'nginx.conf').write_text(nginx)
    had_unit = UNIT.exists()
    if had_unit:
        (snapshot / 'service.unit').write_text(run('sudo', 'cat', str(UNIT)) + '\n')
    assert '# BEGIN GEOD GLOBAL USAGE' not in nginx, 'Existing usage route needs explicit migration review.'
    marker = '    location /api/ {'
    assert nginx.count(marker) == 1
    route = f'''    # BEGIN GEOD GLOBAL USAGE
    location = /api/usage/v1/events {{
        access_log off;
        client_max_body_size 64k;
        limit_req zone=geod_global_usage burst=60 nodelay;
        limit_req_status 429;
        proxy_pass http://127.0.0.1:{PORT}/v1/events;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header Host $host;
        proxy_connect_timeout 3s; proxy_read_timeout 10s;
    }}
    location = /api/usage/health {{
        access_log off;
        proxy_pass http://127.0.0.1:{PORT}/health;
    }}
    location ^~ /api/usage/ {{ return 404; }}
    # END GEOD GLOBAL USAGE
'''
    updated = 'limit_req_zone $binary_remote_addr zone=geod_global_usage:2m rate=120r/m;\n' + nginx.replace(marker, route + marker)
    (stage / 'nginx.conf').write_text(updated)
    (stage / 'service.unit').write_text(f'''[Unit]
Description=GeoD Global anonymous usage receiver
After=network.target
[Service]
Type=simple
User=ubuntu
WorkingDirectory={current}
ExecStart=/usr/bin/python3 -B -u {current}/services/usage/server.py --db {database} --port {PORT}
Restart=on-failure
RestartSec=3
UMask=0077
NoNewPrivileges=true
PrivateTmp=true
ProtectHome=true
ProtectSystem=strict
ReadWritePaths={data}
MemoryMax=192M
TasksMax=64
StandardOutput=journal
StandardError=journal
[Install]
WantedBy=multi-user.target
''')
    receipt = {'release': args.release, 'sha256': args.sha256, 'previous': str(previous) if previous else None,
               'backup': str(snapshot), 'database': str(database), 'port': PORT}
    (snapshot / 'deployment.json').write_text(json.dumps(receipt, indent=2) + '\n')
    temporary = current.with_name(NAME + '-' + args.release)
    temporary.symlink_to(release)
    temporary.replace(current)
    try:
        run('sudo', 'install', '-m', '644', str(stage / 'service.unit'), str(UNIT))
        run('sudo', 'systemctl', 'daemon-reload')
        run('sudo', 'systemctl', 'enable', '--now', NAME)
        run('sudo', 'systemctl', 'restart', NAME)
        health()
        run('sudo', 'install', '-m', '644', str(stage / 'nginx.conf'), str(SITE))
        run('sudo', 'nginx', '-t')
        run('sudo', 'systemctl', 'reload', 'nginx')
        health()
    except Exception:
        run('sudo', 'install', '-m', '644', str(snapshot / 'nginx.conf'), str(SITE))
        run('sudo', 'nginx', '-t')
        run('sudo', 'systemctl', 'reload', 'nginx')
        run('sudo', 'systemctl', 'stop', NAME)
        if previous:
            rollback = current.with_name(NAME + '-rollback')
            rollback.symlink_to(previous)
            rollback.replace(current)
        else:
            current.unlink()
        if had_unit:
            run('sudo', 'install', '-m', '644', str(snapshot / 'service.unit'), str(UNIT))
            run('sudo', 'systemctl', 'daemon-reload')
            run('sudo', 'systemctl', 'start', NAME)
        else:
            run('sudo', 'systemctl', 'disable', NAME)
            run('sudo', 'rm', str(UNIT))
            run('sudo', 'systemctl', 'daemon-reload')
        raise
    receipt.update(verifiedAt=datetime.now(timezone.utc).isoformat(), loopbackHealth=True, nginxValidated=True)
    (snapshot / 'deployment.json').write_text(json.dumps(receipt, indent=2) + '\n')
    print(json.dumps(receipt))


if __name__ == '__main__':
    main()
