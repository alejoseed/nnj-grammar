FROM docker.io/library/rust:1-alpine AS builder

RUN apk add --no-cache build-base curl

WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY grammar ./grammar

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/build/target \
    cargo build --release --bin nnj-grammar-server \
    && cp target/release/nnj-grammar-server /nnj-grammar-server

FROM docker.io/library/alpine:3.22

COPY --from=builder /nnj-grammar-server /usr/local/bin/nnj-grammar-server

ENV NNJ_GRAMMAR_BIND=0.0.0.0:7878
# One log file per day under /logs, in addition to stdout.
ENV NNJ_GRAMMAR_LOG_DIR=/logs

# The server auto-loads grammar/local/ from the working directory if a
# volume is mounted at /app/grammar/local.
WORKDIR /app

EXPOSE 7878
VOLUME /logs

STOPSIGNAL SIGINT
ENTRYPOINT ["/usr/local/bin/nnj-grammar-server"]
