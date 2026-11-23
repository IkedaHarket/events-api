# ==============================================================================
# Stage 1: Recipe Planner (cargo-chef)
# ==============================================================================
FROM lukemathwalker/cargo-chef:latest-rust-1-bookworm AS planner
WORKDIR /app
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# ==============================================================================
# Stage 2: Dependencies Caching & Binary Build
# ==============================================================================
FROM lukemathwalker/cargo-chef:latest-rust-1-bookworm AS builder
WORKDIR /app

# Instala paquetes nativos esenciales
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

# Cachea y compila dependencias de terceros
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json -p events-api

# Compila el binario en release
COPY . .
RUN cargo build --release -p events-api

# ==============================================================================
# Stage 3: Minimal Debian Bookworm Runtime Image (Matching Glibc)
# ==============================================================================
FROM debian:bookworm-slim AS runtime
WORKDIR /app

# Instala certificados SSL del sistema y librerías runtime
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Crea un usuario no-root seguro
RUN useradd -u 10001 -U -s /bin/sh appuser

ENV PORT=8080 \
    HOST=0.0.0.0 \
    RUST_LOG=info,events_api=debug,tower_http=info

COPY --from=builder /app/target/release/events-api /app/events-api
COPY --from=builder /app/services/events-api/migrations /app/migrations

USER appuser:appuser

EXPOSE 8080

ENTRYPOINT ["/app/events-api"]
