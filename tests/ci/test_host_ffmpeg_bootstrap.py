"""Exercise the dependency recipe against an interrupted configure cache."""
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]


class HostFfmpegBootstrapTests(unittest.TestCase):
    def test_partial_sources_are_prepared_without_rebuilding_cached_libraries(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            deps = root / 'apps/host/linux/third-party/build-deps'
            (deps / 'build/FFmpeg/FFmpeg').mkdir(parents=True)
            prefix = root / 'cache/host-ffmpeg'
            (prefix / 'lib').mkdir(parents=True)
            (prefix / 'lib/libavcodec.a').touch()
            verifier = root / 'scripts/build/verify-host-dependency-patches.sh'
            verifier.parent.mkdir(parents=True)
            verifier.write_text('test -f "$1/cmake_install.cmake"\n')
            binary = root / 'bin/cmake'
            binary.parent.mkdir()
            binary.write_text('#!/bin/bash\nset -eu\n'
                              'echo "$1" >> "$RECIPE_CALLS"\n'
                              'test "$1" = -S\n'
                              'touch "$4/cmake_install.cmake"\n')
            binary.chmod(0o755)
            calls = root / 'calls'
            env = dict(os.environ, PLANK_SOURCE_ROOT=str(root),
                       PLANK_DEP_ROOT=str(root / 'cache'), RECIPE_CALLS=str(calls),
                       PATH=f'{binary.parent}:{os.environ["PATH"]}')
            command = ['bash', str(ROOT / 'scripts/ci/dependencies/host-ffmpeg.sh')]
            subprocess.run(command, env=env, check=True)
            subprocess.run(command, env=env, check=True)
            self.assertEqual(calls.read_text().splitlines(), ['-S'])


if __name__ == '__main__':
    unittest.main()
