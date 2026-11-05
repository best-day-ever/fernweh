#!/usr/bin/env python3
"""Exercise package variants; camera absence must not waive normal payload checks."""
import importlib.util
from pathlib import Path
import unittest
import platform
import plistlib
import subprocess
import tempfile
import sys

ROOT = Path(__file__).resolve().parents[2]


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, ROOT / path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


package = load("camera_package", "scripts/test/check-macos-camera-package.py")
permissions = load("camera_permissions", "scripts/test/check-macos-host-permissions.py")


class CameraVariant(unittest.TestCase):
    @unittest.skipUnless(platform.system() == "Darwin", "requires native signing tools")
    def test_actual_signed_camera_free_app_and_tamper_rejection(self):
        with tempfile.TemporaryDirectory(prefix="plank-camera-package-fixture-") as scratch:
            root = Path(scratch)
            app = root / "Fixture.app"
            (app / "Contents/MacOS").mkdir(parents=True)
            source = root / "main.c"
            source.write_text("int main(void) { return 0; }\n")
            subprocess.run(["xcrun", "clang", str(source), "-o", str(app / "Contents/MacOS/fixture")], check=True)
            info = {"CFBundleIdentifier": "org.example.camera-variant-fixture", "CFBundleExecutable": "fixture",
                    "CFBundlePackageType": "APPL", "CFBundleVersion": "1.0", "PLANKCameraExtensionIncluded": False}
            (app / "Contents/Info.plist").write_bytes(plistlib.dumps(info))
            # Uninstalled disposable code-signing fixture, never TCC or release evidence.
            subprocess.run(["codesign", "--force", "--sign", "-", str(app)], check=True, capture_output=True)
            command = [sys.executable, str(ROOT / "scripts/test/check-macos-camera-package.py"),
                       str(app), "--camera-extension", "disabled"]
            subprocess.run(command, check=True, capture_output=True)
            info["PLANKCameraExtensionIncluded"] = True
            (app / "Contents/Info.plist").write_bytes(plistlib.dumps(info))
            self.assertNotEqual(subprocess.run(command, capture_output=True).returncode, 0)

    def test_disabled_requires_boolean_metadata_and_no_profile_extension_entitlement(self):
        package.check_variant({"PLANKCameraExtensionIncluded": False}, {}, False, False, False)
        for info, entitlements, extension, profile in [
            ({}, {}, False, False),
            ({"PLANKCameraExtensionIncluded": 0}, {}, False, False),
            ({"PLANKCameraExtensionIncluded": True}, {}, False, False),
            ({"PLANKCameraExtensionIncluded": False}, {package.INSTALL: False}, False, False),
            ({"PLANKCameraExtensionIncluded": False}, {}, True, False),
            ({"PLANKCameraExtensionIncluded": False}, {}, False, True),
        ]:
            with self.assertRaises(ValueError):
                package.check_variant(info, entitlements, extension, profile, False)

    def test_enabled_preserves_extension_and_entitlement_requirement(self):
        package.check_variant({"PLANKCameraExtensionIncluded": True}, {package.INSTALL: True}, True, True, True)
        for entitlements, extension in [({}, True), ({package.INSTALL: False}, True), ({package.INSTALL: True}, False)]:
            with self.assertRaises(ValueError):
                package.check_variant({"PLANKCameraExtensionIncluded": True}, entitlements, extension, True, True)

    def test_disabled_permissions_still_require_host_payload_and_reject_camera(self):
        required = {"."} | {name for name in permissions.REQUIRED_APP if not name.startswith(permissions.CAMERA)}
        records = {name: permissions.expected_mode(name, name in {".", "Contents", "Contents/MacOS",
            "Contents/Resources", "Contents/_CodeSignature"}, True) for name in required}
        permissions.check_records(records, True, False)
        with self.assertRaises(ValueError):
            permissions.check_records(records, True, True)
        for extra in [permissions.CAMERA, "Contents/embedded.provisionprofile"]:
            with self.assertRaises(ValueError):
                permissions.check_records({**records, extra: "-rw-r--r--"}, True, False)
        for missing in ["Contents/MacOS/plank-host", "Contents/_CodeSignature/CodeResources"]:
            with self.assertRaises(ValueError):
                permissions.check_records({k: v for k, v in records.items() if k != missing}, True, False)


if __name__ == "__main__":
    unittest.main()
