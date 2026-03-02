#!/usr/bin/env bash
# =============================================================================
# build-windows.sh - Cross-compile xEdit for Windows from Linux
# =============================================================================
# Builds the Rust shared library and Lazarus GUI for Windows x86_64 using
# MinGW-w64 and FPC's cross-compiler, then packages into a zip.
#
# Prerequisites:
#   - Rust with x86_64-pc-windows-gnu target
#   - mingw-w64 (gcc-mingw-w64-x86-64)
#   - Lazarus + FPC with win64 cross-compiler
#
# Usage:
#   ./build-windows.sh
#
# Output:
#   ./output/xEditLaz-win64.zip
# =============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

OUTPUT_DIR="./output"

echo "============================================="
echo "  xEdit Windows Cross-Compile Builder"
echo "============================================="
echo ""

# ---------------------------------------------------------------------------
# Step 1: Check / install prerequisites
# ---------------------------------------------------------------------------
echo "[1/5] Checking prerequisites..."

# Check for Rust
if ! command -v cargo &>/dev/null; then
    echo "ERROR: cargo not found. Install Rust from https://rustup.rs"
    exit 1
fi

# Check for the Windows target
if ! rustup target list --installed | grep -q "x86_64-pc-windows-gnu"; then
    echo "  Adding Rust target x86_64-pc-windows-gnu..."
    rustup target add x86_64-pc-windows-gnu
fi

# Check for MinGW-w64
if ! command -v x86_64-w64-mingw32-gcc &>/dev/null; then
    echo "  MinGW-w64 not found. Attempting to install..."
    if command -v pacman &>/dev/null; then
        sudo pacman -S --noconfirm mingw-w64-gcc
    elif command -v apt-get &>/dev/null; then
        sudo apt-get install -y gcc-mingw-w64-x86-64
    elif command -v dnf &>/dev/null; then
        sudo dnf install -y mingw64-gcc
    else
        echo "ERROR: Cannot install mingw-w64 automatically."
        echo "       Please install mingw-w64 for your distribution."
        exit 1
    fi
fi

# Check for lazbuild
if ! command -v lazbuild &>/dev/null; then
    echo "ERROR: lazbuild not found. Install Lazarus IDE."
    exit 1
fi

echo "  All prerequisites satisfied."
echo ""

# ---------------------------------------------------------------------------
# Step 2: Build Rust shared library for Windows
# ---------------------------------------------------------------------------
echo "[2/5] Building Rust shared library for Windows..."

cd "$SCRIPT_DIR/rust-core"
cargo build --release -p xedit_ffi --target x86_64-pc-windows-gnu

RUST_DLL="target/x86_64-pc-windows-gnu/release/xedit_ffi.dll"
if [ ! -f "$RUST_DLL" ]; then
    echo "ERROR: Expected $RUST_DLL not found after build."
    exit 1
fi
echo "  Built: $RUST_DLL"
echo ""

# ---------------------------------------------------------------------------
# Step 3: Build Lazarus GUI for Windows
# ---------------------------------------------------------------------------
echo "[3/5] Building Lazarus GUI for Windows..."

cd "$SCRIPT_DIR"
lazbuild --build-all \
    --widgetset=win32 \
    --os=win64 \
    --cpu=x86_64 \
    lazarus-gui/xEditLaz.lpi

WIN_EXE="lazarus-gui/xEditLaz.exe"
if [ ! -f "$WIN_EXE" ]; then
    echo "ERROR: Expected $WIN_EXE not found after build."
    exit 1
fi
echo "  Built: $WIN_EXE"
echo ""

# ---------------------------------------------------------------------------
# Step 4: Package into zip
# ---------------------------------------------------------------------------
echo "[4/5] Packaging into zip..."

mkdir -p "$OUTPUT_DIR"

STAGING_DIR=$(mktemp -d)
trap "rm -rf '$STAGING_DIR'" EXIT

# Copy and rename the DLL to match Pascal's expected name
cp "rust-core/$RUST_DLL" "$STAGING_DIR/xedit_core.dll"
cp "$WIN_EXE" "$STAGING_DIR/xEditLaz.exe"

cd "$STAGING_DIR"
zip -9 "$SCRIPT_DIR/$OUTPUT_DIR/xEditLaz-win64.zip" xEditLaz.exe xedit_core.dll

cd "$SCRIPT_DIR"

# ---------------------------------------------------------------------------
# Step 5: Done
# ---------------------------------------------------------------------------
echo ""
echo "[5/5] Done."
echo ""
echo "============================================="
echo "  Build complete!"
echo "============================================="
echo ""
echo "  Output: ${OUTPUT_DIR}/xEditLaz-win64.zip"
echo "  Size:   $(du -h "${OUTPUT_DIR}/xEditLaz-win64.zip" | cut -f1)"
echo ""
