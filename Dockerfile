# =============================================================================
# xEdit Rust Port - AppImage Build Dockerfile
# =============================================================================
# Multi-stage build that compiles the Rust shared library, nifly wrapper,
# and Qt6 C++ GUI, then packages into an AppImage.
#
# Usage:
#   docker build -t xedit-appimage .
#   docker run --rm -v "$(pwd)/output:/output" xedit-appimage
# =============================================================================

# ---------------------------------------------------------------------------
# Stage 1: Build the Rust shared library (libxedit_ffi.so)
# ---------------------------------------------------------------------------
FROM rust:1.82-bookworm AS rust-builder

WORKDIR /build/rust-core

# Copy only manifests first for better layer caching
COPY rust-core/Cargo.toml rust-core/Cargo.lock* ./
COPY rust-core/xedit_core/Cargo.toml xedit_core/Cargo.toml
COPY rust-core/xedit_dom/Cargo.toml xedit_dom/Cargo.toml
COPY rust-core/xedit_io/Cargo.toml xedit_io/Cargo.toml
COPY rust-core/xedit_games/Cargo.toml xedit_games/Cargo.toml
COPY rust-core/xedit_tools/Cargo.toml xedit_tools/Cargo.toml
COPY rust-core/xedit_ffi/Cargo.toml xedit_ffi/Cargo.toml
COPY rust-core/xedit_nif/Cargo.toml xedit_nif/Cargo.toml
COPY rust-core/xedit_transpiler/Cargo.toml xedit_transpiler/Cargo.toml

# Create dummy source files so cargo can resolve dependencies and cache them
RUN mkdir -p src xedit_core/src xedit_dom/src xedit_io/src xedit_games/src \
             xedit_tools/src xedit_ffi/src xedit_nif/src xedit_transpiler/src && \
    echo "// dummy" > src/lib.rs && \
    echo "// dummy" > xedit_core/src/lib.rs && \
    echo "// dummy" > xedit_dom/src/lib.rs && \
    echo "// dummy" > xedit_io/src/lib.rs && \
    echo "// dummy" > xedit_games/src/lib.rs && \
    echo "// dummy" > xedit_tools/src/lib.rs && \
    echo "// dummy" > xedit_ffi/src/lib.rs && \
    echo "// dummy" > xedit_nif/src/lib.rs && \
    echo "// dummy" > xedit_transpiler/src/lib.rs && \
    cargo fetch || true

# Now copy the real source code
COPY rust-core/ ./

# Build the FFI crate as a cdylib (produces libxedit_ffi.so)
RUN cargo build --release -p xedit_ffi && \
    ls -la target/release/libxedit_ffi.so

# ---------------------------------------------------------------------------
# Stage 2: Build the nifly wrapper (libnifly_wrapper.so)
# ---------------------------------------------------------------------------
FROM debian:bookworm-slim AS nifly-builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential cmake \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

# Copy nifly source and our wrapper
COPY rust-core/xedit_nif/nifly/ nifly/
COPY rust-core/xedit_nif/nifly_wrapper/ nifly_wrapper/

# Build nifly static library
RUN mkdir -p nifly/build && cd nifly/build && \
    cmake .. -DCMAKE_BUILD_TYPE=Release -DBUILD_TESTING=OFF \
             -DCMAKE_POSITION_INDEPENDENT_CODE=ON && \
    make -j$(nproc)

# Build the wrapper shared library
RUN mkdir -p nifly_wrapper/build && cd nifly_wrapper/build && \
    cmake .. -DCMAKE_BUILD_TYPE=Release && \
    make -j$(nproc) && \
    ls -la libnifly_wrapper.so

# ---------------------------------------------------------------------------
# Stage 3: Build the Qt6 C++ GUI
# ---------------------------------------------------------------------------
FROM ubuntu:24.04 AS qt-gui-builder

ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential cmake \
    qt6-base-dev libgl1-mesa-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

COPY qt-gui/ qt-gui/

RUN cd qt-gui && \
    cmake -B build -DCMAKE_BUILD_TYPE=Release && \
    cmake --build build -j$(nproc) && \
    ls -la build/xEdit

