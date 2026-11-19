import importlib.util
import json
import copy
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

    def desktop(self):
        return dict(application_commit='reviewed', binary_sha256='a'*64, platform='Windows 11 x64',
            os_build='26300', checks={key: True for key in release.DESKTOP_CHECKS},
            metrics={key: 1 for key in release.DESKTOP_METRICS},
            probes={'devices.exe': 'b'*64, 'clipboard.exe': 'c'*64})

    def test_desktop_result_cannot_qualify_a_different_binary_commit_or_probe(self):
        report = self.desktop()
        release.checked_desktop(report, 'reviewed', 'a'*64, report['probes'])
        for key in ['application_commit', 'binary_sha256', 'probes']:
            bad = copy.deepcopy(report)
            bad[key] = 'other'
            with self.subTest(key=key), self.assertRaisesRegex(ValueError, 'exact main'):
                release.checked_desktop(bad, 'reviewed', 'a'*64, report['probes'])

    def test_incomplete_or_unsanitized_desktop_evidence_is_rejected(self):
        report = self.desktop()
        for mutation in [lambda value: value['checks'].update(model_unload=False),
                         lambda value: value.update(clipboard_text='unreviewed fixture text'),
                         lambda value: value['metrics'].update(startup_ms=float('nan')),
                         lambda value: value['metrics'].update(max_pending_chunks=3),
                         lambda value: value.update(checks=[]),
                         lambda value: value.update(os_build=26300)]:
            bad = copy.deepcopy(report)
            mutation(bad)
            with self.assertRaises(ValueError):
                release.checked_desktop(bad, 'reviewed', 'a'*64, report['probes'])

    def test_existing_draft_or_tag_cannot_create_a_duplicate_release(self):
        release.reject_existing_identity([], [{'tag_name': 'v0.1.0-alpha.1'}])
        for refs, releases in [([{'ref': 'refs/tags/' + release.TAG}], []),
                               ([], [{'tag_name': release.TAG, 'draft': True}])]:
            with self.assertRaisesRegex(ValueError, 'already exists'):
                release.reject_existing_identity(refs, releases)
