#!/usr/bin/env bash
# Pre-download vcpkg source tarballs that the build environment (rustdesk-dev)
# cannot fetch directly from github.com / googlesource.com.
#
# Run this on a machine that CAN reach github.com (e.g. E.C.MBP). It downloads
# each tarball through the gh-proxy.com mirror prefix, into ./vcpkg-downloads/.
# The build container mounts that dir to /opt/vcpkg/downloads, and vcpkg reuses
# the cached files by exact filename (skipping the network). See docker/README.md.
#
# After running, rsync ./vcpkg-downloads/ to rustdesk-dev:
#   rsync -a --progress vcpkg-downloads/ \
#     d0o0b@10.35.1.223:/home/d0o0b/Documents/rustdesk-dev/docker/vcpkg-downloads/
#
# gh-proxy.com is a mirror prefix (NOT a CONNECT tunnel), so it must be baked
# into the URL, not passed via HTTPS_PROXY.
set -euo pipefail

PROXY="https://gh-proxy.com/https://github.com"
OUT_DIR="$(cd "$(dirname "$0")" && pwd)/vcpkg-downloads"
mkdir -p "$OUT_DIR"

# Each entry: "<dest-filename>|<github-owner/repo>|<ref>"
#   dest-filename MUST match exactly what vcpkg expects (REPO-REF.tar.gz),
#   otherwise the cache is not picked up and vcpkg re-fetches from the network.
#   ref format follows each port's vcpkg_from_github REF:
#     ffmpeg:  n${VERSION}      (VERSION=7.1 in res/vcpkg/ffmpeg/vcpkg.json)
#     opus:    v${VERSION}      (1.5.2)
#     ffnvcodec: n${VERSION}    (12.1.14.0, from root vcpkg.json override)
#     amd-amf: v${VERSION}      (1.4.35, from root vcpkg.json override)
#     libvpx:  v${VERSION}      (1.15.0)
ENTRIES=(
  "ffmpeg-ffmpeg-n7.1.tar.gz|ffmpeg/ffmpeg|n7.1"
  "xiph-opus-v1.5.2.tar.gz|xiph/opus|v1.5.2"
  "FFmpeg-nv-codec-headers-n12.1.14.0.tar.gz|FFmpeg/nv-codec-headers|n12.1.14.0"
  "GPUOpen-LibrariesAndSDKs-AMF-v1.4.35.tar.gz|GPUOpen-LibrariesAndSDKs/AMF|v1.4.35"
  "webmproject-libvpx-v1.15.2.tar.gz|webmproject/libvpx|v1.15.2"
  # already present in your vcpkg-downloads/ (kept here so the script is complete
  # and idempotent — these are simply skipped by the -s check on reruns):
  "libjpeg-turbo-libjpeg-turbo-3.1.1.tar.gz|libjpeg-turbo/libjpeg-turbo|3.1.1"
  "lu-zero-mfx_dispatch-1.35.1.tar.gz|lu-zero/mfx_dispatch|1.35.1"
  "libyuv-0faf8dd0e004520a61a603a4d2996d5ecc80dc3f.tar.gz|libyuv/libyuv|0faf8dd0e004520a61a603a4d2996d5ecc80dc3f"
)

for entry in "${ENTRIES[@]}"; do
  IFS='|' read -r fname repo ref <<< "$entry"
  dest="$OUT_DIR/$fname"
  url="$PROXY/$repo/archive/$ref.tar.gz"
  if [ -s "$dest" ]; then
    echo "==> skip (exists): $fname"
    continue
  fi
  echo "==> downloading $fname"
  echo "    $url"
  # --retry: gh-proxy sometimes drops mid-stream (curl 18); resume with -C -.
  curl -L --fail --retry 5 --retry-delay 3 -C - "$url" -o "$dest.tmp"
  mv "$dest.tmp" "$dest"
  echo "    saved -> $dest ($(du -h "$dest" | cut -f1))"
done

echo
echo "All tarballs ready in: $OUT_DIR"
echo "Rsync to rustdesk-dev, then rerun the build:"
echo "  rsync -a --progress $OUT_DIR/ \\"
echo "    d0o0b@10.35.1.223:/home/d0o0b/Documents/rustdesk-dev/docker/vcpkg-downloads/"
