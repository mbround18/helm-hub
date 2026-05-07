#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# Helm Hub container entrypoint
#
# Execution order:
#   1. freshclam  — download / refresh ClamAV virus definition database
#   2. clamd      — start the AV daemon in the background (runs as clamav user)
#   3. wait       — poll for the Unix socket to become available
#   4. helm-hub   — start the Rust service (runs as helmhub user)
#
# Signal handling is delegated to tini (PID 1), which forwards SIGTERM/SIGINT
# to the process group and reaps any zombies.
# ─────────────────────────────────────────────────────────────────────────────
set -euo pipefail

CLAMD_SOCKET="${CLAMD_SOCKET:-/var/run/clamav/clamd.ctl}"
SOCKET_TIMEOUT=60   # seconds to wait for clamd to be ready

# Docker volumes replace image-layer ownership metadata, so ensure the runtime
# paths are writable before dropping privileges.
mkdir -p /data /data/charts /tmp/helm-uploads
chown helmhub:helmhub /data /data/charts /tmp/helm-uploads

# ── 1. Update virus definitions ───────────────────────────────────────────────
echo "[entrypoint] Updating ClamAV signature database via freshclam..."
# Run as the clamav user; tolerate mirror failures (|| true) so the container
# still starts when there is no outbound internet access.
if su -s /bin/sh clamav -c "freshclam --stdout --quiet" ; then
    echo "[entrypoint] Virus definitions up to date."
else
    echo "[entrypoint] WARNING: freshclam update failed — using cached definitions."
fi

# ── 2. Start clamd in the background ─────────────────────────────────────────
echo "[entrypoint] Starting clamd daemon..."
# clamd is configured with Foreground=yes so it doesn't try to daemonise
# itself; we background it here via the shell.
su -s /bin/sh clamav -c "clamd --foreground" &
CLAMD_PID=$!
echo "[entrypoint] clamd started (PID ${CLAMD_PID})"

# ── 3. Wait for the Unix socket to become available ───────────────────────────
echo "[entrypoint] Waiting for clamd socket at ${CLAMD_SOCKET} ..."
elapsed=0
until [ -S "${CLAMD_SOCKET}" ] || [ ${elapsed} -ge ${SOCKET_TIMEOUT} ]; do
    sleep 1
    elapsed=$((elapsed + 1))
done

if [ ! -S "${CLAMD_SOCKET}" ]; then
    echo "[entrypoint] ERROR: clamd socket not ready after ${SOCKET_TIMEOUT}s — aborting."
    exit 1
fi

# Make the socket group-accessible for the helmhub user
chmod 660 "${CLAMD_SOCKET}" 2>/dev/null || true
echo "[entrypoint] clamd is ready (${elapsed}s)."

# ── 4. Launch the Rust service ────────────────────────────────────────────────
echo "[entrypoint] Starting Helm Hub backend..."
# exec replaces this shell process; tini continues to own PID 1.
exec su -s /bin/sh helmhub -c "exec /app/helm-hub"
