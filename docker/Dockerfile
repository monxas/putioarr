# --- Builder for amd64 (musl, static) ---
FROM alpine:3.19 AS builder
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

# --- Final image for amd64 ---
FROM ghcr.io/linuxserver/baseimage-alpine:edge
RUN apk add --no-cache musl gcc gcompat
COPY --from=builder /usr/src/putioarr/target/x86_64-unknown-linux-musl/release/putioarr /app/putioarr
EXPOSE 9091
VOLUME /config
ENTRYPOINT ["/app/putioarr"]
CMD ["run", "--config", "/config/config.toml"]
