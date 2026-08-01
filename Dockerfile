# Build stage
FROM rust:1.85-slim AS builder

WORKDIR /app

# Install build dependencies (OpenSSL for reqwest)
RUN apt-get update && \
    apt-get install -y --no-install-recommends pkg-config libssl-dev && \
    rm -rf /var/lib/apt/lists/*

# Copy everything and build
COPY . .
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim AS runtime

# Install ca-certificates for HTTPS requests to elprisetjustnu.se
RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates && \
    rm -rf /var/lib/apt/lists/*

RUN apt-get install tzdata

WORKDIR /app

# Copy the built binary
COPY --from=builder /app/target/release/bitaxe-clocker ./bitaxe-clocker

COPY config.example.toml ./config.toml

# Expose no ports (application connects outbound only)
# Mount your own config.toml: docker run -v ./config.toml:/app/config.toml bitaxe-clocker

ENV TZ=Europe/Berlin

CMD ["./bitaxe-clocker"]
