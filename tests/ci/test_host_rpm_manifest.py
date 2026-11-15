"""Exercise the finished-RPM manifest gate without compiling the product."""
from pathlib import Path
import os
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]

# Product files, not the retired upstream shader/UI/artwork payload.
FILES = """/usr/lib/modules-load.d/plank.conf
/usr/libexec/plank/plank-host-supervisor
/usr/libexec/plank/plank-pam-broker
/usr/libexec/plank/plank-host-certificate
/usr/libexec/plank/plank-host-state
/usr/libexec/plank/plank-display-prepare
/usr/share/licenses/plank-host/LICENSE-Kyber-AGPLv3
/usr/share/licenses/plank-host/LICENSE-Kyber-Notice
/usr/lib/systemd/system/plank-host.service
/usr/lib/systemd/system/plank-display-prepare.service
/etc/plank/host.conf
/etc/logrotate.d/plank-host
/etc/pam.d/plank-host
/etc/pam.d/plank-remote
/usr/lib64/security/pam_plank_handoff.so
/usr/share/plank/collaboration-ui.py
/usr/lib/udev/rules.d/70-plank-host-wacom.rules
/usr/share/X11/xorg.conf.d/90-plank-virtual-wacom.conf
/usr/share/plank/display/virtual-1.edid
/usr/share/plank/display/virtual-2.edid
/usr/share/plank/display/virtual-3.edid
/usr/share/plank/display/virtual-4.edid
"""
LOG_DIRECTORY = '/var/log/plank drwx------ root root'


class HostRpmManifestTests(unittest.TestCase):
    def stage_resources(self, root, helper=True, stale_assets=False):
        script = (ROOT / 'scripts/package/build-host-rpm.sh').read_text()
        start = script.index('mkdir -p "$payload_dir/usr/share/plank"')
        staging = script[start:script.index('\nsource_epoch=', start)]
        source = root / 'source'
        build = root / 'build'
        payload = root / 'payload'
        build.mkdir()
        if helper:
            path = source / 'apps/host/linux/src_assets/linux/assets/collaboration-ui.py'
            path.parent.mkdir(parents=True)
            path.write_text('# required collaboration helper fixture\n')
        if stale_assets:
            path = build / 'assets/web/index.html'
            path.parent.mkdir(parents=True)
            path.write_text('retired web asset')
        command = 'set -euo pipefail\nrepo_dir=$1\nbuild_dir=$2\npayload_dir=$3\n' + staging
        result = subprocess.run(['bash', '-c', command, 'resource-staging-fixture',
                                 str(source), str(build), str(payload)], text=True, capture_output=True)
        return result, payload

    @unittest.skipUnless(sys.platform.startswith('linux'), 'Executes Linux GNU install staging')
    def test_resources_stage_without_retired_build_assets(self):
        with tempfile.TemporaryDirectory() as temporary:
            result, payload = self.stage_resources(Path(temporary))
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            helper = payload / 'usr/share/plank/collaboration-ui.py'
            self.assertEqual(helper.read_text(), '# required collaboration helper fixture\n')
            self.assertEqual(helper.stat().st_mode & 0o777, 0o644)

    @unittest.skipUnless(sys.platform.startswith('linux'), 'Executes Linux GNU install staging')
    def test_missing_required_collaboration_helper_still_fails(self):
        with tempfile.TemporaryDirectory() as temporary:
            result, _payload = self.stage_resources(Path(temporary), helper=False, stale_assets=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('collaboration-ui.py', result.stderr)

    @unittest.skipUnless(sys.platform.startswith('linux'), 'Executes Linux GNU install staging')
    def test_stale_cache_web_assets_are_not_imported(self):
        with tempfile.TemporaryDirectory() as temporary:
            result, payload = self.stage_resources(Path(temporary), stale_assets=True)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            self.assertFalse((payload / 'usr/share/plank/web').exists())
            self.assertTrue((payload / 'usr/share/plank/collaboration-ui.py').is_file())

    def check_manifest(self, files=FILES, log_directory=LOG_DIRECTORY):
        script = (ROOT / 'scripts/package/build-host-rpm.sh').read_text()
        gate = script[script.index('rpm -qpl "$rpm_file" >/dev/null'):
                      script.index('# RPM\'s normal debug stripping')]
        # Replace only external package inspection, then execute the actual
        # production assertions, including pipefail and all required files.
        setup = '''set -euo pipefail
rpm_file=fixture.rpm
build_dir=fixture
package_version=fixture
rpm() {
  case "$1" in
    -qpl) printf '%s' "$TEST_RPM_FILES" ;;
    -qpR) printf '%s\\n' 'libX11.so.6()(64bit)' 'logrotate' 'python3-gobject' 'gtk3' ;;
    -qp) printf '%s\\n' "$TEST_RPM_LOG_DIRECTORY" '/usr/share/plank/collaboration-ui.py -rw-r--r-- root root' ;;
    *) return 2 ;;
  esac
}
readelf() { printf '%s\\n' 'Shared library: libX11.so.6'; }
'''
        return subprocess.run(['bash', '-c', setup + gate],
                              env={**os.environ, 'TEST_RPM_FILES': files,
                                   'TEST_RPM_LOG_DIRECTORY': log_directory},
                              text=True, capture_output=True)

    def test_current_payload_without_retired_assets_passes(self):
        result = self.check_manifest()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn('host_rpm_manifest_gate=pass', result.stdout)

    def test_retired_assets_are_rejected(self):
        for path in ('/usr/share/plank/desktop.png',
                     '/usr/share/plank/shaders/opengl/Scene.frag',
                     '/usr/share/plank/web/index.html',
                     '/usr/share/plank/apps.json'):
            with self.subTest(path=path):
                self.assertNotEqual(self.check_manifest(FILES + path + '\n').returncode, 0)

    def test_missing_edid_and_extra_edid_are_rejected(self):
        self.assertNotEqual(self.check_manifest(FILES.replace(
            '/usr/share/plank/display/virtual-2.edid\n', '')).returncode, 0)
        self.assertNotEqual(self.check_manifest(
            FILES + '/usr/share/plank/display/virtual-5.edid\n').returncode, 0)

    def test_private_log_directory_is_required(self):
        self.assertNotEqual(self.check_manifest(log_directory='').returncode, 0)
        self.assertNotEqual(self.check_manifest(
            log_directory='/var/log/plank drwxr-xr-x root root').returncode, 0)


if __name__ == '__main__':
    unittest.main()
