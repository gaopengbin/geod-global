"""Packaging unit tests use synthetic files, never distributable app binaries."""
import importlib.util
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


if __name__ == '__main__': unittest.main()
