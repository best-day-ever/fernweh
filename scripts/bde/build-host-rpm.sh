#!/usr/bin/env bash
# Native BDE Host builder. Provision build dependencies separately; never install
# a candidate or change platform repositories from this entry point.
set -euo pipefail
if [[ $(uname -s) != Linux || $(uname -m) != x86_64 ]]; then
  echo 'Linux Host builds run natively on the assigned canary, not in Mac/OrbStack containers. See docs/bde/agents.md.' >&2
  exit 2
fi
source /etc/os-release
if [[ $ID != rocky || $VERSION_ID != 9.7 ]]; then
  echo 'The native Host builder requires pinned Rocky Linux 9.7.' >&2
  exit 2
fi
[[ $# == 0 ]] || { echo 'No container options are supported; configure PLANK_* build paths instead.' >&2; exit 2; }
: "${PLANK_DEP_ROOT:?Set the reusable native dependency cache}"
: "${PLANK_WORK_ROOT:?Set this candidate build workspace}"
: "${PLANK_ARTIFACT_ROOT:?Set the candidate artifact directory}"
: "${PLANK_BUILD_BRANCH:?Set a lowercase package branch qualifier}"
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
export PLANK_SOURCE_ROOT="$repo_dir"
export PLANK_CARGO_ROOT=${PLANK_CARGO_ROOT:-$HOME/.cargo}
export PLANK_RUSTUP_ROOT=${PLANK_RUSTUP_ROOT:-$HOME/.rustup}
export PLANK_BUILD_JOBS=${PLANK_BUILD_JOBS:-4}
export CARGO_BUILD_JOBS="$PLANK_BUILD_JOBS"
[[ $PLANK_BUILD_JOBS =~ ^[1-4]$ ]] || { echo 'Use one to four native build jobs.' >&2; exit 2; }
test -z "$(git -C "$repo_dir" status --porcelain)" || { echo 'A clean, exact-source checkout is required.' >&2; exit 1; }
set +u
source /opt/rh/gcc-toolset-14/enable
set -u
mkdir -p "$PLANK_WORK_ROOT/host-build" "$PLANK_CARGO_ROOT/shared-target" "$PLANK_ARTIFACT_ROOT"
# Preserve the product recipe's output locations without a separate Cargo cache
# per checkout. Cargo serializes its own shared target-directory access.
for path in "$PLANK_WORK_ROOT/host-build/quinn-tests" "$PLANK_WORK_ROOT/host-build/plank-transport-cargo"; do
  if [[ ! -e $path && ! -L $path ]]; then ln -s "$PLANK_CARGO_ROOT/shared-target" "$path"; fi
  test "$(readlink -f "$path")" = "$(readlink -f "$PLANK_CARGO_ROOT/shared-target")" || {
    echo 'Existing candidate Cargo output does not use the shared target directory.' >&2; exit 1;
  }
done
cd "$repo_dir" # Apply the pinned rust-toolchain.toml to every dependency gate.
bash "$repo_dir/scripts/ci/bootstrap.sh" linux-host
# The ordinary CI driver creates this link; remove only our matching link when
# resuming an interrupted candidate build.
ffmpeg_link="$repo_dir/apps/host/linux/cmake-build-ffmpeg-x264rgb-install/ffmpeg"
if [[ -L $ffmpeg_link && $(readlink -- "$ffmpeg_link") == "$PLANK_DEP_ROOT/host-ffmpeg" ]]; then rm -- "$ffmpeg_link"; fi
bash "$repo_dir/scripts/ci/build.sh" linux-host
