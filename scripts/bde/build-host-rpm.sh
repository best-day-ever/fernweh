#!/usr/bin/env bash
# bde wrapper: attempt to build the PLANK host RPM for Rocky Linux 9.7 inside
# a rockylinux/rockylinux:9.7 container on this Mac (Docker/OrbStack), using
# --platform linux/amd64 (QEMU emulation on Apple Silicon).
#
# This is NOT a replacement for a real Rocky 9.7 builder VM. It exists to
# answer one question up front: how far can an emulated container get through
# the repo's own documented build path before something needs real x86_64
# hardware, a GPU driver stack, or licensed NVIDIA headers?
#
# It drives the exact sequence PLANK's own GitHub Actions "linux-host" job
# uses (see .github/workflows/build.yml and scripts/ci/{install-linux-deps,
# bootstrap,build}.sh), which is also documented for a real builder VM in
# docs/development/build/builder-vm-bootstrap.md and
# docs/development/build/release-build-runbook.md. Read docs/bde/gap-analysis.md
# section (a) before changing this script.
#
# Usage:
#   scripts/bde/build-host-rpm.sh [--budget-seconds N] [--platform linux/amd64]
#
# Env overrides:
#   PLANK_BDE_BUDGET_SECONDS   wall-clock budget before we give up and report
#                               where it stopped (default 1500 = 25 minutes)
#   PLANK_BDE_PLATFORM         docker --platform value (default linux/amd64)
#   PLANK_BDE_IMAGE            base image ref (default pinned digest below,
#                               same one build.yml uses)
#   PLANK_BDE_BUILD_BRANCH     package branch qualifier (default bde-main);
#                               lowercase kebab-case, becomes the RPM Release
#                               (e.g. bde-broker -> ...0.bde_broker.1.el9)
#   PLANK_BUILD_JOBS           parallel build jobs inside the container
#                               (default 4, matching CI's PLANK_BUILD_JOBS)
#
# Output:
#   scripts/bde/build/log.txt        full container stdout/stderr
#   scripts/bde/build/last-stage.txt last "===stage:...===" marker reached
#   scripts/bde/build/packages/      RPM if the build actually finished
#
# Requires: docker CLI talking to OrbStack (or Docker Desktop) with an
# x86_64 emulation backend. Does not touch any host system outside this
# repo checkout and a named Docker volume (plank-bde-host-deps) used to
# cache the slow FFmpeg/Boost/Cargo bootstrap between attempts.

set -euo pipefail

repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
budget_seconds=${PLANK_BDE_BUDGET_SECONDS:-1500}
platform=${PLANK_BDE_PLATFORM:-linux/amd64}
# Same pinned Rocky 9.7 image build.yml uses for the linux-host CI job.
image=${PLANK_BDE_IMAGE:-rockylinux/rockylinux:9.7@sha256:53f4c6dcb34e1403bd93207351f0af9a593610faeb7165cb8a037346765199b0}
build_jobs=${PLANK_BUILD_JOBS:-4}
deps_volume=${PLANK_BDE_DEPS_VOLUME:-plank-bde-host-deps}
container_name=${PLANK_BDE_CONTAINER_NAME:-plank-bde-host-build}
build_branch=${PLANK_BDE_BUILD_BRANCH:-bde-main}
[[ $build_branch =~ ^[a-z0-9][a-z0-9-]*$ ]] || {
  echo "PLANK_BDE_BUILD_BRANCH must be lowercase kebab-case: $build_branch" >&2
  exit 2
}

while (($#)); do
  case "$1" in
    --budget-seconds) budget_seconds=$2; shift 2 ;;
    --platform) platform=$2; shift 2 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

out_dir="$repo_dir/scripts/bde/build"
log_file="$out_dir/log.txt"
stage_file="$out_dir/last-stage.txt"
rm -rf "$out_dir"
mkdir -p "$out_dir"
echo "(not started)" > "$stage_file"

command -v docker >/dev/null || {
  echo "docker CLI is required (OrbStack/Docker Desktop)" >&2
  exit 1
}

if [[ -n $(git -C "$repo_dir" status --porcelain --untracked-files=normal) ]]; then
  echo "refusing to run a dirty PLANK source tree through the container build" >&2
  git -C "$repo_dir" status --short >&2
  exit 1
fi

echo "==> budget: ${budget_seconds}s, platform: ${platform}, image: ${image}, branch: ${build_branch}"
echo "==> full log: $log_file"

