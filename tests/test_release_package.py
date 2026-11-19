import importlib.util
from pathlib import Path
import tempfile
import tomllib
import unittest
spec=importlib.util.spec_from_file_location('package_native',Path(__file__).resolve().parents[1]/'scripts/package_native.py')
package=importlib.util.module_from_spec(spec);spec.loader.exec_module(package)
class ReleaseTrustBoundary(unittest.TestCase):
    def test_notice_paths_cannot_escape_staging(self):
        with tempfile.TemporaryDirectory() as temporary:
            for name in ['../outside','/absolute','C:/drive','nested\\outside']:
                with self.assertRaises(ValueError):package.legal_path(Path(temporary),name)
            path=package.legal_path(Path(temporary),'component/LICENSE')
            self.assertTrue(path.resolve().is_relative_to(Path(temporary).resolve()))
    def test_vendor_configuration_survives_moving_source_bundle(self):
        configuration='[source.crates-io]\nreplace-with = "vendored-sources"\n[source.vendored-sources]\ndirectory = "D:/runner/temporary/source/vendor"\n'
        parsed=tomllib.loads(package.portable_vendor_config(configuration))
        self.assertEqual(parsed['source']['vendored-sources']['directory'],'vendor')
if __name__=='__main__':unittest.main()
