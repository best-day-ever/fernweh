#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later

set -euo pipefail

case ${1:-} in
  '') performance=1 ;;
  --functional-only) performance=0 ;;
  *) echo 'Usage: run-plank-transport-native-loopback.sh [--functional-only]' >&2; exit 2 ;;
esac
[[ $# -le 1 ]] || exit 2

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
SC_NATIVE_TEST_CERTIFICATE_SHA256=$(openssl dgst -sha256 "$test_tmp/cert.der")
export SC_NATIVE_TEST_CERTIFICATE_SHA256=${SC_NATIVE_TEST_CERTIFICATE_SHA256##* }

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
cargo test ${cargo_profile_args[@]+"${cargo_profile_args[@]}"} --locked --offline --manifest-path "$crate_dir/Cargo.toml" \
  -p plank-transport -p kyproto --lib protocol::driver::av

if [[ ${SC_NATIVE_CARGO_PROFILE:-release} == release && $performance == 1 ]]; then
  target_dir=$(cargo metadata --locked --offline --no-deps --format-version=1 --manifest-path "$crate_dir/Cargo.toml" |
    python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')
  bash "$repo_root/scripts/test/run-source-first-fec.sh" "$target_dir"
fi

# Exact test filters must execute a test, not report success with zero matches.
run_ignored() {
  cargo test ${cargo_profile_args[@]+"${cargo_profile_args[@]}"} --locked --offline --manifest-path "$crate_dir/Cargo.toml" \
    "$1" -- --ignored --exact --nocapture | tee "$test_tmp/result.log"
  grep -q 'test result: ok. 1 passed;' "$test_tmp/result.log" || {
    echo "Required integration test did not run: $1" >&2
    return 1
  }
}

run_ignored native_ffi::data_tests::reliable_data_backpressure_recovers_both_receivers

cargo test ${cargo_profile_args[@]+"${cargo_profile_args[@]}"} --locked --offline --manifest-path "$crate_dir/Cargo.toml" \
  native_ffi::cancellation_tests::cancellation_interrupts_real_auth_endpoints_and_promotion \
  -- --ignored --exact --nocapture

cargo test ${cargo_profile_args[@]+"${cargo_profile_args[@]}"} --locked --offline --manifest-path "$crate_dir/Cargo.toml" \
  native_ffi::cancellation_tests::closed_or_unreachable_peer_releases_live_endpoint \
  -- --ignored --exact --nocapture

cargo test ${cargo_profile_args[@]+"${cargo_profile_args[@]}"} --locked --offline --manifest-path "$crate_dir/Cargo.toml" \
  native_ffi::collaboration_tests:: \
  -- --ignored --nocapture

cargo test ${cargo_profile_args[@]+"${cargo_profile_args[@]}"} --locked --offline --manifest-path "$crate_dir/Cargo.toml" \
  native_ffi::microphone_lane::tests::encrypted_ffi_microphone_mute_reopen_and_bounds \
  -- --ignored --exact --nocapture

cargo test ${cargo_profile_args[@]+"${cargo_profile_args[@]}"} --locked --offline --manifest-path "$crate_dir/Cargo.toml" \
  native_ffi::camera_lane::tests::encrypted_camera_with_and_without_microphone \
  -- --ignored --exact --nocapture

cargo test ${cargo_profile_args[@]+"${cargo_profile_args[@]}"} --locked --offline --manifest-path "$crate_dir/Cargo.toml" \
  native_ffi::camera_lane::tests::encrypted_version_mismatch_disables_only_camera \
  -- --ignored --exact --nocapture

# The throughput gate is timing-sensitive under x86 emulation. A release
# builder may use three passes from a native x86_64 Rocky 9.7 machine, but only
# for the identical compiled test executable: three VBR and three CBR runs.
# All other tests still run here.
# The operator must retain the native test log as release evidence.
if [[ -n ${SC_NATIVE_LOSS_ATTESTED_SHA256:-} ]]; then
  [[ ${SC_NATIVE_CARGO_PROFILE:-release} == release && $(uname -m) == x86_64 ]] || {
    echo "native loss attestation requires an x86_64 release build" >&2
    exit 1
  }
  # A shared target contains old profiles and worktrees. Bind attestation to
  # Cargo's artifact for this exact source/profile/features, never cache age.
  loss_test_binary=$(cargo test ${cargo_profile_args[@]+"${cargo_profile_args[@]}"} \
    --locked --offline --manifest-path "$crate_dir/Cargo.toml" --lib --no-run \
    --message-format=json | python3 "$repo_root/scripts/test/cargo-transport-test-executable.py")
  actual_sha256=$(sha256sum "$loss_test_binary")
  actual_sha256=${actual_sha256%% *}
  [[ $actual_sha256 == "$SC_NATIVE_LOSS_ATTESTED_SHA256" ]] || {
    echo "native loss attestation does not match the compiled test executable" >&2
    exit 1
  }
  echo "native_loss_matrix_attested=pass trials_per_mode=3 modes=vbr,cbr executable_sha256=$actual_sha256"
elif (( performance )); then
  # Three required passes, not retries: set -e stops on the first failure.
  for loss_trial in 1 2 3; do
    echo "native_loss_matrix_trial=$loss_trial"
    cargo test ${cargo_profile_args[@]+"${cargo_profile_args[@]}"} --locked --offline --manifest-path "$crate_dir/Cargo.toml" \
      native::tests::native_raptorq_survives_progressive_transport_loss_at_150_mbps \
      -- --ignored --exact --nocapture
    cargo test ${cargo_profile_args[@]+"${cargo_profile_args[@]}"} --locked --offline --manifest-path "$crate_dir/Cargo.toml" \
      native::tests::native_raptorq_survives_progressive_transport_loss_at_150_mbps_cbr \
      -- --ignored --exact --nocapture
  done
fi
