#!/usr/bin/env python3
"""Deploy Qt with a private plugin view, preserving the qualified Qt install."""
import argparse
from contextlib import contextmanager
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile


@contextmanager
def deployment_view(qt_root, scratch_parent):
    tool = qt_root / "bin/macdeployqt"
    sqlite = qt_root / "plugins/sqldrivers/libqsqlite.dylib"
    if not tool.is_file() or not os.access(tool, os.X_OK):
        raise ValueError("the qualified Qt macdeployqt executable is required")
    if not sqlite.is_file():
        raise ValueError("the qualified Qt SQLite plugin is required")
    for name in ["lib", "libexec", "qml", "plugins"]:
        if not (qt_root / name).is_dir():
            raise ValueError(f"the qualified Qt {name} directory is required")
    if scratch_parent == qt_root or qt_root in scratch_parent.parents:
        raise ValueError("deployment scratch must be outside the qualified Qt installation")

    with tempfile.TemporaryDirectory(prefix=".qt-deploy-", dir=scratch_parent) as temporary:
        view = Path(temporary)
        (view / "bin").mkdir()
        # Copy, not symlink: QLibraryInfo must find this tool's adjacent qt.conf.
        copied_tool = view / "bin/macdeployqt"
        shutil.copy2(tool, copied_tool)
        (view / "lib").symlink_to(qt_root / "lib", target_is_directory=True)
        plugins = view / "plugins"
        plugins.mkdir()
        for category in (qt_root / "plugins").iterdir():
            destination = plugins / category.name
            if category.name == "sqldrivers":
                destination.mkdir()
                (destination / sqlite.name).symlink_to(sqlite)
            else:
                destination.symlink_to(category, target_is_directory=category.is_dir())

        paths = {"Prefix": qt_root, "Libraries": qt_root / "lib",
                 "LibraryExecutables": qt_root / "libexec", "QmlImports": qt_root / "qml",
                 "Translations": qt_root / "translations", "Plugins": plugins}
        (view / "bin/qt.conf").write_text("[Paths]\n" + "".join(
            f"{key} = {json.dumps(str(value), ensure_ascii=False)}\n" for key, value in paths.items()))
        yield copied_tool


def deploy(qt_root, app, qml_dir, scratch_parent):
    for path in [app, qml_dir, scratch_parent]:
        if not path.is_dir():
            raise ValueError(f"deployment input directory is unavailable: {path}")
    if scratch_parent == app or app in scratch_parent.parents:
        raise ValueError("deployment scratch must be outside the app bundle")
    with deployment_view(qt_root, scratch_parent) as tool:
        print(f"macos_qt_deployment_tool={tool} sql_plugins=sqlite-only", flush=True)
        # Preserve ordinary framework/QML/plugin deployment and every tool error.
        result = subprocess.run([str(tool), str(app), "-qmldir=" + str(qml_dir),
                                 "-always-overwrite", "-no-strip"])
        if result.returncode == 0:
            print("macos_client_qt_deployment=pass", flush=True)
        return result.returncode


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--qt-root", required=True, type=Path)
    parser.add_argument("--app", required=True, type=Path)
    parser.add_argument("--qml-dir", required=True, type=Path)
    parser.add_argument("--scratch-parent", required=True, type=Path)
    args = parser.parse_args()
    try:
        return deploy(args.qt_root.resolve(), args.app.resolve(),
                      args.qml_dir.resolve(), args.scratch_parent.resolve())
    except (OSError, ValueError) as error:
        print(f"Qt deployment failed: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
