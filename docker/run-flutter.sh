#!/usr/bin/env bash
#
# Run build-flutter.sh inside the rustdesk-flutter-builder image.
#
# Persists cargo's git dependency cache (/usr/local/cargo/git, which holds the
# large wezterm bare clone) on the host so that `docker run` does not re-clone
# wezterm from scratch every time. Only the `git` subdir is mounted -- never
# the whole CARGO_HOME, to avoid shadowing the toolchain in /usr/local/cargo/bin.
# The crates.io download cache (/usr/local/cargo/registry) is persisted the same
# way, so crate tarballs are not re-downloaded on every run.
#
# Usage:
#   docker/run-flutter.sh [extra docker run args...]
#   docker/run-flutter.sh --memory=16g
#
# Any argument is passed through to `docker run` (e.g. --memory, --cpus).
# Build output is streamed to /tmp/build.log; follow with: tail -f /tmp/build.log

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

IMAGE="${RUSTDESK_FLUTTER_IMAGE:-rustdesk-flutter-builder}"
CARGO_GIT_CACHE="${CARGO_GIT_CACHE:-$HOME/.cache/rustdesk-flutter/cargo-git}"
CARGO_REG_CACHE="${CARGO_REG_CACHE:-$HOME/.cache/rustdesk-flutter/cargo-registry}"
# Host dir holding pre-downloaded vcpkg source tarballs (e.g. amd-amf, ffmpeg
# deps from github). Mounted into the container's vcpkg download cache so vcpkg
# reuses them instead of fetching github.com directly (which is unreachable).
VCPKG_DL_CACHE="${VCPKG_DL_CACHE:-$SCRIPT_DIR/vcpkg-downloads}"

mkdir -p "$CARGO_GIT_CACHE" "$CARGO_REG_CACHE" "$VCPKG_DL_CACHE"

exec docker run --rm \
    -v "$REPO_ROOT:/workspace" \
    -v "$CARGO_GIT_CACHE:/usr/local/cargo/git" \
    -v "$CARGO_REG_CACHE:/usr/local/cargo/registry" \
    -v "$VCPKG_DL_CACHE:/opt/vcpkg/downloads" \
    -e CARGO_HOME=/usr/local/cargo \
    "$@" \
    "$IMAGE" /workspace/docker/build-flutter.sh
