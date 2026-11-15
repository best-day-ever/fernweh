#!/usr/bin/env bash
# Fetch locked public sources on hits too; never cache compiled product objects.
set -euo pipefail
role=${1:?Product role}
target=${2:?Rust target triple}
cd "${PLANK_SOURCE_ROOT:?}"
cargo fetch --locked --target "$target" --manifest-path "$PLANK_SOURCE_ROOT/protocol/plank-transport/Cargo.toml"
# All native source builders run the real socket constructor regressions.
cargo fetch --locked --target "$target" --manifest-path "$PLANK_SOURCE_ROOT/third_party/quinn-udp-0.5.16/Cargo.toml"
if [[ $role == linux-host ]]; then
  # Standalone vendor unit tests have their own pinned test-only dependencies.
  cargo fetch --locked --target "$target" --manifest-path "$PLANK_SOURCE_ROOT/third_party/quinn-proto-0.11.19/Cargo.toml"
fi
