"""Exercise cached submodule seeding against real temporary Git repositories."""
import os
from pathlib import Path
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / 'scripts/build/init-host-candidate-submodules.sh'


class HostSubmoduleSeedTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()
        self.env = dict(os.environ, GIT_CONFIG_NOSYSTEM='1', GIT_CONFIG_GLOBAL=os.devnull)
        self.dependency = self.root / 'dependency'
        self.canonical = self.root / 'canonical'
        self.candidate = self.root / 'candidate'
        for repository in (self.dependency, self.canonical):
            self.git(self.root, 'init', str(repository))
            self.git(repository, 'config', 'user.name', 'Tests')
            self.git(repository, 'config', 'user.email', 'tests@example.org')
        (self.dependency / 'marker').write_text('pinned dependency\n')
        self.git(self.dependency, 'add', 'marker')
        self.git(self.dependency, 'commit', '-m', 'Initial fixture')
        self.pin = self.git(self.dependency, 'rev-parse', 'HEAD').stdout.strip()
        self.git(self.canonical, '-c', 'protocol.file.allow=always', 'submodule', 'add',
                 str(self.dependency), 'third-party/example')
        self.git(self.canonical, 'commit', '-am', 'Pin dependency')
        self.git(self.root, 'clone', str(self.canonical), str(self.candidate))

    def git(self, repository, *arguments, check=True):
        return subprocess.run(['git', '-C', str(repository), *arguments], env=self.env,
                              capture_output=True, text=True, check=check)

    def seed(self):
        return subprocess.run(['bash', str(SCRIPT), str(self.canonical), str(self.candidate)],
                              env=self.env, capture_output=True, text=True)

    def test_empty_canonical_submodule_is_rejected_before_candidate_clone(self):
        self.git(self.canonical, 'submodule', 'deinit', 'third-party/example')
        child = self.canonical / 'third-party/example'
        # The old inside-worktree check incorrectly succeeds in this directory.
        self.assertEqual(self.git(child, 'rev-parse', '--is-inside-work-tree').stdout.strip(), 'true')
        result = self.seed()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('Canonical submodule repository is unavailable:', result.stderr)
        self.assertFalse((self.candidate / '.git/modules/third-party/example').exists())
        self.assertNotEqual(self.git(self.candidate, 'config', '--get',
                                    'submodule.third-party/example.url', check=False).returncode, 0)

    def test_real_cached_submodule_is_pinned_and_repeated_seed_is_unchanged(self):
        for _ in range(2):
            result = self.seed()
            self.assertEqual(result.returncode, 0, result.stderr)
            child = self.candidate / 'third-party/example'
            self.assertEqual(Path(self.git(child, 'rev-parse', '--show-toplevel').stdout.strip()), child)
            self.assertEqual(self.git(child, 'rev-parse', 'HEAD').stdout.strip(), self.pin)
            self.assertEqual(self.git(self.candidate, 'status', '--porcelain').stdout, '')


if __name__ == '__main__':
    unittest.main()
