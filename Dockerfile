# syntax=docker/dockerfile:1
FROM rust:1.98.0-slim-bookworm AS build
WORKDIR /source
COPY docker/build-packages.txt /tmp/build-packages.txt
RUN apt-get update && xargs -a /tmp/build-packages.txt apt-get install -y --no-install-recommends \
    && rm -rf /var/lib/apt/lists/*
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY src ./src
COPY tests ./tests
COPY examples ./examples
FROM build AS test
ENV CARGO_BUILD_JOBS=2 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0
ENV HTTP_PROXY="" HTTPS_PROXY="" ALL_PROXY="" NO_PROXY="127.0.0.1,localhost"
RUN cargo test --locked --all-targets

FROM build AS release
RUN cargo build --locked --release

FROM debian:bookworm-slim AS runtime
COPY docker/runtime-packages.txt /tmp/runtime-packages.txt
RUN apt-get update && xargs -a /tmp/runtime-packages.txt apt-get install -y --no-install-recommends \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --create-home --uid 1000 rustapi \
    && mkdir -p /data /tmp/runtime-rustapi \
    && chown rustapi:rustapi /data /tmp/runtime-rustapi \
    && chmod 700 /tmp/runtime-rustapi
# rfd uses the desktop portal for import/export file dialogs on Linux.
RUN apt-get update && apt-get install -y --no-install-recommends \
    xdg-desktop-portal xdg-desktop-portal-gtk \
    && rm -rf /var/lib/apt/lists/*
COPY --from=release /source/target/release/rustapi /usr/local/bin/rustapi
COPY LICENSE NOTICE /usr/share/doc/rustapi/
COPY docker/supervisord.conf /etc/rustapi/supervisord.conf
COPY docker/start-desktop.sh docker/healthcheck.py docker/check-portal.py /usr/local/bin/
RUN chmod 755 /usr/local/bin/start-desktop.sh
ENV DISPLAY=:99 HOME=/home/rustapi RUSTAPI_DATA_DIR=/data \
    XDG_RUNTIME_DIR=/tmp/runtime-rustapi LIBGL_ALWAYS_SOFTWARE=1 XDG_CURRENT_DESKTOP=GNOME
USER rustapi
EXPOSE 6080
VOLUME ["/data"]
HEALTHCHECK --interval=10s --timeout=5s --start-period=30s --retries=6 \
    CMD ["python3", "/usr/local/bin/healthcheck.py"]
CMD ["supervisord", "-c", "/etc/rustapi/supervisord.conf"]
