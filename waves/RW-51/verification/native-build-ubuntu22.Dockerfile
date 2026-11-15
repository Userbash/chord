FROM ubuntu@sha256:74b12b11ffa550fba1929ad4d8d16533c34f6612728469671afba1b03b66881c
RUN apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends libasound2 libxkbcommon0 libxkbcommon-x11-0 libxcb1 libxcb-xkb1 libx11-6 && rm -rf /var/lib/apt/lists/*
COPY chord-gpui /app/chord-gpui
USER 65534:65534
ENTRYPOINT ["/app/chord-gpui"]
