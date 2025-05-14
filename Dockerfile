FROM rust:1.86-bullseye AS builder
RUN apt-get update

# Get Ubuntu packages
RUN apt install -y libpq5 ca-certificates libpq-dev

RUN apt-get update && apt-get install -y cmake clang

# Update new packages
RUN apt-get update

# Get Rust
RUN curl https://sh.rustup.rs -sSf | bash -s -- -y

ENV PATH="/root/.cargo/bin:${PATH}"

WORKDIR /app
COPY Cargo.toml Cargo.lock .
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN --mount=type=cache,target=/usr/local/cargo/registry cargo build --release

COPY . .

RUN --mount=type=cache,target=/usr/local/cargo/registry <<EOF
  set -e
  cargo build --release
EOF

FROM debian:bullseye-slim AS runtime
# Use jemalloc as memory allocator
RUN apt-get update && apt-get install -y libjemalloc-dev ca-certificates curl
ENV LD_PRELOAD="/usr/lib/x86_64-linux-gnu/libjemalloc.so"
WORKDIR /app

COPY --from=builder /app/target/release/indexer_v2 /usr/local/bin
RUN apt update && apt install -y libpq5 ca-certificates libpq-dev

# Don't run production as root
#RUN addgroup --system --gid 1001 nodejs
#RUN adduser --system --uid 1001 nodejs
#USER nodejs

COPY --from=builder /app .

EXPOSE 2811 2811

CMD ["indexer_v2"]
