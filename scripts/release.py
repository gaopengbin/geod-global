"""Validate version consistency and stage verified Windows release artifacts.

This script never builds, installs, signs, uploads, creates a tag, or publishes a
release. Python 3.11+ and Git are required for `check`; artifact validation itself
is portable and does not execute Windows binaries.
"""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tomllib
import zipfile


ROOT = Path(__file__).resolve().parents[1]
TARGET = 'x86_64-pc-windows-msvc'
IDENTIFIER = 'xyz.laogao.geod.global'
BINARIES = {'geod-global-desktop.exe', 'geod-runtime.exe'}
METADATA_FILES = {'artifacts.json', 'SHA256SUMS.txt'}
MAX_METADATA_BYTES = 16 * 1024 * 1024
MAX_EXPANDED_ZIP_BYTES = 1024 * 1024 * 1024
SEMVER = re.compile(
    r'(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)'
    r'(?:-(?:0|[1-9][0-9]*|[0-9A-Za-z-]*[A-Za-z-][0-9A-Za-z-]*)'
    r'(?:\.(?:0|[1-9][0-9]*|[0-9A-Za-z-]*[A-Za-z-][0-9A-Za-z-]*))*)?'
    r'(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?'
)


def require(condition, message):
    if not condition:
        raise ValueError(message)


def unique_json_object(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result, f'Duplicate JSON key: {key}')
        result[key] = value
    return result


def json_bytes(data, label):
    require(len(data) <= MAX_METADATA_BYTES, f'{label} exceeds the metadata size limit')
    value = json.loads(data.decode('utf-8'), object_pairs_hook=unique_json_object)
    require(isinstance(value, dict), f'{label} must contain a JSON object')
    return value


def read_json(path):
    require(path.stat().st_size <= MAX_METADATA_BYTES, f'{path.name} exceeds the metadata size limit')
    return json_bytes(path.read_bytes(), path.name)


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def valid_version(value):
    require(isinstance(value, str) and SEMVER.fullmatch(value), 'Version must be strict SemVer')
    return value


def valid_commit(value):
    require(isinstance(value, str) and re.fullmatch(r'(?:[0-9a-f]{40}|[0-9a-f]{64})', value),
            'Commit must be a complete lowercase hexadecimal Git SHA (40 or 64 characters)')
    return value


def valid_hash(value, label):
    require(isinstance(value, str) and re.fullmatch(r'[0-9a-f]{64}', value), f'{label} must be a lowercase SHA-256')
    return value


def check_tag(version, tag):
    if tag is not None:
        require(isinstance(tag, str) and tag == 'v' + version, f'Tag must be exactly v{version}')


def repository_version(root=ROOT, tag=None):
    package = read_json(root / 'package.json')
    lock = read_json(root / 'package-lock.json')
    tauri = read_json(root / 'src-tauri/tauri.conf.json')
    require(package.get('name') == 'geod-global', 'Unexpected root package identity')
    require(lock.get('name') == package['name'] and lock.get('packages', {}).get('', {}).get('name') == package['name'],
            'package-lock.json must identify the root GeoD Global package at both levels')
    require(tauri.get('identifier') == IDENTIFIER, 'Unexpected Tauri application identifier')
    versions = {
        'package.json': package.get('version'),
        'package-lock.json': lock.get('version'),
        'package-lock.json packages[""]': lock['packages'][''].get('version'),
        'src-tauri/tauri.conf.json': tauri.get('version'),
    }
    workspace = tomllib.loads((root / 'Cargo.toml').read_text(encoding='utf-8'))
    cargo_lock = tomllib.loads((root / 'Cargo.lock').read_text(encoding='utf-8'))
    for manifest, name in [('crates/geod-runtime/Cargo.toml', 'geod-runtime'), ('src-tauri/Cargo.toml', 'geod-global-desktop')]:
        crate = tomllib.loads((root / manifest).read_text(encoding='utf-8'))['package']
        require(crate.get('name') == name, f'Unexpected crate identity in {manifest}')
        version = crate.get('version')
        if version == {'workspace': True}:
            version = workspace.get('workspace', {}).get('package', {}).get('version')
        versions[manifest] = version
        records = [entry for entry in cargo_lock.get('package', []) if entry.get('name') == name]
        require(len(records) == 1 and 'source' not in records[0], f'Cargo.lock must contain exactly one local {name} package')
        versions['Cargo.lock ' + name] = records[0].get('version')
    version = valid_version(package.get('version'))
    for label, candidate in versions.items():
        require(candidate == version, f'Version mismatch: {label} is {candidate!r}, expected {version}')
    check_tag(version, tag)
    return version


