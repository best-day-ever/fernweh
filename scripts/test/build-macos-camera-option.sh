#!/bin/bash
# Real capability negotiation and setup view, with OS permission boundaries faked.
set -euo pipefail
[[ $# = 2 && $1 = /* && $2 = /* ]] || exit 2
source_root=$1; output=$2
source "$source_root/scripts/build/macos-host-target.sh"
plank_macos_host_target
mkdir "$output"
cd "$source_root"
for camera in 0 1; do
    flags=(-mmacosx-version-min="$MACOSX_DEPLOYMENT_TARGET" -fobjc-arc -Wall -Wextra -Werror
        -Werror=unguarded-availability -Werror=unguarded-availability-new "-DPLANK_MACOS_CAMERA_EXTENSION=$camera")
    xcrun clang "${flags[@]}" -Iapps/host/macos/control tests/protocol/macos-camera-option.m \
        apps/host/macos/control/fixed-capture.m -framework Foundation -framework CoreGraphics \
        -o "$output/negotiation-$camera"
    "$output/negotiation-$camera" "$source_root/tests/protocol/media-feature-negotiation-v1.json"
    xcrun clang "${flags[@]}" tests/auth/macos-permission-setup.m \
        -framework Foundation -framework AppKit -framework ApplicationServices -framework CoreAudio \
        -o "$output/setup-$camera"
    "$output/setup-$camera"
done
