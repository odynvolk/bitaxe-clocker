# Build stage
FROM rust:1.85-slim AS builder

WORKDIR /app

# Install build dependencies (OpenSSL for reqwest)
RUN apt-get update && \
    apt-get install -y --no-install-recommends pkg-config libssl-dev && \
    rm -rf /var/lib/apt/lists/*

# Bring in the source.
#
# `rust-toolchain.toml` is excluded via .dockerignore, so the toolchain this image
# is built on is the one that runs this build. If it were copied in, rustup would
# see its request for `stable` and, finding the image's toolchain registered under
# an exact version name instead, try to download another one at build time.
COPY . .

# Build the release.
#
# `--locked` refuses to re-resolve dependencies: it fails the build if Cargo.lock
# is out of sync with Cargo.toml, so the image is guaranteed to contain exactly
# the dependency versions that were reviewed and committed.
#
# Everything under the lockfile's declared MSRV, so this image is not yet a
# constraint: of the 93 crates actually compiled for x86_64-unknown-linux-gnu the
# highest requirement is 1.77 (thiserror). The 1.87 requirements in the lockfile
# belong to wasip2/wit-bindgen, which are pruned for this target.
RUN cargo build --release --locked

# Runtime stage
FROM debian:bookworm-slim AS runtime

# ca-certificates: HTTPS requests to elprisetjustnu.se
# tzdata: the binary resolves the local timezone via chrono, and the container
#   sets TZ below, which is meaningless without zoneinfo installed.
# One layer, so the apt lists removed at the end cannot be resurrected by a
# later `install` in a layer of their own.
RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates tzdata && \
    rm -rf /var/lib/apt/lists/*

# The application exposes no ports and is never dialled, so it needs no
# privileges. UID is fixed rather than allocated so mounted-config permissions
# can be reasoned about deterministically.
RUN useradd --uid 10001 --user-group --no-create-home bitaxe

WORKDIR /app

# Copy the built binary
COPY --from=builder --chown=bitaxe:bitaxe /app/target/release/bitaxe-clocker ./bitaxe-clocker

# No config is baked in, deliberately. The image used to ship
# `config.example.toml` as `/app/config.toml`, so a bind mount that silently failed to
# provide a file left the container running against the example's hosts (192.168.8.1 and
# .8.2) instead of failing. For this application a wrong default is worse than no default:
# it PATCHes frequency and restarts whatever answers at those addresses. With nothing
# baked in, a missing mount now stops at startup with a message naming the fix.
#
# `config.example.toml` remains in the repository as the template to copy from.

USER bitaxe

# Expose no ports (application connects outbound only)
# Mount your own config.toml: docker run -v $(pwd)/config.toml:/app/config.toml:ro bitaxe-clocker
# The mount source must already exist on the host. If it does not, Docker creates an empty
# directory at that path and starts anyway; the application then reports that config.toml
# is a directory. It must also be world-readable, since it is read by uid 10001, not root.

ENV TZ=Europe/Berlin

# The application exposes no inbound ports (it connects outbound only), so the
# healthcheck probes process liveness: PID 1 is the bitaxe-clocker binary, and
# kill -0 (signal 0) verifies it is alive without sending a real signal.
# Same uid as PID 1 here, so the signal check remains permitted.
HEALTHCHECK --interval=30s --timeout=5s --start-period=15s --retries=3 \
    CMD kill -0 1 || exit 1

CMD ["./bitaxe-clocker"]