def git(root, *args):
    result = subprocess.run(['git', '-C', str(root), *args], capture_output=True, text=True, encoding='utf-8', check=False)
    require(result.returncode == 0, 'Git check failed: ' + result.stderr.strip())
    return result.stdout.strip()


def check(root=ROOT, tag=None, commit=None, require_clean=False):
    version = repository_version(root, tag)
    current = valid_commit(git(root, 'rev-parse', 'HEAD'))
    if commit is not None:
        require(valid_commit(commit) == current, 'Requested commit differs from the checked-out HEAD')
    dirty = bool(git(root, 'status', '--porcelain', '--untracked-files=normal'))
    require(not require_clean or not dirty, 'The release checkout has uncommitted or untracked changes')
    return {'version': version, 'commit': current, 'dirty': dirty, 'versionsChecked': 8, 'tag': tag}


def safe_filename(value):
    require(isinstance(value, str) and len(value) <= 240 and re.fullmatch(r'[A-Za-z0-9][A-Za-z0-9._+-]*', value)
            and value not in ('.', '..'), 'Artifact file must be a plain safe filename, without a path')
    return value


def plain_directory(path):
    path = Path(path)
    require(not path.is_symlink() and not getattr(path, 'is_junction', lambda: False)(), 'Release directory must not be a symlink or junction')
    resolved = path.resolve(strict=True)
    require(resolved.is_dir(), 'Release location must be a directory')
    return resolved


def plain_file(directory, name):
    safe_filename(name)
    path = directory / name
    require(path.is_file() and not path.is_symlink(), f'Missing or redirected artifact: {name}')
    require(path.resolve(strict=True).parent == directory, f'Artifact escapes its directory: {name}')
    return path


def verify_record(directory, record):
    require(isinstance(record, dict), 'Artifact record must be an object')
    name = safe_filename(record.get('file'))
    require(type(record.get('bytes')) is int and record['bytes'] > 0, f'Invalid byte count for {name}')
    valid_hash(record.get('sha256'), name)
    path = plain_file(directory, name)
    require(path.stat().st_size == record['bytes'] and digest(path) == record['sha256'], f'Artifact byte count or SHA-256 mismatch: {name}')
    return path


