#!/usr/bin/env bash
# =============================================================================
# build-appimage.sh - Build the xEdit AppImage using Docker
# =============================================================================
# This script automates the entire AppImage build process:
#   1. Builds a multi-stage Docker image that compiles the Rust shared
#      library and the Qt6 C++ GUI.
#   2. Extracts the final AppImage artifact from the Docker build.
#
# Prerequisites:
#   - Docker (or Podman with Docker CLI compatibility)
#   - Internet access (for pulling base images and downloading appimagetool)
#
# Usage:
#   ./build-appimage.sh
#
# Output:
#   ./output/xEdit-x86_64.AppImage
# =============================================================================

set -euo pipefail

# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------

# Name of the Docker image used during the build
IMAGE_NAME="xedit-appimage-builder"

# Directory where the final AppImage will be placed
OUTPUT_DIR="./output"

# Name of the final AppImage file
APPIMAGE_NAME="xEdit-x86_64.AppImage"

# ---------------------------------------------------------------------------
# Resolve the project root (directory containing this script)
# ---------------------------------------------------------------------------
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

echo "============================================="
echo "  xEdit AppImage Builder"
echo "============================================="
echo ""
echo "Project root: ${SCRIPT_DIR}"
echo ""

# ---------------------------------------------------------------------------
# Step 1: Build the Docker image
# ---------------------------------------------------------------------------
# The multi-stage Dockerfile compiles the Rust library, the Qt6 C++ GUI,
# and packages them into an AppImage. The final stage is a scratch image
# containing only the AppImage artifact.
echo "[1/3] Building Docker image (this may take a while on first run)..."
echo ""

docker build \
    --tag "${IMAGE_NAME}" \
    --target appimage-builder \
    "${SCRIPT_DIR}"

echo ""
echo "[1/3] Docker image built successfully."
echo ""

# ---------------------------------------------------------------------------
# Step 2: Extract the AppImage from the container
# ---------------------------------------------------------------------------
# Create a temporary container, copy the AppImage out, then remove it.
echo "[2/3] Extracting AppImage from container..."

mkdir -p "${OUTPUT_DIR}"

# Create a container (without starting it) and copy the artifact out
CONTAINER_ID=$(docker create "${IMAGE_NAME}")
docker cp "${CONTAINER_ID}:/build/xEdit-x86_64.AppImage" "${OUTPUT_DIR}/${APPIMAGE_NAME}"
docker rm "${CONTAINER_ID}" > /dev/null

echo "[2/3] AppImage extracted."
echo ""

# ---------------------------------------------------------------------------
# Step 3: Make the AppImage executable
# ---------------------------------------------------------------------------
echo "[3/3] Setting executable permissions..."

chmod +x "${OUTPUT_DIR}/${APPIMAGE_NAME}"

echo ""
echo "============================================="
echo "  Build complete!"
echo "============================================="
echo ""
echo "  AppImage: ${OUTPUT_DIR}/${APPIMAGE_NAME}"
echo "  Size:     $(du -h "${OUTPUT_DIR}/${APPIMAGE_NAME}" | cut -f1)"
echo ""
echo "  Run it with:"
echo "    ./${OUTPUT_DIR}/${APPIMAGE_NAME}"
echo ""
