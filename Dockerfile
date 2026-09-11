# Build arguments
ARG RUST_VERSION=1.96.1
ARG GIT_COMMIT_SHA

# Frontend Build Stage
FROM oven/bun:1 AS frontend-builder

WORKDIR /app

# Install zstd for pre-compression
RUN apt-get update && apt-get install -y --no-install-recommends zstd && rm -rf /var/lib/apt/lists/*

# Copy frontend package files and install dependencies
COPY ./web/package.json ./web/bun.lock* ./
RUN bun install --frozen-lockfile

# Copy frontend source code
COPY ./web ./

# Backend Cargo.toml, read at build time for the version. It lands after the install
# because its version line changes on every release, and copying it earlier would
# invalidate the dependency install for a file that install never reads.
COPY ./Cargo.toml ./

# PostHog host is needed at build time for the CSP reportOnly header in svelte.config.js.
# Defaults to the official PostHog EU ingestion endpoint; override at build time if using a
# self-hosted or proxied instance (e.g. --build-arg PUBLIC_POSTHOG_HOST=https://observe.example.com).
ARG PUBLIC_POSTHOG_HOST="https://us.posthog.com"
ENV PUBLIC_POSTHOG_HOST=${PUBLIC_POSTHOG_HOST}

# Build SSR output, then pre-compress static client assets (gzip, brotli, zstd).
# Maps are dropped first: nothing serves them, so compressing them is pure build time.
RUN bun run build \
    && find build -name '*.map' -delete \
    && bun run scripts/compress-assets.ts

# Production Dependency Stage
# Dropping devDependencies sheds the build toolchain; dropping peers sheds vite and its platform
# binaries, which reach the tree through @sveltejs/kit and are never loaded at runtime.
FROM oven/bun:1 AS frontend-deps

WORKDIR /app

COPY ./web/package.json ./web/bun.lock* ./
RUN bun install --frozen-lockfile --production --omit=peer

# Chef Base Stage
# Both this stage and the runtime pin the Debian codename: the binary links
# against the builder's glibc, so an unpinned base can drift ahead of the runtime.
FROM lukemathwalker/cargo-chef:latest-rust-${RUST_VERSION}-trixie AS chef
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

# mold + clang for faster linking (matches .cargo/config.toml's linker override);
# cmake builds aws-lc-sys, reqwest's rustls crypto provider.
RUN apt-get update && apt-get install -y \
    mold \
    clang \
    cmake \
    pkg-config \
    libssl-dev \
    git \
    && rm -rf /var/lib/apt/lists/*
COPY .cargo ./.cargo

# Copy recipe from planner and build dependencies only
COPY --from=planner /app/recipe.json recipe.json
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked \
    --mount=type=cache,target=/app/target,sharing=locked \
    cargo chef cook --release --recipe-path recipe.json --bin banner

# Copy source code
COPY Cargo.toml Cargo.lock ./
COPY build.rs ./
COPY .git* ./
COPY src ./src
COPY migrations ./migrations
COPY .sqlx ./.sqlx

# Stamped into the binary by build.rs and surfaced at /api/status. It changes on every commit, so
# it sits below the dependency cook: any layer beneath it is re-run on every deploy. build.rs
# declares rerun-if-env-changed for it, so the crate still picks up a new value here.
ARG GIT_COMMIT_SHA
ENV GIT_COMMIT_SHA=${GIT_COMMIT_SHA}

# Build with embedded assets; SQLX_OFFLINE uses the .sqlx cache (no DB needed at build time)
ENV SQLX_OFFLINE=true
# target/ is a BuildKit cache mount so incremental artifacts survive between builds. The mount is
# invisible to the image layer, so the binary has to be stripped and copied out of it within the
# same RUN or it disappears with it.
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked \
    --mount=type=cache,target=/app/target,sharing=locked \
    cargo build --release --bin banner \
    && strip target/release/banner \
    && cp target/release/banner /banner

# Node runtime for the SvelteKit SSR server
FROM node:24-trixie-slim

ARG APP=/app
ARG APP_USER=appuser
ARG UID=1001
ARG GID=1001

# Install runtime dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    tzdata \
    && rm -rf /var/lib/apt/lists/*

ARG TZ=Etc/UTC
ENV TZ=${TZ}

# Create user with specific UID/GID
RUN groupadd --gid $GID $APP_USER \
    && useradd --uid $UID --gid $GID --no-create-home $APP_USER \
    && mkdir -p ${APP}

# Ordered by how often each input changes: a COPY's cache key covers its parents, so anything
# below the binary is re-copied and re-exported every deploy.
COPY --from=frontend-deps --chown=$APP_USER:$APP_USER /app/node_modules ${APP}/web/node_modules

# Console logger preload, normalizing SSR output to the JSON log format
COPY --from=frontend-builder --chown=$APP_USER:$APP_USER /app/console-logger.js ${APP}/web/console-logger.js

COPY --chown=$APP_USER:$APP_USER web/scripts/check-ssr-imports.mjs ${APP}/web/scripts/check-ssr-imports.mjs

# Copy SvelteKit SSR build output
COPY --from=frontend-builder --chown=$APP_USER:$APP_USER /app/build ${APP}/web/build

# Turns a package missing from the production install into a build failure, not a crash loop.
RUN node ${APP}/web/scripts/check-ssr-imports.mjs ${APP}/web/build

# Copy Rust binary
COPY --from=builder --chown=$APP_USER:$APP_USER --chmod=755 /banner ${APP}/banner

# The loader resolves every symbol before main, so a runtime whose glibc is older
# than the builder's fails here; --version exits before config, database or socket.
RUN ${APP}/banner --version

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

# No HEALTHCHECK: the chart's own probes hit /api/health, and Docker's needed a wget install.

# Can be explicitly overriden with different hosts & ports
ENV HOSTS=0.0.0.0,[::]

# Rust runs as PID 1 and supervises the SSR child process
ENTRYPOINT ["/app/banner"]
