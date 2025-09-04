FROM ghcr.io/cross-rs/armv7-unknown-linux-gnueabihf:main

# Enable additional architectures and install libpcap-dev for ARM64
RUN dpkg --add-architecture arm64 \
    && apt-get update \
    && apt-get install -y libpcap-dev:arm64 pkg-config