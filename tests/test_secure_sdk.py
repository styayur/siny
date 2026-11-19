import importlib.util
import json
from pathlib import Path
import tarfile
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('secure_sdk', Path(__file__).resolve().parents[1] / 'scripts/prepare_native.py')
sdk = importlib.util.module_from_spec(spec)
spec.loader.exec_module(sdk)


class SecureSdkBoundary(unittest.TestCase):
    def test_rejects_corrupt_sdk_before_opening_archive(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / 'Cargo.lock').write_text('[[package]]\nname="sherpa-onnx-sys"\nversion="1.13.8+tinytts.zip9.1"\n')
            (root / 'docs').mkdir()
            manifest = dict(ort_version='1.28.3', sherpa_abi='1.13.8', archive='sdk.tar.bz2', archive_bytes=3, archive_sha256='0'*64)
            (root / 'docs/native-sdk.json').write_text(json.dumps(manifest))
            (root / '.native').mkdir()
            (root / '.native/sdk.tar.bz2').write_bytes(b'bad')
            with patch.object(sdk, 'ROOT', root), patch.object(sdk.tarfile, 'open') as extract:
                with self.assertRaisesRegex(ValueError, 'identity mismatch'):
                    sdk.prepare()
                extract.assert_not_called()

    def test_rejects_traversal_links_and_duplicate_members(self):
        for name in ['sdk/../outside', '/sdk/x', 'sdk/..\\x', 'sdk/C:x', 'elsewhere/x']:
            with self.subTest(name=name), self.assertRaisesRegex(ValueError, 'Unsafe'):
                sdk.validate_members([tarfile.TarInfo(name)], 'sdk')
        link = tarfile.TarInfo('sdk/link')
        link.type = tarfile.SYMTYPE
        link.linkname = '../outside'
        for members in [[link], [tarfile.TarInfo('sdk/x'), tarfile.TarInfo('sdk/x')]]:
            with self.assertRaisesRegex(ValueError, 'Unsafe'):
                sdk.validate_members(members, 'sdk')
