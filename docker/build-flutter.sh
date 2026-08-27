#!/usr/bin/env bash
#
# Build RustDesk Flutter inside the docker/Dockerfile.flutter image.
# Source is expected at /workspace (mounted from the host).
# Stops after `cargo build` succeeds. With BUILD_DEB=1 it additionally runs
# `flutter build linux --release` and packages a .deb into /workspace.
#
# Usage (in container):
#   /workspace/docker/build-flutter.sh
#
# Optional env:
#   JOBS        parallel cargo jobs (default: auto)
#   SKIP_VCPKG set to skip vcpkg install (assume already built)
#   BUILD_DEB  set to "1" to also build the Flutter Linux bundle and run build.py
#              to produce rustdesk-<ver>.deb at /workspace (off by default)
#   FORCE_SUBMODULE / FORCE_PUBGET / FORCE_BRIDGE
#               set any to force re-run the corresponding idempotent step

set -euo pipefail

cd /workspace

# Disable Flutter's automatic self-upgrade probe (git fetch --tags against
# github.com), which fails on flaky networks without blocking the build.
export FLUTTER_UPGRADE_DISABLED=true

# Harden git against flaky networks (large submodule fetches may otherwise fail
# with RPC failed / early EOF / unexpected disconnect).
git config --global http.postBuffer 524288000
git config --global http.lowSpeedLimit 0
git config --global http.lowSpeedTime 999999
git config --global http.version HTTP/1.1

# Route GitHub git dependencies (e.g. rustdesk-org/hwcodec, rust-webm, nokhwa)
# through a mirror/proxy, since the build environment cannot reach github.com
# directly. gh-proxy.com transparently proxies any GitHub path, preserving commit
# hashes, so cargo's locked git deps still resolve.
git config --global url."https://gh-proxy.com/https://github.com/".insteadOf "https://github.com/"

# hwcodec pulls a nested submodule (rustdesk-org/externals.git) that gh-proxy.com
# rejects with 403; redirect just this path to ghproxy.net, which serves it.
git config --global url."https://ghproxy.net/https://github.com/rustdesk-org/externals.git".insteadOf "https://github.com/rustdesk-org/externals.git"

# rust-webm pulls a nested submodule (src/sys/libwebm) from
# chromium.googlesource.com/webm/libwebm, which the build environment cannot reach
# directly and gh-proxy returns 403 for it; redirect to the GitHub mirror
# webmproject/libwebm (verified reachable via gh-proxy).
git config --global url."https://gh-proxy.com/https://github.com/webmproject/libwebm".insteadOf "https://chromium.googlesource.com/webm/libwebm"

# aom's source is redirected in res/vcpkg/aom/portfile.cmake directly (vcpkg's
# internal git fetch ignores insteadOf, unlike cargo's git CLI). Left here only
# as a no-op safety net; the effective redirect lives in the portfile.
git config --global url."https://gh-proxy.com/https://github.com/AOMediaCodec/libaom".insteadOf "https://aomedia.googlesource.com/aom"

# Large repos (e.g. rustdesk-org/wezterm) may disconnect mid-fetch through the
# proxy; let cargo use the git CLI (so the insteadOf/retry config above applies)
# and disable compression / raise buffers to survive big clones.
export CARGO_NET_GIT_FETCH_WITH_CLI=true
git config --global http.postBuffer 1048576000
git config --global core.compression 0
git config --global http.lowSpeedLimit 1000
git config --global http.lowSpeedTime 300

# wezterm is a huge repo; cargo has no shallow-clone switch for git deps, so
# pre-clone it shallowly (--depth 1, bare) into cargo's git db. Cargo then
# reuses the local clone instead of fetching the whole history over the proxy,
# which otherwise disconnects mid-transfer (early EOF).
WEZTERM_DB="${CARGO_HOME:-/usr/local/cargo}/git/db/wezterm-af88228f2de8e1aa"
WEZTERM_URL="https://gh-proxy.com/https://github.com/rustdesk-org/wezterm"
WEZTERM_COMMIT="80174f8009f41565f0fa8c66dab90d4f9211ae16"
if [ ! "$WEZTERM_DB/HEAD" ]; then
    rm -rf "$WEZTERM_DB"
    git clone --depth 1 --bare --branch rustdesk/pty_based_0.8.1 "$WEZTERM_URL" "$WEZTERM_DB"
    git -C "$WEZTERM_DB" fetch --depth 1 origin "$WEZTERM_COMMIT" 2>/dev/null || true
fi

