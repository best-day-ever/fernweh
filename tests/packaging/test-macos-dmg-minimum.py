#!/usr/bin/env python3
"""Run the production DMG target gate against finite, real pipe producers."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts/package/build-macos-client-dmg.sh"
TOOL = r'''#!/usr/bin/env python3
import json, os
from pathlib import Path
import sys
for value in json.loads(os.environ["PLANK_TEST_MINIMA"]):
    sys.stdout.write("Load command 1\n      cmd LC_BUILD_VERSION\n    minos " + value + "\n")
sys.stdout.flush()
for _ in range(int(os.environ.get("PLANK_TEST_PADDING_BLOCKS", "256"))):
    sys.stdout.write(" x" * 8192 + "\n")
sys.stdout.flush()
Path(os.environ["PLANK_TEST_TOOL_RECEIPT"]).write_text("output fully written")
sys.exit(int(os.environ.get("PLANK_TEST_TOOL_EXIT", "0")))
'''


class MacDmgMinimum(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="mac minimum pipe ")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.tool = self.root / "otool"
        self.tool.write_text(TOOL)
        self.tool.chmod(0o755)
        self.binary = self.root / "fixture binary"
        self.binary.touch()
        self.receipt = self.root / "tool-finished"

    def run_gate(self, minima=("15.0",), **environment):
        command = 'source "$1"; plank_macos_client_check_binary_target "$2" "$3"'
        return subprocess.run(["bash", "-c", command, "bash", str(SCRIPT),
            str(self.binary), "15.0"], capture_output=True, text=True, timeout=10,
            env={**os.environ, "PATH": str(self.root) + os.pathsep + os.environ["PATH"],
                 "PLANK_TEST_MINIMA": json.dumps(minima),
                 "PLANK_TEST_TOOL_RECEIPT": str(self.receipt), **environment})

    def test_large_producer_is_fully_consumed(self):
        result = self.run_gate()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.receipt.read_text(), "output fully written")

    def test_fixture_reproduces_the_early_exit_failure(self):
        command = ('set -o pipefail; otool -l "$1" | '
                   "awk '/LC_BUILD_VERSION/{f=1} f&&/minos/{print $2; exit}'")
        result = subprocess.run(["bash", "-c", command, "bash", str(self.binary)],
            capture_output=True, text=True, timeout=10,
            env={**os.environ, "PATH": str(self.root) + os.pathsep + os.environ["PATH"],
                 "PLANK_TEST_MINIMA": '["15.0"]',
                 "PLANK_TEST_TOOL_RECEIPT": str(self.receipt)})
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(result.stdout.strip(), "15.0")
        self.assertFalse(self.receipt.exists())

    def test_accepts_older_target(self):
        result = self.run_gate(("14.0",))
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_rejects_newer_target(self):
        self.assertNotEqual(self.run_gate(("15.1",)).returncode, 0)

    def test_rejects_missing_minimum(self):
        self.assertNotEqual(self.run_gate(()).returncode, 0)

    def test_preserves_first_minimum(self):
        result = self.run_gate(("15.0", "16.0"))
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_first_newer_minimum_remains_rejected(self):
        self.assertNotEqual(self.run_gate(("16.0", "14.0")).returncode, 0)

    def test_producer_error_still_propagates_after_valid_minimum(self):
        result = self.run_gate(PLANK_TEST_TOOL_EXIT="27", PLANK_TEST_PADDING_BLOCKS="0")
        self.assertEqual(result.returncode, 27, result.stderr)
        self.assertEqual(self.receipt.read_text(), "output fully written")


if __name__ == "__main__":
    unittest.main()