def packaging_module():
    spec = importlib.util.spec_from_file_location('geod_windows_packaging', Path(__file__).with_name('package-windows.py'))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def verify_zip_manifest(path, summary, commit, version):
    # Bound untrusted decompression before the packaging verifier reads entries.
    with zipfile.ZipFile(path) as archive:
        entries = archive.infolist()
        require(len(entries) <= 20000 and sum(entry.file_size for entry in entries) <= MAX_EXPANDED_ZIP_BYTES,
                'Release ZIP exceeds verification resource limits')
        manifests = [entry for entry in entries if entry.filename.endswith('/release-manifest.json')]
        require(len(manifests) == 1 and manifests[0].file_size <= MAX_METADATA_BYTES, 'Release ZIP requires one bounded manifest')
        strict_manifest = json_bytes(archive.read(manifests[0]), 'Embedded release manifest')
    manifest = packaging_module().verify_archive(path)
    require(manifest == strict_manifest, 'Embedded manifest changed while validating the archive')
    require(manifest.get('schemaVersion') == 'geod-windows-release/v1', 'Unsupported embedded release manifest schema')
    require(manifest.get('product') == 'GeoD Global' and manifest.get('identifier') == IDENTIFIER, 'Embedded product identity differs from GeoD Global')
    require(manifest.get('version') == version, 'Embedded release version differs from the repository')
    require(manifest.get('profile') == 'release' and manifest.get('target') == TARGET, 'Only native Windows x64 release-profile artifacts can be staged')
    build = manifest.get('build')
    require(isinstance(build, dict), 'Embedded build receipt must be an object')
    for label, source in [('source', manifest.get('source')), ('build.source', build.get('source'))]:
        require(isinstance(source, dict) and source.get('commit') == commit and source.get('dirty') is False,
                f'Embedded {label} must be clean and match the requested commit')
    require(manifest['source'].get('treeSha256') == summary['sourceTreeSha256'], 'Source tree hash differs between artifact summary and embedded manifest')
    require(build.get('schemaVersion') == 'geod-windows-build/v1' and build.get('target') == TARGET and build.get('profile') == 'release',
            'Embedded build receipt must describe a Windows x64 release build')
    require(build.get('targetRustflags') == '-Ctarget-feature=+crt-static', 'Build receipt does not record the required static CRT build')
    records = build.get('binaries')
    require(isinstance(records, list) and len(records) == 2 and all(isinstance(item, dict) for item in records) and {item.get('file') for item in records} == BINARIES,
            'Build receipt must identify exactly the desktop and runtime executables')
    files = {item['path']: item for item in manifest['files']}
    signatures = manifest.get('signatures', {})
    require(isinstance(signatures, dict) and set(signatures) == BINARIES and all(isinstance(value, dict) and value.get('status') == 'NotSigned' for value in signatures.values()),
            'Embedded executable signing status must be explicitly NotSigned')
    for record in records:
        name = record['file']
        valid_hash(record.get('sha256'), 'Build receipt ' + name)
        require(type(record.get('bytes')) is int and record['bytes'] > 0, 'Invalid build receipt binary size')
        require(name in files and files[name].get('bytes') == record['bytes'] and files[name].get('sha256') == record['sha256'],
                f'Embedded executable differs from the build receipt: {name}')
    return manifest


def verify_directory(directory, commit, version, exact_files=True):
    commit = valid_commit(commit)
    directory = plain_directory(directory)
    summary = read_json(plain_file(directory, 'artifacts.json'))
    require(summary.get('schemaVersion') == 'geod-windows-artifacts/v1', 'Unsupported artifact summary schema')
    require(summary.get('sourceCommit') == commit and summary.get('dirty') is False, 'Artifact summary must be clean and match the requested commit')
    require(summary.get('version') == version, 'Artifact summary version differs from the repository')
    require(summary.get('signatureStatus') == 'unsigned', 'Only explicitly unsigned evaluation artifacts are accepted')
    valid_hash(summary.get('sourceTreeSha256'), 'Artifact source tree')
    records = summary.get('artifacts')
    require(isinstance(records, list) and len(records) == 2, 'Exactly one portable ZIP and one setup EXE are required')
    paths = [verify_record(directory, record) for record in records]
    names = [path.name for path in paths]
    require(len({name.casefold() for name in names}) == 2, 'Duplicate artifact filename')
    zips = [path for path in paths if path.name.endswith('.zip')]
    setups = [path for path in paths if path.name.endswith('-setup.exe')]
    require(len(zips) == 1 and len(setups) == 1 and setups[0].name == zips[0].stem + '-setup.exe',
            'Artifacts must be one matching portable .zip and -setup.exe pair')
    checksums = plain_file(directory, 'SHA256SUMS.txt')
    expected = ''.join(f"{record['sha256']}  {record['file']}\n" for record in records).encode('utf-8')
    # The Windows packager's text writer emits CRLF; Linux-generated fixtures or
    # downloaded normalized text may use LF. Require one exact form, not a loose
    # checksum parser that accepts extra files, whitespace, or duplicate entries.
    require(checksums.stat().st_size <= MAX_METADATA_BYTES and checksums.read_bytes() in (expected, expected.replace(b'\n', b'\r\n')),
            'SHA256SUMS.txt must exactly match the artifact records (including order and final newline)')
    allowed = set(names) | METADATA_FILES
    if exact_files:
        require({entry.name for entry in directory.iterdir()} == allowed, 'Staged directory must contain exactly the four release files')
    verify_zip_manifest(zips[0], summary, commit, version)
    # Cross-check again after archive validation; never publish a file replaced
    # between the initial hash check and the embedded-manifest checks.
    for record in records:
        verify_record(directory, record)
    return {'version': version, 'commit': commit, 'directory': str(directory), 'files': sorted(allowed), 'signatureStatus': 'unsigned'}