# The in-container script mirrors scripts/ci/{install-linux-deps,bootstrap,build}.sh
# exactly (same three scripts the real CI job calls), running as root directly
# against the bind-mounted checkout instead of GitHub Actions' ephemeral
# worktree-copy dance. Dependency downloads (FFmpeg/x264/x265 build, Boost,
# rustup/cargo) land in a named Docker volume so a second attempt does not
# redo the slow parts. Every stage prints a "===stage:NAME===" marker to
# stdout so a killed/timed-out attempt still tells us exactly where it stopped.
container_script=$(cat <<'EOS'
set -euo pipefail
echo "===stage:container-start==="
date -u +%FT%TZ
uname -m
export PLANK_CANONICAL_ROOT=/workspace
export PLANK_SOURCE_ROOT=/workspace
export PLANK_WORK_ROOT=/deps/work
export PLANK_DEP_ROOT=/deps
export PLANK_ARTIFACT_ROOT=/deps/packages
export PLANK_RUSTUP_ROOT=/deps/rustup
export PLANK_CARGO_ROOT=/deps/cargo
# scripts/package/package-version.sh derives PLANK_BUILD_BRANCH from the
# current git branch when unset, then requires it to match
# ^[a-z0-9][a-z0-9-]*$ (no slashes: it becomes part of RPM Release/DEB
# version strings and package filenames). Our branch policy is bde/main
# (see fork CLAUDE.md); export the sanitized equivalent explicitly rather
# than renaming the branch, per AGENTS.md's own "set PLANK_BUILD_BRANCH
# explicitly for every detached candidate worktree" guidance.
export PLANK_BUILD_BRANCH=${PLANK_BUILD_BRANCH:?}
mkdir -p "$PLANK_WORK_ROOT" "$PLANK_DEP_ROOT" "$PLANK_ARTIFACT_ROOT"

echo "===stage:prepare-checkout-tools==="
# Mirrors build.yml's "Prepare checkout tools" step: the base rockylinux:9.7
# image has neither git nor dnf-plugins-core (needed for `dnf config-manager`,
# which install-linux-deps.sh uses immediately). Pin the vault repo up front
# here too, same as that step does.
dnf --disablerepo='*' \
  --repofrompath=ci-baseos,https://dl.rockylinux.org/vault/rocky/9.7/BaseOS/x86_64/os/ \
  --repofrompath=ci-appstream,https://dl.rockylinux.org/vault/rocky/9.7/AppStream/x86_64/os/ \
  --enablerepo=ci-baseos --enablerepo=ci-appstream \
  install -y git tar gzip python3 dnf-plugins-core

echo "===stage:install-linux-deps==="
bash /workspace/scripts/ci/install-linux-deps.sh host

# install-linux-deps.sh is what installs git in the first place; the
# safe.directory exception can only be set once git itself exists.
git config --global --add safe.directory /workspace

echo "===stage:bootstrap==="
# bootstrap.sh does: submodule update, rustup/cargo 1.89.0 pin, cargo fetch
# --locked for the plank-transport crate, then (host role) Boost 1.89.0
# fetch + the static Host FFmpeg/x264/x265 build via third-party/build-deps.
# The FFmpeg build is the single largest time sink in this whole script.
bash /workspace/scripts/ci/bootstrap.sh linux-host

echo "===stage:build==="
# build.sh (host role) wires PLANK_HOST_FFMPEG_* / PLANK_BOOST_SOURCE_DIR and
# calls the repo's own scripts/package/build-host-rpm.sh, which compiles the
# CMake/CUDA host binaries then rpmbuild's the payload from
# packaging/host/linux/rpm/plank-host.spec.
bash /workspace/scripts/ci/build.sh linux-host

echo "===stage:done==="
find "$PLANK_ARTIFACT_ROOT" -maxdepth 1 -type f -name '*.rpm' -print
EOS
)

docker rm -f "$container_name" >/dev/null 2>&1 || true

# Run in the background under our own wall-clock budget: the Bash tool this
# script is normally launched from caps a single foreground call at 10
# minutes, and macOS zsh has no GNU `timeout` by default.
docker run --rm --platform "$platform" --name "$container_name" \
  -e PLANK_BUILD_BRANCH="$build_branch" \
  -v "$repo_dir:/workspace" \
  -v "${deps_volume}:/deps" \
  "$image" \
  bash -lc "$container_script" \
  > "$log_file" 2>&1 &
docker_pid=$!

deadline=$((SECONDS + budget_seconds))
status="timed-out"
while ((SECONDS < deadline)); do
  if ! kill -0 "$docker_pid" 2>/dev/null; then
    wait "$docker_pid"
    status=$?
    status=$([[ $status == 0 ]] && echo "success" || echo "failed(${status})")
    break
  fi
  tail -1 "$log_file" 2>/dev/null | grep -o '===stage:[a-z-]*===' | tail -1 > "$stage_file" || true
  sleep 10
done

if [[ $status == "timed-out" ]]; then
  echo "==> budget exceeded (${budget_seconds}s); stopping container" >&2
  docker kill "$container_name" >/dev/null 2>&1 || true
  wait "$docker_pid" 2>/dev/null || true
fi

grep -o '===stage:[a-z-]*===' "$log_file" | tail -1 > "$stage_file" || true
echo "==> result: $status"
echo "==> last stage reached: $(cat "$stage_file")"
echo "==> last 40 log lines:"
tail -40 "$log_file"

if [[ $status == success ]]; then
  mkdir -p "$out_dir/packages"
  docker run --rm -v "${deps_volume}:/deps" "$image" \
    sh -c 'cp /deps/packages/*.rpm /deps/packages/*.sha256 2>/dev/null || true' >/dev/null 2>&1 || true
  echo "==> RPM(s) reported above under /deps/packages inside volume '${deps_volume}'"
fi

[[ $status == success ]]
