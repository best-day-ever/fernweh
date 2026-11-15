#!/usr/bin/env python3
"""Exercise the deployment view through real child processes and filesystems."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
HELPER = ROOT / "scripts/package/deploy-macos-client-qt.py"
STUB = r'''#!/usr/bin/env python3
import configparser, json, os
from pathlib import Path
import sys
tool = Path(sys.argv[0])
conf = configparser.ConfigParser()
conf.read(tool.parent / "qt.conf")
paths = {key: json.loads(value) for key, value in conf["Paths"].items()}
plugins = Path(paths["plugins"])
receipt = {"tool": str(tool), "tool_is_symlink": tool.is_symlink(),
           "paths": paths, "args": sys.argv[1:],
           "plugins": {p.name: sorted(x.name for x in p.iterdir()) for p in plugins.iterdir()},
           "library_link": str((tool.parent.parent / "lib").resolve()),
           "environment": os.environ.get("QT_PLUGIN_PATH")}
Path(os.environ["STUB_RECEIPT"]).write_text(json.dumps(receipt))
(tool.parent.parent / "created-by-child").write_text("owned scratch")
sys.exit(int(os.environ.get("STUB_EXIT", "0")))
'''


def snapshot(root):
    return {str(p.relative_to(root)): (p.lstat().st_mode,
            os.readlink(p) if p.is_symlink() else hashlib.sha256(p.read_bytes()).hexdigest())
            for p in root.rglob("*") if p.is_file() or p.is_symlink()}


class DeploymentView(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="qt deploy test ")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()
        self.qt = self.root / "qualified Qt, original"
        for folder in ["bin", "lib", "libexec", "qml", "translations", "plugins/platforms",
                       "plugins/imageformats", "plugins/sqldrivers"]:
            (self.qt / folder).mkdir(parents=True, exist_ok=True)
        self.tool = self.qt / "bin/macdeployqt"
        self.tool.write_text(STUB)
        self.tool.chmod(0o755)
        for name in ["qsqlite", "qsqlmimer", "qsqlodbc", "qsqlpsql"]:
            (self.qt / f"plugins/sqldrivers/lib{name}.dylib").write_text(name)
        (self.qt / "plugins/platforms/libqcocoa.dylib").write_text("cocoa")
        (self.qt / "plugins/imageformats/libqsvg.dylib").write_text("svg")
        self.app = self.root / "Candidate.app"
        self.app.mkdir()
        self.qml = self.root / "source qml"
        self.qml.mkdir()
        self.scratch = self.root / "scratch"
        self.scratch.mkdir()
        self.receipt = self.root / "child.json"

    def invoke(self, **environment):
        return subprocess.run([sys.executable, str(HELPER), "--qt-root", str(self.qt),
            "--app", str(self.app), "--qml-dir", str(self.qml),
            "--scratch-parent", str(self.scratch)], capture_output=True, text=True, timeout=10,
            env={**os.environ, "STUB_RECEIPT": str(self.receipt), **environment})

    def test_filters_only_sql_before_deployment_and_preserves_original(self):
        before = snapshot(self.qt)
        result = self.invoke()
        self.assertEqual(result.returncode, 0, result.stderr)
        receipt = json.loads(self.receipt.read_text())
        self.assertEqual(receipt["plugins"], {"sqldrivers": ["libqsqlite.dylib"],
            "platforms": ["libqcocoa.dylib"], "imageformats": ["libqsvg.dylib"]})
        self.assertEqual(snapshot(self.qt), before)
        self.assertEqual(list(self.scratch.iterdir()), [])
        self.assertFalse(receipt["tool_is_symlink"])
        self.assertNotEqual(receipt["tool"], str(self.tool))
        self.assertFalse(Path(receipt["tool"]).exists())
        self.assertEqual(receipt["library_link"], str(self.qt / "lib"))
        for key, folder in [("prefix", ""), ("libraries", "lib"),
                ("libraryexecutables", "libexec"), ("qmlimports", "qml"),
                ("translations", "translations")]:
            self.assertEqual(receipt["paths"][key], str(self.qt / folder))

    def test_preserves_deployment_arguments_and_process_environment(self):
        result = self.invoke(QT_PLUGIN_PATH="caller sentinel")
        self.assertEqual(result.returncode, 0, result.stderr)
        receipt = json.loads(self.receipt.read_text())
        self.assertEqual(receipt["args"], [str(self.app), "-qmldir=" + str(self.qml),
                                        "-always-overwrite", "-no-strip"])
        self.assertEqual(receipt["environment"], "caller sentinel")

    def test_child_failure_propagates_and_cleans_exact_scratch(self):
        sentinel = self.scratch / ".qt-deploy-unrelated"
        sentinel.mkdir()
        (sentinel / "keep").write_text("retain")
        before = snapshot(self.qt)
        result = self.invoke(STUB_EXIT="37")
        self.assertEqual(result.returncode, 37, result.stderr)
        self.assertEqual(list(self.scratch.iterdir()), [sentinel])
        self.assertEqual(snapshot(self.qt), before)

    def test_requires_sqlite_before_starting_child(self):
        (self.qt / "plugins/sqldrivers/libqsqlite.dylib").unlink()
        self.assertNotEqual(self.invoke().returncode, 0)
        self.assertFalse(self.receipt.exists())
        self.assertEqual(list(self.scratch.iterdir()), [])

    def test_requires_tool_before_starting_child(self):
        self.tool.unlink()
        self.assertNotEqual(self.invoke().returncode, 0)
        self.assertFalse(self.receipt.exists())
        self.assertEqual(list(self.scratch.iterdir()), [])

    def test_rejects_nonexecutable_tool(self):
        self.tool.chmod(0o644)
        self.assertNotEqual(self.invoke().returncode, 0)
        self.assertFalse(self.receipt.exists())

    def test_spawn_failure_cleans_scratch(self):
        self.tool.write_bytes(b"not an executable format")
        self.assertNotEqual(self.invoke().returncode, 0)
        self.assertEqual(list(self.scratch.iterdir()), [])
        self.assertFalse(self.receipt.exists())

    def test_missing_qt_source_directory_is_not_repaired_or_ignored(self):
        (self.qt / "libexec").rmdir()
        self.assertNotEqual(self.invoke().returncode, 0)
        self.assertFalse((self.qt / "libexec").exists())
        self.assertFalse(self.receipt.exists())

    def test_optional_translations_remain_at_the_original_configured_path(self):
        # The qualified SDK may omit its optional translation package.
        (self.qt / "translations").rmdir()
        self.assertEqual(self.invoke().returncode, 0)
        receipt = json.loads(self.receipt.read_text())
        self.assertEqual(receipt["paths"]["translations"], str(self.qt / "translations"))
        self.assertFalse((self.qt / "translations").exists())

    def test_uses_unique_views_without_removing_other_scratch(self):
        sentinel = self.scratch / "keep"
        sentinel.write_text("unrelated")
        self.assertEqual(self.invoke().returncode, 0)
        first = json.loads(self.receipt.read_text())["tool"]
        self.assertEqual(self.invoke().returncode, 0)
        self.assertNotEqual(first, json.loads(self.receipt.read_text())["tool"])
        self.assertEqual(list(self.scratch.iterdir()), [sentinel])
        self.assertEqual(sentinel.read_text(), "unrelated")

    def test_scratch_cannot_enter_the_app_bundle(self):
        self.scratch = self.app
        self.assertNotEqual(self.invoke().returncode, 0)
        self.assertEqual(list(self.app.iterdir()), [])
        self.assertFalse(self.receipt.exists())

    def test_scratch_cannot_modify_the_qualified_qt_tree(self):
        self.scratch = self.qt
        before = snapshot(self.qt)
        self.assertNotEqual(self.invoke().returncode, 0)
        self.assertEqual(snapshot(self.qt), before)
        self.assertFalse(self.receipt.exists())


if __name__ == "__main__":
    unittest.main()
