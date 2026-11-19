import importlib.util
import json
from pathlib import Path
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('release_windows', Path(__file__).resolve().parents[1] / 'scripts/release_windows.py')
release = importlib.util.module_from_spec(spec)
spec.loader.exec_module(release)


class QualificationBoundary(unittest.TestCase):
    def test_failed_running_and_wrong_commit_runs_are_rejected(self):
        for status, conclusion, commit in [('completed', 'failure', 'reviewed'),
                                           ('in_progress', None, 'reviewed'),
                                           ('completed', 'success', 'other')]:
            result = dict(status=status, conclusion=conclusion, head_sha=commit)
            with self.subTest(result=result), patch.object(release, 'gh', return_value=json.dumps(result)):
                with self.assertRaisesRegex(ValueError, 'exact-commit qualification'):
                    release.checked_run('123', 'reviewed')
