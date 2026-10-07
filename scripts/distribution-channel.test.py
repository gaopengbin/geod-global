import base64,importlib.util,json,unittest
from pathlib import Path
root=Path(__file__).resolve().parents[1]
spec=importlib.util.spec_from_file_location('distribution_channel',Path(__file__).with_name('distribution-channel.py'));module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
fixtures=root/'src-tauri/src/distribution/fixtures'

class ChannelTests(unittest.TestCase):
    def test_independent_product_https_and_public_key(self):
        channel={'product':module.PRODUCT,'endpoint':'https://example.test/global/latest.json','pubkey':(fixtures/'key.pub').read_text().strip(),'messagesEndpoint':'https://example.test/global/notifications.json'}
        self.assertEqual(module.public_channel(channel),channel)
        for change in [{'product':'GeoD Agent'},{'endpoint':'http://example.test/latest.json'},{'pubkey':'private or invalid'},{'messagesEndpoint':'javascript:alert(1)'}]:
            with self.subTest(change=change),self.assertRaises(ValueError):module.public_channel({**channel,**change})

    def test_manifest_uses_exact_signed_version_and_global_product(self):
        signature=(fixtures/'update.bin.sig').read_text().strip()
        result=module.update_manifest('0.1.1',signature,'https://example.test/global/setup.exe','Original notes')
        self.assertEqual(result['product'],module.PRODUCT);self.assertEqual(result['platforms']['windows-x86_64']['signature'],signature)
        with self.assertRaisesRegex(ValueError,'exactly this version'):module.update_manifest('0.2.0',signature,'https://example.test/global/setup.exe','Original notes')
        with self.assertRaises(ValueError):module.update_manifest('0.1.1',signature,'http://example.test/setup.exe','Notes')

    def test_legacy_unsigned_version_cannot_be_published_as_a_new_update(self):
        with self.assertRaisesRegex(ValueError,'exactly this version'):module.update_manifest('0.1.1',(fixtures/'legacy.sig').read_text().strip(),'https://example.test/global/setup.exe','Notes')

if __name__=='__main__':unittest.main()
