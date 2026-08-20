# syntax=docker/dockerfile:1
# Build with: DOCKER_BUILDKIT=1 docker build [OPTIONS] .

# ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
# Stage 1 — Frontend build
#   Node 24 Alpine: install dependencies, run Vite production build.
# ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
FROM node:24-alpine AS frontend-builder

RUN npm install -g pnpm@10

WORKDIR /app

ARG VITE_FARO_URL
ARG VITE_OTEL_COLLECTOR_ENDPOINT
ENV VITE_FARO_URL=$VITE_FARO_URL
ENV VITE_OTEL_COLLECTOR_ENDPOINT=$VITE_OTEL_COLLECTOR_ENDPOINT

# Restore dependencies in a separate layer so they are cached unless
# package.json / lockfile change.
COPY frontend/package.json frontend/pnpm-lock.yaml ./frontend/
COPY package.json pnpm-lock.yaml pnpm-workspace.yaml ./
RUN --mount=type=cache,target=/root/.pnpm-store \
    pnpm install --frozen-lockfile

COPY frontend/ ./frontend/

RUN --mount=type=cache,target=/root/.pnpm-store \
    pnpm -r build
# Output: /app/frontend/dist


# ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
# Stage 2a — Rust dependency cache (cargo-chef planner)
#   Compute a recipe.json that captures only the dependency fingerprint.
#   This layer is invalidated only when Cargo.toml / Cargo.lock change.
# ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
FROM rust:1.98-slim-bookworm AS rust-planner

RUN cargo install cargo-chef --locked

WORKDIR /app
COPY backend/ .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    cargo chef prepare --recipe-path recipe.json


# ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
# Stage 2b — Rust dependency cook
#   Builds and caches all transitive dependencies using the recipe.
#   libsqlite3-sys uses the "bundled" feature so no host sqlite3-dev needed.
# ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
FROM rust:1.98 AS rust-cacher

RUN apt-get update && apt-get install -y \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

RUN cargo install cargo-chef --locked

WORKDIR /app
COPY --from=rust-planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json


# ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
# Stage 2c — Rust test runner
#   Runs the full test suite. If tests fail, the build fails immediately.
#   This stage can be skipped during development with:
#     docker build --target backend-builder -t helm-hub-dev
# ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
FROM rust:1.98 AS backend-tester

RUN apt-get update && apt-get install -y \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Pull in the pre-built dependency cache
COPY --from=rust-cacher /app/target target
COPY --from=rust-cacher /usr/local/cargo /usr/local/cargo

COPY backend/ .

# Run all tests — this stage fails if any test fails
# Including:
#   • Unit tests (src/**/*.rs)
#   • Integration tests (tests/**/*.rs)
#   • Doc tests
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=target \
    cargo test --release

# Run clippy to catch potential bugs
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=target \
    cargo clippy --all-targets --all-features -- -D warnings


# ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
# Stage 2d — Rust application build
#   Only this layer is rebuilt when application source changes.
# ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
FROM rust:1.98 AS backend-builder

RUN apt-get update && apt-get install -y \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Pull in the pre-built dependency cache
COPY --from=rust-cacher /app/target target
COPY --from=rust-cacher /usr/local/cargo /usr/local/cargo

COPY backend/ .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    cargo build --release --bin helm-hub
# Output: /app/target/release/helm-hub

# ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
# Stage 3 — Runtime image
#   Debian Bookworm slim + ClamAV daemon + freshclam + tini.
#   The bundled SQLite in the Rust binary requires only glibc (already present).
# ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
FROM debian:trixie AS runtime

# ── System packages ───────────────────────────────────────────────────────────
RUN apt-get update && apt-get install -y \
    # ClamAV engine + daemon + auto-updater
    clamav \
    clamav-daemon \
    clamav-freshclam \
    # Lightweight init — handles signal forwarding and zombie reaping
    tini \
    # TLS certificates (freshclam contacts the update mirrors over HTTPS)
    ca-certificates \
    libpq5 \
    glibc-source \
    libc6 \    
    && rm -rf /var/lib/apt/lists/*

# ── ClamAV configuration ──────────────────────────────────────────────────────
#
# Debian ships a template at /etc/clamav/clamd.conf.  We patch exactly the
# settings that matter for containerised use; everything else stays at its
# well-tested default.
RUN sed -i \
    # Unix socket path — must match CLAMD_SOCKET env var in the Rust service
    -e 's|^LocalSocket .*|LocalSocket /var/run/clamav/clamd.ctl|' \
    # Allow connections from processes not owned by clamav (our Rust binary)
    -e 's|^LocalSocketMode .*|LocalSocketMode 660|' \
    # Allow the clamav group to connect
    -e 's|^LocalSocketGroup .*|LocalSocketGroup clamav|' \
    # Disable remote TCP socket — Unix socket only
    -e 's|^#TCPSocket|#TCPSocket|' \
    # Let clamd log to the foreground process stream; explicit log files
    # against /dev/stdout or /proc/self/fd/1 fail in this container.
    -e '/^#\?LogFile /d' \
    -e 's|^#LogVerbose.*|LogVerbose yes|' \
    # Do not detach (we manage the process directly)
    -e 's|^Foreground .*|Foreground yes|' \
    /etc/clamav/clamd.conf || true

# freshclam.conf: remove file logging and let the CLI write to stdout
RUN sed -i \
    -e '/^#\?UpdateLogFile /d' \
    -e 's|^Foreground .*|Foreground yes|' \
    /etc/clamav/freshclam.conf || true

# ── App user & directories ────────────────────────────────────────────────────
#
# The Rust binary runs as "helmhub" which is added to the "clamav" group so it
# can open the Unix socket (mode 660, group clamav).
RUN useradd -r -s /sbin/nologin -G clamav helmhub

RUN mkdir -p \
    /app \
    /data/charts \
    /tmp/helm-uploads \
    /var/run/clamav \
    && chown clamav:clamav /var/run/clamav \
    && chmod 775 /var/run/clamav \
    && chown helmhub:helmhub /data/charts /tmp/helm-uploads /app

# ── Copy artefacts ────────────────────────────────────────────────────────────
COPY --from=backend-builder /app/target/release/helm-hub /app/helm-hub
COPY --from=frontend-builder /app/frontend/dist /app/static

# ── Entrypoint script ─────────────────────────────────────────────────────────
COPY entrypoint.sh /app/entrypoint.sh
RUN chmod +x /app/entrypoint.sh

# ── Runtime defaults ──────────────────────────────────────────────────────────
ENV DATABASE_URL=/data/helm-hub.db \
    CHARTS_STORAGE_PATH=/data/charts \
    TEMP_UPLOAD_DIR=/tmp/helm-uploads \
    CLAMD_SOCKET=/var/run/clamav/clamd.ctl \
    CLAMAV_ENABLED=true \
    HOST=0.0.0.0 \
    PORT=3000 \
    MGMT_PORT=9090 \
    RUST_LOG=helm_hub_backend=info,tower_http=info

EXPOSE 3000 9090

# tini as PID 1 — forwards signals and reaps zombie processes created when
# we background clamd in the entrypoint.
ENTRYPOINT ["/usr/bin/tini", "--", "/app/entrypoint.sh"]
CMD ["/app/helm-hub"]
