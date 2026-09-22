"""Release gate regression tests: all executables are harmless synthetic bytes."""
import copy
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest import mock
import zipfile

spec = importlib.util.spec_from_file_location('geod_release', Path(__file__).with_name('release.py'))
release = importlib.util.module_from_spec(spec)
spec.loader.exec_module(release)

COMMIT = 'a' * 40
OTHER_COMMIT = 'd' * 40
TREE = 'b' * 64


def write_json(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2) + '\n', encoding='utf-8')


def hash_bytes(value):
    return hashlib.sha256(value).hexdigest()


def repository(root, version='0.1.0'):
    root.mkdir(parents=True, exist_ok=True)
    write_json(root / 'package.json', {'name': 'geod-global', 'version': version})
    write_json(root / 'package-lock.json', {'name': 'geod-global', 'version': version, 'packages': {'': {'name': 'geod-global', 'version': version}}})
    write_json(root / 'src-tauri/tauri.conf.json', {'identifier': release.IDENTIFIER, 'version': version})
    (root / 'Cargo.toml').write_text('[workspace]\nmembers=["crates/geod-runtime", "src-tauri"]\n', encoding='utf-8')
    for filename, name in [('crates/geod-runtime/Cargo.toml', 'geod-runtime'), ('src-tauri/Cargo.toml', 'geod-global-desktop')]:
        target = root / filename
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(f'[package]\nname="{name}"\nversion="{version}"\n', encoding='utf-8')
    (root / 'Cargo.lock').write_text('\n'.join(f'[[package]]\nname="{name}"\nversion="{version}"\n' for name in ['geod-global-desktop', 'geod-runtime']), encoding='utf-8')


def package(directory, version='0.1.0', mutate_manifest=None, extra_member=None):
    directory.mkdir(parents=True, exist_ok=True)
    zip_name = f'GeoD-Global_{version}_windows-x64_release_fixture.zip'
    setup_name = zip_name.removesuffix('.zip') + '-setup.exe'
    contents = {name: ('NONEXECUTABLE TEST FIXTURE: ' + name).encode() for name in sorted(release.BINARIES)}
    contents['README.md'] = b'This archive contains no runnable software.'
    files = [{'path': name, 'bytes': len(data), 'sha256': hash_bytes(data)} for name, data in contents.items()]
    manifest = {
        'schemaVersion': 'geod-windows-release/v1', 'product': 'GeoD Global', 'identifier': release.IDENTIFIER,
        'version': version, 'target': release.TARGET, 'profile': 'release',
        'source': {'commit': COMMIT, 'dirty': False, 'treeSha256': TREE},
        'build': {'schemaVersion': 'geod-windows-build/v1', 'target': release.TARGET, 'profile': 'release',
                  'targetRustflags': '-Ctarget-feature=+crt-static',
                  'source': {'commit': COMMIT, 'dirty': False, 'treeSha256': 'c' * 64},
                  'binaries': [{'file': name, 'bytes': len(contents[name]), 'sha256': hash_bytes(contents[name])} for name in sorted(release.BINARIES)]},
        'signatures': {name: {'status': 'NotSigned', 'subject': None} for name in sorted(release.BINARIES)}, 'files': files,
    }
    if mutate_manifest:
        mutate_manifest(manifest)
    with zipfile.ZipFile(directory / zip_name, 'w', compression=zipfile.ZIP_DEFLATED) as archive:
        archive.writestr('GeoD-Global/release-manifest.json', json.dumps(manifest))
        for name, data in contents.items():
            archive.writestr('GeoD-Global/' + name, data)
        if extra_member:
            archive.writestr(extra_member, b'not allowed')
    (directory / setup_name).write_bytes(b'NONEXECUTABLE INSTALLER TEST FIXTURE')
    summary = {
        'schemaVersion': 'geod-windows-artifacts/v1', 'version': version, 'sourceCommit': COMMIT,
        'sourceTreeSha256': TREE, 'dirty': False, 'signatureStatus': 'unsigned',
        'artifacts': [{'file': name, 'bytes': (directory / name).stat().st_size, 'sha256': release.digest(directory / name)} for name in [zip_name, setup_name]],
    }
    save_summary(directory, summary)
    return summary


def save_summary(directory, summary):
    write_json(directory / 'artifacts.json', summary)
    (directory / 'SHA256SUMS.txt').write_bytes(''.join(f"{record['sha256']}  {record['file']}\n" for record in summary['artifacts']).encode())


class ReleaseTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.directory = Path(self.temporary.name)
        self.root = self.directory / 'repository'
        repository(self.root)
        self.package_root = self.directory / 'packages'
        self.source = self.package_root / 'only-completed-package'
        self.destination = self.directory / 'staged'

    def test_versions_match_all_eight_locations_and_strict_prerelease_tags(self):
        for version in ['0.1.0', '1.2.3-rc.1', '1.2.3-alpha-beta.0+build.3']:
            repository(self.root, version)
            self.assertEqual(release.repository_version(self.root, 'v' + version), version)
        for version in ['01.2.3', '1.02.3', '1.2.3-01', '1.2.3-', '1.2.3;', '1.2.3$(echo bad)', '1.2.3\n', '1.2.3-alpha..1']:
            with self.subTest(version=version), self.assertRaises(ValueError):
                release.valid_version(version)
        repository(self.root)
        for tag in ['0.1.0', 'v0.1.1', 'v0.1.0; echo bad', 'v0.1.0\nnext=value']:
            with self.assertRaisesRegex(ValueError, 'Tag must'):
                release.repository_version(self.root, tag)

    def test_each_version_source_mismatch_is_rejected(self):
        json_changes = [('package.json', ['version']), ('package-lock.json', ['version']),
                        ('package-lock.json', ['packages', '', 'version']), ('src-tauri/tauri.conf.json', ['version'])]
        for filename, keys in json_changes:
            with self.subTest(filename=filename, keys=keys):
                repository(self.root)
                document = json.loads((self.root / filename).read_text(encoding='utf-8'))
                target = document
                for key in keys[:-1]: target = target[key]
                target[keys[-1]] = '9.9.9'
                write_json(self.root / filename, document)
                with self.assertRaisesRegex(ValueError, 'Version mismatch'):
                    release.repository_version(self.root)
        for filename in ['crates/geod-runtime/Cargo.toml', 'src-tauri/Cargo.toml', 'Cargo.lock']:
            with self.subTest(filename=filename):
                repository(self.root)
                path = self.root / filename
                path.write_text(path.read_text(encoding='utf-8').replace('0.1.0', '9.9.9', 1), encoding='utf-8')
                with self.assertRaisesRegex(ValueError, 'Version mismatch'):
                    release.repository_version(self.root)

    def test_identity_and_duplicate_or_remote_local_crates_are_rejected(self):
        config = self.root / 'src-tauri/tauri.conf.json'
        write_json(config, {'identifier': 'domestic.other', 'version': '0.1.0'})
        with self.assertRaisesRegex(ValueError, 'identifier'): release.repository_version(self.root)
        repository(self.root)
        path = self.root / 'Cargo.lock'
        path.write_text(path.read_text(encoding='utf-8') + '[[package]]\nname="geod-runtime"\nversion="0.1.0"\nsource="registry+https://example.invalid"\n', encoding='utf-8')
        with self.assertRaisesRegex(ValueError, 'exactly one local'): release.repository_version(self.root)

    def test_check_requires_expected_head_and_clean_worktree_when_requested(self):
        with mock.patch.object(release, 'git', side_effect=[COMMIT, '']):
            result = release.check(self.root, 'v0.1.0', COMMIT, True)
        self.assertEqual(result['versionsChecked'], 8)
        self.assertFalse(result['dirty'])
        with mock.patch.object(release, 'git', return_value=COMMIT), self.assertRaisesRegex(ValueError, 'HEAD'):
            release.check(self.root, commit=OTHER_COMMIT)
        with mock.patch.object(release, 'git', side_effect=[COMMIT, '?? secret.txt']), self.assertRaisesRegex(ValueError, 'uncommitted'):
            release.check(self.root, require_clean=True)
        for commit in ['a' * 7, COMMIT.upper(), '--help', COMMIT + '\n']:
            with self.assertRaises(ValueError): release.valid_commit(commit)

    def test_stage_copies_only_four_allowlisted_files_and_can_be_reverified(self):
        summary = package(self.source)
        (self.source / 'GeoD-Global').mkdir()
        (self.source / 'GeoD-Global' / 'private.log').write_text('do not upload', encoding='utf-8')
        (self.source / 'uninstall-files.nsh').write_text('not a release asset', encoding='utf-8')
        result = release.stage(self.package_root, self.destination, COMMIT, 'v0.1.0', self.root)
        expected = {record['file'] for record in summary['artifacts']} | release.METADATA_FILES
        self.assertEqual(set(result['files']), expected)
        self.assertEqual({path.name for path in self.destination.iterdir()}, expected)
        self.assertEqual(release.verify(self.destination, COMMIT, 'v0.1.0', self.root)['signatureStatus'], 'unsigned')
        for name in expected:
            self.assertEqual((self.destination / name).read_bytes(), (self.source / name).read_bytes())

    def test_package_selection_rejects_none_nested_and_multiple_completed_packages(self):
        self.package_root.mkdir()
        with self.assertRaisesRegex(ValueError, 'exactly one direct child'):
            release.stage(self.package_root, self.destination, COMMIT, root=self.root)
        package(self.package_root / 'nested' / 'too-deep')
        with self.assertRaisesRegex(ValueError, 'exactly one direct child'):
            release.stage(self.package_root, self.destination, COMMIT, root=self.root)
        package(self.source)
        package(self.package_root / 'second-completed')
        with self.assertRaisesRegex(ValueError, 'exactly one direct child'):
            release.stage(self.package_root, self.destination, COMMIT, root=self.root)
        self.assertFalse(self.destination.exists())

    def test_destination_is_never_overwritten_and_staged_extras_are_rejected(self):
        package(self.source)
        self.destination.mkdir()
        keep = self.destination / 'keep.txt'
        keep.write_text('user data', encoding='utf-8')
        with self.assertRaisesRegex(ValueError, 'never overwritten'):
            release.stage(self.package_root, self.destination, COMMIT, root=self.root)
        self.assertEqual(keep.read_text(encoding='utf-8'), 'user data')
        empty = self.directory / 'empty-existing'
        empty.mkdir()
        release.stage(self.package_root, empty, COMMIT, root=self.root)
        (empty / 'extra.txt').write_text('unlisted', encoding='utf-8')
        with self.assertRaisesRegex(ValueError, 'exactly the four'):
            release.verify(empty, COMMIT, root=self.root)

    def test_post_copy_verification_rejects_bytes_changed_while_staging(self):
        package(self.source)
        def altered_copy(reader, writer, length):
            data = reader.read()
            writer.write(data + b'changed' if str(reader.name).endswith('-setup.exe') else data)
        with mock.patch.object(release.shutil, 'copyfileobj', side_effect=altered_copy):
            with self.assertRaisesRegex(ValueError, 'byte count or SHA-256 mismatch'):
                release.stage(self.package_root, self.destination, COMMIT, root=self.root)

    def test_cli_stage_and_verify_emit_only_verified_output(self):
        package(self.source)
        output = self.directory / 'github-output'
        stdout = io.StringIO()
        with mock.patch.object(release, 'repository_version', return_value='0.1.0'), mock.patch.dict(os.environ, {'GITHUB_OUTPUT': str(output)}), mock.patch('sys.stdout', stdout):
            release.main(['stage', '--package-root', str(self.package_root), '--destination', str(self.destination), '--commit', COMMIT, '--tag', 'v0.1.0'])
            release.main(['verify', '--directory', str(self.destination), '--commit', COMMIT, '--tag', 'v0.1.0'])
        responses = [json.loads(line) for line in stdout.getvalue().splitlines()]
        self.assertEqual(len(responses), 2)
        self.assertTrue(all(response['verified'] and response['version'] == '0.1.0' for response in responses))
        self.assertEqual(output.read_text(encoding='utf-8').count('directory='), 2)

    def test_rejects_changed_zip_installer_size_and_hash(self):
        for index in [0, 1]:
            for change in ['bytes', 'file']:
                with self.subTest(index=index, change=change):
                    summary = package(self.source)
                    record = summary['artifacts'][index]
                    if change == 'bytes':
                        record['bytes'] += 1
                        save_summary(self.source, summary)
                    else:
                        path = self.source / record['file']
                        data = bytearray(path.read_bytes())
                        data[-1] ^= 1
                        path.write_bytes(data)
                    with self.assertRaisesRegex(ValueError, 'byte count or SHA-256 mismatch'):
                        release.verify_directory(self.source, COMMIT, '0.1.0')

    def test_checksum_file_is_exact_except_platform_newline(self):
        package(self.source)
        checksums = self.source / 'SHA256SUMS.txt'
        correct = checksums.read_bytes()
        checksums.write_bytes(correct.replace(b'\n', b'\r\n'))
        release.verify_directory(self.source, COMMIT, '0.1.0')
        for changed in [correct[:-1], correct + b'\n', b'\n'.join(reversed(correct.rstrip(b'\n').split(b'\n'))) + b'\n', correct.replace(b'  ', b' '), correct + b'0' * 64 + b'  extra.exe\n']:
            checksums.write_bytes(changed)
            with self.assertRaisesRegex(ValueError, 'SHA256SUMS.txt must exactly match'):
                release.verify_directory(self.source, COMMIT, '0.1.0')

    def test_summary_clean_commit_version_schema_and_unsigned_gate(self):
        original = package(self.source)
        for key, value in [('dirty', True), ('dirty', 0), ('sourceCommit', OTHER_COMMIT), ('version', '9.9.9'), ('signatureStatus', 'signed'), ('schemaVersion', 'unknown/v9')]:
            with self.subTest(key=key, value=value):
                changed = copy.deepcopy(original)
                changed[key] = value
                save_summary(self.source, changed)
                with self.assertRaises(ValueError): release.verify_directory(self.source, COMMIT, '0.1.0')

    def test_artifact_names_cannot_escape_and_pair_must_be_complete(self):
        original = package(self.source)
        for name in ['../escape.zip', 'C:/escape.zip', 'x\\escape.zip', '/escape.zip', 'bad\nname.zip', '-option.zip']:
            changed = copy.deepcopy(original)
            changed['artifacts'][0]['file'] = name
            save_summary(self.source, changed)
            with self.subTest(name=name), self.assertRaisesRegex(ValueError, 'plain safe filename'):
                release.verify_directory(self.source, COMMIT, '0.1.0')
        for records in [original['artifacts'][:1], original['artifacts'] * 2, [original['artifacts'][0]] * 2]:
            changed = copy.deepcopy(original)
            changed['artifacts'] = records
            save_summary(self.source, changed)
            with self.assertRaises(ValueError): release.verify_directory(self.source, COMMIT, '0.1.0')

    def test_embedded_provenance_identity_profile_and_binary_receipt_must_agree(self):
        changes = [
            (['source', 'commit'], OTHER_COMMIT), (['source', 'dirty'], True),
            (['source', 'treeSha256'], 'f' * 64), (['build', 'source', 'commit'], OTHER_COMMIT),
            (['build', 'source', 'dirty'], True), (['version'], '9.9.9'), (['profile'], 'debug'),
            (['target'], 'aarch64-pc-windows-msvc'), (['identifier'], 'domestic.app'),
            (['product'], 'Some other product'), (['build', 'profile'], 'debug'),
            (['build', 'targetRustflags'], ''), (['build', 'binaries', 0, 'sha256'], '0' * 64),
            (['signatures', 'geod-runtime.exe', 'status'], 'Valid'),
        ]
        for keys, value in changes:
            def mutate(manifest):
                target = manifest
                for key in keys[:-1]: target = target[key]
                target[keys[-1]] = value
            with self.subTest(keys=keys, value=value):
                package(self.source, mutate_manifest=mutate)
                with self.assertRaises(ValueError): release.verify_directory(self.source, COMMIT, '0.1.0')

    def test_archive_verifier_is_used_even_when_outer_artifact_hashes_match(self):
        package(self.source, extra_member='../../escape.txt')
        with self.assertRaisesRegex(ValueError, 'Unsafe or duplicate archive'):
            release.verify_directory(self.source, COMMIT, '0.1.0')
        with mock.patch.object(release, 'MAX_EXPANDED_ZIP_BYTES', 1), self.assertRaisesRegex(ValueError, 'resource limits'):
            release.verify_directory(self.source, COMMIT, '0.1.0')

    def test_redirected_artifact_is_rejected(self):
        summary = package(self.source)
        filename = summary['artifacts'][1]['file']
        target = self.source / filename
        outside = self.directory / 'outside-fixture.exe'
        target.replace(outside)
        try:
            target.symlink_to(outside)
        except (OSError, NotImplementedError):
            self.skipTest('Host does not permit filesystem symlink fixtures')
        with self.assertRaisesRegex(ValueError, 'redirected artifact'):
            release.verify_directory(self.source, COMMIT, '0.1.0')

    def test_duplicate_json_keys_are_rejected(self):
        package(self.source)
        path = self.source / 'artifacts.json'
        text = path.read_text(encoding='utf-8')
        path.write_text(text.replace('"dirty": false', '"dirty": true, "dirty": false'), encoding='utf-8')
        with self.assertRaisesRegex(ValueError, 'Duplicate JSON key'):
            release.verify_directory(self.source, COMMIT, '0.1.0')

    def test_github_outputs_are_written_only_when_configured_and_cannot_inject_lines(self):
        output = self.directory / 'github-output'
        result = {'directory': str(self.destination), 'version': '0.1.0'}
        with mock.patch.dict(os.environ, {}, clear=True):
            release.emit_outputs(result)
        self.assertFalse(output.exists())
        with mock.patch.dict(os.environ, {'GITHUB_OUTPUT': str(output)}):
            release.emit_outputs(result)
            self.assertEqual(output.read_text(encoding='utf-8'), f'version=0.1.0\ndirectory={self.destination}\n')
            with self.assertRaisesRegex(ValueError, 'single-line'):
                release.emit_outputs({'directory': 'normal\nevil=value', 'version': '0.1.0'})


if __name__ == '__main__':
    unittest.main()
