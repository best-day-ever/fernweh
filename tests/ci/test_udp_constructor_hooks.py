"""Exercise constructor invocation, output placement and mandatory test results."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]


@unittest.skipUnless(os.name == "posix", "POSIX build entrypoints")
class UdpConstructorHookTests(unittest.TestCase):
    def run_hook(self, hook, mode):
        with tempfile.TemporaryDirectory(prefix="plank-udp-hook-") as directory:
            fixture = Path(directory).resolve()
            source = fixture / "source"
            home = fixture / "home"
            tools = fixture / "tools"
            tools.mkdir()
            home.mkdir()
            for name in (
                "scripts/build/macos-client-target.sh",
                "scripts/build/build-paths.sh",
                "scripts/package/package-version.sh",
            ):
                destination = source / name
                destination.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(ROOT / name, destination)
            (source / "packaging").mkdir()
            (source / "packaging/VERSION").write_text("1.2.37\n")

            def executable(name, body):
                path = tools / name
                path.write_text("#!" + sys.executable + "\n" + body)
                path.chmod(0o755)

            executable("cargo", """
import json, os
from pathlib import Path
import sys
args = sys.argv[1:]
manifest = Path(args[args.index('--manifest-path') + 1])
target = (Path(args[args.index('--target-dir') + 1]) if '--target-dir' in args
          else manifest.parent / 'target')
target.mkdir(parents=True, exist_ok=True)
(target / 'constructor-output').write_text('fixture')
Path(os.environ['HOOK_REQUEST']).write_text(json.dumps({
    'args': args, 'cwd': os.getcwd(), 'target': str(target),
    'jobs': os.environ.get('CARGO_BUILD_JOBS')}))
print('test result: ok. ' + ('0' if os.environ['HOOK_MODE'] == 'zero' else '2') + ' passed;')
sys.exit(5 if os.environ['HOOK_MODE'] == 'failure' else 0)
""")
            executable("python3", """
import os
from pathlib import Path
import sys
if sys.argv[1].endswith(('test-client-policy-upgrade.py', 'deployment/test_configure.py')):
    Path(os.environ['HOOK_NEXT_STAGE']).write_text('reached')
    sys.exit(73)
""")
            executable("qmake", "print('6.10.2')\n")
            executable("rustc", "print('rustc 1.99.1 (fixture)')\n")
            executable("uname", "import sys; print('Darwin' if sys.argv[1] == '-s' else 'arm64')\n")
            executable("xcrun", "import sys; print('27.0' if '--show-sdk-version' in sys.argv else '/example/sdk')\n")
            request = fixture / "request.json"
            next_stage = fixture / "next-stage"
            env = dict(os.environ,
                       PATH=str(tools) + os.pathsep + os.environ["PATH"],
                       HOME=str(home), PLANK_SOURCE_ROOT=str(source),
                       PLANK_WORK_ROOT=str(fixture / "work"),
                       PLANK_DEP_ROOT=str(fixture / "dependencies"),
                       PLANK_MAC_CLIENT_DEPS=str(fixture / "mac-dependencies"),
                       PLANK_QT_ROOT=str(fixture / "qt"),
                       PLANK_RUSTUP_ROOT=str(fixture / "rustup"),
                       PLANK_CARGO_ROOT=str(fixture / "cargo"),
                       PLANK_BUILD_BRANCH="fixture", FERNWEH_BROKER_ENABLED="0",
                       PLANK_MAC_CLIENT_MIN_MACOS="15.0",
                       HOOK_MODE=mode, HOOK_REQUEST=str(request),
                       HOOK_NEXT_STAGE=str(next_stage),
                       CARGO_TARGET_DIR=str(source / "unwanted-target"))
            env.pop("CARGO_ENCODED_RUSTFLAGS", None)
            if hook == "ci":
                command = ["bash", str(ROOT / "scripts/ci/build.sh"), "linux-client"]
            else:
                command = ["bash", str(ROOT / "scripts/build/build-macos-client.sh"),
                           str(source), str(fixture / "work")]
            result = subprocess.run(command, cwd=fixture, env=env,
                                    capture_output=True, text=True, timeout=15)
            self.assertTrue(request.exists(), result.stderr)
            invocation = json.loads(request.read_text())
            self.assertEqual(invocation["cwd"], str(source))
            self.assertEqual(invocation["jobs"], "2")
            self.assertEqual(invocation["args"][invocation["args"].index("--manifest-path") + 1],
                             str(source / "third_party/quinn-udp-0.5.16/Cargo.toml"))
            for option in ("--locked", "--offline", "--release"):
                self.assertIn(option, invocation["args"])
            self.assertEqual(invocation["args"][invocation["args"].index("--features") + 1],
                             "plank-bounded-receive-buffer")
            self.assertIn("plank_receive_buffer", invocation["args"])
            if mode == "success":
                self.assertEqual(result.returncode, 73, result.stderr)
                self.assertTrue(next_stage.exists())
                self.assertEqual(invocation["target"], str(home / ".cargo/shared-target"))
                self.assertFalse((source / "third_party/quinn-udp-0.5.16/target").exists())
                self.assertFalse((source / "unwanted-target").exists())
            else:
                self.assertNotEqual(result.returncode, 0)
                self.assertNotEqual(result.returncode, 73)
                self.assertFalse(next_stage.exists())

    def test_ci_uses_shared_target_and_preserves_constructor_contract(self):
        self.run_hook("ci", "success")

    def test_macos_uses_shared_target_and_preserves_constructor_contract(self):
        self.run_hook("macos", "success")

    def test_ci_rejects_zero_matching_tests(self):
        self.run_hook("ci", "zero")

    def test_macos_rejects_zero_matching_tests(self):
        self.run_hook("macos", "zero")

    def test_ci_propagates_failed_constructor(self):
        self.run_hook("ci", "failure")

    def test_macos_propagates_failed_constructor(self):
        self.run_hook("macos", "failure")


if __name__ == "__main__":
    unittest.main()