# aom (res/vcpkg/aom overlay port) is fetched by vcpkg's own git, which ignores
# git global insteadOf and the build environment cannot reach
# aomedia.googlesource.com directly. The source must therefore be provided as a
# local git repo (cloned on the host, e.g. on a machine that CAN reach
# googlesource, then copied into the build tree). The portfile reads it via a
# file:// URL so vcpkg skips the network entirely. Verify it is present and
# contains both required REFs; we cannot clone it from inside the container.
# Point the overlay ports at these local repos via env vars (the portfiles fall
# back to the upstream googlesource URLs when these are unset, e.g. on CI).
export AOM_SRC_URL="file:///workspace/docker/aom-src/aom_aomedia.googlesource.com"
export LIBYUV_SRC_URL="file:///workspace/docker/libyuv-src/libyuv"
AOM_DIR=docker/aom-src/aom_aomedia.googlesource.com
AOM_REF_3121="10aece4157eb79315da205f39e19bf6ab3ee30d0"
AOM_REF_391="8ad484f8a18ed1853c094e7d3a4e023b2a92df28"
# The local aom repo is owned by the host uid (not root), which git rejects
# inside the container as "dubious ownership". Whitelist all directories so git
# (and vcpkg's internal git fetch) can read the file:// source.
git config --global --add safe.directory "*"
if [ ! -d "$AOM_DIR/.git" ]; then
    echo "ERROR: aom local source not found at /workspace/$AOM_DIR" >&2
    echo "       Clone it on a host that can reach aomedia.googlesource.com:" >&2
    echo "         git clone https://aomedia.googlesource.com/aom $AOM_DIR" >&2
    echo "         git -C $AOM_DIR fetch origin $AOM_REF_3121 $AOM_REF_391" >&2
    exit 1
fi
for ref in "$AOM_REF_3121" "$AOM_REF_391"; do
    if ! git -C "$AOM_DIR" cat-file -t "$ref" >/dev/null 2>&1; then
        echo "ERROR: aom local source at /workspace/$AOM_DIR is missing REF $ref" >&2
        echo "       Run on the host: git -C $AOM_DIR fetch origin $ref" >&2
        exit 1
    fi
done
echo "    aom local source present at /workspace/$AOM_DIR (both REFs found)"

# libyuv (res/vcpkg/libyuv overlay port) is fetched by vcpkg's own git from
# chromium.googlesource.com, which the build environment cannot reach (and vcpkg's
# git ignores insteadOf). Provide it as a local git repo, same as aom.
LIBYUV_DIR=docker/libyuv-src/libyuv
LIBYUV_REF="0faf8dd0e004520a61a603a4d2996d5ecc80dc3f"
if [ ! -d "$LIBYUV_DIR/.git" ]; then
    echo "ERROR: libyuv local source not found at /workspace/$LIBYUV_DIR" >&2
    echo "       Clone it on a host that can reach the source, e.g.:" >&2
    echo "         git clone https://github.com/libyuv/libyuv $LIBYUV_DIR" >&2
    echo "         git -C $LIBYUV_DIR fetch origin $LIBYUV_REF" >&2
    exit 1
fi
if ! git -C "$LIBYUV_DIR" cat-file -t "$LIBYUV_REF" >/dev/null 2>&1; then
    echo "ERROR: libyuv local source at /workspace/$LIBYUV_DIR is missing REF $LIBYUV_REF" >&2
    echo "       Run on the host: git -C $LIBYUV_DIR fetch origin $LIBYUV_REF" >&2
    exit 1
fi
echo "    libyuv local source present at /workspace/$LIBYUV_DIR (REF found)"

echo ""
echo "==> [check] toolchain versions"
rustc --version
cargo --version
flutter --version
flutter_rust_bridge_codegen --version

# Step markers live in the mounted source tree so they survive container restarts.
STEP_MARKERS=/workspace/.build-flutter-markers
mkdir -p "$STEP_MARKERS"

echo ""
echo "==> [1/5] git submodules"
if [ -n "${FORCE_SUBMODULE:-}" ]; then
    rm -f "$STEP_MARKERS/submodule"
fi
if [ -f "$STEP_MARKERS/submodule" ]; then
    echo "    already done, skipping (set FORCE_SUBMODULE=1 to redo)"
else
    git submodule update --init --recursive
    touch "$STEP_MARKERS/submodule"
fi

echo ""
echo "==> [1.5/5] flutter pub get (resolves flutter/ .dart_tool/package_config.json for ffigen)"
echo "    PUB_CACHE=$PUB_CACHE"
if [ -n "${FORCE_PUBGET:-}" ]; then
    rm -f "$STEP_MARKERS/pubget"
fi
if [ -f "$STEP_MARKERS/pubget" ]; then
    echo "    already done, skipping (set FORCE_PUBGET=1 to redo)"
else
    cd /workspace/flutter
    flutter pub get --verbose 2>&1 | grep -iE 'flutter_gpu_texture_renderer|ffigen|Got dependencies|Failed to connect|Could not|error' | head -20 || true
    flutter pub get
    cd /workspace
    touch "$STEP_MARKERS/pubget"
fi

