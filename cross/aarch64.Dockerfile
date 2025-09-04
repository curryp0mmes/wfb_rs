FROM ghcr.io/cross-rs/aarch64-unknown-linux-gnu:main

# Enable additional architectures and install libpcap-dev for ARM64
RUN dpkg --add-architecture arm64 \
    && apt-get update \
    && apt-get install -y libpcap-dev:arm64 pkg-config