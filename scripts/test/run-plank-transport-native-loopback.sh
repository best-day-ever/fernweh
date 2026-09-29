#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later

set -euo pipefail

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
crate_dir="$repo_root/protocol/plank-transport"
test_tmp=$(mktemp -d /tmp/plank-native-kyproto.XXXXXX)
trap 'rm -rf -- "$test_tmp"' EXIT

openssl req -x509 -newkey rsa:2048 -nodes -days 1 \
  -config "$repo_root/probes/macos/loopback-cert.cnf" \
  -keyout "$test_tmp/key.pem" \
  -out "$test_tmp/cert.pem" >/dev/null 2>&1
openssl x509 -in "$test_tmp/cert.pem" -outform DER -out "$test_tmp/cert.der"

export SC_NATIVE_TEST_CERTIFICATE="$test_tmp/cert.pem"
export SC_NATIVE_TEST_PRIVATE_KEY="$test_tmp/key.pem"
SC_NATIVE_TEST_CERTIFICATE_SHA256=$(sha256sum "$test_tmp/cert.der")
export SC_NATIVE_TEST_CERTIFICATE_SHA256=${SC_NATIVE_TEST_CERTIFICATE_SHA256%% *}

cc -std=c11 -O2 -Wall -Wextra -Werror -I"$crate_dir/include" \
  "$repo_root/tests/protocol/camera-v1.c" -o "$test_tmp/camera-wire"
"$test_tmp/camera-wire" "$repo_root/tests/protocol/camera-v1.hex"

cargo_profile_args=()
# Performance qualification uses optimized product code. Explicit debug runs
# are still useful diagnostics, but are not release performance qualification.
case ${SC_NATIVE_CARGO_PROFILE:-release} in
  release) cargo_profile_args+=(--release) ;;
  debug) ;;
  *) echo "SC_NATIVE_CARGO_PROFILE must be release or debug" >&2; exit 2 ;;
esac
if [[ -n ${PLANK_TRANSPORT_CARGO_FEATURES:-} ]]; then
  cargo_profile_args+=(--features "$PLANK_TRANSPORT_CARGO_FEATURES")
fi

# Dependency tests are not included by an ordinary product cargo test. Run the
# actual audio/video FEC parsers and receiver state machines with our lockfile.
cargo test "${cargo_profile_args[@]}" --locked --offline --manifest-path "$crate_dir/Cargo.toml" \
  -p plank-transport -p kyproto --lib protocol::driver::av

cargo test "${cargo_profile_args[@]}" --locked --offline --manifest-path "$crate_dir/Cargo.toml" \
  native::version_tests::incompatible_peers_fail_tls_before_setup_or_media \
  -- --ignored --exact --nocapture

cargo test "${cargo_profile_args[@]}" --locked --offline --manifest-path "$crate_dir/Cargo.toml" \
  native::tests::native_kyproto_round_trip_preserves_all_initial_lanes \
  -- --ignored --exact --nocapture

cargo test "${cargo_profile_args[@]}" --locked --offline --manifest-path "$crate_dir/Cargo.toml" \
  native_ffi::data_tests::reliable_data_overflow_fails_both_receivers \
  -- --ignored --exact --nocapture

cargo test "${cargo_profile_args[@]}" --locked --offline --manifest-path "$crate_dir/Cargo.toml" \
  native_ffi::cancellation_tests::cancellation_interrupts_real_auth_endpoints_and_promotion \
  -- --ignored --exact --nocapture

cargo test "${cargo_profile_args[@]}" --locked --offline --manifest-path "$crate_dir/Cargo.toml" \
  native_ffi::cancellation_tests::closed_or_unreachable_peer_releases_live_endpoint \
  -- --ignored --exact --nocapture

cargo test "${cargo_profile_args[@]}" --locked --offline --manifest-path "$crate_dir/Cargo.toml" \
  native_ffi::microphone_lane::tests::encrypted_ffi_microphone_mute_reopen_and_bounds \
  -- --ignored --exact --nocapture

cargo test "${cargo_profile_args[@]}" --locked --offline --manifest-path "$crate_dir/Cargo.toml" \
  native_ffi::camera_lane::tests::encrypted_camera_with_and_without_microphone \
  -- --ignored --exact --nocapture

# The throughput gate is timing-sensitive under x86 emulation. A release
# builder may use three passes from a native x86_64 Rocky 9.7 machine, but only
# for the identical compiled test executable. All other tests still run here.
# The operator must retain the native test log as release evidence.
if [[ -n ${SC_NATIVE_LOSS_ATTESTED_SHA256:-} ]]; then
  [[ ${SC_NATIVE_CARGO_PROFILE:-release} == release && $(uname -m) == x86_64 ]] || {
    echo "native loss attestation requires an x86_64 release build" >&2
    exit 1
  }
  mapfile -t loss_test_binaries < <(find "${CARGO_TARGET_DIR:?}/release/deps" \
    -maxdepth 1 -type f -name 'plank_transport-*' -perm -u+x)
  [[ ${#loss_test_binaries[@]} == 1 ]] || {
    echo "expected exactly one compiled plank-transport test executable" >&2
    exit 1
  }
  actual_sha256=$(sha256sum "${loss_test_binaries[0]}")
  actual_sha256=${actual_sha256%% *}
  [[ $actual_sha256 == "$SC_NATIVE_LOSS_ATTESTED_SHA256" ]] || {
    echo "native loss attestation does not match the compiled test executable" >&2
    exit 1
  }
  echo "native_loss_matrix_attested=pass trials=3 executable_sha256=$actual_sha256"
else
  # Three required passes, not retries: set -e stops on the first failure.
  for loss_trial in 1 2 3; do
    echo "native_loss_matrix_trial=$loss_trial"
    cargo test "${cargo_profile_args[@]}" --locked --offline --manifest-path "$crate_dir/Cargo.toml" \
      native::tests::native_raptorq_survives_progressive_transport_loss_at_150_mbps \
      -- --ignored --exact --nocapture
  done
fi
