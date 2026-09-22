#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later

set -euo pipefail

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
crate_dir="$repo_root/protocol/plank-transport"
test_tmp=$(mktemp -d /tmp/plank-native-kyproto.XXXXXX)
trap 'rm -rf -- "$test_tmp"' EXIT

openssl req -x509 -newkey rsa:2048 -nodes -days 1 \
  -subj '/CN=localhost' \
  -keyout "$test_tmp/key.pem" \
  -out "$test_tmp/cert.pem" >/dev/null 2>&1
openssl x509 -in "$test_tmp/cert.pem" -outform DER -out "$test_tmp/cert.der"

export SC_NATIVE_TEST_CERTIFICATE="$test_tmp/cert.pem"
export SC_NATIVE_TEST_PRIVATE_KEY="$test_tmp/key.pem"
SC_NATIVE_TEST_CERTIFICATE_SHA256=$(sha256sum "$test_tmp/cert.der")
export SC_NATIVE_TEST_CERTIFICATE_SHA256=${SC_NATIVE_TEST_CERTIFICATE_SHA256%% *}

run_native_test() {
  local test_name=$1
  if [[ ${SC_NATIVE_CARGO_PROFILE:-debug} == release ]]; then
    cargo test --release --locked --offline --manifest-path "$crate_dir/Cargo.toml" \
      "$test_name" -- --ignored --exact
  else
    cargo test --locked --offline --manifest-path "$crate_dir/Cargo.toml" \
      "$test_name" -- --ignored --exact
  fi
}

run_native_test native::tests::native_kyproto_round_trip_preserves_all_initial_lanes
run_native_test native::tests::native_raptorq_survives_progressive_transport_loss_at_150_mbps
