"""Privilege boundary and cleanup tests; all external commands are mocked."""
import contextlib
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('owned_loss', ROOT / 'scripts/test/run-linux-owned-socket-loss.py')
runner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runner)
artifact_spec = importlib.util.spec_from_file_location('transport_artifact', ROOT / 'scripts/test/cargo-transport-test-executable.py')
artifact = importlib.util.module_from_spec(artifact_spec)
artifact_spec.loader.exec_module(artifact)

class OwnedSocketRunnerTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.binary = self.root / 'trusted-elf'
        self.binary.write_bytes(b'\x7fELFfixture')
        self.configuration = self.root / 'cert.cnf'
        self.configuration.write_text('fixture')
        self.digest = hashlib.sha256(self.binary.read_bytes()).hexdigest()
        self.commands = []
        self.fail_test = False
        self.bad_copy = False
        self.missing_test = False
    def command(self, command):
        self.commands.append(command)
        if command[-1] == '--list':
            tests = runner.TESTS[:1] if self.missing_test else runner.TESTS
            return '\n'.join(test + ': test' for test in tests)
        if '/usr/bin/mktemp' in command:
            return '/tmp/plank-owned-loss.ABCDef12\n'
        if '/usr/bin/sha256sum' in command:
            return ('0' * 64 if self.bad_copy else self.digest) + '  root-owned-copy\n'
        if command[:2] == ['openssl', 'x509']:
            Path(command[command.index('-out') + 1]).write_bytes(b'DER-fixture')
        if '/usr/bin/env' in command:
            if self.fail_test:
                raise subprocess.CalledProcessError(101, command, output='preserved failing fixture evidence')
            return 'fec_loss_owned_udp_kernel_drops=0 owned_kernel_loss_free=true\nfec_loss_matrix_frames=900 source=220500 missing=1 unrecovered=0\ntest result: ok. 1 passed;\n'
        return ''
    def qualify(self):
        with patch.object(runner.sys, 'platform', 'linux'), patch.object(runner.os, 'geteuid', return_value=1000), patch.object(runner, 'run_command', side_effect=self.command), contextlib.redirect_stdout(io.StringIO()):
            runner.qualify(self.binary, self.configuration, self.digest)
    def test_only_exact_allowlisted_tests_and_four_environment_inputs(self):
        self.qualify()
        elevated = [command for command in self.commands if '/usr/bin/env' in command]
        self.assertEqual(len(elevated), 6)
        self.assertEqual([command[command.index('--exact') + 1] for command in elevated], list(runner.TESTS) * 3)
        for command in elevated:
            self.assertEqual(command[:5], ['sudo', '-n', '--', '/usr/bin/env', '-i'])
            self.assertEqual({argument.split('=', 1)[0] for argument in command[5:9]}, runner.FIXTURE_ENV)
            self.assertNotIn('cargo', command)
            self.assertIn('/tmp/plank-owned-loss.ABCDef12/loopback-test', command)
        self.assertIn('/usr/bin/rm', self.commands[-2])
        self.assertIn('/usr/bin/rmdir', self.commands[-1])
    def test_failure_stops_without_retry_and_cleans_root_copy(self):
        self.fail_test = True
        with self.assertRaises(subprocess.CalledProcessError):
            self.qualify()
        self.assertEqual(sum('/usr/bin/env' in command for command in self.commands), 1)
        self.assertIn('/usr/bin/rm', self.commands[-2])
        self.assertIn('/usr/bin/rmdir', self.commands[-1])
    def test_wrong_hash_never_invokes_privileged_commands(self):
        self.digest = '0' * 64
        with self.assertRaises(ValueError):
            self.qualify()
        self.assertEqual(self.commands, [])
    def test_rejects_other_tests_and_extra_environment(self):
        fixture = {name: 'fixture' for name in runner.FIXTURE_ENV}
        with self.assertRaises(ValueError):
            runner.privileged_test_command(self.binary, 'native_ffi::anything', fixture)
        with self.assertRaises(ValueError):
            runner.privileged_test_command(self.binary, runner.TESTS[0], dict(fixture, LD_PRELOAD='forbidden'))
    def test_changed_root_copy_is_never_executed_and_is_cleaned(self):
        self.bad_copy = True
        with self.assertRaises(ValueError):
            self.qualify()
        self.assertFalse(any('/usr/bin/env' in command for command in self.commands))
        self.assertIn('/usr/bin/rm', self.commands[-2])
        self.assertIn('/usr/bin/rmdir', self.commands[-1])
    def test_missing_allowlisted_test_prevents_elevation(self):
        self.missing_test = True
        with self.assertRaises(ValueError):
            self.qualify()
        self.assertFalse(any(command[0] == 'sudo' for command in self.commands))
    def test_entire_runner_cannot_be_launched_as_root(self):
        with patch.object(runner.sys, 'platform', 'linux'), patch.object(runner.os, 'geteuid', return_value=0), patch.object(runner, 'run_command') as execute:
            with self.assertRaises(ValueError):
                runner.qualify(self.binary, self.configuration, self.digest)
            execute.assert_not_called()

    def test_cargo_artifact_selection_ignores_unrelated_cached_profiles(self):
        def message(name, test, executable):
            return json.dumps({'reason': 'compiler-artifact', 'target': {'name': name}, 'profile': {'test': test}, 'executable': executable})
        selected = message('plank_transport', True, '/shared/current-test')
        self.assertEqual(artifact.select_executable([
            message('plank_transport', False, '/shared/old-product'),
            message('wire_vectors', True, '/shared/other-test'), selected, selected,
        ]), '/shared/current-test')
        with self.assertRaises(ValueError):
            artifact.select_executable([selected, message('plank_transport', True, '/shared/different-test')])
        with self.assertRaises(ValueError):
            artifact.select_executable([])

if __name__ == '__main__':
    unittest.main()
