#!/usr/bin/env python3
"""Exercise retained version-object invalidation without compiling the client."""
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
HELPER = ROOT / 'scripts/build/macos-client-version-cache.sh'


class VersionCache(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix='client version cache ')
        self.addCleanup(self.temporary.cleanup)
        self.build = Path(self.temporary.name)
        self.release = self.build / 'app/release'
        self.release.mkdir(parents=True)
        self.stamp = self.build / '.plank-client-compiled-version'
        self.populate()

    def populate(self):
        for name in ('main.o', 'systemproperties.o', 'session.o', 'main.d'):
            (self.release / name).write_text('retained artifact\n')

    def invoke(self, operation, version):
        subprocess.run(['bash', '-euc',
                        'source "$1"; "plank_macos_client_${2}_version" "$3" "$4"',
                        'test', str(HELPER), operation, str(self.build), version],
                       check=True, capture_output=True, text=True)

    def assert_invalidated(self):
        for name in ('main.o', 'systemproperties.o'):
            self.assertFalse((self.release / name).exists(), name)
        for name in ('session.o', 'main.d'):
            self.assertEqual((self.release / name).read_text(), 'retained artifact\n')
        self.assertFalse(self.stamp.exists())

    def test_unstamped_existing_cache_is_invalidated(self):
        self.invoke('prepare', '1.2.3-canary')
        self.assert_invalidated()

    def test_successful_same_version_preserves_objects(self):
        self.invoke('record', '1.2.3-canary')
        self.invoke('prepare', '1.2.3-canary')
        self.assertEqual(self.stamp.read_text(), '1.2.3-canary\n')
        for name in ('main.o', 'systemproperties.o', 'session.o', 'main.d'):
            self.assertEqual((self.release / name).read_text(), 'retained artifact\n')

    def test_branch_suffix_change_invalidates_version_objects(self):
        self.invoke('record', '1.2.3-canary')
        self.invoke('prepare', '1.2.3-canary-r2')
        self.assert_invalidated()

    def test_failed_partial_build_cannot_reuse_old_stamp_on_rollback(self):
        self.invoke('record', '1.2.3-canary')
        self.invoke('prepare', '1.2.3-canary-r2')
        # Simulate compilation followed by failure: no successful record call.
        self.populate()
        self.invoke('prepare', '1.2.3-canary')
        self.assert_invalidated()

    def test_truncated_stamp_invalidates_objects(self):
        self.stamp.write_text('1.2.')
        self.invoke('prepare', '1.2.3-canary')
        self.assert_invalidated()


if __name__ == '__main__':
    unittest.main()
