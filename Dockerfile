FROM rust:1.86-bullseye AS builder
RUN apt-get update

# Get Ubuntu packages
RUN apt install -y libpq5 ca-certificates libpq-dev

RUN apt-get update && apt-get install -y cmake clang

# Update new packages
RUN apt-get update

WORKDIR /app
COPY ./Cargo.toml ./Cargo.toml 
COPY ./Cargo.lock ./Cargo.lock 

RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN --mount=type=cache,target=/usr/local/cargo/registry cargo build  --release
RUN rm -f target/release/deps/birds-indexer*
RUN rm -rf ./src
COPY ./src ./src

RUN --mount=type=cache,target=/usr/local/cargo/registry <<EOF
  set -e
  touch /app/src/main.rs
  cargo build --release
EOF

FROM debian:bullseye-slim AS runtime
# Use jemalloc as memory allocator
RUN apt-get update && apt-get install -y libjemalloc-dev ca-certificates curl
WORKDIR /app

COPY --from=builder /app/target/release/birds-indexer /usr/local/bin

RUN apt update && apt install -y libpq5 ca-certificates libpq-dev

EXPOSE 2811 2811

CMD ["/usr/local/bin/birds-indexer"]