echo ""
echo "==> [2/5] generate flutter_rust_bridge"
if [ -n "${FORCE_BRIDGE:-}" ]; then
    rm -f "$STEP_MARKERS/bridge"
fi
if [ -f "$STEP_MARKERS/bridge" ]; then
    echo "    already done, skipping (set FORCE_BRIDGE=1 to redo)"
else
    # Ensure flutter deps (incl. ffigen, needed by codegen's dart binding step)
    # are resolved; [1.5/5] may have been skipped by the idempotent marker.
    ( cd /workspace/flutter && flutter pub get )
    flutter_rust_bridge_codegen \
        --rust-input ./src/flutter_ffi.rs \
        --dart-output ./flutter/lib/generated_bridge.dart \
        --rust-output ./src/bridge_generated.rs \
        --c-output ./flutter/macos/Runner/bridge_generated.h
    touch "$STEP_MARKERS/bridge"
fi

echo ""
echo "==> [3/5] vcpkg install (x64-linux, idempotent)"
export VCPKG_ROOT=/opt/vcpkg
export PATH="$VCPKG_ROOT:$PATH"
# vcpkg (manifest mode, driven by /workspace/vcpkg.json) installs into
# /workspace/vcpkg_installed, but hwcodec's build.rs resolves ffmpeg headers/libs
# via $VCPKG_ROOT/installed/<triplet> (it ignores VCPKG_INSTALLED_ROOT). Bridge
# the two so hwcodec finds the ffmpeg artifacts without forcing a rebuild.
#
# The symlink must point at /opt/vcpkg/installed itself. A bare `ln -sfn SRC DST`
# where DST already exists as a real directory creates SRC *inside* DST instead of
# replacing it, which silently breaks the bridge and makes scrap fail to find
# libvpx.a. Use -T to treat DST as the link name, and ensure the target exists
# first so the link is never dangling.
mkdir -p /workspace/vcpkg_installed
if [ ! -L "$VCPKG_ROOT/installed" ]; then
    rm -rf "$VCPKG_ROOT/installed"
fi
ln -sfnT /workspace/vcpkg_installed "$VCPKG_ROOT/installed"
# The aom overlay port pulls source from a local file:// repo. vcpkg's default
# shallow clone (git fetch --depth 1) fails to resolve the pinned REF from a
# local non-bare repo, so force a full clone instead.
export VCPKG_USE_SHALLOW=0
# Several vcpkg ports download source tarballs directly from github.com (e.g.
# amd-amf, ffmpeg deps), which the build environment cannot reach and is NOT
# covered by git insteadOf (curl, not git). Pre-download those tarballs on a
# machine that CAN reach github (via e.g. https://gh-proxy.com/https://github.com/...)
# and place them in the vcpkg downloads cache, mounted from the host at
# /opt/vcpkg/downloads (see run-flutter.sh). vcpkg reuses cached files and skips
# the network. See docker/README.md ("vcpkg downloads cache").
if [ -n "${SKIP_VCPKG:-}" ]; then
    echo "    SKIP_VCPKG set, skipping vcpkg install"
elif [ -x "$VCPKG_ROOT/vcpkg" ] && [ -d "$VCPKG_ROOT/installed/x64-linux" ] \
    && [ -f "$VCPKG_ROOT/installed/x64-linux/include/libavcodec/avcodec.h" ]; then
    echo "    vcpkg already installed at $VCPKG_ROOT/installed/x64-linux, skipping"
else
    if [ -d "$VCPKG_ROOT/installed/x64-linux" ]; then
        echo "    installed/x64-linux exists but ffmpeg headers missing, reinstalling"
    fi
    vcpkg install --triplet x64-linux
fi

echo ""
echo "==> [4/5] cargo build (flutter lib, release)"
cargo build --locked --lib --release \
    --features hwcodec,flutter,unix-file-copy-paste,toml-config-import \
    ${JOBS:+--jobs "$JOBS"}

echo ""
echo "==> [5/5] cargo lib built"
if ls target/release/liblibrustdesk.so target/release/librustdesk.so 1>/dev/null 2>&1; then
    echo "    Rust library built:"
    ls -lh target/release/librustdesk.so target/release/liblibrustdesk.so 2>/dev/null
fi

if [ "${BUILD_DEB:-}" = "1" ]; then
    echo ""
    echo "==> [6/6] flutter build + deb (BUILD_DEB=1)"
    # build.py --flutter --skip-cargo packages build/linux/x64/release/bundle/
    # into the deb, so the Flutter Linux bundle must be produced here first.
    # flutter build copies the rustdesk lib built in [5/5] into the bundle's lib/.
    ( cd flutter && flutter build linux --release )
    export DEB_ARCH=amd64
    python3 ./build.py --flutter --skip-cargo
    echo "    deb produced at /workspace/rustdesk-*.deb"
fi
