# syntax=docker/dockerfile:1

FROM docker.io/lukemathwalker/cargo-chef:latest-rust-1-alpine3.22 AS chef
# build-base: C toolchain for aws-lc-sys and other -sys crates.
# curl: jmdict's build script shells out to it to fetch the dictionary.
RUN apk add --no-cache build-base curl
WORKDIR /build

FROM chef AS planner
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder

#Fetching it up front (with the checksum the crate verifies) 
# keeps jmdict fresh after cook.
RUN mkdir -p /root/.cache/rust-jmdict \
    && curl --fail --silent --output /root/.cache/rust-jmdict/entrypack-v1-2021-07-19.json.gz \
        https://dl.xyrillian.de/jmdict/entrypack-v1-2021-07-19.json.gz \
    && echo "6d539f6b1841c213815ec9daa89bf9e5c1046e627f96db50ce800e995c1ca9ca  /root/.cache/rust-jmdict/entrypack-v1-2021-07-19.json.gz" \
        | sha256sum -c - >/dev/null
COPY --from=planner /build/recipe.json recipe.json

# expensive layer that CI cache stores.
RUN cargo chef cook --release --bin nnj-grammar-server --recipe-path recipe.json
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY grammar ./grammar
RUN cargo build --release --bin nnj-grammar-server \
    && cp target/release/nnj-grammar-server /nnj-grammar-server

FROM docker.io/library/alpine:3.22

COPY --from=builder /nnj-grammar-server /usr/local/bin/nnj-grammar-server

ENV NNJ_GRAMMAR_BIND=0.0.0.0:7878
# One log file per day under /logs, in addition to stdout.
ENV NNJ_GRAMMAR_LOG_DIR=/logs

WORKDIR /app

EXPOSE 7878
VOLUME /logs

STOPSIGNAL SIGINT
ENTRYPOINT ["/usr/local/bin/nnj-grammar-server"]
