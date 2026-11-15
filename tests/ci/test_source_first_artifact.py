"""Select the linked FEC dependency without guessing from a shared build cache."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]


class SourceFirstArtifactTests(unittest.TestCase):
    def run_gate(self, mode):
        with tempfile.TemporaryDirectory() as directory:
            work = Path(directory)
            tools = work / 'bin'
            tools.mkdir()
            target = work / 'shared-target'
            deps = target / 'release/deps'
            deps.mkdir(parents=True)
            current = deps / 'libraptorq-current.rlib'
            stale = deps / 'libraptorq-other-toolchain.rlib'
            current.touch()
            stale.touch()
            record = lambda path: {
                'reason': 'compiler-artifact', 'target': {'name': 'raptorq'},
                'filenames': [str(path)],
            }
            records = [record(current)]
            if mode == 'ambiguous':
                records.append(record(stale))
            elif mode == 'missing':
                current.unlink()
            elif mode == 'absent':
                records = []
            cargo = tools / 'cargo'
            cargo.write_text('''#!/usr/bin/env python3
import os, sys
from pathlib import Path
Path(os.environ['CARGO_ARGUMENTS']).write_text('\u005cn'.join(sys.argv[1:]))
print(os.environ['COMPILER_ARTIFACTS'])
''')
            rustc = tools / 'rustc'
            rustc.write_text('''#!/usr/bin/env python3
import os, sys
from pathlib import Path
Path(os.environ['RUST_ARGUMENTS']).write_text('\u005cn'.join(sys.argv[1:]))
output = Path(sys.argv[sys.argv.index('-o') + 1])
output.write_text('#!/bin/sh\\necho source_first_fixture=pass\\n')
output.chmod(0o755)
''')
            cargo.chmod(0o755)
            rustc.chmod(0o755)
            result = subprocess.run(
                ['bash', str(ROOT / 'scripts/test/run-source-first-fec.sh'), str(target)],
                env={**os.environ, 'PATH': str(tools) + os.pathsep + os.environ['PATH'],
                     'COMPILER_ARTIFACTS': '\n'.join(json.dumps(value) for value in records),
                     'CARGO_ARGUMENTS': str(work / 'cargo-args'),
                     'RUST_ARGUMENTS': str(work / 'rust-args'),
                     'PLANK_TRANSPORT_CARGO_FEATURES': 'quinn-telemetry'},
                capture_output=True, text=True)
            arguments = (work / 'rust-args').read_text() if (work / 'rust-args').exists() else ''
            self.assertFalse(list(target.glob('source-first-fec.*')))
            return result, arguments, str(current), (work / 'cargo-args').read_text()

    def test_shared_cache_uses_the_current_locked_product_artifact(self):
        result, arguments, current, cargo = self.run_gate('current')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn('raptorq=' + current, arguments)
        self.assertNotIn('other-toolchain', arguments)
        self.assertIn('--features\nquinn-telemetry', cargo)
        self.assertIn('--locked\n--offline', cargo)

    def test_ambiguous_or_unavailable_artifacts_stop_before_compilation(self):
        for mode in ('ambiguous', 'missing', 'absent'):
            with self.subTest(mode=mode):
                result, arguments, _, _ = self.run_gate(mode)
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual(arguments, '')


if __name__ == '__main__':
    unittest.main()
