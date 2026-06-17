# ---- Builder stage ----
# Build inside Fedora 41 so the resulting binary's glibc/GStreamer linkage
# matches the runtime image below.
FROM fedora:41 AS builder

RUN dnf install -y \
    gcc gcc-c++ make cmake perl pkgconf-pkg-config \
    openssl-devel \
    alsa-lib-devel \
    gstreamer1-devel \
    gstreamer1-plugins-base-devel \
    gstreamer1-rtsp-server-devel && \
    dnf clean all

# Current stable Rust (the crate uses edition 2024, which needs >= 1.85).
RUN curl https://sh.rustup.rs -sSf | sh -s -- -y --default-toolchain stable --profile minimal
ENV PATH="/root/.cargo/bin:${PATH}"

WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release

# ---- Runtime stage ----
FROM fedora:41

# Install GStreamer (1.24) and plugin packages required to run carechords
RUN dnf install -y \
    gstreamer1 \
    gstreamer1-plugins-base \
    gstreamer1-plugins-good \
    gstreamer1-plugins-bad-free \
	gstreamer1-plugin-libav \
	gstreamer1-rtsp-server \
	gstreamer1-rtsp-server-devel \
    gstreamer1-plugins-ugly-free && \
    dnf clean all

COPY --from=builder /build/target/release/carechords /usr/local/bin/carechords

ENV CARECHORDS_DATA_DIR=/media/tank8/carechords
WORKDIR /media/tank8/carechords
EXPOSE 7755 8554

RUN chmod +x /usr/local/bin/carechords && \
    mkdir -p /media/tank8/carechords/cache /media/tank8/carechords/music

VOLUME ["/media/tank8/carechords"]

ENTRYPOINT ["/usr/local/bin/carechords"]
