"""Build and package an unsigned GeoD Global Windows evaluation distribution.

No installation, registry writes, signing, upload, or sibling checkout access.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import shutil
import struct
import subprocess
import sys
import tarfile
import time
import urllib.error
import urllib.request
import zipfile

ROOT = Path(__file__).resolve().parents[1]
TARGET = 'x86_64-pc-windows-msvc'


def command(*args, capture=False, env=None):
    result = subprocess.run(args, cwd=ROOT, capture_output=capture, encoding='utf-8', errors='strict', env=env)
    if result.returncode:
        raise RuntimeError(f'Command failed ({result.returncode}): {args[0]}\n{result.stderr or ""}')
    return result.stdout.strip() if capture else None


def write_json(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def relative(path, base):
    return path.relative_to(base).as_posix()


def is_build_input(name):
    if name in ['README.md','prototype/README.md','src-tauri/README.md'] or (name.startswith('crates/') and name.endswith('/README.md')) or name.startswith(('docs/', 'examples/', 'prototype/qa/')):
        return False
    return name in ['Cargo.toml','Cargo.lock','package.json','package-lock.json','rust-toolchain','rust-toolchain.toml'] or name.startswith(('prototype/','crates/','src-tauri/','.cargo/','schemas/'))


def source_identity(build_only=False):
    paths = command('git', 'ls-files', '--cached', '--others', '--exclude-standard', '-z', capture=True).split('\0')
    records = [{'path': name, 'sha256': digest(ROOT / name)} for name in sorted(set(paths)) if name and (ROOT / name).is_file() and (not build_only or is_build_input(name))]
    return {'commit': command('git', 'rev-parse', 'HEAD', capture=True),
            'dirty': bool(command('git', 'status', '--porcelain', capture=True)),
            'treeSha256': hashlib.sha256(json.dumps(records, sort_keys=True).encode()).hexdigest(),
            'files': records}


def build_receipt_path(profile):
    return ROOT / '.verification' / f'windows-build-{TARGET}-{profile}.json'


def cargo_build_command(profile):
    args = ['cargo', 'build', '--locked', '--jobs', '2', '--target', TARGET, '--target-dir', str(ROOT / 'target'), '-p', 'geod-global-desktop', '-p', 'geod-runtime', '--features', 'geod-global-desktop/custom-protocol']
    if profile == 'release': args.append('--release')
    return args


def build_binaries(profile, rust):
    inputs = source_identity(build_only=True)
    print('Building frontend, desktop and CLI; documentation may change independently.', flush=True)
    command('npm.cmd', 'run', 'build')
    cargo = cargo_build_command(profile)
    if os.environ.get('RUSTFLAGS') or os.environ.get('CARGO_ENCODED_RUSTFLAGS'):
        raise RuntimeError('Clear custom RUSTFLAGS/CARGO_ENCODED_RUSTFLAGS for a reproducible packaging build')
    build_env = os.environ.copy()
    build_env['CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUSTFLAGS'] = '-Ctarget-feature=+crt-static'
    command(*cargo, env=build_env)
    if source_identity(build_only=True)['treeSha256'] != inputs['treeSha256']:
        raise RuntimeError('Build inputs changed during compilation; rebuild before packaging')
    binaries = []
    for name in ['geod-global-desktop.exe','geod-runtime.exe']:
        path = ROOT / 'target' / TARGET / profile / name
        verify_pe(path)
        binaries.append({'file':name,'bytes':path.stat().st_size,'sha256':digest(path)})
    receipt = {'schemaVersion':'geod-windows-build/v1','target':TARGET,'profile':profile,
               'createdAt':time.strftime('%Y-%m-%dT%H:%M:%SZ',time.gmtime()), 'source':inputs,
               'rustc':rust,'node':command('node','--version',capture=True),
               'cargoArgs':[arg if arg != str(ROOT / 'target') else 'target' for arg in cargo[1:]],'targetDirectory':'repository target/','targetRustflags':'-Ctarget-feature=+crt-static','binaries':binaries}
    write_json(build_receipt_path(profile),receipt)
    return receipt


def verified_build_receipt(profile):
    path = build_receipt_path(profile)
    if not path.is_file(): raise RuntimeError('No completed build receipt; use --build-only first')
    receipt = json.loads(path.read_text(encoding='utf-8'))
    if receipt['target'] != TARGET or receipt['profile'] != profile or receipt['source']['treeSha256'] != source_identity(build_only=True)['treeSha256']:
        raise RuntimeError('Build inputs changed since the receipt; rebuild binaries first')
    if {item['file'] for item in receipt['binaries']} != {'geod-global-desktop.exe','geod-runtime.exe'}:
        raise RuntimeError('Build receipt has an unexpected binary list')
    for item in receipt['binaries']:
        binary = ROOT / 'target' / TARGET / profile / item['file']
        if binary.stat().st_size != item['bytes'] or digest(binary) != item['sha256']:
            raise RuntimeError(f"Binary changed since build: {item['file']}")
    return receipt


def copy_verified_binary(original, destination, expected):
    shutil.copyfile(original, destination)
    if destination.stat().st_size != expected['bytes'] or digest(destination) != expected['sha256']:
        raise RuntimeError(f"Copied binary differs from build receipt: {expected['file']}")
    verify_pe(destination)


def verify_pe(path):
    with path.open('rb') as stream:
        if stream.read(2) != b'MZ':
            raise ValueError(f'Not a Windows executable: {path.name}')
        stream.seek(0x3c)
        offset = struct.unpack('<I', stream.read(4))[0]
        stream.seek(offset)
        if stream.read(4) != b'PE\0\0' or struct.unpack('<H', stream.read(2))[0] != 0x8664:
            raise ValueError(f'Expected an x64 PE executable: {path.name}')


def pe_imports(path):
    """Record native DLL prerequisites without executing the binary."""
    data = path.read_bytes()
    header = struct.unpack_from('<I', data, 0x3c)[0]
    count = struct.unpack_from('<H', data, header + 6)[0]
    optional_size = struct.unpack_from('<H', data, header + 20)[0]
    optional = header + 24
    directories = optional + (112 if struct.unpack_from('<H', data, optional)[0] == 0x20b else 96)
    imports_rva = struct.unpack_from('<I', data, directories + 8)[0]
    sections = optional + optional_size
    def offset(rva):
        for index in range(count):
            section = sections + index * 40
            size, address, raw_size, raw = struct.unpack_from('<IIII', data, section + 8)
            if address <= rva < address + max(size, raw_size): return raw + rva - address
        raise ValueError('Invalid PE import RVA')
    names = []
    if imports_rva:
        entry = offset(imports_rva)
        while any(data[entry:entry + 20]):
            name = offset(struct.unpack_from('<I', data, entry + 12)[0])
            names.append(data[name:data.index(b'\0', name)].decode('ascii'))
            entry += 20
    return sorted(set(names), key=str.lower)


def signature(path):
    # Pass the filename as a positional argument, never interpolate it as shell code.
    script = 'param($p) $ErrorActionPreference = "Stop"; Import-Module (Join-Path $PSHOME "Modules/Microsoft.PowerShell.Security/Microsoft.PowerShell.Security.psd1") -ErrorAction Stop; Import-Module (Join-Path $PSHOME "Modules/Microsoft.PowerShell.Utility/Microsoft.PowerShell.Utility.psd1") -ErrorAction Stop; $s = Get-AuthenticodeSignature -LiteralPath $p; @{status=$s.Status.ToString(); subject=if($s.SignerCertificate){$s.SignerCertificate.Subject}else{$null}} | ConvertTo-Json -Compress'
    helper = ROOT / '.verification' / 'package-authenticode.ps1'
    helper.parent.mkdir(exist_ok=True)
    helper.write_text(script, encoding='utf-8')
    return json.loads(command('powershell', '-NoProfile', '-File', str(helper), str(path), capture=True))


def license_files(directory):
    return sorted(path for path in directory.rglob('*') if path.is_file()
                  and len(path.relative_to(directory).parts) <= 2
                  and path.name.lower().startswith(('license', 'licence', 'copying', 'notice', 'copyright')))


def fetch_upstream_licenses(package, source, destination):
    """Resolve missing registry license texts at the published crate's exact Git commit."""
    vcs_path = source / '.cargo_vcs_info.json'
    repository = package.get('repository') or ''
    match = re.match(r'https://github.com/([^/]+)/([^/#]+)', repository)
    if not match or not vcs_path.is_file():
        return []
    vcs = json.loads(vcs_path.read_text(encoding='utf-8'))
    commit = vcs.get('git', {}).get('sha1', '')
    if not re.fullmatch(r'[0-9a-f]{40}', commit):
        return []
    owner, repo = match.groups()
    repo = repo.removesuffix('.git')
    cache = ROOT / '.verification' / 'license-cache' / f'{owner}-{repo}-{commit}'
    cache.mkdir(parents=True, exist_ok=True)
    results = []
    candidates = ['LICENSE', 'LICENSE-MIT', 'LICENSE-APACHE', 'LICENSE.txt', 'LICENSE.md', 'COPYING', 'license.txt', 'LICENSE_MIT', 'LICENSE_APACHE-2.0']
    for name in candidates:
        cached = cache / name
        missing = cache / (name + '.missing')
        if missing.is_file(): continue
        url = f'https://raw.githubusercontent.com/{owner}/{repo}/{commit}/{name}'
        if not cached.is_file():
            try:
                request = urllib.request.Request(url, headers={'User-Agent': 'GeoD-Global-license-packaging/0.1'})
                with urllib.request.urlopen(request, timeout=20) as response:
                    data = response.read(2 * 1024 * 1024)
                cached.write_bytes(data)
            except urllib.error.HTTPError as error:
                if error.code == 404:
                    missing.touch()
                    continue
                raise
        target = destination / ('upstream-' + name)
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(cached, target)
        results.append({'file': target.name, 'source': url, 'sha256': digest(target)})
    return results


