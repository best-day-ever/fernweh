"""Audit the non-importable GitHub cache workflow contract."""
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]


class DependencyCacheTests(unittest.TestCase):
    def test_workflow_uses_exact_component_cache_for_each_product(self):
        workflow = (ROOT / '.github/workflows/build.yml').read_text()
        for job, next_job in (('linux-host', 'linux-client'),
                              ('linux-client', 'macos'),
                              ('macos', 'macos-signed')):
            section = workflow.split(f'  {job}:\n', 1)[1].split(f'  {next_job}:\n', 1)[0]
            self.assertEqual(section.count('uses: ./.github/actions/dependency-cache'), 2, job)
            self.assertIn('operation: restore', section, job)
            self.assertIn('operation: save', section, job)
            self.assertIn("github.event_name != 'pull_request'", section, job)
            self.assertLess(section.index('operation: restore'), section.index('bash scripts/ci/bootstrap.sh'), job)
            self.assertLess(section.index('bash scripts/ci/bootstrap.sh'), section.index('operation: save'), job)
            self.assertLess(section.index('operation: save'), section.index('bash scripts/ci/build.sh'), job)
        signed = workflow.split('  macos-signed:\n', 1)[1]
        self.assertEqual(signed.count('uses: ./.github/actions/dependency-cache'), 2)
        self.assertLess(signed.index('operation: restore'), signed.index('operation: save'))
        self.assertLess(signed.index('operation: save'), signed.index('Build, sign, notarize and verify'))

    def test_composite_cache_rejects_partial_or_untrusted_restores(self):
        action = (ROOT / '.github/actions/dependency-cache/action.yml').read_text()
        for component in ('rust', 'cargo', 'ffmpeg', 'boost', 'native', 'qt'):
            self.assertIn(f'outputs.{component}-key', action)
            self.assertIn(f'CACHE_MATCHED_{component.upper()}', action)
            self.assertIn(f'outputs.{component}-hit', action)
        self.assertEqual(action.count('actions/cache/restore@'), 6)
        self.assertEqual(action.count('actions/cache/save@'), 6)
        self.assertNotIn('restore-keys:', action)
        self.assertIn('cache-dependencies.py verify', action)
        self.assertIn('cache-dependencies.py seal', action)
        self.assertIn('Pull requests cannot save dependency caches', action)


if __name__ == '__main__':
    unittest.main()
