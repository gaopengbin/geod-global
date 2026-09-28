"""Packaging unit tests use synthetic files, never distributable app binaries."""
import importlib.util
import hashlib
import io
import json
from pathlib import Path
import struct
import tempfile
import unittest
from unittest import mock
import zipfile

spec = importlib.util.spec_from_file_location('package_windows', Path(__file__).with_name('package-windows.py'))
packaging = importlib.util.module_from_spec(spec)
spec.loader.exec_module(packaging)


class PackagingTests(unittest.TestCase):
    @staticmethod
    def vendored_sources(root):
        for name, expected in packaging.VENDORED_UI.items():
            source = root / 'third-party' / name
            source.mkdir(parents=True)
            (source / expected['licenseFile']).write_text('MIT license test fixture\n', encoding='utf-8')
            (source / 'component.jsx').write_text('// Original source test fixture\n', encoding='utf-8')
            (source / 'SOURCE.md').write_text(f"Source: {expected['repository']} at {'a' * 40}\n", encoding='utf-8')
            raw_base = expected['repository'].replace('https://github.com/', 'https://raw.githubusercontent.com/')
            provenance = {'repository': expected['repository'], 'commit': 'a' * 40, 'license': 'MIT', 'files': [
                {'path': filename, 'url': f"{raw_base}/{'a' * 40}/{filename}", 'sha256': packaging.digest(source / filename)}
                for filename in [expected['licenseFile'], 'component.jsx']
            ]}
            packaging.write_json(source / 'provenance.json', provenance)

    def test_vendored_license_and_source_records_are_in_delivery(self):
        with tempfile.TemporaryDirectory() as folder, mock.patch.object(packaging, 'ROOT', Path(folder)):
            root = Path(folder)
            self.vendored_sources(root)
            payload = root / 'payload'
            records = packaging.collect_vendored_notices(payload)
            self.assertEqual({record['name'] for record in records}, set(packaging.VENDORED_UI))
            for record in records:
                source = root / 'third-party' / record['name']
                directory = payload / record['directory']
                self.assertEqual({path.name for path in directory.iterdir()}, {packaging.VENDORED_UI[record['name']]['licenseFile'], 'SOURCE.md', 'provenance.json'})
                for copied in record['texts'] + record['sourceRecords']:
                    self.assertEqual((directory / copied['file']).read_bytes(), (source / copied['file']).read_bytes())
                    self.assertEqual(packaging.digest(directory / copied['file']), copied['sha256'])

    def test_vendored_missing_or_empty_required_notices_stop_packaging(self):
        for filename in ['LICENSE', 'SOURCE.md', 'provenance.json']:
            for empty in [False, True]:
                with self.subTest(filename=filename, empty=empty), tempfile.TemporaryDirectory() as folder, mock.patch.object(packaging, 'ROOT', Path(folder)):
                    root = Path(folder)
                    self.vendored_sources(root)
                    path = root / 'third-party/beautiful-ui' / filename
                    if empty:
                        path.write_text('', encoding='utf-8')
                    else:
                        path.unlink()
                    with self.assertRaisesRegex(RuntimeError, 'notice is missing or empty'):
                        packaging.collect_vendored_notices(root / 'payload')

    def test_vendored_unreviewed_or_changed_sources_stop_packaging(self):
        for change, expected in [('unknown-source', 'Unreviewed or missing'), ('unknown-license', 'license is unreviewed'), ('unrecorded-file', 'Unrecorded'), ('changed-license', 'checksum mismatch'), ('missing-license-record', 'license is missing from provenance')]:
            with self.subTest(change=change), tempfile.TemporaryDirectory() as folder, mock.patch.object(packaging, 'ROOT', Path(folder)):
                root = Path(folder)
                self.vendored_sources(root)
                source = root / 'third-party/beautiful-ui'
                path = source / 'provenance.json'
                provenance = json.loads(path.read_text(encoding='utf-8'))
                if change == 'unknown-source':
                    (root / 'third-party/unreviewed-library').mkdir()
                elif change == 'unknown-license':
                    provenance['license'] = 'UNLICENSED'
                elif change == 'unrecorded-file':
                    (source / 'another-license.txt').write_text('Unreviewed notice', encoding='utf-8')
                elif change == 'changed-license':
                    (source / 'LICENSE').write_text('Changed permission terms', encoding='utf-8')
                else:
                    provenance['files'] = [entry for entry in provenance['files'] if entry['path'] != 'LICENSE']
                packaging.write_json(path, provenance)
                with self.assertRaisesRegex(RuntimeError, expected):
                    packaging.collect_vendored_notices(root / 'payload')

    @staticmethod
    def license_response(data=b'license text', length=None):
        response = io.BytesIO(data)
        response.headers = {} if length is None else {'Content-Length':str(length)}
        return response

    def test_license_retries_transient_http_and_network_errors_then_succeeds(self):
        url = 'https://example.test/LICENSE'
        errors = [packaging.urllib.error.HTTPError(url, code, 'temporary', {}, None) for code in [408,429,500,503,599]]
        errors += [packaging.urllib.error.URLError('connection reset'), TimeoutError('timed out')]
        for error in errors:
            with self.subTest(error=error), mock.patch.object(packaging.urllib.request,'urlopen',side_effect=[error,error,self.license_response()]) as request, mock.patch.object(packaging.time,'sleep') as sleep:
                self.assertEqual(packaging.download_license(url,100),b'license text')
                self.assertEqual(request.call_count,3)
                self.assertTrue(all(call.kwargs['timeout'] == 20 for call in request.call_args_list))
                self.assertEqual(sleep.call_args_list,[mock.call(1),mock.call(2)])

    def test_license_retry_exhaustion_does_not_cache_partial_result(self):
        with tempfile.TemporaryDirectory() as folder, mock.patch.object(packaging,'ROOT',Path(folder)), mock.patch.object(packaging.urllib.request,'urlopen',side_effect=packaging.urllib.error.URLError('offline')) as request, mock.patch.object(packaging.time,'sleep'):
            with self.assertRaises(packaging.urllib.error.URLError):
                packaging.standard_license('MPL-2.0',Path(folder) / 'notices')
            self.assertEqual(request.call_count,3)
            self.assertFalse(list(Path(folder).rglob('*.txt')))

    def test_license_404_and_other_permanent_http_errors_are_not_retried(self):
        url = 'https://example.test/LICENSE'
        for code in [400,401,403,404,410]:
            with self.subTest(code=code), mock.patch.object(packaging.urllib.request,'urlopen',side_effect=packaging.urllib.error.HTTPError(url,code,'permanent',{},None)) as request, mock.patch.object(packaging.time,'sleep') as sleep:
                with self.assertRaises(packaging.urllib.error.HTTPError): packaging.download_license(url,100)
                request.assert_called_once()
                sleep.assert_not_called()

    def test_license_size_limit_rejects_oversized_chunked_and_declared_responses(self):
        for response in [self.license_response(b'123456'),self.license_response(b'',length=6)]:
            with self.subTest(response=response), mock.patch.object(packaging.urllib.request,'urlopen',return_value=response) as request, mock.patch.object(packaging.time,'sleep') as sleep:
                with self.assertRaisesRegex(ValueError,'size limit'): packaging.download_license('https://example.test/LICENSE',5)
                request.assert_called_once()
                sleep.assert_not_called()

    def test_license_short_response_is_retried_and_only_complete_body_returned(self):
        with mock.patch.object(packaging.urllib.request,'urlopen',side_effect=[self.license_response(b'par',length=12),self.license_response(b'license text',length=12)]) as request, mock.patch.object(packaging.time,'sleep'):
            self.assertEqual(packaging.download_license('https://example.test/LICENSE',12),b'license text')
            self.assertEqual(request.call_count,2)

    def test_spdx_cache_is_pinned_to_verified_commit_and_does_not_reuse_old_cache(self):
        with tempfile.TemporaryDirectory() as folder, mock.patch.object(packaging,'ROOT',Path(folder)), mock.patch.object(packaging.urllib.request,'urlopen',return_value=self.license_response()) as request:
            old = Path(folder) / '.verification/license-cache/spdx/MPL-2.0.txt'
            old.parent.mkdir(parents=True)
            old.write_bytes(b'old floating main cache')
            destination = Path(folder) / 'notices'
            record = packaging.standard_license('MPL-2.0',destination)
            expected_url = 'https://raw.githubusercontent.com/spdx/license-list-data/31ba1a50e5397e00a304dbadc76531740e89ee48/text/MPL-2.0.txt'
            self.assertEqual(request.call_args.args[0].full_url,expected_url)
            self.assertEqual(record['source'],expected_url)
            self.assertEqual((destination / 'MPL-2.0.txt').read_bytes(),b'license text')
            self.assertEqual((old.parent / packaging.SPDX_LICENSE_LIST_COMMIT / 'MPL-2.0.txt').read_bytes(),b'license text')
            packaging.standard_license('MPL-2.0',destination)
            request.assert_called_once()

    def test_pinned_npm_license_checks_download_and_cached_bytes(self):
        body = b'Official license fixture\n'
        expected = {'version': '1.2.4', 'license': 'MIT', 'url': 'https://example.test/commit/LICENSE',
                    'sha256': hashlib.sha256(body).hexdigest()}
        with tempfile.TemporaryDirectory() as folder, mock.patch.object(packaging, 'ROOT', Path(folder)), \
                mock.patch.dict(packaging.NPM_UPSTREAM_LICENSES, {'@napi-rs/wasm-runtime': expected}), \
                mock.patch.object(packaging, 'download_license', return_value=body) as download:
            package = {'name': '@napi-rs/wasm-runtime', 'version': '1.2.4', 'license': 'MIT'}
            destination = Path(folder) / 'notices'
            record = packaging.pinned_npm_license(package, destination)[0]
            self.assertEqual(record['source'], expected['url'])
            self.assertEqual((destination / 'LICENSE').read_bytes(), body)
            packaging.pinned_npm_license(package, destination)
            download.assert_called_once_with(expected['url'], 1024 * 1024)
            cache = Path(folder) / '.verification/license-cache/npm/@napi-rs_wasm-runtime/1.2.4/LICENSE'
            cache.write_bytes(b'altered cache')
            with self.assertRaisesRegex(RuntimeError, 'Cached npm license checksum mismatch'):
                packaging.pinned_npm_license(package, destination)

    def test_pinned_npm_license_rejects_unreviewed_metadata_and_response(self):
        with tempfile.TemporaryDirectory() as folder, mock.patch.object(packaging, 'ROOT', Path(folder)), \
                mock.patch.object(packaging, 'download_license', return_value=b'wrong license'):
            package = {'name': 'saxes', 'version': '6.0.0', 'license': 'ISC'}
            for key, changed in [('version', '6.0.1'), ('license', 'MIT')]:
                with self.subTest(key=key), self.assertRaisesRegex(RuntimeError, 'Unreviewed npm license/version'):
                    packaging.pinned_npm_license({**package, key: changed}, Path(folder) / 'notices')
            with self.assertRaisesRegex(RuntimeError, 'Upstream npm license checksum mismatch'):
                packaging.pinned_npm_license(package, Path(folder) / 'notices')
            self.assertFalse(list((Path(folder) / '.verification').rglob('LICENSE')))

    def test_upstream_missing_license_remains_missing_and_404_cache_is_reused(self):
        with tempfile.TemporaryDirectory() as folder, mock.patch.object(packaging,'ROOT',Path(folder)), mock.patch.object(packaging.urllib.request,'urlopen',side_effect=packaging.urllib.error.HTTPError('https://example.test/LICENSE',404,'missing',{},None)) as request, mock.patch.object(packaging.time,'sleep') as sleep:
            source = Path(folder) / 'crate'
            source.mkdir()
            (source / '.cargo_vcs_info.json').write_text(json.dumps({'git':{'sha1':'1' * 40}}),encoding='utf-8')
            package = {'repository':'https://github.com/example/crate'}
            self.assertEqual(packaging.fetch_upstream_licenses(package,source,Path(folder) / 'notices'),[])
            attempts = request.call_count
            self.assertGreater(attempts,0)
            self.assertEqual(len(list((Path(folder) / '.verification/license-cache').rglob('*.missing'))),attempts)
            self.assertEqual(packaging.fetch_upstream_licenses(package,source,Path(folder) / 'notices'),[])
            self.assertEqual(request.call_count,attempts)
            sleep.assert_not_called()

    def test_build_freeze_includes_runtime_inputs_but_not_delivery_docs(self):
        for name in ['prototype/src/main.jsx','prototype/public/fonts/Inter.woff2','prototype/public/terms.md','src-tauri/tauri.conf.json','src-tauri/src/main.rs','crates/geod-runtime/src/lib.rs','schemas/raster-recipe-v1.schema.json','Cargo.lock','package-lock.json']:
            self.assertTrue(packaging.is_build_input(name),name)
        for name in ['docs/releases/acceptance.md','README.md','prototype/README.md','prototype/qa/result.png','src-tauri/README.md','examples/clip.recipe.json']:
            self.assertFalse(packaging.is_build_input(name),name)

    def test_cargo_build_pins_output_despite_environment_target_dir(self):
        with mock.patch.dict(packaging.os.environ, {'CARGO_TARGET_DIR':'some-other-target'}):
            args = packaging.cargo_build_command('release')
            self.assertEqual(args[args.index('--target-dir')+1],str(packaging.ROOT / 'target'))
            self.assertEqual(args[args.index('--target')+1],packaging.TARGET)
            self.assertIn('--release',args)

    def test_pe_requires_x64_and_valid_header(self):
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / 'fixture-only.exe'
            data = bytearray(128)
            data[:2] = b'MZ'
            data[0x3c:0x40] = struct.pack('<I',64)
            data[64:70] = b'PE\0\0' + struct.pack('<H',0x8664)
            path.write_bytes(data)
            packaging.verify_pe(path)
            data[68:70] = struct.pack('<H',0x14c)
            path.write_bytes(data)
            with self.assertRaises(ValueError): packaging.verify_pe(path)

    def test_binary_copy_must_match_receipt_after_copy(self):
        with tempfile.TemporaryDirectory() as folder:
            source = Path(folder) / 'source.exe'
            destination = Path(folder) / 'copied.exe'
            source.write_bytes(b'new binary after receipt validation')
            expected = {'file':'source.exe','bytes':3,'sha256':'0' * 64}
            with self.assertRaisesRegex(RuntimeError,'differs from build receipt'):
                packaging.copy_verified_binary(source,destination,expected)

    def test_archive_checks_every_file_and_rejects_tampering(self):
        with tempfile.TemporaryDirectory() as folder:
            directory = Path(folder)
            payload = directory / 'fixture-only'
            payload.mkdir()
            (payload / 'not-an-application.txt').write_text('fixture',encoding='utf-8')
            manifest = {'version':'fixture-only','files':packaging.manifest_files(payload)}
            packaging.write_json(payload / 'release-manifest.json',manifest)
            archive = directory / 'fixture.zip'
            with zipfile.ZipFile(archive,'w') as handle:
                for path in payload.iterdir(): handle.write(path,'fixture-only/' + path.name)
            self.assertEqual(packaging.verify_archive(archive)['version'],'fixture-only')
            with zipfile.ZipFile(archive,'w') as handle:
                handle.writestr('fixture-only/release-manifest.json',json.dumps(manifest))
                handle.writestr('fixture-only/not-an-application.txt','modified')
            with self.assertRaises(ValueError): packaging.verify_archive(archive)

    def test_archive_rejects_extra_and_traversing_files(self):
        with tempfile.TemporaryDirectory() as folder:
            archive = Path(folder) / 'fixture.zip'
            for name in ['../escape.txt', 'fixture/extra.txt']:
                with zipfile.ZipFile(archive,'w') as handle:
                    handle.writestr('fixture/release-manifest.json',json.dumps({'files':[]}))
                    handle.writestr(name,'fixture')
                with self.assertRaises(ValueError): packaging.verify_archive(archive)

    def test_archive_rejects_non_relative_roots_and_ambiguous_paths(self):
        with tempfile.TemporaryDirectory() as folder:
            archive = Path(folder) / 'fixture.zip'
            for name in ['C:/outside/release-manifest.json','C:outside/release-manifest.json','/root/release-manifest.json','root\\release-manifest.json','./root/release-manifest.json','root//release-manifest.json','root/../release-manifest.json','root/nested/release-manifest.json']:
                with self.subTest(name=name):
                    with zipfile.ZipFile(archive,'w') as handle:
                        info = zipfile.ZipInfo('placeholder')
                        info.filename = name  # Preserve raw Windows separators for this malicious ZIP fixture.
                        handle.writestr(info,json.dumps({'files':[]}))
                    with self.assertRaises(ValueError): packaging.verify_archive(archive)

    def test_uninstaller_has_no_recursive_or_appdata_delete(self):
        script = Path(__file__).with_name('package-windows.nsi').read_text(encoding='utf-8')
        self.assertNotIn('RMDir /r',script)
        self.assertNotIn('RmDir /r',script)
        self.assertIn('RequestExecutionLevel user',script)
        self.assertNotIn('RequestExecutionLevel admin',script)
        self.assertIn('xyz.laogao.geod.global',script)
        self.assertNotIn('Delete "$LOCALAPPDATA',script)
        self.assertNotIn('Delete "$APPDATA',script)
        self.assertIn('ReadRegStr $0 HKCU "${UNINSTALL_KEY}" "InstallLocation"',script)
        self.assertIn('${If} $0 == "$INSTDIR"\n    Delete "$SMPROGRAMS\\GeoD Global.lnk"\n    DeleteRegKey HKCU "${UNINSTALL_KEY}"\n  ${EndIf}',script)

    def test_nsis_paths_escape_macro_characters(self):
        self.assertEqual(packaging.nsis_escape('a$b"c'),'a$$b$\\"c')

    def test_nsis_numeric_version_accepts_semver_prerelease_and_metadata(self):
        for version, expected in [
            ('0.1.0','0.1.0.0'), ('0.0.0','0.0.0.0'),
            ('0.1.0-rc.1','0.1.0.0'), ('1.2.3+build.001','1.2.3.0'),
            ('1.2.3-alpha.0.x-y+build.001.sha','1.2.3.0'),
            ('65535.65535.65535-rc.1+build.0','65535.65535.65535.0'),
        ]:
            with self.subTest(version=version):
                self.assertEqual(packaging.nsis_numeric_version(version),expected)

    def test_nsis_numeric_version_rejects_invalid_injected_and_out_of_range_values(self):
        versions = [None,123,'','v1.2.3','1.2','1.2.3.4','01.2.3','1.02.3','1.2.03',
                    '-1.2.3','1.2.3-01','1.2.3-rc..1','1.2.3+','1.2.3-',
                    '1.2.3+${INJECT}','1.2.3"','1.2.3\n!include evil.nsh',
                    '1.2.3\n','１.2.3','65536.0.0','0.65536.0','0.0.65536',
                    '1' * 5000 + '.0.0']
        for version in versions:
            with self.subTest(version=repr(version)[:80]):
                with self.assertRaises(ValueError): packaging.nsis_numeric_version(version)

    def test_invalid_version_stops_every_packaging_phase_before_build_or_subprocess(self):
        with tempfile.TemporaryDirectory() as folder, mock.patch.object(packaging,'ROOT',Path(folder)), mock.patch.object(packaging.sys,'platform','win32'), mock.patch.object(packaging,'command') as command, mock.patch.object(packaging,'build_binaries') as build, mock.patch.object(packaging,'verified_build_receipt') as receipt:
            packaging.write_json(Path(folder) / 'src-tauri/tauri.conf.json',{'identifier':'xyz.laogao.geod.global','version':'0.1.0-rc.01'})
            packaging.write_json(Path(folder) / 'package.json',{'version':'0.1.0-rc.01'})
            for phase in [[],['--build-only'],['--package-only']]:
                with self.subTest(phase=phase), mock.patch.object(packaging.sys,'argv',['package-windows.py',*phase]):
                    with self.assertRaisesRegex(ValueError,'strict SemVer'): packaging.main()
            command.assert_not_called()
            build.assert_not_called()
            receipt.assert_not_called()

    def test_installer_passes_full_display_and_separate_numeric_version(self):
        with tempfile.TemporaryDirectory() as folder, mock.patch.object(packaging.shutil,'which',return_value='fixture-makensis'), mock.patch.object(packaging,'command') as command:
            payload = Path(folder) / 'payload'
            payload.mkdir()
            (payload / 'fixture.txt').write_text('fixture',encoding='utf-8')
            packaging.build_installer(payload,Path(folder) / 'fixture-only.exe','0.1.0-rc.1+test.007')
            args = command.call_args.args
            self.assertIn('/DAPP_VERSION=0.1.0-rc.1+test.007',args)
            self.assertIn('/DAPP_NUMERIC_VERSION=0.1.0.0',args)
            script = Path(__file__).with_name('package-windows.nsi').read_text(encoding='utf-8')
            self.assertIn('VIProductVersion "${APP_NUMERIC_VERSION}"',script)
            self.assertIn('VIAddVersionKey "FileVersion" "${APP_VERSION}"',script)


if __name__ == '__main__': unittest.main()
