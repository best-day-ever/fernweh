#!/usr/bin/env python3
"""Qualify only two trusted loopback tests with fixture-owned socket headroom."""
import argparse
import hashlib
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile

TESTS = (
    'native::tests::native_raptorq_survives_progressive_transport_loss_at_150_mbps',
    'native::tests::native_raptorq_survives_progressive_transport_loss_at_150_mbps_cbr',
)
FIXTURE_ENV = frozenset((
    'SC_NATIVE_TEST_CERTIFICATE', 'SC_NATIVE_TEST_PRIVATE_KEY',
    'SC_NATIVE_TEST_CERTIFICATE_SHA256', 'PLANK_TEST_FORCE_OWNED_RCVBUF',
))

def sha256(path):
    digest = hashlib.sha256()
    with path.open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            digest.update(block)
    return digest.hexdigest()

def run_command(command):
    return subprocess.run(command, text=True, stdout=subprocess.PIPE,
                          stderr=subprocess.STDOUT, check=True).stdout

def privileged_test_command(binary, test, fixture):
    if test not in TESTS or set(fixture) != FIXTURE_ENV:
        raise ValueError('Only the two exact loopback tests and four fixture inputs are allowed')
    return ['sudo', '-n', '--', '/usr/bin/env', '-i',
            *(name + '=' + fixture[name] for name in sorted(FIXTURE_ENV)),
            '/usr/bin/timeout', '--signal=TERM', '--kill-after=10s', '120s',
            str(binary), '--ignored', '--exact', test, '--nocapture', '--test-threads=1']

def qualify(binary, configuration, expected):
    if sys.platform != 'linux' or os.geteuid() == 0:
        raise ValueError('Launch this Linux runner as the ordinary build user; only selected tests use sudo')
    if not binary.is_absolute() or not configuration.is_absolute():
        raise ValueError('Absolute trusted executable and certificate-configuration paths are required')
    binary = binary.resolve(strict=True)
    configuration = configuration.resolve(strict=True)
    if not re.fullmatch(r'[0-9a-f]{64}', expected) or sha256(binary) != expected:
        raise ValueError('Trusted executable SHA-256 does not match')
    with binary.open('rb') as stream:
        if stream.read(4) != b'\x7fELF':
            raise ValueError('The selected trusted fixture must be a Linux ELF executable')
    listing = run_command([str(binary), '--list']).splitlines()
    if any(test + ': test' not in listing for test in TESTS):
        raise ValueError('The trusted executable lacks one of the exact allowed tests')
    root_directory = None
    try:
        # A root-owned copy prevents a concurrent Cargo relink from changing
        # the privileged executable after validation. No persistent grant exists.
        candidate = run_command(['sudo', '-n', '/usr/bin/mktemp', '-d', '/tmp/plank-owned-loss.XXXXXXXX']).strip()
        if not re.fullmatch(r'/tmp/plank-owned-loss\.[A-Za-z0-9]{8}', candidate):
            raise ValueError('Unexpected private root scratch directory')
        root_directory = Path(candidate)
        selected = root_directory / 'loopback-test'
        run_command(['sudo', '-n', '/usr/bin/install', '-m', '0500', '-o', 'root', '-g', 'root', '--', str(binary), str(selected)])
        copied_hash = run_command(['sudo', '-n', '/usr/bin/sha256sum', '--', str(selected)]).split()[0]
        if copied_hash != expected:
            raise ValueError('Root-owned executable copy differs from the trusted artifact')
        print('native_loss_fixture_preparation=owned-socket-force executable_sha256=' + expected, flush=True)
        with tempfile.TemporaryDirectory(prefix='plank-owned-loss-tls-') as temporary:
            scratch = Path(temporary)
            run_command(['openssl', 'req', '-x509', '-newkey', 'rsa:2048', '-nodes', '-days', '1', '-config', str(configuration), '-keyout', str(scratch / 'key.pem'), '-out', str(scratch / 'cert.pem')])
            run_command(['openssl', 'x509', '-in', str(scratch / 'cert.pem'), '-outform', 'DER', '-out', str(scratch / 'cert.der')])
            fixture = {
                'SC_NATIVE_TEST_CERTIFICATE': str(scratch / 'cert.pem'),
                'SC_NATIVE_TEST_PRIVATE_KEY': str(scratch / 'key.pem'),
                'SC_NATIVE_TEST_CERTIFICATE_SHA256': sha256(scratch / 'cert.der'),
                'PLANK_TEST_FORCE_OWNED_RCVBUF': '1',
            }
            # Three required passes per mode, never retries after a failure.
            for trial in (1, 2, 3):
                for test in TESTS:
                    print(f'native_loss_matrix_trial={trial} exact_test={test}', flush=True)
                    result = run_command(privileged_test_command(selected, test, fixture))
                    print(result, end='', flush=True)
                    required = ('test result: ok. 1 passed;', 'fec_loss_matrix_frames=900',
                                'unrecovered=0', 'fec_loss_owned_udp_kernel_drops=0 owned_kernel_loss_free=true')
                    if any(marker not in result for marker in required):
                        raise ValueError('Required exact loss matrix evidence is missing')
        if run_command(['sudo', '-n', '/usr/bin/sha256sum', '--', str(selected)]).split()[0] != expected:
            raise ValueError('Privileged executable changed during qualification')
    finally:
        if root_directory is not None:
            original_error = sys.exc_info()[0] is not None
            cleanup_error = None
            for command in (
                    ['sudo', '-n', '/usr/bin/rm', '-f', '--', str(root_directory / 'loopback-test')],
                    ['sudo', '-n', '/usr/bin/rmdir', '--', str(root_directory)]):
                try:
                    run_command(command)
                except subprocess.CalledProcessError as error:
                    cleanup_error = error
                    print(f'Fixture cleanup incomplete; retained root scratch {root_directory}', file=sys.stderr)
            if cleanup_error is not None and not original_error:
                raise cleanup_error
    print('native_loss_matrix_attested=pass trials_per_mode=3 modes=vbr,cbr executable_sha256=' + expected, flush=True)

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--plank-owned-socket-headroom', action='store_true', required=True)
    parser.add_argument('--sha256', required=True)
    parser.add_argument('binary', type=Path)
    parser.add_argument('certificate_configuration', type=Path)
    args = parser.parse_args()
    try:
        qualify(args.binary, args.certificate_configuration, args.sha256.lower())
    except subprocess.CalledProcessError as error:
        print(error.stdout or '', end='', file=sys.stderr)
        raise SystemExit(error.returncode)
    except (ValueError, OSError) as error:
        parser.exit(1, str(error) + '\n')

if __name__ == '__main__':
    main()
