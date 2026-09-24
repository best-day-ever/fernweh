"""Workflow wiring and cache lifecycle regression tests."""
import importlib.util
import io
import json
from pathlib import Path
import re
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('cache_lifecycle', ROOT / 'scripts/ci/cache-dependencies.py')
cache = importlib.util.module_from_spec(spec)
spec.loader.exec_module(cache)


class DependencyCacheTests(unittest.TestCase):
    def test_workflow_has_exact_restore_and_trusted_save_for_every_product(self):
        workflow = (ROOT / '.github/workflows/build.yml').read_text()
        self.assertEqual(workflow.count('actions/cache/restore@55cc8345863c7cc4c66a329aec7e433d2d1c52a9'), 4)
        self.assertEqual(workflow.count('actions/cache/save@55cc8345863c7cc4c66a329aec7e433d2d1c52a9'), 4)
        self.assertNotIn('restore-keys:', workflow)
        self.assertEqual(workflow.count('test "$MATCHED_KEY" = "$CACHE_KEY"'), 4)
        unsigned = workflow.split('  macos-signed:')[0]
        for save in unsigned.split('- name: Save dependencies')[1:]:
            save = save.split('- name:')[0]
            self.assertIn("github.event_name != 'pull_request'", save)
            self.assertIn('!inputs.clean_bootstrap', save)
        signed = workflow.split('  macos-signed:')[1]
        self.assertLess(signed.index('Remove temporary signing material'), signed.index('Save dependencies'))


if __name__ == '__main__':
    unittest.main()
