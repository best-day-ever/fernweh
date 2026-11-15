#!/usr/bin/env bash
# Check the production source iterator against the exact linked RaptorQ.
set -euo pipefail
repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
target_dir=${1:?Usage: run-source-first-fec.sh CARGO_TARGET_DIR}
target_dir=$(cd -- "$target_dir" && pwd)
cd "$repo_root"
test_tmp=$(mktemp -d "$target_dir/source-first-fec.XXXXXX")
trap 'rm -rf -- "$test_tmp"' EXIT
features=()
if [[ -n ${PLANK_TRANSPORT_CARGO_FEATURES:-} ]]; then
    features=(--features "$PLANK_TRANSPORT_CARGO_FEATURES")
fi
# The shared target can contain several toolchains and feature combinations.
# Cargo identifies the exact dependency used by this locked product build.
cargo build --locked --offline --release --target-dir "$target_dir" \
    --manifest-path "$repo_root/protocol/plank-transport/Cargo.toml" \
    ${features[@]+"${features[@]}"} --message-format=json > "$test_tmp/artifacts.jsonl"
fec_rlib=$(python3 - "$test_tmp/artifacts.jsonl" <<'PY'
import json
import sys
from pathlib import Path

matches = set()
for line in Path(sys.argv[1]).read_text().splitlines():
    record = json.loads(line)
    if record.get("reason") == "compiler-artifact" and record["target"]["name"] == "raptorq":
        matches.update(path for path in record["filenames"] if path.endswith(".rlib"))
if len(matches) != 1:
    raise SystemExit("Expected one RaptorQ artifact from the exact locked product build")
selected = matches.pop()
if not Path(selected).is_file():
    raise SystemExit("The selected RaptorQ artifact is missing")
print(selected)
PY
)
rustc --edition=2024 --test -O "$repo_root/tests/protocol/source-first-fec.rs" \
    -L "dependency=$target_dir/release/deps" --extern "raptorq=$fec_rlib" \
    -o "$test_tmp/source-first-test"
"$test_tmp/source-first-test" --nocapture --test-threads=1