def standard_license(license_id, destination):
    cache = ROOT / '.verification/license-cache/spdx' / (license_id + '.txt')
    url = f'https://raw.githubusercontent.com/spdx/license-list-data/main/text/{license_id}.txt'
    if not cache.is_file():
        cache.parent.mkdir(parents=True, exist_ok=True)
        with urllib.request.urlopen(url, timeout=20) as response:
            cache.write_bytes(response.read(1024 * 1024))
    target = destination / (license_id + '.txt')
    target.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(cache, target)
    return {'file':target.name, 'source':url, 'sha256':digest(target)}


def collect_notices(payload):
    notices = payload / 'THIRD-PARTY'
    records, gaps = [], []
    metadata = json.loads(command('cargo', 'metadata', '--locked', '--format-version', '1', '--filter-platform', TARGET, capture=True))
    for package in metadata['packages']:
        if not package.get('source'):
            continue
        source = Path(package['manifest_path']).parent
        label = f"{package['name']}-{package['version']}"
        destination = notices / 'rust' / label
        copied = []
        for original in license_files(source):
            target = destination / original.relative_to(source)
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(original, target)
            copied.append({'file': relative(target, destination), 'source': 'published crate', 'sha256': digest(target)})
        if not copied:
            copied = fetch_upstream_licenses(package, source, destination)
        canonical_fallback = None
        # These published sources omit license files at their recorded upstream commit.
        # Retain the complete original source (including copyright headers), declaration,
        # and unmodified standard terms instead of inventing an upstream copyright notice.
        if not copied and package['name'] in ['proj4rs', 'selectors']:
            canonical_fallback = 'MPL-2.0' if package['license'] == 'MPL-2.0' else 'Apache-2.0' if 'Apache-2.0' in package['license'] else None
            if canonical_fallback:
                copied = [standard_license(canonical_fallback, destination)]
                shutil.copyfile(source / 'Cargo.toml', destination / 'published-Cargo.toml')
        if not copied: gaps.append(f"Rust {label}: no license text found")
        source_archive = None
        if 'MPL-2.0' in (package.get('license') or '') or canonical_fallback:
            # Include exact published source for file-level source-availability obligations.
            source_archive = notices / 'source' / (label + '.tar.gz')
            source_archive.parent.mkdir(parents=True, exist_ok=True)
            with tarfile.open(source_archive, 'w:gz') as archive:
                archive.add(source, arcname=label)
        records.append({'ecosystem':'cargo', 'name':package['name'], 'version':package['version'],
                        'license':package.get('license'), 'repository':package.get('repository'),
                        'authors':package.get('authors', []), 'texts':copied,
                        'standardTermsWithOriginalSource':canonical_fallback,
                        'directory':relative(destination, payload),
                        'sourceArchive':relative(source_archive, payload) if source_archive else None})
    lock = json.loads((ROOT / 'package-lock.json').read_text(encoding='utf-8'))
    for name, locked in lock['packages'].items():
        if not name or not name.startswith('node_modules/'):
            continue
        source = ROOT / name
        if not (source / 'package.json').is_file():
            if not locked.get('optional'):
                raise RuntimeError(f'Required dependency is missing: {name}; run npm ci before packaging')
            continue  # Other-platform optional packages are not included in this build.
        package = json.loads((source / 'package.json').read_text(encoding='utf-8'))
        label = re.sub(r'[^a-zA-Z0-9._-]', '_', package['name']) + '-' + package['version']
        destination = notices / 'npm' / label
        copied = []
        for original in license_files(source):
            target = destination / original.relative_to(source)
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(original, target)
            copied.append({'file':relative(target, destination), 'source':'installed package', 'sha256':digest(target)})
        parent_package = {'@rolldown/binding-win32-x64-msvc':'rolldown', '@tauri-apps/cli-win32-x64-msvc':'@tauri-apps/cli'}.get(package['name'])
        if not copied and parent_package:
            parent_source = ROOT / 'node_modules' / parent_package
            parent_metadata = json.loads((parent_source / 'package.json').read_text(encoding='utf-8'))
            if parent_metadata['version'] != package['version'] or parent_metadata['license'] != package['license']:
                raise RuntimeError(f'Matching parent package license/version is required for {label}')
            for original in license_files(parent_source):
                target = destination / original.name
                target.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(original,target)
                copied.append({'file':target.name, 'source':f"{parent_package}@{package['version']} parent package", 'sha256':digest(target)})
        if not copied and package['name'] == 'lerc' and package['license'] == 'Apache-2.0':
            copied = [standard_license('Apache-2.0',destination)]
            shutil.copyfile(source / 'LercDecode.js',destination / 'LercDecode-original-source.js')
            shutil.copyfile(source / 'package.json',destination / 'published-package.json')
        if not copied:
            gaps.append(f'NPM {label}: no license text found')
        records.append({'ecosystem':'npm', 'name':package['name'], 'version':package['version'],
                        'license':package.get('license', locked.get('license')), 'texts':copied,
                        'directory':relative(destination, payload)})
    fonts = notices / 'fonts'
    fonts.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(ROOT / 'prototype/public/fonts/LICENSE-Inter.txt', fonts / 'LICENSE-Inter.txt')
    shutil.copyfile(ROOT / 'docs/releases/THIRD-PARTY-ASSETS.md', notices / 'ASSETS.md')
    shutil.copyfile(ROOT / 'prototype/public/samples/manifest.json', notices / 'sample-data-provenance.json')
    standard_license('CC-BY-SA-4.0',notices)
    write_json(notices / 'inventory.json', {'scope':'Resolved Rust graph and installed npm graph, including build-time and optional dependencies; not a claim every listed package is linked', 'packages':records, 'missingLicenseTexts':gaps})
    if gaps:
        raise RuntimeError('License text coverage is incomplete; packaging stopped:\n' + '\n'.join(gaps))
    return len(records)


