FROM rust:latest AS builder
ARG TARGETPLATFORM
ARG TARGETARCH
WORKDIR /usr/src

RUN apt update && apt upgrade -y
RUN apt-get update && apt-get install -y musl-tools wget

# Set the right architecture
COPY docker/build.sh /usr/local/bin

# Create blank project
RUN USER=root cargo new putioarr

# We want dependencies cached, so copy those first.
COPY Cargo.toml Cargo.lock .cargo/ /usr/src/putioarr/
COPY .cargo/ /usr/src/putioarr/.cargo/

# Set the working directory
WORKDIR /usr/src/putioarr

# Cache dependency build
RUN TARGETPLATFORM=$TARGETPLATFORM TARGETARCH=$TARGETARCH /usr/local/bin/build.sh

# Now copy in the rest of the sources
COPY src /usr/src/putioarr/src/

## Touch main.rs to prevent cached release build
RUN touch /usr/src/putioarr/src/main.rs

# This is the actual application build.
RUN TARGETPLATFORM=$TARGETPLATFORM TARGETARCH=$TARGETARCH /usr/local/bin/build.sh -c /usr/bin

RUN wget https://musl.cc/x86_64-linux-musl-cross.tgz && \
    tar -xzf x86_64-linux-musl-cross.tgz -C /opt

# Set environment variables for musl cross-compilation
ENV PATH="/opt/x86_64-linux-musl-cross/bin:$PATH"
ENV CC_x86_64_unknown_linux_musl=x86_64-linux-musl-gcc
ENV AR_x86_64_unknown_linux_musl=x86_64-linux-musl-ar
ENV CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER=x86_64-linux-musl-gcc

### Runtime

FROM ghcr.io/linuxserver/baseimage-alpine:edge
ARG TARGETPLATFORM
ARG TARGETARCH

# set version label
ARG BUILD_DATE
ARG VERSION

LABEL maintainer="wouterdebie"
LABEL org.opencontainers.image.architecture="${TARGETARCH}"

RUN apk add musl gcc gcompat
COPY --from=builder /usr/bin/putioarr /usr/bin

# add local files
COPY root/ /

# ports and volumes
EXPOSE 9091
VOLUME /config
