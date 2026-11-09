#!/usr/bin/env python3
"""Verify the signed Host's explicit camera variant without changing any service."""
import argparse
from pathlib import Path
import plistlib
import subprocess

CAMERA = "Contents/Library/SystemExtensions/la.instinctual.PLANK.Host.Camera.systemextension"
INSTALL = "com.apple.developer.system-extension.install"


def check_variant(info, entitlements, extension_present, profile_present, enabled):
    # Require a real plist Boolean, not truthiness of a string or integer.
    if info.get("PLANKCameraExtensionIncluded") is not enabled:
        raise ValueError("Host camera variant metadata disagrees with the requested build")
    if enabled:
        if not extension_present or entitlements.get(INSTALL) is not True:
            raise ValueError("camera-enabled Host lacks its extension or installation entitlement")
    elif extension_present or profile_present or INSTALL in entitlements:
        raise ValueError("camera-disabled Host contains camera payload/profile/installation entitlement")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("app", type=Path)
    parser.add_argument("--camera-extension", choices=("enabled", "disabled"), default="enabled")
    args = parser.parse_args()
    # Always verify the normal app signature. This option never bypasses signing.
    subprocess.run(["codesign", "--verify", "--strict", str(args.app)], check=True)
    result = subprocess.run(["codesign", "-d", "--entitlements", ":-", str(args.app)],
                            capture_output=True, check=True)
    entitlements = plistlib.loads(result.stdout) if result.stdout.strip() else {}
    info = plistlib.loads((args.app / "Contents/Info.plist").read_bytes())
    check_variant(info, entitlements, (args.app / CAMERA).exists(),
                  (args.app / "Contents/embedded.provisionprofile").exists(),
                  args.camera_extension == "enabled")
    print("macos_camera_package=pass")


if __name__ == "__main__":
    main()