def manifest_files(directory):
    return [{'path':relative(path, directory), 'bytes':path.stat().st_size, 'sha256':digest(path)}
            for path in sorted(directory.rglob('*')) if path.is_file() and path.name != 'release-manifest.json']


def verify_archive(path):
    with zipfile.ZipFile(path) as archive:
        members = archive.namelist()
        def safe_relative(name):
            return isinstance(name, str) and bool(name) and not PurePosixPath(name).is_absolute() and ':' not in name and '\\' not in name and all(part not in ('', '.', '..') for part in name.split('/'))
        if len(set(members)) != len(members) or not all(safe_relative(info.orig_filename) and info.orig_filename == info.filename for info in archive.infolist()):
            raise ValueError('Unsafe or duplicate archive member')
        roots = {name.split('/')[0] for name in members}
        if len(roots) != 1 or any(len(name.split('/')) < 2 for name in members):
            raise ValueError('Archive requires one relative root directory')
        manifests = [name for name in members if name.endswith('/release-manifest.json')]
        if len(manifests) != 1 or len(manifests[0].split('/')) != 2:
            raise ValueError('Archive requires one release manifest')
        manifest = json.loads(archive.read(manifests[0]))
        paths = [item['path'] for item in manifest['files']]
        if len(set(paths)) != len(paths) or not all(safe_relative(name) for name in paths):
            raise ValueError('Unsafe or duplicate manifest path')
        prefix = manifests[0].removesuffix('release-manifest.json')
        expected = {prefix + item['path'] for item in manifest['files']} | {manifests[0]}
        if expected != set(members):
            raise ValueError('Archive files differ from manifest')
        for item in manifest['files']:
            data = archive.read(prefix + item['path'])
            if len(data) != item['bytes'] or hashlib.sha256(data).hexdigest() != item['sha256']:
                raise ValueError(f"Archive hash mismatch: {item['path']}")
        return manifest


