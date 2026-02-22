# Build arguments
ARG RUST_VERSION=1.89.0
ARG RAILWAY_GIT_COMMIT_SHA

# --- Frontend Build Stage ---
FROM oven/bun:1 AS frontend-builder

WORKDIR /app

# Install zstd for pre-compression
RUN apt-get update && apt-get install -y --no-install-recommends zstd && rm -rf /var/lib/apt/lists/*

# Copy backend Cargo.toml for build-time version retrieval
COPY ./Cargo.toml ./

# Copy frontend package files
COPY ./web/package.json ./web/bun.lock* ./

# Install dependencies
RUN bun install --frozen-lockfile

# Copy frontend source code
COPY ./web ./

# Build frontend, then pre-compress static assets (gzip, brotli, zstd)
RUN bun run build && bun run scripts/compress-assets.ts

# --- Chef Base Stage ---
FROM lukemathwalker/cargo-chef:latest-rust-${RUST_VERSION} AS chef
WORKDIR /app

# --- Planner Stage ---
FROM chef AS planner
COPY Cargo.toml Cargo.lock ./
COPY build.rs ./
COPY src ./src
# Migrations & .sqlx specifically left out to avoid invalidating cache
RUN cargo chef prepare --recipe-path recipe.json --bin banner

# --- Rust Build Stage ---
FROM chef AS builder

# Set build-time environment variable for Railway Git commit SHA
ARG RAILWAY_GIT_COMMIT_SHA
ENV RAILWAY_GIT_COMMIT_SHA=${RAILWAY_GIT_COMMIT_SHA}

# Copy recipe from planner and build dependencies only
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json --bin banner

# Install build dependencies for final compilation
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    git \
    && rm -rf /var/lib/apt/lists/*

# Copy source code and built frontend assets
COPY Cargo.toml Cargo.lock ./
COPY build.rs ./
COPY .git* ./
COPY src ./src
COPY migrations ./migrations
COPY .sqlx ./.sqlx
COPY --from=frontend-builder /app/dist ./web/dist

# Build web app with embedded assets; SQLX_OFFLINE uses the .sqlx cache (no DB needed at build time)
ENV SQLX_OFFLINE=true
RUN cargo build --release --bin banner

# Strip the binary to reduce size
RUN strip target/release/banner

# --- Runtime Stage ---
FROM debian:12-slim

ARG APP=/usr/src/app
ARG APP_USER=appuser
ARG UID=1000
ARG GID=1000

# Install runtime dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    tzdata \
    wget \
    && rm -rf /var/lib/apt/lists/*

ARG TZ=Etc/UTC
ENV TZ=${TZ}

# Create user with specific UID/GID
RUN addgroup --gid $GID $APP_USER \
    && adduser --uid $UID --disabled-password --gecos "" --ingroup $APP_USER $APP_USER \
    && mkdir -p ${APP}

# Copy application binary
COPY --from=builder --chown=$APP_USER:$APP_USER /app/target/release/banner ${APP}/banner

# Set proper permissions
RUN chmod +x ${APP}/banner

USER $APP_USER
WORKDIR ${APP}

# Build-time arg for PORT, default to 8000
ARG PORT=8000
# Runtime environment var for PORT, default to build-time arg
ENV PORT=${PORT}
EXPOSE ${PORT}

# Add health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD wget --no-verbose --tries=1 --spider http://localhost:${PORT}/health || exit 1

# Can be explicitly overriden with different hosts & ports
ENV HOSTS=0.0.0.0,[::]

# Implicitly uses PORT environment variable
# Runs all services: web, bot, and scraper
CMD ["sh", "-c", "exec ./banner"]
