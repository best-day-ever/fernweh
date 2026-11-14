#!/usr/bin/env bash
set -euo pipefail

# Run under agent-heavy-run on the shared BDE development Mac.
# QMAKE=/path/to/Qt/6.10.2/macos/bin/qmake ./render.sh /path/to/apps/client
fixtures=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
client=${1:?Usage: render.sh /absolute/path/to/apps/client}
qmake=${QMAKE:-qmake}
work_dir=$(mktemp -d "${TMPDIR:-/tmp}/fernweh-doc-capture.XXXXXX")
trap 'rm -rf "$work_dir"' EXIT

export FERNWEH_BROKER_ENABLED=0
python3 "$client/deployment/configure.py" --output "$work_dir/config.h" --qml-output "$work_dir/qml"
export FERNWEH_CAPTURE_RESOURCES="$work_dir/qml/qml.qrc"
cd "$work_dir"
"$qmake" "$fixtures/capture.pro"
make -j2
export QT_QPA_PLATFORM=offscreen
export QT_QUICK_BACKEND=software
export QT_SCALE_FACTOR=2
export QT_QUICK_CONTROLS_MATERIAL_VARIANT=Dense
./fernweh-doc-capture "$fixtures" 0 "$fixtures/../display-matcher.png"
./fernweh-doc-capture "$fixtures" 1 "$fixtures/../display-advanced.png"
