FROM rust:1.70-slim as builder

WORKDIR /app

# Copy workspace files
COPY Cargo.toml Cargo.lock ./

# Copy health-worker source
COPY health-worker ./health-worker

# Build the health-worker
RUN cd health-worker && cargo build --release

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/health-worker/target/release/health-worker .

CMD ["./health-worker"]