def nsis_escape(value):
    return str(value).replace('$', '$$').replace('"', '$\\"')


def build_installer(payload, output, version):
    compiler = shutil.which('makensis.exe') or shutil.which('makensis')
    if not compiler:
        candidates = [Path(os.environ.get('ProgramFiles(x86)', 'C:/Program Files (x86)')) / 'NSIS/makensis.exe']
        compiler = next((str(path) for path in candidates if path.is_file()), None)
    if not compiler:
        raise RuntimeError('NSIS is not installed. Use --installer none to generate only the portable ZIP; this script never installs tools.')
    include = payload.parent / 'uninstall-files.nsh'
    files = sorted(path for path in payload.rglob('*') if path.is_file())
    lines = [f'  Delete "$INSTDIR\\{nsis_escape(relative(path, payload).replace(chr(47), chr(92)))}"' for path in files]
    directories = sorted((path for path in payload.rglob('*') if path.is_dir()), key=lambda path:len(path.parts), reverse=True)
    lines += [f'  RMDir "$INSTDIR\\{nsis_escape(relative(path, payload).replace(chr(47), chr(92)))}"' for path in directories]
    include.write_text('\n'.join(lines) + '\n', encoding='utf-8')
    command(compiler, '/INPUTCHARSET', 'UTF8', '/DPAYLOAD=' + str(payload), '/DOUTPUT=' + str(output),
            '/DAPP_VERSION=' + version, '/DUNINSTALL_FILES=' + str(include), str(ROOT / 'scripts/package-windows.nsi'))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--profile', choices=['debug', 'release'], default='release')
    parser.add_argument('--installer', choices=['nsis', 'none'], default='nsis')
    parser.add_argument('--output', type=Path, default=ROOT / '.verification/packages')
    parser.add_argument('--verify-zip', type=Path)
    phase = parser.add_mutually_exclusive_group()
    phase.add_argument('--build-only', action='store_true')
    phase.add_argument('--package-only', action='store_true')
    args = parser.parse_args()
    if args.verify_zip:
        print(json.dumps({'verified':True, 'version':verify_archive(args.verify_zip)['version']}))
        return
    if sys.platform != 'win32':
        raise RuntimeError('Windows packaging must run on Windows')
    rust = command('rustc', '-vV', capture=True)
    if f'host: {TARGET}' not in rust:
        raise RuntimeError('This packager currently supports native Windows x64 MSVC builds only')
    config = json.loads((ROOT / 'src-tauri/tauri.conf.json').read_text(encoding='utf-8'))
    package = json.loads((ROOT / 'package.json').read_text(encoding='utf-8'))
    if config['identifier'] != 'xyz.laogao.geod.global' or config['version'] != package['version']:
        raise RuntimeError('Global application identity or package versions do not match')
    if not args.package_only: build_binaries(args.profile,rust)
    receipt = verified_build_receipt(args.profile)
    if args.build_only:
        print(json.dumps({'buildReceipt':str(build_receipt_path(args.profile)), 'binaries':receipt['binaries']},indent=2))
        return
    identity = source_identity()
    stamp = time.strftime('%Y%m%dT%H%M%SZ', time.gmtime())
    name = f"GeoD-Global_{config['version']}_windows-x64_{args.profile}_{identity['commit'][:10]}{'-dirty' if identity['dirty'] else ''}_{stamp}"
    destination = args.output.resolve() / name
    destination.mkdir(parents=True, exist_ok=False)
    # Keep staging paths short on Windows; the ZIP still carries the versioned root name.
    payload = destination / 'GeoD-Global'
    payload.mkdir()
    for item in receipt['binaries']:
        binary = item['file']
        original = ROOT / 'target' / TARGET / args.profile / binary
        copy_verified_binary(original, payload / binary, item)
    shutil.copytree(ROOT / 'examples', payload / 'examples')
    shutil.copytree(ROOT / 'docs/workflows', payload / 'docs/workflows')
    shutil.copyfile(ROOT / 'docs/mcp.md', payload / 'docs/mcp.md')
    shutil.copytree(ROOT / 'schemas', payload / 'schemas')
    shutil.copyfile(ROOT / 'crates/geod-runtime/README.md', payload / 'docs/runtime.md')
    shutil.copyfile(ROOT / 'docs/releases/WINDOWS-README.md', payload / 'README.md')
    shutil.copyfile(ROOT / 'docs/releases/FIRST-PARTY-NOTICE.txt', payload / 'FIRST-PARTY-NOTICE.txt')
    dependencies = collect_notices(payload)
    if source_identity() != identity:
        raise RuntimeError('Source changed during packaging. Freeze changes and rebuild; these files are not a final release.')
    signatures = {name:signature(payload / name) for name in ['geod-global-desktop.exe','geod-runtime.exe']}
    native_imports = {name:pe_imports(payload / name) for name in signatures}
    if any(name.lower().startswith(('vcruntime','msvcp')) for names in native_imports.values() for name in names):
        raise RuntimeError('Static CRT packaging still imports a Visual C++ runtime DLL; do not distribute this incomplete portable package')
    cli_help = json.loads(command(str(payload / 'geod-runtime.exe'), '--help', capture=True))
    if 'commands' not in cli_help: raise RuntimeError('Packaged CLI --help smoke check failed')
    if any(value['status'] != 'NotSigned' for value in signatures.values()):
        raise RuntimeError('This evaluation packager expects unsigned binaries; review signing policy before proceeding')
    manifest = {'schemaVersion':'geod-windows-release/v1', 'product':'GeoD Global', 'version':config['version'],
                'identifier':config['identifier'], 'channel':'local-evaluation', 'createdAt':stamp,
                'target':TARGET, 'profile':args.profile, 'source':identity,
                'build':{**receipt,'cliHelpSmoke':'passed'},
                'signatures':signatures, 'nativeDllImports':native_imports, 'thirdPartyPackageCount':dependencies,
                'runtimeRequirements':['Windows 10/11 x64', 'Microsoft Edge WebView2 Evergreen runtime'],
                'userDataPolicy':'Retain application-local data when uninstalling or deleting the portable folder',
                'distributionStatus':'Unsigned local evaluation; no public publication or remote CI result implied',
                'files':manifest_files(payload)}
    write_json(payload / 'release-manifest.json', manifest)
    portable = destination / (name + '.zip')
    with zipfile.ZipFile(portable, 'w', compression=zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
        for path in sorted(payload.rglob('*')):
            if path.is_file(): archive.write(path, name + '/' + relative(path,payload))
    verify_archive(portable)
    artifacts = [portable]
    if args.installer == 'nsis':
        installer = destination / (name + '-setup.exe')
        build_installer(payload, installer, config['version'])
        if signature(installer)['status'] != 'NotSigned':
            raise RuntimeError('Unexpected installer signing status; review before distribution')
        artifacts.append(installer)
    summary = {'schemaVersion':'geod-windows-artifacts/v1', 'version':config['version'], 'sourceCommit':identity['commit'],
               'sourceTreeSha256':identity['treeSha256'], 'dirty':identity['dirty'], 'signatureStatus':'unsigned',
               'artifacts':[{'file':path.name,'bytes':path.stat().st_size,'sha256':digest(path)} for path in artifacts],
               'verification':{'portableArchiveEveryFile':'passed','installerExecuted':False,'nativeUIAcceptance':'not asserted by packaging'}}
    write_json(destination / 'artifacts.json', summary)
    (destination / 'SHA256SUMS.txt').write_text(''.join(f"{item['sha256']}  {item['file']}\n" for item in summary['artifacts']), encoding='utf-8')
    print(json.dumps({'output':str(destination), **summary}, indent=2))


if __name__ == '__main__':
    try:
        main()
    except Exception as error:
        print(json.dumps({'error':str(error)}, ensure_ascii=False), file=sys.stderr)
        sys.exit(1)
