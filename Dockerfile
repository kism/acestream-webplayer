# FROM rust:alpine AS builder
FROM rust:bookworm AS builder
WORKDIR /app
COPY . .
# RUN apk add --no-cache musl-dev openssl-dev
RUN apt-get update && apt-get install -y musl-dev libssl-dev
RUN curl 'https://cdn.jsdelivr.net/npm/hls.js@1' >static/hls.js
# RUN cargo build
RUN cargo build --release
CMD ["/bin/chmod", "a+rx", "/app/target/release/ace-stream-webplayer"]

FROM debian:bookworm-slim
WORKDIR /app
RUN apt-get update && apt-get install -y libssl3
RUN rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/ace-stream-webplayer /app/ace-stream-webplayer
COPY --from=builder /app/static /app/static
COPY --from=builder /app/templates /app/templates
COPY --from=builder /app/Rocket.toml /app/Rocket.toml

ENTRYPOINT ["/app/ace-stream-webplayer"]
