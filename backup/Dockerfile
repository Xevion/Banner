FROM oven/bun:1-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    postgresql-client ca-certificates coreutils \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY package.json bun.lock ./
RUN bun install --frozen-lockfile --production
COPY script.ts ./script.ts

ENTRYPOINT ["timeout", "--kill-after=10", "300", "bun", "run", "/app/script.ts"]
