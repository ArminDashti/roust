# Cross-compile roust Windows release binaries from Linux (Docker Desktop / CI).
# Runtime requires bare-metal Windows with WinDivert — this image only builds artifacts.
#
# Build:  docker compose run --rm build
# Output: ./dist/roust.exe, roust-setup.exe, WinDivert.dll, WinDivert64.sys

FROM messense/cargo-xwin:latest AS builder

ARG WINDIVERT_ZIP_URL=https://github.com/basil00/WinDivert/releases/download/v2.2.2/WinDivert-2.2.2-A.zip

WORKDIR /build

COPY core/Cargo.toml core/Cargo.lock core/build.rs ./core/
COPY core/.cargo ./core/.cargo
COPY core/src ./core/src
COPY core/WinDivert-2.2.2-A ./core/WinDivert-2.2.2-A

# WinDivert import/runtime libs are often not committed; fetch the official SDK when missing.
RUN set -eux; \
    if [ ! -f core/WinDivert-2.2.2-A/x64/WinDivert.lib ]; then \
      apt-get update; \
      apt-get install -y --no-install-recommends curl unzip ca-certificates; \
      rm -rf /var/lib/apt/lists/*; \
      curl -fsSL -o /tmp/windivert.zip "${WINDIVERT_ZIP_URL}"; \
      unzip -q /tmp/windivert.zip -d /tmp/windivert; \
      mkdir -p core/WinDivert-2.2.2-A; \
      cp -r /tmp/windivert/WinDivert-2.2.2-A/. core/WinDivert-2.2.2-A/; \
      rm -rf /tmp/windivert /tmp/windivert.zip; \
    fi

WORKDIR /build/core

RUN rustup target add x86_64-pc-windows-msvc \
    && cargo xwin build --release --bins --target x86_64-pc-windows-msvc

FROM debian:bookworm-slim AS artifacts

COPY --from=builder /build/core/target/x86_64-pc-windows-msvc/release/roust.exe /artifacts/
COPY --from=builder /build/core/target/x86_64-pc-windows-msvc/release/roust-setup.exe /artifacts/
COPY --from=builder /build/core/WinDivert-2.2.2-A/x64/WinDivert.dll /artifacts/
COPY --from=builder /build/core/WinDivert-2.2.2-A/x64/WinDivert64.sys /artifacts/
