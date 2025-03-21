FROM fedora:41

ARG BINARY_PATH=target/release/carechords

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

COPY ${BINARY_PATH} /usr/local/bin/carechords

RUN chmod +x /usr/local/bin/carechords

ENTRYPOINT ["/usr/local/bin/carechords"]
