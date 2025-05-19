# --- Builder for amd64 (musl, static) ---
FROM --platform=linux/amd64 alpine:3.19 AS builder-amd64
WORKDIR /usr/src/putioarr
RUN apk add --no-cache build-base musl-dev openssl-dev curl
RUN curl https://sh.rustup.rs -sSf | sh -s -- -y
ENV PATH="/root/.cargo/bin:$PATH"
RUN rustup target add x86_64-unknown-linux-musl
COPY Cargo.toml Cargo.lock .cargo/ ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release --target x86_64-unknown-linux-musl || true
COPY src ./src
RUN touch src/main.rs
RUN cargo build --release --target x86_64-unknown-linux-musl

# --- Builder for arm64 (gnu, dynamic) ---
FROM rust:latest AS builder-arm64
WORKDIR /usr/src/putioarr
RUN apt-get update && apt-get install -y gcc-aarch64-linux-gnu libc6-dev-arm64-cross
ENV CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc
COPY Cargo.toml Cargo.lock .cargo/ ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN rustup target add aarch64-unknown-linux-gnu
RUN cargo build --release --target aarch64-unknown-linux-gnu || true
COPY src ./src
RUN touch src/main.rs
RUN cargo build --release --target aarch64-unknown-linux-gnu

# --- Final image ---
FROM ghcr.io/linuxserver/baseimage-alpine:edge
ARG TARGETARCH

# set version label
ARG BUILD_DATE
ARG VERSION

LABEL maintainer="wouterdebie"
LABEL org.opencontainers.image.architecture="${TARGETARCH}"

RUN apk add --no-cache musl gcc gcompat

# Only copy the correct binary for the current platform
COPY --from=builder-amd64 /usr/src/putioarr/target/x86_64-unknown-linux-musl/release/putioarr /app/putioarr
COPY --from=builder-arm64 /usr/src/putioarr/target/aarch64-unknown-linux-gnu/release/putioarr /app/putioarr

# add local files
COPY root/ /

# ports and volumes
EXPOSE 9091
VOLUME /config

ENTRYPOINT ["/app/putioarr"]
