#!/usr/bin/env bash
set -euo pipefail

VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/')
NAME="barad-dur"
DIST="dist/${NAME}-${VERSION}"

echo "=== Building ${NAME} v${VERSION} ==="
mkdir -p "${DIST}"

# --- 1. Linux static binary (musl) ---
echo ""
echo ">>> Linux static binary (x86_64-unknown-linux-musl)"
docker build --target builder -t "${NAME}-builder" .
CONTAINER_ID=$(docker create "${NAME}-builder")
docker cp "${CONTAINER_ID}:/build/target/x86_64-unknown-linux-musl/release/${NAME}" "${DIST}/${NAME}-${VERSION}-linux-x86_64"
docker rm "${CONTAINER_ID}" > /dev/null
echo "    -> ${DIST}/${NAME}-${VERSION}-linux-x86_64"

# --- 2. Windows executable (mingw cross-compile) ---
echo ""
echo ">>> Windows executable (x86_64-pc-windows-gnu)"
docker run --rm \
  -v "$(pwd)":/build \
  -w /build \
  rust:1.85-alpine sh -c '
    apk add --no-cache musl-dev openssl-dev openssl-libs-static pkgconf cmake make perl git mingw-w64-gcc &&
    rustup target add x86_64-pc-windows-gnu &&
    OPENSSL_STATIC=1 cargo build --release --target x86_64-pc-windows-gnu
  '
cp "target/x86_64-pc-windows-gnu/release/${NAME}.exe" "${DIST}/${NAME}-${VERSION}-windows-x86_64.exe"
echo "    -> ${DIST}/${NAME}-${VERSION}-windows-x86_64.exe"

# --- 3. Docker image ---
echo ""
echo ">>> Docker image"
docker build -t "${NAME}:${VERSION}" -t "${NAME}:latest" .
echo "    -> ${NAME}:${VERSION}"

# --- Summary ---
echo ""
echo "=== Release artifacts ==="
ls -lh "${DIST}"/
echo ""
echo "Docker images:"
docker images "${NAME}" --format "  {{.Repository}}:{{.Tag}}  {{.Size}}"
