#!/usr/bin/env bash
# Prove the same qmake configuration path for a generic and an operator build.
set -euo pipefail
source_root=${1:?usage: test-fernweh-deployment.sh SOURCE BUILD}
build_root=${2:?usage: test-fernweh-deployment.sh SOURCE BUILD}
qmake_bin=${QMAKE:-qmake}
qmake_args=()
if [[ $(uname -s) == Darwin ]]; then
  qmake_args+=(QMAKE_MACOSX_DEPLOYMENT_TARGET=15.0 QMAKE_APPLE_DEVICE_ARCHS=arm64)
fi
client="$source_root/apps/client"
python3 "$client/deployment/test_configure.py"
mkdir -p "$build_root"
(
  unset FERNWEH_BROKER_HOST FERNWEH_BROKER_PORT FERNWEH_BROKER_PINS
  unset FERNWEH_PASSKEY_RP_ID FERNWEH_UPDATES_ENABLED
  for profile in generic configured; do
    if [[ $profile == configured ]]; then
      export FERNWEH_BROKER_HOST=broker.example.org
      export FERNWEH_BROKER_PORT=29443
      export FERNWEH_BROKER_PINS=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
      export FERNWEH_PASSKEY_RP_ID=ipa.example.org
      export FERNWEH_UPDATES_ENABLED=1
    fi
    mkdir -p "$build_root/$profile"
    (
      cd "$build_root/$profile"
      "$qmake_bin" "$client/tests/deployment/deployment.pro" CONFIG+=release CONFIG-=app_bundle "${qmake_args[@]}"
      make -j2
      ./deployment
    )
  done
  # qmake must reject a bad pin even when a previous valid header exists.
  export FERNWEH_BROKER_PINS=invalid
  cd "$build_root/configured"
  if "$qmake_bin" "$client/tests/deployment/deployment.pro" CONFIG+=release CONFIG-=app_bundle "${qmake_args[@]}"; then
    echo 'Invalid deployment configuration unexpectedly passed qmake' >&2
    exit 1
  fi
)
echo 'Fernweh deployment configuration: generic/configured builds and rejection passed'
