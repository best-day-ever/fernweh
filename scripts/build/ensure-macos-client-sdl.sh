#!/usr/bin/env bash
# Incrementally rebuild only SDL when the packaged Cocoa input patch changes.
set -euo pipefail
[[ $# == 1 && $1 == /* ]] || { echo 'usage: ensure-macos-client-sdl.sh SOURCE' >&2; exit 2; }
source_root=$1
: "${PLANK_MAC_CLIENT_DEPS:?}"
source "$source_root/scripts/build/macos-client-target.sh"
plank_macos_client_target
source "$source_root/scripts/build/build-paths.sh"
plank_build_path_flags "$source_root" "$PLANK_MAC_CLIENT_DEPS"

sdl_source="$PLANK_MAC_CLIENT_DEPS/src/SDL3-3.4.2"
sdl_build="$PLANK_MAC_CLIENT_DEPS/build-SDL3-3.4.2"
prefix="$PLANK_MAC_CLIENT_DEPS/install"
library="$prefix/lib/libSDL3.0.dylib"
stamp="$sdl_build/.plank-posted-pointer-build"
patch_file="$source_root/packaging/client/macos/sdl-patches/0001-preserve-posted-pointer-position.patch"
jobs=${PLANK_BUILD_JOBS:-8}
[[ $jobs =~ ^[1-9][0-9]*$ ]] || exit 2
[[ -f $sdl_source/CMakeLists.txt ]] || {
    echo 'Bootstrap the pinned SDL 3.4.2 sources before building the Client' >&2; exit 2;
}

if ! patch --force --reverse --dry-run -d "$sdl_source" -p1 < "$patch_file" >/dev/null 2>&1; then
    # Refuse unknown or partially modified sources instead of guessing.
    patch --force --forward --dry-run -d "$sdl_source" -p1 < "$patch_file"
    patch --force --forward -d "$sdl_source" -p1 < "$patch_file"
fi
patch --force --reverse --dry-run -d "$sdl_source" -p1 < "$patch_file" >/dev/null
patch_hash=$(shasum -a 256 "$patch_file" | awk '{print $1}')
library_hash=missing
[[ ! -f $library ]] || library_hash=$(shasum -a 256 "$library" | awk '{print $1}')
expected="$patch_hash $MACOSX_DEPLOYMENT_TARGET $library_hash"
if [[ -f $library && -f $stamp && $(cat "$stamp") == "$expected" ]]; then
    exit 0
fi

# Remove success evidence before compiling: a partial build cannot bless an
# older installed library. Retain all unrelated dependencies and object caches.
rm -f "$stamp"
export PATH="$prefix/bin:/Applications/CMake.app/Contents/bin:$PATH"
cmake -S "$sdl_source" -B "$sdl_build" \
    -DCMAKE_BUILD_TYPE=Release -DCMAKE_INSTALL_PREFIX="$prefix" \
    "-DCMAKE_C_FLAGS=$PLANK_C_FILE_FLAGS" "-DCMAKE_CXX_FLAGS=$PLANK_C_FILE_FLAGS" \
    -DCMAKE_PREFIX_PATH="$prefix" -DCMAKE_OSX_ARCHITECTURES=arm64 \
    -DCMAKE_OSX_DEPLOYMENT_TARGET="$MACOSX_DEPLOYMENT_TARGET" \
    -DBUILD_SHARED_LIBS=ON -DSDL_TESTS=OFF -DSDL_TEST_LIBRARY=OFF -DSDL_SHARED=ON -DSDL_STATIC=OFF
cmake --build "$sdl_build" --parallel "$jobs"
cmake --install "$sdl_build"
library_hash=$(shasum -a 256 "$library" | awk '{print $1}')
printf '%s\n' "$patch_hash $MACOSX_DEPLOYMENT_TARGET $library_hash" > "$stamp"