def stage(package_root, destination, commit, tag=None, root=ROOT):
    version = repository_version(root, tag)
    valid_commit(commit)
    package_root = plain_directory(package_root)
    candidates = [child for child in package_root.iterdir() if child.is_dir() and (child / 'artifacts.json').is_file()]
    require(len(candidates) == 1, 'Package root must have exactly one direct child directory containing artifacts.json')
    source = plain_directory(candidates[0])
    require(source.parent == package_root, 'Package directory escapes the requested package root')
    checked = verify_directory(source, commit, version, exact_files=False)
    destination = Path(destination)
    require(not destination.is_symlink() and not getattr(destination, 'is_junction', lambda: False)(), 'Staging destination must not be a symlink or junction')
    destination = destination.resolve()
    require('\n' not in str(destination) and '\r' not in str(destination), 'Staging path must be a single line')
    require(destination != source and source not in destination.parents and destination != package_root,
            'Staging destination must be separate from the source package')
    if destination.exists():
        require(destination.is_dir() and not any(destination.iterdir()), 'Staging destination must be empty; existing files are never overwritten')
    else:
        destination.mkdir(parents=True, exist_ok=False)
    for name in checked['files']:
        original = plain_file(source, name)
        with original.open('rb') as reader, (destination / name).open('xb') as writer:
            shutil.copyfileobj(reader, writer, length=1024 * 1024)
    return verify_directory(destination, commit, version)


def verify(directory, commit, tag=None, root=ROOT):
    return verify_directory(directory, commit, repository_version(root, tag))


def emit_outputs(result):
    output = os.environ.get('GITHUB_OUTPUT')
    if output:
        values = {'version': result['version']}
        if 'directory' in result:
            values['directory'] = result['directory']
        require(all('\n' not in value and '\r' not in value for value in values.values()), 'GitHub output values must be single-line strings')
        with Path(output).open('a', encoding='utf-8', newline='\n') as stream:
            for key, value in values.items():
                stream.write(f'{key}={value}\n')


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest='command', required=True)
    preflight = commands.add_parser('check', help='Check repository versions, optional tag/HEAD, and optional clean checkout')
    preflight.add_argument('--tag')
    preflight.add_argument('--commit')
    preflight.add_argument('--require-clean', action='store_true')
    staging = commands.add_parser('stage', help='Validate one completed package and copy only its four release files')
    staging.add_argument('--package-root', required=True, type=Path)
    staging.add_argument('--destination', required=True, type=Path)
    staging.add_argument('--commit', required=True)
    staging.add_argument('--tag')
    validation = commands.add_parser('verify', help='Revalidate a staged four-file release directory')
    validation.add_argument('--directory', required=True, type=Path)
    validation.add_argument('--commit', required=True)
    validation.add_argument('--tag')
    args = parser.parse_args(argv)
    if args.command == 'check':
        result = check(tag=args.tag, commit=args.commit, require_clean=args.require_clean)
    elif args.command == 'stage':
        result = stage(args.package_root, args.destination, args.commit, args.tag)
    else:
        result = verify(args.directory, args.commit, args.tag)
    emit_outputs(result)
    print(json.dumps({'verified': True, **result}, ensure_ascii=False))


if __name__ == '__main__':
    try:
        main()
    except Exception as error:
        print(json.dumps({'error': str(error)}, ensure_ascii=False), file=sys.stderr)
        sys.exit(1)
