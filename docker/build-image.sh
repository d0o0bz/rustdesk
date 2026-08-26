#!/usr/bin/env bash
#
# Build the rustdesk-flutter-builder image, retrying the whole `docker build`
# up to 5 times. Combined with the BuildKit cache mount in Dockerfile.flutter
# (Layer a2, cmake download), a failed build resumes the partial download via
# curl -C - instead of starting over.
#
# Usage:
#   docker/build-image.sh [extra docker build args...]
#   docker/build-image.sh --build-arg CMAKE_VERSION=3.30.1
#
# Any argument is passed through to `docker build`. Requires BuildKit
# (DOCKER_BUILDKIT=1, default on modern Docker).

set -euo pipefail

MAX_TRIES=5

# Ensure BuildKit is used (needed for --mount=type=cache in the Dockerfile).
export DOCKER_BUILDKIT=1

ARGS=("$@")
if [ ${#ARGS[@]} -eq 0 ]; then
    ARGS=(-f docker/Dockerfile.flutter -t rustdesk-flutter-builder .)
fi

for i in $(seq 1 "$MAX_TRIES"); do
    echo "==> docker build attempt $i/$MAX_TRIES"
    if docker build "${ARGS[@]}"; then
        echo "==> build succeeded on attempt $i"
        exit 0
    fi
    echo "==> build attempt $i failed; retrying (partial downloads cached)..." >&2
done

echo "==> build failed after $MAX_TRIES attempts" >&2
exit 1
