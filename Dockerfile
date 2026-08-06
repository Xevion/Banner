# Build arguments
ARG RUST_VERSION=1.89.0
ARG RAILWAY_GIT_COMMIT_SHA

# Frontend Build Stage
FROM oven/bun:1 AS frontend-builder

WORKDIR /app

# Install zstd for pre-compression
RUN apt-get update && apt-get install -y --no-install-recommends zstd && rm -rf /var/lib/apt/lists/*

# Copy backend Cargo.toml for build-time version retrieval
COPY ./Cargo.toml ./

# Copy frontend package files and install dependencies
COPY ./web/package.json ./web/bun.lock* ./
RUN bun install --frozen-lockfile

# Copy frontend source code
COPY ./web ./

# PostHog host is needed at build time for the CSP reportOnly header in svelte.config.js.
# Defaults to the official PostHog EU ingestion endpoint; override at build time if using a
# self-hosted or proxied instance (e.g. --build-arg PUBLIC_POSTHOG_HOST=https://observe.example.com).
ARG PUBLIC_POSTHOG_HOST="https://us.posthog.com"
ENV PUBLIC_POSTHOG_HOST=${PUBLIC_POSTHOG_HOST}

# Build SSR output, then pre-compress static client assets (gzip, brotli, zstd)
RUN bun run build && bun run scripts/compress-assets.ts

# Chef Base Stage
FROM lukemathwalker/cargo-chef:latest-rust-${RUST_VERSION} AS chef
WORKDIR /app

# Planner Stage
FROM chef AS planner
COPY Cargo.toml Cargo.lock ./
COPY build.rs ./
COPY src ./src
# Migrations & .sqlx specifically left out to avoid invalidating cache
RUN cargo chef prepare --recipe-path recipe.json --bin banner

# Rust Build Stage
FROM chef AS builder

# Set build-time environment variable for Railway Git commit SHA
ARG RAILWAY_GIT_COMMIT_SHA
ENV RAILWAY_GIT_COMMIT_SHA=${RAILWAY_GIT_COMMIT_SHA}

# mold + clang for faster linking (matches .cargo/config.toml's linker override)
RUN apt-get update && apt-get install -y \
    mold \
    clang \
    pkg-config \
    libssl-dev \
    git \
    && rm -rf /var/lib/apt/lists/*
COPY .cargo ./.cargo

# Copy recipe from planner and build dependencies only
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json --bin banner

# Copy source code
COPY Cargo.toml Cargo.lock ./
COPY build.rs ./
COPY .git* ./
COPY src ./src
COPY migrations ./migrations
COPY .sqlx ./.sqlx

# Copy SSR client assets for embedding (Rust serves /_app/* from binary)
COPY --from=frontend-builder /app/build/client ./web/build/client

# Build with embedded assets; SQLX_OFFLINE uses the .sqlx cache (no DB needed at build time)
ENV SQLX_OFFLINE=true
RUN cargo build --release --bin banner

# Strip the binary to reduce size
RUN strip target/release/banner

# Node runtime for the SvelteKit SSR server
FROM node:24-slim

ARG APP=/app
ARG APP_USER=appuser
ARG UID=1001
ARG GID=1001

# Install runtime dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    tzdata \
    wget \
    && rm -rf /var/lib/apt/lists/*

ARG TZ=Etc/UTC
ENV TZ=${TZ}

# Create user with specific UID/GID
RUN groupadd --gid $GID $APP_USER \
    && useradd --uid $UID --gid $GID --no-create-home $APP_USER \
    && mkdir -p ${APP}

# Copy Rust binary
COPY --from=builder --chown=$APP_USER:$APP_USER /app/target/release/banner ${APP}/banner
RUN chmod +x ${APP}/banner

# Copy SvelteKit SSR build output
COPY --from=frontend-builder --chown=$APP_USER:$APP_USER /app/build ${APP}/web/build

# Console logger preload, normalizing SSR output to the JSON log format
COPY --from=frontend-builder --chown=$APP_USER:$APP_USER /app/console-logger.js ${APP}/web/console-logger.js

# adapter-node leaves dependencies as bare imports rather than bundling them,
# so the SSR server still resolves packages from node_modules at runtime.
COPY --from=frontend-builder --chown=$APP_USER:$APP_USER /app/node_modules ${APP}/web/node_modules

USER $APP_USER
WORKDIR ${APP}

# Build-time arg for PORT, default to 8000
ARG PORT=8000
# Runtime environment var for PORT, default to build-time arg
ENV PORT=${PORT}

# Presence of this variable is what makes the Rust process supervise SSR; it is
# deliberately unset in development, where Vite serves SSR instead.
ENV SSR_COMMAND="node --import ${APP}/web/console-logger.js ${APP}/web/build/index.js"

EXPOSE ${PORT}

# Health check hits Rust (public-facing server)
HEALTHCHECK --interval=30s --timeout=3s --start-period=10s --retries=3 \
    CMD wget --no-verbose --tries=1 --spider http://localhost:${PORT}/api/health || exit 1

# Can be explicitly overriden with different hosts & ports
ENV HOSTS=0.0.0.0,[::]

# Rust runs as PID 1 and supervises the SSR child process
ENTRYPOINT ["/app/banner"]
