#!/usr/bin/env bash
#
# Run `cargo check` inside the rustdesk-flutter-builder image, with the same
# vcpkg / cargo / git-proxy environment that docker/build-flutter.sh sets up.
#
# Unlike run-flutter.sh (which runs the full build-flutter.sh), this script
# performs only the minimal environment setup needed for `cargo check`, so it
# is handy for fast compile-error feedback loops while developing on the Rust
# side (e.g. with --features toml-config-import).
#
# It mirrors build-flutter.sh's environment exactly:
#   - VCPKG_ROOT + symlink /opt/vcpkg/installed -> /workspace/vcpkg_installed
#     (manifest-mode vcpkg install), so crates like magnum-opus find their
#     native headers (opus/opus_multistream.h).
#   - git insteadOf proxy rewrites + CARGO_NET_GIT_FETCH_WITH_CLI, so GitHub
#     git deps resolve through gh-proxy.com inside the container.
#
# Usage:
#   docker/check-flutter.sh [extra docker run args...]
#   docker/check-flutter.sh --memory=16g
#   SKIP_VCPKG=1 docker/check-flutter.sh          # reuse an existing vcpkg_installed
#   CARGO_FEATURES="toml-config-import" docker/check-flutter.sh
#
# Env (all optional):
#   RUSTDESK_FLUTTER_IMAGE  image name (default: rustdesk-flutter-builder)
#   CARGO_GIT_CACHE         host cargo git cache dir
#   CARGO_REG_CACHE         host cargo registry cache dir
#   VCPKG_DL_CACHE          host vcpkg downloads cache dir
#   PUB_CACHE               host flutter pub cache dir
#   CARGO_FEATURES          extra --features value passed to cargo check
#                           (default: toml-config-import)
#   SKIP_VCPKG              set to skip `vcpkg install` (assume already built)
#   CARGO_CHECK_ARGS        extra args appended to `cargo check`

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

IMAGE="${RUSTDESK_FLUTTER_IMAGE:-rustdesk-flutter-builder}"
CARGO_GIT_CACHE="${CARGO_GIT_CACHE:-$HOME/.cache/rustdesk-flutter/cargo-git}"
CARGO_REG_CACHE="${CARGO_REG_CACHE:-$HOME/.cache/rustdesk-flutter/cargo-registry}"
VCPKG_DL_CACHE="${VCPKG_DL_CACHE:-$SCRIPT_DIR/vcpkg-downloads}"
PUB_CACHE="${PUB_CACHE:-$HOME/.cache/rustdesk-flutter/pub-cache}"

mkdir -p "$CARGO_GIT_CACHE" "$CARGO_REG_CACHE" "$VCPKG_DL_CACHE" "$PUB_CACHE"

# Pass-through env for the in-container script (cannot export through docker
# run without -e, so re-inject them inside the bash -c).
INNER_ENV=""
[ -n "${SKIP_VCPKG:-}" ]      && INNER_ENV+="SKIP_VCPKG=$SKIP_VCPKG "
[ -n "${CARGO_FEATURES:-}" ]  && INNER_ENV+="CARGO_FEATURES=$CARGO_FEATURES "
[ -n "${CARGO_CHECK_ARGS:-}" ] && INNER_ENV+="CARGO_CHECK_ARGS=$CARGO_CHECK_ARGS "

exec docker run --rm \
    -v "$REPO_ROOT:/workspace" \
    -v "$CARGO_GIT_CACHE:/usr/local/cargo/git" \
    -v "$CARGO_REG_CACHE:/usr/local/cargo/registry" \
    -v "$VCPKG_DL_CACHE:/opt/vcpkg/downloads" \
    -v "$PUB_CACHE:/root/.pub-cache" \
    -e CARGO_HOME=/usr/local/cargo \
    -e PUB_CACHE=/root/.pub-cache \
    "$@" \
    "$IMAGE" \
    bash -c '
      set -euo pipefail
      cd /workspace

      # git proxy / network hardening (from build-flutter.sh)
      git config --global http.postBuffer 524288000
      git config --global http.lowSpeedLimit 0
      git config --global http.lowSpeedTime 999999
      git config --global http.version HTTP/1.1
      git config --global url."https://gh-proxy.com/https://github.com/".insteadOf "https://github.com/"
      export CARGO_NET_GIT_FETCH_WITH_CLI=true

      # vcpkg environment (from build-flutter.sh)
      export VCPKG_ROOT=/opt/vcpkg
      export PATH="$VCPKG_ROOT:$PATH"
      export VCPKG_USE_SHALLOW=0
      mkdir -p /workspace/vcpkg_installed
      if [ ! -L "$VCPKG_ROOT/installed" ]; then
          rm -rf "$VCPKG_ROOT/installed"
      fi
      ln -sfnT /workspace/vcpkg_installed "$VCPKG_ROOT/installed"

      if [ -z "${SKIP_VCPKG:-}" ]; then
          vcpkg install --triplet x64-linux
      fi

      FEATURES="${CARGO_FEATURES:-toml-config-import}"
      # shellcheck disable=SC2086
      cargo check --features "$FEATURES" ${CARGO_CHECK_ARGS:-}
    '