# ---------------------------------------------------------------------------
# Stage 4: Package everything into an AppImage
# ---------------------------------------------------------------------------
FROM ubuntu:24.04 AS appimage-builder

ENV DEBIAN_FRONTEND=noninteractive

# Install runtime dependencies and tools for AppImage creation
RUN apt-get update && apt-get install -y --no-install-recommends \
    file wget ca-certificates \
    libqt6widgets6 libqt6gui6 libqt6core6 libqt6dbus6 libqt6printsupport6 \
    qt6-wayland \
    && rm -rf /var/lib/apt/lists/*

# Download appimagetool
RUN wget -q "https://github.com/AppImage/AppImageKit/releases/download/continuous/appimagetool-x86_64.AppImage" \
        -O /usr/local/bin/appimagetool && \
    chmod +x /usr/local/bin/appimagetool

WORKDIR /build

# Create the AppDir structure
RUN mkdir -p AppDir/usr/bin AppDir/usr/lib \
             AppDir/usr/lib/qt6/plugins/platforms \
             AppDir/usr/lib/qt6/plugins/wayland-shell-integration \
             AppDir/usr/share/applications \
             AppDir/usr/share/icons/hicolor/256x256/apps

# Copy the Rust shared library from stage 1
COPY --from=rust-builder /build/rust-core/target/release/libxedit_ffi.so AppDir/usr/lib/

# Copy the nifly wrapper from stage 2
COPY --from=nifly-builder /build/nifly_wrapper/build/libnifly_wrapper.so AppDir/usr/lib/

# Copy the Qt6 C++ GUI binary from stage 3
COPY --from=qt-gui-builder /build/qt-gui/build/xEdit AppDir/usr/bin/

# Bundle Qt6 runtime libraries
RUN for lib in libQt6Core libQt6Gui libQt6Widgets libQt6DBus libQt6PrintSupport; do \
      find /usr/lib -name "${lib}.so*" -exec cp -P {} AppDir/usr/lib/ \; 2>/dev/null || true; \
    done

# Bundle Qt6 plugins (platforms, styles, wayland, etc.)
RUN QT6_PLUGIN_BASE=$(find /usr/lib -type d -name plugins -path '*/qt6/*' | head -1) && \
    if [ -n "$QT6_PLUGIN_BASE" ]; then \
      for plugin_type in platforms imageformats \
                         xcbglintegrations egldeviceintegrations \
                         wayland-shell-integration \
                         wayland-decoration-client wayland-graphics-integration-client; do \
        if [ -d "$QT6_PLUGIN_BASE/$plugin_type" ]; then \
          mkdir -p "AppDir/usr/plugins/$plugin_type" && \
          cp -a "$QT6_PLUGIN_BASE/$plugin_type/"*.so "AppDir/usr/plugins/$plugin_type/" 2>/dev/null || true; \
        fi; \
      done; \
    fi

# Copy the AppRun and desktop file from the build context
COPY AppDir/AppRun AppDir/AppRun
COPY AppDir/xedit.desktop AppDir/xedit.desktop
RUN cp AppDir/xedit.desktop AppDir/usr/share/applications/xedit.desktop

# Placeholder icon
RUN echo "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==" \
    | base64 -d > AppDir/usr/share/icons/hicolor/256x256/apps/xedit.png && \
    ln -sf usr/share/icons/hicolor/256x256/apps/xedit.png AppDir/xedit.png && \
    ln -sf usr/share/icons/hicolor/256x256/apps/xedit.png AppDir/.DirIcon

# Make AppRun and binary executable
RUN chmod +x AppDir/AppRun AppDir/usr/bin/xEdit

# Build the AppImage
ENV ARCH=x86_64
RUN appimagetool --appimage-extract-and-run AppDir/ xEdit-x86_64.AppImage

# ---------------------------------------------------------------------------
# Stage 5: Minimal output stage - just the AppImage artifact
# ---------------------------------------------------------------------------
FROM scratch AS output

COPY --from=appimage-builder /build/xEdit-x86_64.AppImage /xEdit-x86_64.AppImage
