FROM lukemathwalker/cargo-chef:latest-rust-1 AS chef
RUN apt-get update

# Get Ubuntu packages
RUN apt install -y libpq5 ca-certificates libpq-dev

RUN apt-get update && apt-get install -y cmake clang

# Update new packages
RUN apt-get update
WORKDIR /app


FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder 
COPY --from=planner /app/recipe.json recipe.json

RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
RUN cargo build --release

FROM debian:bullseye-slim AS runtime
# Use jemalloc as memory allocator
RUN apt-get update && apt-get install -y libjemalloc-dev ca-certificates curl
ENV LD_PRELOAD="/usr/lib/x86_64-linux-gnu/libjemalloc.so"
WORKDIR /app

COPY --from=builder /app/target/release/birds-indexer /usr/local/bin
RUN apt update && apt install -y libpq5 ca-certificates libpq-dev

# Don't run production as root
#RUN addgroup --system --gid 1001 nodejs
#RUN adduser --system --uid 1001 nodejs
#USER nodejs

COPY --from=builder /app .

EXPOSE 2811 2811

CMD ["birds-indexer"]
