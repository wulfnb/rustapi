FROM rust:1.99.0-slim-bookworm
WORKDIR /source
COPY docker/build-packages.txt docker/coverage-packages.txt /tmp/
RUN apt-get update \
    && xargs -a /tmp/build-packages.txt apt-get install -y --no-install-recommends \
    && xargs -a /tmp/coverage-packages.txt apt-get install -y --no-install-recommends \
    && rm -rf /var/lib/apt/lists/*
RUN rustup component add llvm-tools-preview clippy rustfmt
ARG TARGETARCH
RUN case "$TARGETARCH" in \
      arm64) cov_arch=aarch64; cov_sha=49c0085628392a8d727c4464d91ec417c613c5015f5197cd16d5c05aa3fb5ad1 ;; \
      amd64) cov_arch=x86_64; cov_sha=3fca950394a3c49457657c158b1619cec8dfd2647ae5b48746734c0ab969a522 ;; \
      *) exit 1 ;; \
    esac \
    && python3 -c 'import sys, urllib.request; urllib.request.urlretrieve(sys.argv[1], "/tmp/cov.tar.gz")' "https://github.com/taiki-e/cargo-llvm-cov/releases/download/v0.9.1/cargo-llvm-cov-${cov_arch}-unknown-linux-musl.tar.gz" \
    && echo "$cov_sha  /tmp/cov.tar.gz" | sha256sum --check \
    && tar -xzf /tmp/cov.tar.gz -C /usr/local/bin cargo-llvm-cov \
    && rm /tmp/cov.tar.gz
ENV CARGO_BUILD_JOBS=2 LIBGL_ALWAYS_SOFTWARE=1 XDG_CURRENT_DESKTOP=GNOME \
    NO_PROXY=127.0.0.1,localhost
