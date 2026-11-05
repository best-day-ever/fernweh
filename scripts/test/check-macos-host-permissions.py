#!/usr/bin/env python3
"""Fail closed on inaccessible Host bundles, staged payloads and actual PKG BOMs."""
import argparse
from pathlib import Path, PurePosixPath
import stat
import subprocess
import tempfile


CAMERA = "Contents/Library/SystemExtensions/la.instinctual.PLANK.Host.Camera.systemextension"
EXECUTABLES = {"Contents/MacOS/plank-host", "Contents/Resources/uninstall.sh", "Contents/Resources/plank-configure", CAMERA + "/Contents/MacOS/plank-camera"}
REQUIRED_APP = {
    "Contents", "Contents/MacOS", "Contents/Resources", "Contents/_CodeSignature",
    "Contents/MacOS/plank-host", "Contents/Info.plist", "Contents/Resources/plank.icns",
    "Contents/Resources/plank-configure", "Contents/Resources/host.conf.example",
    "Contents/_CodeSignature/CodeResources",
    CAMERA, CAMERA + "/Contents/Info.plist", CAMERA + "/Contents/MacOS/plank-camera",
    CAMERA + "/Contents/_CodeSignature/CodeResources",
}
APP = "Applications/PLANK Host.app"
DRIVER = "Library/Audio/Plug-Ins/HAL/PLANK Microphone.driver"
OUTPUT_DRIVER = "Library/Audio/Plug-Ins/HAL/PLANK Output.driver"


def expected_mode(name, directory, app_only=False):
    if directory:
        return "drwxr-xr-x"
    relative = name if app_only else name.removeprefix(APP + "/")
    return "-rwxr-xr-x" if relative in EXECUTABLES or name in {DRIVER + "/Contents/MacOS/plank-microphone", OUTPUT_DRIVER + "/Contents/MacOS/plank-output"} else "-rw-r--r--"


def check_records(records, app_only=False, camera_extension=True):
    required_app = REQUIRED_APP if camera_extension else {name for name in REQUIRED_APP if not name.startswith(CAMERA)}
    if not camera_extension:
        prefix = "" if app_only else APP + "/"
        if any(name.startswith(prefix + CAMERA) or name == prefix + "Contents/embedded.provisionprofile" for name in records):
            raise ValueError("camera-disabled package contains a camera extension or provisioning profile")
    required = {"."} | (required_app if app_only else {
        "Applications", APP, "Library", "Library/LaunchAgents", "Library/LaunchDaemons",
        APP + "/Contents/Resources/uninstall.sh",
        "Library/LaunchDaemons/la.instinctual.PLANK.Host.machine.plist",
        "Library/LaunchAgents/la.instinctual.PLANK.Host.desktop.plist",
        "Library/LaunchAgents/la.instinctual.PLANK.Host.sign-in.plist",
        DRIVER, DRIVER + "/Contents/Info.plist", DRIVER + "/Contents/MacOS/plank-microphone",
        DRIVER + "/Contents/_CodeSignature/CodeResources",
        OUTPUT_DRIVER, OUTPUT_DRIVER + "/Contents/Info.plist", OUTPUT_DRIVER + "/Contents/MacOS/plank-output",
        OUTPUT_DRIVER + "/Contents/_CodeSignature/CodeResources",
    } | {APP + "/" + name for name in required_app})
    missing = required - records.keys()
    if missing:
        raise ValueError("missing required package entries: " + ", ".join(sorted(missing)))
    for name, mode in records.items():
        if PurePosixPath(name).is_absolute() or ".." in PurePosixPath(name).parts:
            raise ValueError("unsafe package entry")
        if PurePosixPath(name).name.startswith("._"):
            raise ValueError("unexpected physical AppleDouble package entry")
        expected = expected_mode(name, mode.startswith("d"), app_only)
        if mode != expected:
            raise ValueError(f"{name}: permissions {mode}, expected {expected}")


def check_tree(root, app_only=False, camera_extension=True):
    # lstat refuses to treat a symlink as an ordinary file or directory.
    records = {".": stat.filemode(root.lstat().st_mode)}
    for path in root.rglob("*"):
        records[path.relative_to(root).as_posix()] = stat.filemode(path.lstat().st_mode)
    check_records(records, app_only, camera_extension)


def check_bom(text, payload=None, camera_extension=True):
    records = {}
    for line in text.splitlines():
        name, mode = line.rsplit("\t", 1)
        name = name.removeprefix("./")
        if name in records:
            raise ValueError("duplicate BOM entry")
        # lsbom's symbolic mode column includes a trailing space on macOS.
        records[name] = mode.strip()
    # On 26.7 pkgbuild records xattrs as paired AppleDouble BOM entries with
    # the source's mode, including directory/executable modes. expand-full
    # restores these as xattrs, not files. Accept that representation only
    # against the independently validated payload; never ignore real files,
    # orphan entries or mismatched permissions.
    metadata = {}
    for name, mode in records.items():
        path = PurePosixPath(name)
        if not path.name.startswith("._"):
            continue
        paired = path.with_name(path.name[2:]).as_posix()
        if (payload is None or paired not in records or records[paired] != mode or
                (payload / name).exists() or (payload / name).is_symlink()):
            raise ValueError("unverified AppleDouble BOM metadata")
        metadata[name] = mode
    for name in metadata:
        del records[name]
    check_records(records, camera_extension=camera_extension)


def check_package(package, camera_extension=True):
    with tempfile.TemporaryDirectory(prefix="plank-host-pkg-check-") as scratch:
        expanded = Path(scratch) / "expanded"
        subprocess.run(["pkgutil", "--expand-full", str(package), str(expanded)], check=True)
        boms = list(expanded.rglob("Bom"))
        if len(boms) != 1:
            raise ValueError("expected exactly one Host component BOM")
        payload = boms[0].parent / "Payload"
        check_tree(payload, camera_extension=camera_extension)
        check_bom(subprocess.check_output(["lsbom", "-p", "fM", str(boms[0])], text=True), payload, camera_extension)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument("--app", type=Path)
    group.add_argument("--payload", type=Path)
    group.add_argument("--pkg", type=Path)
    parser.add_argument("--camera-extension", choices=("enabled", "disabled"), default="enabled")
    args = parser.parse_args()
    camera_extension = args.camera_extension == "enabled"
    if args.pkg:
        check_package(args.pkg.resolve(), camera_extension)
    else:
        check_tree(args.app or args.payload, app_only=bool(args.app), camera_extension=camera_extension)
    print("macos_host_permissions=pass")


if __name__ == "__main__":
    main()
